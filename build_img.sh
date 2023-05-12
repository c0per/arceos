# make fat32 disk image
rm disk.img
dd if=/dev/zero of=disk.img bs=1M count=128
mkfs.vfat -F 32 disk.img

# copy test cases
mkdir -p mnt
sudo mount disk.img mnt
sudo cp -r ./testsuits/riscv-syscalls-testing/user/riscv64/* ./mnt
sudo umount mnt
rm -r mnt
chmod 777 disk.img
