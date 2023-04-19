pub fn putchar(c: u8) {
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c as usize);
}

pub fn getchar() -> Option<u8> {
    #[allow(deprecated)]
    match sbi_rt::legacy::console_getchar() as isize {
        -1 => None,
        c => Some(c as u8),
    }
}

cfg_if::cfg_if! {
if #[cfg(feature = "syscall")] {

    struct SyscallIOImpl;

    #[crate_interface::impl_interface]
    impl axsyscall::io::SyscallIO for SyscallIOImpl {
        fn write(fd: usize, buf: *const u8, len: usize) -> isize {
            // TODO: translate user address
            // TODO: fs other than stdout
            if fd != 1 {
                unimplemented!()
            }

            let buf = unsafe { core::slice::from_raw_parts(buf, len) };

            crate::console::write_bytes(buf);

            buf.len() as isize
        }
    }
}
}
