use core::slice::{from_raw_parts, from_raw_parts_mut};

use super::scheduler::CurrentTask;
use axsyscall::fs::SyscallFs;
use crate_interface::impl_interface;

struct SyscallFsImpl;

#[cfg(feature = "fs")]
#[crate_interface::impl_interface]
impl SyscallFs for SyscallFsImpl {
    fn open_at(fd: usize, filename: *const u8, flags: u32, mode: i32) -> isize {
        todo!()
    }

    fn close(fd: usize) -> isize {
        let current = CurrentTask::try_get().expect("No current task");
        let mut fd_table = current.fd_table().lock();

        if let Some(file) = fd_table.query_fd_mut(fd) {
            file.take();

            0
        } else {
            // closing a fd doesn't exist or alread closed
            // EBADF
            -1
        }
    }

    fn read(fd: usize, buf: *const u8, count: usize) -> isize {
        let current = CurrentTask::try_get().expect("No current task");
        let fd_table = current.fd_table().lock();

        let buf = unsafe { from_raw_parts_mut(buf as *mut u8, count) };

        if let Some(file) = fd_table.query_fd(fd) {
            file.borrow_mut().read(buf).map_or(-1, |res| res as isize)
        } else {
            -1
        }
    }

    fn write(fd: usize, buf: *const u8, count: usize) -> isize {
        let current = CurrentTask::try_get().expect("No current task");
        let fd_table = current.fd_table().lock();

        let buf = unsafe { from_raw_parts(buf, count) };

        if let Some(file) = fd_table.query_fd(fd) {
            file.borrow_mut().write(buf).map_or(-1, |res| res as isize)
        } else {
            -1
        }
    }
}

#[cfg(not(feature = "fs"))]
#[crate_interface::impl_interface]
impl SyscallFs for SyscallFsImpl {
    fn open_at(fd: usize, filename: *const u8, flags: u32, mode: i32) -> isize {
        unimplemented!()
    }

    fn close(fd: usize) -> isize {
        unimplemented!()
    }

    fn read(fd: usize, buf: *const u8, count: usize) -> isize {
        unimplemented!()
    }

    fn write(fd: usize, buf: *const u8, count: usize) -> isize {
        let buf = unsafe { from_raw_parts(buf, count) };

        console::write_bytes(buf);

        if fd == 1 || fd == 2 {
            Ok(buf.len())
        } else {
            -1
        }
    }
}
