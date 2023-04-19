use crate_interface::{call_interface, def_interface};

pub(super) fn write(fd: usize, buf: *const u8, len: usize) -> isize {
    // TODO: translate user address
    // TODO: fs other than stdout
    // if fd != 1 {
    //     unimplemented!()
    // }
    //
    // let buf = unsafe { core::slice::from_raw_parts(buf, len) };
    //
    // axhal::console::write_bytes(buf);
    //
    // buf.len() as isize

    call_interface!(SyscallIO::write, fd, buf, len)
}

#[def_interface]
pub trait SyscallIO {
    fn write(fd: usize, buf: *const u8, len: usize) -> isize;
}
