#![no_std]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;

mod fs;
pub mod scheduler;
mod stack;
mod syscall;
mod task;

#[cfg(feature = "fs")]
mod stdio;

#[cfg(feature = "fs")]
mod fd;

pub use task::{Task, TaskState};

struct KernelGuardIfImpl;

#[crate_interface::impl_interface]
impl kernel_guard::KernelGuardIf for KernelGuardIfImpl {
    fn disable_preempt() {}

    fn enable_preempt() {}
}
