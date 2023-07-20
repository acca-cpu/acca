#!/bin/bash

set -e

LLVM_BUILD_DIR=$1

SRC_DIR=$(pwd)
BUILD_DIR="${SRC_DIR}/build"

mkdir -p "${BUILD_DIR}"

LLVM_MAJOR_VER=$(${LLVM_BUILD_DIR}/bin/llvm-config --version | cut -d. -f 1)

CLANG_BUILTIN_INCLUDE="${LLVM_BUILD_DIR}/lib/clang/${LLVM_MAJOR_VER}/include"

${LLVM_BUILD_DIR}/bin/clang \
	-target acca-none-elf \
	"${SRC_DIR}/hello.c" \
	-o "${BUILD_DIR}/hello" \
	-ffreestanding \
	-nostdinc \
	-nostdlib \
	"-I${CLANG_BUILTIN_INCLUDE}" \
	"-I${SRC_DIR}/../include" \
	-g \
	-O0 \
	"-fuse-ld=${LLVM_BUILD_DIR}/bin/ld.lld" \
	"-Wl,-T,${SRC_DIR}/../link.ld" \
	-MJ "${BUILD_DIR}/hello.json"

${LLVM_BUILD_DIR}/bin/llvm-objcopy \
	-O binary \
	"${BUILD_DIR}/hello" \
	"${BUILD_DIR}/hello.bin"
