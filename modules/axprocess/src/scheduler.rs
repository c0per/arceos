use crate::{Task, TaskState};
use alloc::sync::Arc;
use axhal::arch::write_page_table_root;
use core::{mem::ManuallyDrop, ops::Deref};
use lazy_init::LazyInit;
use scheduler::{BaseScheduler, FifoTask};
use spinlock::SpinNoIrq;

pub type AxTask = scheduler::FifoTask<Task>;
pub type AxScheduler = scheduler::FifoScheduler<Task>;
pub struct Scheduler(AxScheduler);

pub(crate) static SCHEDULER: LazyInit<SpinNoIrq<Scheduler>> = LazyInit::new();

impl Scheduler {
    pub fn add_task(&mut self, task: Task) {
        self.0.add_task(Arc::new(FifoTask::new(task)));
    }

    pub fn start(task: Task) -> ! {
        let task = Arc::new(scheduler::FifoTask::new(task));
        CurrentTask::init(task.clone());

        unsafe {
            // Task only exists in CurrentTask when excuting.
            assert_eq!(Arc::strong_count(&task), 2);

            // enter_as_init won't return, we need to decrement the Arc count.
            let ptr = Arc::into_raw(task);
            Arc::decrement_strong_count(ptr);

            (&*ptr).inner().enter_as_init();
        }
    }

    pub fn exit_current(&mut self) -> ! {
        let mut curr = CurrentTask::try_get().expect("Current task not found");

        if curr.0.is_init() {
            axhal::misc::terminate();
        } else {
            curr.0.set_state(TaskState::Exited);

            self.reschedule();
        }
        unreachable!("Task already exited");
    }

    pub fn yield_current(&mut self) {
        self.reschedule();
    }

    pub fn clone_current(&mut self) -> usize {
        let mut curr = CurrentTask::try_get().expect("Current task not found");

        let new_task = curr.clone();
        let new_tid = new_task.tid;

        self.add_task(new_task);

        new_tid
    }
}

impl Scheduler {
    fn reschedule(&mut self) {
        debug!("re-scheduling");
        let prev = CurrentTask::try_get().expect("Current task not found");
        if prev.is_running() {
            prev.set_state(TaskState::Ready);
            self.0.put_prev_task(prev.0.deref().clone(), false);
        }

        let next = self.0.pick_next_task().expect("TODO: idle task");

        self.switch_to(prev, next);
    }

    fn switch_to(&mut self, prev: CurrentTask, next: Arc<AxTask>) {
        next.set_state(TaskState::Running);

        let prev_ctx = prev.ctx_mut_ptr();
        let next_ctx = next.ctx_mut_ptr();

        let next_pid = next.pid;
        let next_tid = next.tid;

        let next_page_table_ppn = next.page_table_ppn();

        CurrentTask::set(next);

        debug!(
            "[switch] from ({}, {}) to ({}, {})",
            prev.pid, prev.tid, next_pid, next_tid
        );
        unsafe {
            write_page_table_root(next_page_table_ppn);
            (*prev_ctx).switch_to(&mut *next_ctx);
        }
    }
}

pub struct CurrentTask(ManuallyDrop<Arc<AxTask>>);

impl CurrentTask {
    pub(crate) fn try_get() -> Option<Self> {
        let ptr = axhal::cpu::current_task_ptr::<AxTask>();
        if ptr.is_null() {
            None
        } else {
            Some(Self(ManuallyDrop::new(unsafe { Arc::from_raw(ptr) })))
        }
    }

    pub(crate) fn init(task: Arc<AxTask>) {
        let ptr = Arc::into_raw(task);
        unsafe {
            axhal::cpu::set_current_task_ptr(ptr);
        }
    }

    pub(crate) fn set(task: Arc<AxTask>) {
        let ptr = axhal::cpu::current_task_ptr::<AxTask>();
        unsafe {
            Arc::decrement_strong_count(ptr);
        }

        let ptr = Arc::into_raw(task);
        unsafe {
            axhal::cpu::set_current_task_ptr(ptr);
        }
    }
}

impl Deref for CurrentTask {
    type Target = Task;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

pub fn init() {
    let scheduler = scheduler::FifoScheduler::new();

    SCHEDULER.init_by(SpinNoIrq::new(Scheduler(scheduler)));
}

pub fn start(task: Task) -> ! {
    Scheduler::start(task)
}
