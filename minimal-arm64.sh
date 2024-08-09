#!/bin/bash

set -e

PROFILE=release

SYSROOT=`pwd`/support/arm64/servo-arm64-sysroot
TOOLCHAIN=${HOME}/.mozbuild/clang/bin

export PKG_CONFIG_SYSROOT_DIR=${SYSROOT}
export PKG_CONFIG_ALLOW_CROSS=1

export CC=${TOOLCHAIN}/clang
export CXX=${TOOLCHAIN}/clang++
export AR=${TOOLCHAIN}/llvm-ar

CXX_INCLUDES="-I${SYSROOT}/usr/include/c++/8/"

# We need -fuse-ld=lld here for jemalloc-sys
export TARGET_CFLAGS="--sysroot=${SYSROOT} -fuse-ld=lld -I${SYSROOT}/usr/include/aarch64-linux-gnu"
export TARGET_CXXFLAGS="--sysroot=${SYSROOT} $CXX_INCLUDES"

export CFLAGS="$TARGET_CFLAGS -I${SYSROOT}/usr/include/aarch64-linux-gnu"

# Needed for mozjs bindgen
export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=${SYSROOT} $CXX_INCLUDES"

# Needed for cmake
export LDFLAGS="-L${SYSROOT}/lib/aarch64-linux-gnu -fuse-ld=lld"

cargo build --target aarch64-unknown-linux-gnu --profile ${PROFILE} -p libservo --example winit_minimal

llvm-strip target/aarch64-unknown-linux-gnu/${PROFILE}/examples/winit_minimal

scp ./target/aarch64-unknown-linux-gnu/${PROFILE}/examples/winit_minimal mobian@mobian:/home/mobian/
