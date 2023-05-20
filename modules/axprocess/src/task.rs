use crate::{
    fd::FdList,
    scheduler::CurrentTask,
    stack::TaskStack,
    stdio::{Stderr, Stdin, Stdout},
};
use alloc::{string::String, sync::Arc, vec, vec::Vec};
use axconfig::TASK_STACK_SIZE;
use axhal::{
    arch::{enter_user, write_page_table_root, TaskContext, TrapFrame},
    mem::{PhysAddr, VirtAddr, PAGE_SIZE_4K},
    paging::MappingFlags,
};
use axmem::MemorySet;
use core::{
    alloc::Layout,
    arch::asm,
    cell::{RefCell, UnsafeCell},
    mem::size_of,
    ptr::{copy_nonoverlapping, NonNull},
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};
use riscv::register::sstatus::Sstatus;
use spinlock::SpinNoIrq;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TaskState {
    Running = 1,
    Ready = 2,
    Blocked = 3,
    Exited = 4,
}

pub struct Task {
    pub pid: usize,
    pub tid: usize,
    state: AtomicU8,
    ctx: UnsafeCell<TaskContext>,

    memory_set: Arc<MemorySet>,

    /// TaskStack is simply a pointer to memory in memory_set.
    /// Kernel stack is mapped in "free memory" region.
    kstack: TaskStack,
    /// User stack is mapped in user space (highest address)
    ustack: TaskStack,

    #[cfg(feature = "fs")]
    fd_table: SpinNoIrq<FdList>,
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

impl Task {
    /// Create a Task from elf data, only used for init process. Other processes are spawned by
    /// clone (fork) + execve.
    /// This function will allocate kernel stack and put `TrapFrame` (including `argv`) into place.
    pub fn from_elf_data(elf_data: &[u8], argv: &[&str]) -> Self {
        let mut memory_set = MemorySet::new_with_kernel_mapped();

        unsafe {
            write_page_table_root(memory_set.page_table_root_ppn());
        }

        let elf = xmas_elf::ElfFile::new(elf_data).expect("Error parsing app ELF file.");
        memory_set.map_elf(&elf);

        let kstack = TaskStack::alloc(axconfig::TASK_STACK_SIZE);

        // This should be aligned to 4K
        let max_va = VirtAddr::from(
            elf.program_iter()
                .map(|ph| {
                    if ph.get_type() == Ok(xmas_elf::program::Type::Load) {
                        ph.virtual_addr() + ph.mem_size()
                    } else {
                        0
                    }
                })
                .max()
                .map(|elf_end| (elf_end as usize + PAGE_SIZE_4K - 1) / PAGE_SIZE_4K * PAGE_SIZE_4K)
                .unwrap_or_default(),
        );

        // skip protection page
        let ustack_bottom = max_va + PAGE_SIZE_4K;

        // Allocate memory and hold it in memory_set
        memory_set.alloc_region(
            ustack_bottom,
            TASK_STACK_SIZE,
            MappingFlags::USER | MappingFlags::READ | MappingFlags::WRITE,
            None,
        );

        let ustack = TaskStack::new(
            NonNull::new(usize::from(ustack_bottom) as *mut u8)
                .expect("Error creating NonNull<u8> for ustack bottom"),
            Layout::from_size_align(axconfig::TASK_STACK_SIZE, 16)
                .expect("Error creating layout for ustack"),
        );

        let pid = TASK_ID_ALLOCATOR.fetch_add(1, Ordering::Relaxed);

        // handle trap frame
        let trap_frame = gen_trap_frame(ustack.top(), elf.header.pt2.entry_point() as usize);

        unsafe {
            core::ptr::write(
                (usize::from(kstack.top()) - core::mem::size_of::<TrapFrame>()) as *mut TrapFrame,
                trap_frame.clone(),
            );

            riscv::register::sstatus::set_sum();
        }

        let mut task = Self {
            pid,
            tid: pid,
            state: AtomicU8::new(TaskState::Ready as u8),
            ctx: UnsafeCell::new(TaskContext::new()),

            memory_set: Arc::new(memory_set),
            kstack,
            ustack,

            #[cfg(feature = "fs")]
            fd_table: SpinNoIrq::new(FdList::default()),
        };

        unsafe { task.push_argv(argv) };

        task
    }

