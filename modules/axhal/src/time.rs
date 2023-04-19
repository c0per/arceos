pub type TimeValue = core::time::Duration;

pub use crate::platform::time::{
    current_ticks, nanos_to_ticks, set_oneshot_timer, ticks_to_nanos, TIMER_IRQ_NUM,
};

pub const MILLIS_PER_SEC: u64 = 1_000;
pub const MICROS_PER_SEC: u64 = 1_000_000;
pub const NANOS_PER_SEC: u64 = 1_000_000_000;
pub const NANOS_PER_MILLIS: u64 = 1_000_000;
pub const NANOS_PER_MICROS: u64 = 1_000;

pub fn current_time_nanos() -> u64 {
    ticks_to_nanos(current_ticks())
}

pub fn current_time() -> TimeValue {
    TimeValue::from_nanos(current_time_nanos())
}

cfg_if::cfg_if! {
if #[cfg(feature = "syscall")] {

    struct SyscallTimeImpl;

    #[crate_interface::impl_interface]
    impl axsyscall::time::SyscallTime for SyscallTimeImpl {
        fn get_time_of_day(tv: *mut axsyscall::time::TimeVal, _tz: usize) -> isize {
            unsafe { *tv = current_time().into(); }

            0
        }
    }

}
}
