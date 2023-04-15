#![no_std]
#![no_main]

// Build with libax to get global_allocator and stuff.
extern crate libax;

use libax::time::Instant;

#[no_mangle]
fn main() -> i32 {
    let _ = Instant::now();

    0
}
