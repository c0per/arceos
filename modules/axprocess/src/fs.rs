use super::scheduler::CurrentTask;
use axsyscall::fs::SyscallFs;
use bitflags::bitflags;
use core::slice::{from_raw_parts, from_raw_parts_mut};
use crate_interface::impl_interface;

struct SyscallFsImpl;

#[cfg(feature = "fs")]
#[crate_interface::impl_interface]
impl SyscallFs for SyscallFsImpl {
    /// Open a file under directory noted by fd.
    /// flags: access mode, creation flags and status flags
    fn open_at(fd: usize, pathname: *const u8, flags: u32, mode: i32) -> isize {
        use axfs::api::OpenOptions;

        let pathname = unsafe { raw_ptr_to_ref_str(pathname) };

        bitflags! {
            /// File open flags from linux fcntl.h
            pub struct OpenFlags: u32 {
                // access mode
                const O_RDONLY = 0;
                const O_WRONLY = 1 << 0;
                const O_RDWR = 1 << 1;

                // creation flags
                // const O_CREAT = 1 << 6;
                // const O_EXCL = 1 << 7;
                // const O_NOCTTY = 1 << 8;
                // const O_TRUNC = 1 << 9;
                // const O_DIRECTORY = 1 << 16;
                // const O_NOFOLLOW = 1 << 17;
                // const O_CLOEXEC = 1 << 19;

                // status flags
                // const O_APPEND = 1 << 10;
            }
        }

        let flags = OpenFlags::from_bits(flags).expect("Unsupported file open flags");
        let mut open_opt = OpenOptions::new();

        if flags.contains(OpenFlags::O_RDONLY) {
            open_opt.read(true);
        } else if flags.contains(OpenFlags::O_WRONLY) {
            open_opt.write(true);
        } else {
            open_opt.read(true).write(true);
        }

        // TODO: open_at
        if let Ok(file) = open_opt.open(pathname) {
            let current = CurrentTask::try_get().expect("No current task");
            let mut fd_table = current.fd_table().lock();

            fd_table.alloc_fd(file)
        } else {
            -1
        }
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

// util functions from maturin

unsafe fn get_str_len(start: *const u8) -> usize {
    let mut ptr = start as usize;
    while *(ptr as *const u8) != 0 {
        ptr += 1;
    }
    ptr - start as usize
}

unsafe fn raw_ptr_to_ref_str(start: *const u8) -> &'static str {
    let len = get_str_len(start);
    // 因为这里直接用用户空间提供的虚拟地址来访问，所以一定能连续访问到字符串，不需要考虑物理地址是否连续
    let slice = core::slice::from_raw_parts(start, len);
    if let Ok(s) = core::str::from_utf8(slice) {
        s
    } else {
        error!("not utf8 slice");
        for c in slice {
            error!("{c} ");
        }
        error!("");
        &"p"
    }
}
