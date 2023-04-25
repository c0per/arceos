use crate_interface::{call_interface, def_interface};

pub(super) fn exit(status: i32) -> ! {
    call_interface!(SyscallTask::exit, status)
}

pub(super) fn clone() -> isize {
    call_interface!(SyscallTask::clone)
}

pub(super) fn sched_yield() -> isize {
    call_interface!(SyscallTask::sched_yield)
}

#[def_interface]
pub trait SyscallTask {
    fn exit(status: i32) -> !;
    fn clone() -> isize;
    fn sched_yield() -> isize;
}
