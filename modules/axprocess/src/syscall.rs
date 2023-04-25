use crate::scheduler::SCHEDULER;

struct SyscallTaskImpl;

#[crate_interface::impl_interface]
impl axsyscall::task::SyscallTask for SyscallTaskImpl {
    fn exit(status: i32) -> ! {
        SCHEDULER.lock().exit_current()
    }

    fn clone() -> isize {
        SCHEDULER.lock().clone_current() as isize
    }

    fn sched_yield() -> isize {
        SCHEDULER.lock().yield_current();
        0
    }
}
