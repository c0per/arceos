cfg_if::cfg_if! {
    if #[cfg(all(
        any(target_arch = "riscv32", target_arch = "riscv64"),
        feature = "platform-qemu-virt-riscv"
    ))] {
        mod qemu_virt_riscv;
        pub use self::qemu_virt_riscv::*;
    } else if #[cfg(all(
        target_arch = "aarch64",
        feature = "platform-qemu-virt-aarch64"
    ))] {
        mod qemu_virt_aarch64;
        pub use self::qemu_virt_aarch64::*;
    } else {
        mod dummy;
        pub use self::dummy::*;
    }
}
