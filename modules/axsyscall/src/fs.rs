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

pub(super) fn write_v(fd: usize, io_vec: *const IoVec, io_v_cnt: isize) -> isize {
    call_interface!(SyscallFs::write_v, fd, io_vec, io_v_cnt)
}

pub(super) fn fstat(fd: usize, kst: *mut Kstat) -> isize {
    call_interface!(SyscallFs::fstat, fd, kst)
}

#[def_interface]
pub trait SyscallFs {
    fn open_at(fd: usize, filename: *const u8, flags: u32, mode: i32) -> isize;
    fn close(fd: usize) -> isize;
    fn read(fd: usize, buf: *const u8, count: usize) -> isize;
    fn write(fd: usize, buf: *const u8, count: usize) -> isize;
    fn write_v(fd: usize, io_vec: *const IoVec, io_v_cnt: isize) -> isize;
    fn fstat(fd: usize, kst: *mut Kstat) -> isize;
}

#[repr(C)]
pub struct IoVec {
    pub io_v_base: *mut u8,
    pub io_v_len: usize,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Kstat {
    /// 设备
    pub st_dev: u64,
    /// inode 编号
    pub st_ino: u64,
    /// 文件类型
    pub st_mode: u32,
    /// 硬链接数
    pub st_nlink: u32,
    /// 用户id
    pub st_uid: u32,
    /// 用户组id
    pub st_gid: u32,
    /// 设备号
    pub st_rdev: u64,
    pub _pad0: u64,
    /// 文件大小
    pub st_size: u64,
    /// 块大小
    pub st_blksize: u32,
    pub _pad1: u32,
    /// 块个数
    pub st_blocks: u64,
    /// 最后一次访问时间(秒)
    pub st_atime_sec: isize,
    /// 最后一次访问时间(纳秒)
    pub st_atime_nsec: isize,
    /// 最后一次修改时间(秒)
    pub st_mtime_sec: isize,
    /// 最后一次修改时间(纳秒)
    pub st_mtime_nsec: isize,
    /// 最后一次改变状态时间(秒)
    pub st_ctime_sec: isize,
    /// 最后一次改变状态时间(纳秒)
    pub st_ctime_nsec: isize,
}
