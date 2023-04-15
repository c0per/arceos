cfg_if::cfg_if! {
if #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))] {
    fn syscall(id: SyscallId, args: [usize; 4]) -> isize {
        let mut ret: isize;
        unsafe {
            core::arch::asm!(
                "ecall",
                inlateout("x10") args[0] => ret,
                in("x11") args[1],
                in("x12") args[2],
                in("x13") args[3],
                in("x17") id as usize
            );
        }
        ret
    }
} else {
    fn syscall(id: SyscallId, args: [usize; 4]) -> isize {
        unimplemented!();
    }
}
}

use axsyscall::SyscallId;

pub(crate) use axsyscall::time::TimeVal;

pub(crate) fn get_time_of_day(tv: &mut TimeVal) -> isize {
    let result = syscall(
        SyscallId::GetTimeOfDay,
        [tv as *mut TimeVal as usize, 0, 0, 0],
    );

    assert_eq!(result, 0, "Error calling get_time_of_day()");

    result
}

pub(crate) fn exit(status: i32) -> ! {
    syscall(SyscallId::Exit, [status as usize, 0, 0, 0]);
    unreachable!("Task already called exit().")
}

pub(crate) fn write(fd: usize, buf: *const u8, len: usize) -> isize {
    syscall(SyscallId::Write, [fd, buf as usize, len, 0])
}

#[no_mangle]
extern "C" fn _user_start() {
    extern "Rust" {
        fn main() -> i32;
    }

    let return_value = unsafe { main() };

    exit(return_value);
}
