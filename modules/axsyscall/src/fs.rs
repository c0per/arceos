use crate_interface::{call_interface, def_interface};

pub(super) fn open_at(fd: usize, filename: *const u8, flags: u32, mode: i32) -> isize {
    call_interface!(SyscallFs::open_at, fd, filename, flags, mode)
}

pub(super) fn close(fd: usize) -> isize {
    call_interface!(SyscallFs::close, fd)
}

pub(super) fn read(fd: usize, buf: *const u8, count: usize) -> isize {
    call_interface!(SyscallFs::read, fd, buf, count)
}

pub(super) fn write(fd: usize, buf: *const u8, count: usize) -> isize {
    call_interface!(SyscallFs::write, fd, buf, count)
}

#[def_interface]
pub trait SyscallFs {
    fn open_at(fd: usize, filename: *const u8, flags: u32, mode: i32) -> isize;
    fn close(fd: usize) -> isize;
    fn read(fd: usize, buf: *const u8, count: usize) -> isize;
    fn write(fd: usize, buf: *const u8, count: usize) -> isize;
}
