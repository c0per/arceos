use axsyscall::SyscallId;

pub use axsyscall::time::TimeVal;

pub fn get_time_of_day(tv: &mut TimeVal) -> isize {
    let result = crate::syscall::syscall(
        SyscallId::GetTimeOfDay,
        [tv as *mut TimeVal as usize, 0, 0, 0],
    );

    core::assert_eq!(result, 0, "Error calling get_time_of_day()");

    result
}

pub use core::time::Duration;

/// A measurement of a monotonically nondecreasing clock.
/// Opaque and useful only with [`Duration`].
#[derive(Debug)]
pub struct Instant(Duration);

impl Instant {
    /// Returns an instant corresponding to "now".
    pub fn now() -> Instant {
        let mut tv = TimeVal::default();
        get_time_of_day(&mut tv);

        Instant(tv.into())
    }

    /// Returns the amount of time elapsed from another instant to this one,
    /// or zero duration if that instant is later than this one.
    ///
    /// # Panics
    ///
    /// Previous rust versions panicked when `earlier` was later than `self`. Currently this
    /// method saturates. Future versions may reintroduce the panic in some circumstances.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.0 - earlier.0
    }

    /// Returns the amount of time elapsed since this instant was created.
    ///
    /// # Panics
    ///
    /// Previous rust versions panicked when the current time was earlier than self. Currently this
    /// method returns a Duration of zero in that case. Future versions may reintroduce the panic.
    pub fn elapsed(&self) -> Duration {
        Instant::now().0 - self.0
    }
}
