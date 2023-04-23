#![no_std]
#![feature(panic_info_message)]
#![feature(linkage)]

extern crate core;

pub mod io;
pub mod syscall;
pub mod time;

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _user_start() {
    clear_bss();
    extern "Rust" {
        fn main() -> i32;
    }

    let return_value = unsafe { main() };

    syscall::exit(return_value);
}

#[linkage = "weak"]
#[link_section = ".text.entry"]
fn main() -> i32 {
    panic!("No user main defined");
}

fn clear_bss() {
    extern "C" {
        fn start_bss();
        fn end_bss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(
            start_bss as usize as *mut u8,
            end_bss as usize - start_bss as usize,
        )
        .fill(0);
    }
}
