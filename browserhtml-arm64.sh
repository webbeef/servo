#!/bin/bash

set -e

PROFILE=${1:-"release"}

SYSROOT=`pwd`/support/arm64/servo-arm64-sysroot

export PKG_CONFIG_SYSROOT_DIR=${SYSROOT}
export PKG_CONFIG_ALLOW_CROSS=1

export CC=clang
export CXX=clang++
export AR=llvm-ar

CXX_INCLUDES="-I${SYSROOT}/usr/include/c++/8/"

# We need -fuse-ld=lld here for jemalloc-sys
export TARGET_CFLAGS=" --sysroot=${SYSROOT} -fuse-ld=lld -I${SYSROOT}/usr/include/aarch64-linux-gnu"
export TARGET_CXXFLAGS=" --sysroot=${SYSROOT} $CXX_INCLUDES"

export CFLAGS="$TARGET_CFLAGS -I${SYSROOT}/usr/include/aarch64-linux-gnu"

# Needed for mozjs bindgen
export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=${SYSROOT} $CXX_INCLUDES"

# Needed for cmake
export LDFLAGS="-L${SYSROOT}/lib/aarch64-linux-gnu -fuse-ld=lld"

# Build without tray icon or global key support
cargo build --target aarch64-unknown-linux-gnu --profile ${PROFILE} -p browserhtml \
    --no-default-features \
    --features="libservo/clipboard,js_jit,max_log_level,tracing,webgpu"

llvm-strip target/aarch64-unknown-linux-gnu/${PROFILE}/browserhtml

REMOTE_DIR=/home/mobian/browserhtml

echo "Pushing update..."

# Create the browserhtml directory if needed.
ssh mobian@mobian 'mkdir -p /home/mobian/browserhtml'

# rsync the binary
rsync -vz --progress target/aarch64-unknown-linux-gnu/${PROFILE}/browserhtml \
                     mobian@mobian:${REMOTE_DIR}/browserhtml

# rsync the resources
rsync -avz --progress resources mobian@mobian:${REMOTE_DIR}/

echo "done!"
