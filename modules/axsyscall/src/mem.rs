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

#[def_interface]
pub trait SyscallMem {
    fn mmap(start: usize, len: usize, prot: u32, flags: u32, fd: usize, offset: usize) -> isize;
}
