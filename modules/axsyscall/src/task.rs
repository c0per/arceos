use crate_interface::{call_interface, def_interface};

pub(super) fn exit(status: i32) -> ! {
    call_interface!(SyscallTask::exit, status)
}

#[def_interface]
pub trait SyscallTask {
    fn exit(status: i32) -> !;
}
