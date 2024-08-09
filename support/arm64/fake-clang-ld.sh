#!/usr/bin/env bash

# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

MOZBUILD=$HOME/.mozbuild
SYSROOT=`pwd`/support/arm64/servo-arm64-sysroot

PARAMS=""

FORBIDDEN=("--strip-debug", "--strip-all", "--as-needed", "--eh-frame-hdr", "--gc-sections", "-znoexecstack", "-zrelro", "-znow")

for param in $*; do

case "${FORBIDDEN[@]}" in
  *"$param"*)
    echo "Variable is in the forbidden set: $param"
    ;;
  *)
    PARAMS+="$param "
    ;;
esac

done

$MOZBUILD/clang/bin/clang --target=aarch64-linux-gnu --sysroot=${SYSROOT} -fuse-ld=lld $PARAMS
