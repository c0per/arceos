#![no_std]

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

mod io;
pub mod time;

#[derive(FromPrimitive)]
#[repr(usize)]
pub enum SyscallId {
    Write = 64,
    Exit = 93,
    GetTimeOfDay = 169,
}

pub fn syscall(syscall_id: usize, args: [usize; 4]) -> isize {
    if let Some(id) = SyscallId::from_usize(syscall_id) {
        use SyscallId::*;
        match id {
            Write => io::write(args[0], args[1] as *const u8, args[2]),
            Exit => axtask::exit(args[0] as i32),
            GetTimeOfDay => time::get_time_of_day(args[0] as *mut time::TimeVal, args[1]),
        }
    } else {
        unimplemented!()
    }
}
