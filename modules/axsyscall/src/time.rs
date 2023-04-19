use crate_interface::{call_interface, def_interface};

#[allow(dead_code)]
pub struct TimeVal {
    secs: isize,
    usecs: isize,
}

impl From<core::time::Duration> for TimeVal {
    fn from(value: core::time::Duration) -> Self {
        Self {
            secs: value.as_secs() as isize,
            usecs: value.as_micros() as isize,
        }
    }
}

impl From<TimeVal> for core::time::Duration {
    fn from(value: TimeVal) -> Self {
        Self::from_micros(value.usecs as u64)
    }
}

impl Default for TimeVal {
    fn default() -> Self {
        Self { secs: 0, usecs: 0 }
    }
}

pub(super) fn get_time_of_day(tv: *mut TimeVal, tz: usize) -> isize {
    call_interface!(SyscallTime::get_time_of_day, tv, tz)
}

#[def_interface]
pub trait SyscallTime {
    fn get_time_of_day(tv: *mut TimeVal, _tz: usize) -> isize;
}
