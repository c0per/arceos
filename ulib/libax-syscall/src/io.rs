struct Logger;

impl core::fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        crate::syscall::write(1, s.as_ptr(), s.len());
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::io::__print_impl(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::io::__print_impl(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}

#[doc(hidden)]
pub fn __print_impl(args: core::fmt::Arguments) {
    use core::fmt::Write;
    Logger.write_fmt(args).unwrap();
}
