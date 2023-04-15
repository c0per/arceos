#![no_std]
#![no_main]

// Build with libax to get global_allocator and stuff.
#[macro_use]
extern crate libax;

use libax::time::Instant;

#[no_mangle]
fn main() -> i32 {
    let t = Instant::now();

    println!("Hello, world!. It's {:?}", t);

    0
}
