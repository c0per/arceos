#![no_std]

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

pub mod io;
pub mod task;
pub mod time;

#[derive(FromPrimitive)]
#[repr(usize)]
pub enum SyscallId {
    Write = 64,
    Exit = 93,
    SchedYield = 124,
    GetTimeOfDay = 169,
    Clone = 220,
}

pub fn syscall(syscall_id: usize, args: [usize; 4]) -> isize {
    if let Some(id) = SyscallId::from_usize(syscall_id) {
        use SyscallId::*;
        match id {
            Write => io::write(args[0], args[1] as *const u8, args[2]),
            Exit => task::exit(args[0] as i32),
            SchedYield => task::sched_yield(),
            GetTimeOfDay => time::get_time_of_day(args[0] as *mut time::TimeVal, args[1]),
            Clone => task::clone(),
        }
    } else {
        unimplemented!()
    }
}