    pub fn set_state(&self, state: TaskState) {
        self.state.store(state as u8, Ordering::Release);
    }

    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::Acquire) == TaskState::Running as u8
    }

    pub fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == TaskState::Ready as u8
    }

    pub fn is_blocked(&self) -> bool {
        self.state.load(Ordering::Acquire) == TaskState::Blocked as u8
    }

    pub fn is_init(&self) -> bool {
        self.pid == 1
    }

    pub fn page_table_ppn(&self) -> PhysAddr {
        self.memory_set.page_table_root_ppn()
    }

    pub fn trap_frame_ptr(&self) -> *const axhal::arch::TrapFrame {
        (usize::from(self.kstack.top()) - core::mem::size_of::<TrapFrame>()) as *const TrapFrame
    }

    pub fn trap_frame_ptr_mut(&self) -> *mut axhal::arch::TrapFrame {
        (usize::from(self.kstack.top()) - core::mem::size_of::<TrapFrame>()) as *mut TrapFrame
    }

    pub fn ctx_mut_ptr(&self) -> *mut TaskContext {
        self.ctx.get()
    }

    /// Push in argv to stack in `TrapFrame` in kernel stack and update sp, a0, a1 in `TrapFrame`.
    /// This function need to be when with SUM set.
    pub unsafe fn push_argv(&self, argv: &[&str]) {
        let trap_frame = &mut *self.trap_frame_ptr_mut();

        let mut sp = trap_frame.regs.sp;
        let argc = argv.len();

        // argc
        let argc_ptr = (sp - size_of::<usize>()) as *mut isize;
        *argc_ptr = argc as isize;

        sp -= (argc + 1) * size_of::<usize>();
        let argv_ptr = sp as *mut *const u8;

        argv.iter().enumerate().for_each(|(idx, arg)| {
            sp -= argv.len() + 1;
            *argv_ptr.offset(idx as isize) = sp as *const u8;
            copy_nonoverlapping(arg.as_ptr(), sp as *mut u8, argv.len());
            *((sp + argv.len()) as *mut u8) = b'\0';
        });

        trap_frame.regs.sp = sp;
        trap_frame.regs.a0 = argc_ptr as usize;
        trap_frame.regs.a1 = argv_ptr as usize;
    }

    /// For Init (pid = 1) task only. Must be run after switching to user page table.
    pub unsafe fn enter_as_init(&self) -> ! {
        if self.pid != 1 || self.tid != 1 {
            panic!("Calling enter_user() for task other than (1, 1)");
        }

        self.set_state(TaskState::Running);

        enter_user(self.kstack.top().into());
    }

    pub fn set_in_wait_queue(&self, _: bool) {}

    pub fn id_name(&self) -> String {
        format!("Task({}, {})", self.pid, self.tid)
    }

    // fork
    pub fn clone(&self) -> Self {
        extern "C" {
            fn task_entry();
        }

        let pid = TASK_ID_ALLOCATOR.fetch_add(1, Ordering::Release);

        let mut memory_set = self.memory_set.clone_mapped();

        // Create new U stack
        // let max_va = memory_set.max_va();
        // let ustack_bottom = max_va + PAGE_SIZE_4K;
        // memory_set.alloc_region(
        //     ustack_bottom,
        //     TASK_STACK_SIZE,
        //     MappingFlags::USER | MappingFlags::READ | MappingFlags::WRITE,
        //     None,
        // );
        // let ustack = TaskStack::new(
        //     NonNull::new(usize::from(ustack_bottom) as *mut u8)
        //         .expect("Error creating NonNull<u8> for ustack bottom"),
        //     Layout::from_size_align(axconfig::TASK_STACK_SIZE, 16)
        //         .expect("Error creating layout for ustack"),
        // );

        // Create new S stack
        let kstack = TaskStack::alloc(axconfig::TASK_STACK_SIZE);
        let trap_frame =
            (usize::from(kstack.top()) - core::mem::size_of::<TrapFrame>()) as *mut TrapFrame;
        unsafe {
            core::ptr::copy_nonoverlapping(self.trap_frame_ptr(), trap_frame, 1);
        }

        let trap_frame = unsafe { &mut *trap_frame };
        trap_frame.regs.a0 = 0;
        // trap_frame.regs.sp = ustack.top().into();

        let mut ctx = TaskContext::new();
        ctx.init(
            task_entry as usize,
            kstack.top() - core::mem::size_of::<TrapFrame>(),
        );

        Self {
            pid,
            tid: pid,
            state: AtomicU8::new(TaskState::Ready as u8),
            ctx: UnsafeCell::new(ctx),

            memory_set: Arc::new(memory_set),

            kstack,
            ustack: self.ustack.clone(),

            #[cfg(feature = "fs")]
            fd_table: SpinNoIrq::new(self.fd_table.lock().clone()),
        }
    }

    #[cfg(feature = "fs")]
    pub fn fd_table(&self) -> &SpinNoIrq<FdList> {
        &self.fd_table
    }
}

fn gen_trap_frame(ustack_top: VirtAddr, entry_point: usize) -> TrapFrame {
    let mut trap_frame = TrapFrame::default();

    trap_frame.regs.sp = ustack_top.into();
    trap_frame.sepc = entry_point;

    // restore kernel gp, tp
    let tp: usize;
    let gp: usize;
    unsafe {
        asm!(
            "mv {}, tp",
            "mv {}, gp",
            out(reg) tp,
            out(reg) gp,
        );
    }
    trap_frame.regs.tp = tp;
    trap_frame.regs.gp = gp;

    // set SPP to User, SUM to 1
    let sstatus_reg = riscv::register::sstatus::read();
    unsafe {
        trap_frame.sstatus =
            *(&sstatus_reg as *const Sstatus as *const usize) & !(1 << 8) | (1 << 18);
    }

    trap_frame
}

static TASK_ID_ALLOCATOR: AtomicUsize = AtomicUsize::new(1);
