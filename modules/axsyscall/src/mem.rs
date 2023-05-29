use crate_interface::{call_interface, def_interface};

pub(super) fn mmap(
    start: usize,
    len: usize,
    prot: u32,
    flags: u32,
    fd: usize,
    offset: usize,
) -> isize {
    call_interface!(SyscallMem::mmap, start, len, prot, flags, fd, offset)
}

pub(super) fn munmap(start: usize, len: usize) -> isize {
    call_interface!(SyscallMem::munmap, start, len)
}

pub(super) fn mprotect(start: usize, len: usize, prot: u32) -> isize {
    call_interface!(SyscallMem::mprotect, start, len, prot)
}

#[def_interface]
pub trait SyscallMem {
    fn mmap(start: usize, len: usize, prot: u32, flags: u32, fd: usize, offset: usize) -> isize;
    fn munmap(start: usize, len: usize) -> isize;
    fn mprotect(start: usize, len: usize, prot: u32) -> isize;
}
