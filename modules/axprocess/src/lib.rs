#![no_std]

#[macro_use]
extern crate alloc;

pub mod scheduler;
mod stack;
mod syscall;
mod task;

pub use task::{Task, TaskState};

struct KernelGuardIfImpl;

#[crate_interface::impl_interface]
impl kernel_guard::KernelGuardIf for KernelGuardIfImpl {
    fn disable_preempt() {}

    fn enable_preempt() {}
}
