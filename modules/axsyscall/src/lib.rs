#![no_std]

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

pub mod fs;
pub mod mem;
pub mod task;
pub mod time;

#[derive(FromPrimitive)]
#[repr(usize)]
pub enum SyscallId {
    OpenAt = 56,
    Close = 57,
    Read = 63,
    Write = 64,
    Fstat = 80,
    Exit = 93,
    SchedYield = 124,
    GetTimeOfDay = 169,
    MUnmap = 215,
    Clone = 220,
    MMap = 222,
}

/// syscall dispatcher
pub fn syscall(syscall_id: usize, args: [usize; 6]) -> isize {
    if let Some(id) = SyscallId::from_usize(syscall_id) {
        use SyscallId::*;
        match id {
            OpenAt => fs::open_at(
                args[0],
                args[1] as *const u8,
                args[2] as u32,
                args[3] as i32,
            ),
            Close => fs::close(args[0]),
            Read => fs::read(args[0], args[1] as *const u8, args[2]),
            Write => fs::write(args[0], args[1] as *const u8, args[2]),
            Fstat => fs::fstat(args[0], args[1] as *mut fs::Kstat),
            Exit => task::exit(args[0] as i32),
            SchedYield => task::sched_yield(),
            GetTimeOfDay => time::get_time_of_day(args[0] as *mut time::TimeVal, args[1]),
            MUnmap => mem::munmap(args[0], args[1]),
            Clone => task::clone(),
            MMap => mem::mmap(
                args[0],
                args[1],
                args[2] as u32,
                args[3] as u32,
                args[4],
                args[5],
            ),
        }
    } else {
        unimplemented!("syscall id: {}", syscall_id)
    }
}
