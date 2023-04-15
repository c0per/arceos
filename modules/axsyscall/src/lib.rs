#![no_std]

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

pub mod time;

#[derive(FromPrimitive)]
#[repr(usize)]
pub enum SyscallId {
    Exit = 93,
    GetTimeOfDay = 169,
}

pub fn syscall(syscall_id: usize, args: [usize; 4]) -> isize {
    if let Some(id) = SyscallId::from_usize(syscall_id) {
        use SyscallId::*;
        match id {
            Exit => axtask::exit(args[0] as i32),
            GetTimeOfDay => time::get_time_of_day(args[0] as *mut time::TimeVal, args[1]),
        }
    } else {
        unimplemented!()
    }
}
