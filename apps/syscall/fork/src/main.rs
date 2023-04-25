#![no_std]
#![no_main]

// Build with libax to get global_allocator and stuff.
#[macro_use]
extern crate libax;

use libax::{
    task::{fork, sched_yield},
    time::Instant,
};

#[no_mangle]
fn main() -> i32 {
    let t = Instant::now();

    println!("Hello, world!. It's {:?}", t);

    if fork() == 0 {
        println!("Child process");
    } else {
        println!("Parent process");
        sched_yield();
        println!("Parent process: after yield");
    }

    0
}
