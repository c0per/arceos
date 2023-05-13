# run qemu
qemu-system-riscv64 -machine virt -m 128M -smp 1 \
    -bios default -kernel kernel.bin -nographic \
    -device virtio-blk-device,drive=disk0 \
    -drive id=disk0,if=none,format=raw,file=disk.img
