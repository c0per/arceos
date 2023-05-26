#![no_std]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;

mod fs;
mod loader;
mod mem;
pub mod scheduler;
mod stack;
mod syscall;
mod task;
mod utils;

#[cfg(feature = "fs")]
mod stdio;

#[cfg(feature = "fs")]
mod fd;

pub use loader::Loader;
pub use task::{Task, TaskState};

struct KernelGuardIfImpl;

#[crate_interface::impl_interface]
impl kernel_guard::KernelGuardIf for KernelGuardIfImpl {
    fn disable_preempt() {}

    fn enable_preempt() {}
}
