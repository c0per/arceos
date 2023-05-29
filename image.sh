# make fat32 disk image
rm disk.img
dd if=/dev/zero of=disk.img bs=1M count=128
mkfs.vfat -F 32 disk.img

# copy test cases
# !!You need to compile it first:
# cd testsuits/riscv-syscalls-testing/user
# ./build-oscomp.sh
mkdir -p mnt
sudo mount disk.img mnt
sudo cp -r ./testsuits/riscv-syscalls-testing/user/riscv64/* ./mnt

# copy musl libc and argv.dout
sudo cp libc/lib* libc/argv.dout mnt/
sudo mkdir mnt/lib
sudo cp libc/ld-musl-riscv64.so.1 mnt/lib/

# unmount
sudo umount mnt
rm -r mnt
chmod 777 disk.img
