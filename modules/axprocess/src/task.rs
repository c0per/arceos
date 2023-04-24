use crate::stack::TaskStack;
use alloc::string::String;
use axconfig::TASK_STACK_SIZE;
use axhal::{
    arch::enter_user,
    mem::{VirtAddr, PAGE_SIZE_4K},
    paging::MappingFlags,
};
use axmem::MemorySet;
use core::{
    alloc::Layout,
    arch::asm,
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
    pid: usize,
    tid: usize,
    state: AtomicU8,

    entry_point: u64,
    memory_set: MemorySet,

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

        let pid = TASK_ID_ALLOCATOR.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

        Self {
            pid,
            tid: pid,
            state: AtomicU8::new(TaskState::Ready as u8),
            entry_point: elf.header.pt2.entry_point(),
            memory_set,
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

        // TODO: need?
        unsafe {
            core::ptr::write(
                (usize::from(self.kstack.top()) - core::mem::size_of::<TrapFrame>())
                    as *mut TrapFrame,
                trap_frame.clone(),
            );
        }

        // set SPP to User, SUM to 1
        let sstatus_reg = sstatus::read();
        unsafe {
            trap_frame.sstatus =
                *(&sstatus_reg as *const Sstatus as *const usize) & !(1 << 8) | (1 << 18);
        }

        trap_frame
    }

    /// For Init (pid = 1) task only.
    /// Wwitch to its page_table
    /// Create a dummy TrapFrame to go to _user_start
    pub unsafe fn enter_as_init(&self) -> ! {
        if self.pid != 1 || self.tid != 1 {
            panic!("Calling enter_user() for task other than (1, 1)");
        }

        let trap_frame = self.dummy_trap_frame();

        enter_user(&trap_frame, self.kstack.top().into());
    }

    pub fn set_in_wait_queue(&self, _: bool) {}

    pub fn id_name(&self) -> String {
        format!("Task({}, {})", self.pid, self.tid)
    }
}

static TASK_ID_ALLOCATOR: AtomicUsize = AtomicUsize::new(1);
