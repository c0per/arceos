use crate::{scheduler::CurrentTask, stack::TaskStack};
use alloc::{string::String, sync::Arc};
use axconfig::TASK_STACK_SIZE;
use axhal::{
    arch::{enter_user, TaskContext, TrapFrame},
    mem::{PhysAddr, VirtAddr, PAGE_SIZE_4K},
    paging::MappingFlags,
};
use axmem::MemorySet;
use core::{
    alloc::Layout,
    arch::asm,
    cell::UnsafeCell,
    ptr::NonNull,
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

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

    entry_point: u64,
    memory_set: Arc<MemorySet>,

    /// TaskStack is simply a pointer to memory in memory_set.
    /// Kernel stack is mapped in "free memory" region.
    kstack: TaskStack,
    /// User stack is mapped in user space (highest address)
    ustack: TaskStack,
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

impl Task {
    pub fn from_elf_data(elf_data: &[u8]) -> Self {
        use axhal::arch::write_page_table_root;

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
                        ph.virtual_addr()
                    } else {
                        0
                    }
                })
                .max()
                .unwrap_or_default() as usize,
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

        Self {
            pid,
            tid: pid,
            state: AtomicU8::new(TaskState::Ready as u8),
            ctx: UnsafeCell::new(TaskContext::new()),
            entry_point: elf.header.pt2.entry_point(),
            memory_set: Arc::new(memory_set),
            kstack,
            ustack,
        }
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

    pub fn dummy_trap_frame(&self) -> axhal::arch::TrapFrame {
        use axhal::arch::TrapFrame;
        use riscv::register::sstatus::{self, Sstatus};

        let mut trap_frame = TrapFrame::default();

        trap_frame.regs.sp = self.ustack.top().into();
        trap_frame.sepc = self.entry_point as usize;

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
        let sstatus_reg = sstatus::read();
        unsafe {
            trap_frame.sstatus =
                *(&sstatus_reg as *const Sstatus as *const usize) & !(1 << 8) | (1 << 18);
        }

        trap_frame
    }

    pub fn trap_frame_ptr(&self) -> *const axhal::arch::TrapFrame {
        (usize::from(self.kstack.top()) - core::mem::size_of::<TrapFrame>()) as *const TrapFrame
    }

    pub fn ctx_mut_ptr(&self) -> *mut TaskContext {
        self.ctx.get()
    }

    /// For Init (pid = 1) task only.
    /// Wwitch to its page_table
    /// Create a dummy TrapFrame to go to _user_start
    pub unsafe fn enter_as_init(&self) -> ! {
        if self.pid != 1 || self.tid != 1 {
            panic!("Calling enter_user() for task other than (1, 1)");
        }

        self.set_state(TaskState::Running);

        let trap_frame = self.dummy_trap_frame();
        unsafe {
            core::ptr::write(
                (usize::from(self.kstack.top()) - core::mem::size_of::<TrapFrame>())
                    as *mut TrapFrame,
                trap_frame.clone(),
            );
        }

        enter_user(&trap_frame, self.kstack.top().into());
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

            entry_point: self.entry_point,
            memory_set: Arc::new(memory_set),

            kstack,
            ustack: self.ustack.clone(),
        }
    }
}

static TASK_ID_ALLOCATOR: AtomicUsize = AtomicUsize::new(1);
