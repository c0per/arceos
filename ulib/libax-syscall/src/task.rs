use crate::syscall::syscall;
use axsyscall::SyscallId;

pub fn exit(status: i32) -> ! {
    syscall(SyscallId::Exit, [status as usize, 0, 0, 0]);
    unreachable!("Task already called exit().")
}

pub fn fork() -> isize {
    syscall(SyscallId::Clone, [0, 0, 0, 0])
}

pub fn sched_yield() -> isize {
    syscall(SyscallId::SchedYield, [0, 0, 0, 0])
}
