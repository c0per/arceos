#![cfg_attr(not(test), no_std)]
#![feature(const_trait_impl)]

#[macro_use]
extern crate log;

struct KernelGuardIfImpl;

#[crate_interface::impl_interface]
impl kernel_guard::KernelGuardIf for KernelGuardIfImpl {
    fn disable_preempt() {
        #[cfg(all(feature = "multitask", feature = "preempt"))]
        if let Some(curr) = current_may_uninit() {
            curr.disable_preempt();
        }
    }

    fn enable_preempt() {
        #[cfg(all(feature = "multitask", feature = "preempt"))]
        if let Some(curr) = current_may_uninit() {
            curr.enable_preempt(true);
        }
    }
}

#[cfg(feature = "syscall")]
mod syscall;

cfg_if::cfg_if! {
if #[cfg(feature = "multitask")] {

extern crate alloc;

mod run_queue;
mod task;
mod timers;
mod wait_queue;

#[cfg(test)]
mod tests;

use alloc::sync::Arc;

use self::run_queue::{AxRunQueue, RUN_QUEUE};
use self::task::{CurrentTask, TaskInner};

pub use self::task::TaskId;
pub use self::wait_queue::WaitQueue;

cfg_if::cfg_if! {
    // Use fifo scheduler if not specified
    if #[cfg(any(feature = "sched_fifo", feature = "multitask"))] {
        type AxTask = scheduler::FifoTask<TaskInner>;
        type Scheduler = scheduler::FifoScheduler<TaskInner>;
    } else if #[cfg(feature = "sched_rr")] {
        const MAX_TIME_SLICE: usize = 5;
        type AxTask = scheduler::RRTask<TaskInner, MAX_TIME_SLICE>;
        type Scheduler = scheduler::RRScheduler<TaskInner, MAX_TIME_SLICE>;
    }
}

type AxTaskRef = Arc<AxTask>;

pub fn current_may_uninit() -> Option<CurrentTask> {
    CurrentTask::try_get()
}

pub fn current() -> CurrentTask {
    CurrentTask::get()
}

pub fn init_scheduler() {
    info!("Initialize scheduling...");

    self::run_queue::init();
    self::timers::init();

    if cfg!(feature = "sched_fifo") {
        info!("  use FIFO scheduler.");
    } else if cfg!(feature = "sched_rr") {
        info!("  use Round-robin scheduler.");
    }
}

pub fn init_scheduler_secondary() {
    self::run_queue::init_secondary();
}

/// Handle periodic timer ticks for task manager, e.g. advance scheduler, update timer.
pub fn on_timer_tick() {
    self::timers::check_events();
    RUN_QUEUE.lock().scheduler_timer_tick();
}

pub fn spawn<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let task = TaskInner::new(f, "", axconfig::TASK_STACK_SIZE);
    RUN_QUEUE.lock().add_task(task);
}

pub fn yield_now() {
    RUN_QUEUE.lock().yield_current();
}

pub fn sleep(dur: core::time::Duration) {
    let deadline = axhal::time::current_time() + dur;
    RUN_QUEUE.lock().sleep_until(deadline);
}

pub fn sleep_until(deadline: axhal::time::TimeValue) {
    RUN_QUEUE.lock().sleep_until(deadline);
}

pub fn exit(exit_code: i32) -> ! {
    RUN_QUEUE.lock().exit_current(exit_code)
}

} else { // if #[cfg(feature = "multitask")]

pub fn yield_now() {
    axhal::arch::wait_for_irqs();
}

pub fn exit(exit_code: i32) -> ! {
    debug!("main task exited: exit_code={}", exit_code);
    axhal::misc::terminate()
}

pub fn sleep(dur: core::time::Duration) {
    let deadline = axhal::time::current_time() + dur;
    sleep_until(deadline)
}

pub fn sleep_until(deadline: axhal::time::TimeValue) {
    while axhal::time::current_time() < deadline {
        core::hint::spin_loop();
    }
}

} // else
} // cfg_if::cfg_if!

pub fn run_idle() -> ! {
    loop {
        #[cfg(feature = "multitask")]
        yield_now();
        debug!("idle task: waiting for IRQs...");
        axhal::arch::wait_for_irqs();
    }
}

cfg_if::cfg_if! {
if #[cfg(feature = "syscall")] {

use core::{alloc::Layout, arch::asm, borrow::Borrow, ptr::NonNull, sync::atomic::AtomicUsize};

use axconfig::TASK_STACK_SIZE;
use axhal::{
    arch::{enter_user, write_page_table_root},
    mem::virt_to_phys,
    paging::MappingFlags,
};
use axmem::MemorySet;
use memory_addr::{VirtAddr, PAGE_SIZE_4K};
use riscv::register::sstatus::{self, set_sum, Sstatus};
// use task::TaskStack;

extern crate alloc;

pub struct TaskStack {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl TaskStack {
    pub fn new(ptr: NonNull<u8>, layout: Layout) -> Self {
        Self { ptr, layout }
    }

    pub fn alloc(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 16).unwrap();
        Self {
            ptr: NonNull::new(unsafe { alloc::alloc::alloc(layout) }).unwrap(),
            layout,
        }
    }

    pub const fn top(&self) -> VirtAddr {
        unsafe { core::mem::transmute(self.ptr.as_ptr().add(self.layout.size())) }
    }

    pub fn bottom(&self) -> VirtAddr {
        VirtAddr::from(self.ptr.as_ptr() as usize)
    }
}

impl Drop for TaskStack {
    fn drop(&mut self) {
        unsafe { alloc::alloc::dealloc(self.ptr.as_ptr(), self.layout) }
    }
}

pub struct Task {
    pid: usize,
    tid: usize,

    entry_point: u64,
    memory_set: MemorySet,

    /// TaskStack is simply a pointer to memory in memory_set.
    /// Kernel stack is mapped in "free memory" region.
    kstack: TaskStack,
    /// User stack is mapped in user space (highest address)
    ustack: TaskStack,
}

impl Task {
    pub fn from_elf_data(elf_data: &[u8]) -> Self {
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
            axconfig::TASK_STACK_SIZE,
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
            entry_point: elf.header.pt2.entry_point(),
            memory_set,
            kstack,
            ustack,
        }
    }

    /// For Init (pid = 1) task only.
    /// Wwitch to its page_table
    /// Create a dummy TrapFrame to go to _user_start
    pub unsafe fn enter_as_init(&self) -> ! {
        use axhal::arch::TrapFrame;

        if self.pid != 1 || self.tid != 1 {
            panic!("Calling enter_user() for task other than (1, 1)");
        }

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
        core::ptr::write(
            (usize::from(self.kstack.top()) - core::mem::size_of::<TrapFrame>()) as *mut TrapFrame,
            trap_frame.clone(),
        );

        // set SPP to User, SUM to 1
        let sstatus_reg = sstatus::read();
        trap_frame.sstatus =
            *(&sstatus_reg as *const Sstatus as *const usize) & !(1 << 8) | (1 << 18);

        enter_user(&trap_frame, self.kstack.top().into());
    }
}

static TASK_ID_ALLOCATOR: AtomicUsize = AtomicUsize::new(1);

}
}
