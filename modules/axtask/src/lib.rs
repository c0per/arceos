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

/* type AxTask = scheduler::FifoTask<Task>;
type Scheduler = scheduler::FifoScheduler<Task>;

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
use task::{TaskStack, TaskState};

/* pub struct TaskStack {
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
} */


static TASK_ID_ALLOCATOR: AtomicUsize = AtomicUsize::new(1);

pub fn init_task(task: Task) -> ! {
    let task_ref = Arc::new(task);
    RUN_QUEUE.lock().add_task(task_ref);

    RUN_QUEUE.lock().switch_to_init();
} */

cfg_if::cfg_if! {
    // Use fifo scheduler if not specified
    if #[cfg(feature = "sched_fifo")] {
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
