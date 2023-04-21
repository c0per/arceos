#![no_std]
#![no_main]

extern crate axruntime;

core::arch::global_asm! {
    ".section .data",

    ".global app_start",
    ".global app_end",
    ".align 3",

    "app_start:",
    r#".incbin "./app.elf""#, // relative to ArceOS root directory
    "app_end:"
}
