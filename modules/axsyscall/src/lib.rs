#![no_std]

#[macro_use]
extern crate log;

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

pub mod fs;
pub mod mem;
pub mod task;
pub mod time;

#[derive(FromPrimitive)]
#[repr(usize)]
pub enum SyscallId {
    Fcntl = 25,
    IoCtl = 29,
    OpenAt = 56,
    Close = 57,
    Read = 63,
    Write = 64,
    WriteV = 66,
    Fstat = 80,
    Exit = 93,
    ExitGroup = 94,
    SetTidAddress = 96,
    SchedYield = 124,
    GetTimeOfDay = 169,
    MUnmap = 215,
    Clone = 220,
    MMap = 222,
    MProtect = 226,
}

/// syscall dispatcher
pub fn syscall(syscall_id: usize, args: [usize; 6]) -> isize {
    if let Some(id) = SyscallId::from_usize(syscall_id) {
        use SyscallId::*;
        match id {
            Fcntl => fs::fcntl(args[0], args[1], args[2]),
            IoCtl => {
                warn!("Unimplemented syscall: 29 ioctl, ignored.");
                0
            }
            OpenAt => fs::open_at(
                args[0],
                args[1] as *const u8,
                args[2] as u32,
                args[3] as i32,
            ),
            Close => fs::close(args[0]),
            Read => fs::read(args[0], args[1] as *const u8, args[2]),
            Write => fs::write(args[0], args[1] as *const u8, args[2]),
            WriteV => fs::write_v(args[0], args[1] as *const fs::IoVec, args[2] as isize),
            Fstat => fs::fstat(args[0], args[1] as *mut fs::Kstat),
            Exit => task::exit(args[0] as i32),
            ExitGroup => {
                warn!("Unimplemented syscall: 94 exit_group, call exit() instead.");
                task::exit(args[0] as i32)
            }
            SetTidAddress => {
                warn!("Unimplemented syscall: 96 set_tid_address, ignored.");
                0
            }
            SchedYield => task::sched_yield(),
            GetTimeOfDay => time::get_time_of_day(args[0] as *mut time::TimeVal, args[1]),
            MUnmap => mem::munmap(args[0], args[1]),
            Clone => task::clone(args[0], args[1]),
            MMap => mem::mmap(
                args[0],
                args[1],
                args[2] as u32,
                args[3] as u32,
                args[4],
                args[5],
            ),
            MProtect => mem::mprotect(args[0], args[1], args[2] as u32),
        }
    } else {
        unimplemented!("syscall id: {}", syscall_id)
    }
}
