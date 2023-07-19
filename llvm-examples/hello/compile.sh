#!/bin/bash

LLVM_BUILD_DIR=$1

${LLVM_BUILD_DIR}/bin/clang -target acca-none-elf hello.c -o hello -ffreestanding -nostdinc -nostdlib -g -O0 -fuse-ld=${LLVM_BUILD_DIR}/bin/ld.lld -Wl,-T,hello.ld
${LLVM_BUILD_DIR}/bin/llvm-objcopy -O binary hello hello.bin
