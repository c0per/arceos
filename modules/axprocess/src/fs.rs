use super::scheduler::CurrentTask;
use crate::utils::raw_ptr_to_ref_str;
use alloc::sync::Arc;
use axsyscall::fs::{IoVec, Kstat, SyscallFs};
use bitflags::bitflags;
use core::slice::{from_raw_parts, from_raw_parts_mut};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use spinlock::SpinNoIrq;

struct SyscallFsImpl;

#[cfg(feature = "fs")]
#[crate_interface::impl_interface]
impl SyscallFs for SyscallFsImpl {
    fn fcntl(fd: usize, cmd: usize, arg: usize) -> isize {
        #[derive(FromPrimitive)]
        #[repr(usize)]
        #[allow(non_camel_case_types)]
        enum FcntlCmd {
            // Duplicating a file descriptor
            F_DUPFD = 0,
            // F_DUPFD_CLOEXEC = 1030,

            // File descriptor flags
            // F_GETFD = 1,
            // F_SETFD = 2,

            // File status flags
            // F_GETFL = 3,
            // F_SETFL = 3,
        }

        let Some(cmd) = FcntlCmd::from_usize(cmd) else {
            return -1;
        };

        let current = CurrentTask::try_get().expect("No current task");
        let mut fd_table = current.fd_table().lock();
        let Some(file) = fd_table.query_fd(fd) else {
            return -1;
        };

        use FcntlCmd::*;
        match cmd {
            F_DUPFD => {
                // clone a `Arc` pointing to the same file. *Duplicated fd share offset and file
                // struct.*
                let file = file.clone();

                fd_table.alloc_hint(arg, file) as isize
            }
        }
    }

    /// Open a file under directory noted by fd.
    /// flags: access mode, creation flags and status flags
    fn open_at(fd: usize, pathname: *const u8, flags: u32, mode: i32) -> isize {
        use axfs::api::OpenOptions;

        let pathname = unsafe { raw_ptr_to_ref_str(pathname) };

        bitflags! {
            /// File open flags from linux fcntl.h
            #[derive(Debug)]
            pub struct OpenFlags: u32 {
                // access mode
                const O_RDONLY = 0;
                const O_WRONLY = 1 << 0;
                const O_RDWR = 1 << 1;

                // creation flags
                const O_CREAT = 1 << 6;
                // const O_EXCL = 1 << 7;
                // const O_NOCTTY = 1 << 8;
                // const O_TRUNC = 1 << 9;
                // const O_DIRECTORY = 1 << 16;
                const O_NOFOLLOW = 1 << 17;
                const O_CLOEXEC = 1 << 19; // unimplemented

                // status flags
                // const O_APPEND = 1 << 10;
            }
        }

        info!("open flags: {:b}", flags);
        let flags = OpenFlags::from_bits(flags).expect("Unsupported file open flags");
        let mut open_opt = OpenOptions::new();

        if flags.contains(OpenFlags::O_WRONLY) {
            open_opt.write(true);
        } else if flags.contains(OpenFlags::O_RDWR) {
            open_opt.read(true).write(true);
        } else {
            open_opt.read(true);
        }

        info!("open open_opt: {:?}, flags: {:?}", open_opt, flags);

        // TODO: open_at
        if let Ok(file) = open_opt.open(pathname) {
            let current = CurrentTask::try_get().expect("No current task");
            let mut fd_table = current.fd_table().lock();

            fd_table.alloc(Arc::new(SpinNoIrq::new(file))) as isize
        } else {
            -1
        }
    }

    fn close(fd: usize) -> isize {
        let current = CurrentTask::try_get().expect("No current task");
        let mut fd_table = current.fd_table().lock();

        info!("[close] fd: {}", fd);

        if fd < fd_table.len() {
            // This function will panic if fd is out of bound.
            fd_table.remove(fd);
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

        let read_res = if let Some(file) = fd_table.query_fd(fd) {
            file.lock().read_full(buf).map_or(-1, |res| res as isize)
        } else {
            -1
        };

        info!(
            "[read] read len: {} / {}; buffer vaddr: 0x{:x}",
            read_res,
            buf.len(),
            buf.as_ptr() as usize
        );

        read_res
    }

    fn write(fd: usize, buf: *const u8, count: usize) -> isize {
        let current = CurrentTask::try_get().expect("No current task");
        let fd_table = current.fd_table().lock();

        let buf = unsafe { from_raw_parts(buf, count) };

        if let Some(file) = fd_table.query_fd(fd) {
            file.lock().write(buf).map_or(-1, |res| res as isize)
        } else {
            -1
        }
    }

    fn write_v(fd: usize, io_vec: *const IoVec, io_v_cnt: isize) -> isize {
        let mut io_vec = io_vec;

        debug!("write_v, {} vecs", io_v_cnt);
        let mut total_size = 0;
        for _ in 0..io_v_cnt {
            let vec = unsafe { &*io_vec };
            debug!("write_v to {:?}, len: {}", vec.io_v_base, vec.io_v_len);
            if vec.io_v_len > 0 {
                total_size += Self::write(fd, vec.io_v_base, vec.io_v_len);
            }

            io_vec = unsafe { io_vec.offset(1) };
        }

        debug!("write_v total: {}", total_size);

        total_size
    }

    // TODO
    fn fstat(fd: usize, kst: *mut axsyscall::fs::Kstat) -> isize {
        warn!("TODO: fstat");
        let current = CurrentTask::try_get().expect("No current task");
        let fd_table = current.fd_table().lock();

        if let Some(_) = fd_table.query_fd(fd) {
            let stat = Kstat {
                st_dev: 1,
                st_ino: 1,
                // st_mode: normal_file_mode(StMode::S_IFREG).bits(),
                // st_nlink: get_link_count(&FilePath::new(self.path.as_str())) as u32,
                st_mode: 0,
                st_nlink: 1,
                st_uid: 0,
                st_gid: 0,
                st_rdev: 0,
                _pad0: 0,
                // st_size: raw_metadata.size() as u64,
                st_size: 0,
                st_blksize: 0,
                _pad1: 0,
                // st_blocks: raw_metadata.blocks() as u64,
                st_blocks: 0,
                // st_atime_sec: stat.atime as isize,
                st_atime_sec: 0,
                st_atime_nsec: 0,
                // st_mtime_sec: stat.mtime as isize,
                st_mtime_sec: 0,
                st_mtime_nsec: 0,
                // st_ctime_sec: stat.ctime as isize,
                st_ctime_sec: 0,
                st_ctime_nsec: 0,
            };

            unsafe { *kst = stat };
            0
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
