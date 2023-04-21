use axsyscall::SyscallId;

pub(crate) fn syscall(id: SyscallId, args: [usize; 4]) -> isize {
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

pub fn exit(status: i32) -> ! {
    syscall(SyscallId::Exit, [status as usize, 0, 0, 0]);
    unreachable!("Task already called exit().")
}

pub fn write(fd: usize, buf: *const u8, len: usize) -> isize {
    syscall(SyscallId::Write, [fd, buf as usize, len, 0])
}

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let err = info.message().unwrap();
    if let Some(location) = info.location() {
        crate::println!(
            "Panicked at {}:{}, {}",
            location.file(),
            location.line(),
            err
        );
    } else {
        crate::println!("Panicked: {}", err);
    }

    exit(-1);
}
