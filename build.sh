#!/usr/bin/env bash

export LOG=debug

# go to modules/axuser to build kernel
rm kernel.bin
cd modules/axuser
cargo b -p axuser
cd -
rust-objcopy -B riscv64 -S -O binary target/riscv64gc-unknown-none-elf/debug/axuser kernel.bin

# build testsuits
# cd testsuits/riscv-syscalls-testing/user
# rm -r ./build ./riscv64
# ./build-oscomp.sh
# cd -
