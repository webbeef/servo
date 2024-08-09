#!/bin/bash

set -x
set -e

sysroot="servo-arm64-sysroot"

packages="
  linux-libc-dev
  libasound2-dev
  libstdc++-8-dev
  libfontconfig1-dev
  libfreetype6-dev
  libgconf2-dev
  libgcc-8-dev
  libgtk-3-dev
  libpango1.0-dev
  libpulse-dev
  libudev-dev
  libx11-xcb-dev
  libxt-dev
  $*
"

echo "deb http://snapshot.debian.org/archive/debian/20230611T210420Z buster main" | mmdebstrap \
    --architectures=arm64 \
    --variant=extract \
    --mode=fakeroot \
    --include=$(echo $packages | tr ' ' ,) \
    buster $sysroot \
    --dpkgopt=path-exclude="*" \
    --dpkgopt=path-include="/lib/*" \
    --dpkgopt=path-include="/lib32/*" \
    --dpkgopt=path-include="/usr/include/*" \
    --dpkgopt=path-include="/usr/lib/*" \
    --dpkgopt=path-include="/usr/lib32/*" \
    --dpkgopt=path-exclude="/usr/lib/debug/*" \
    --dpkgopt=path-exclude="/usr/lib/python*" \
    --dpkgopt=path-include="/usr/share/pkgconfig/*" \

# Remove more unwanted files
rm -rf $sysroot/etc $sysroot/dev $sysroot/tmp $sysroot/var

# Remove empty directories
find $sysroot -depth -type d -empty -delete

# Adjust symbolic links to link into the sysroot instead of absolute
# paths that end up pointing at the host system.
find $sysroot -type l | while read l; do
  t=$(readlink $l)
  case "$t" in
  /*)
    # We have a path in the form "$sysroot/a/b/c/d" and we want ../../..,
    # which is how we get from d to the root of the sysroot. For that,
    # we start from the directory containing d ("$sysroot/a/b/c"), remove
    # all non-slash characters, leaving is with "///", replace each slash
    # with "../", which gives us "../../../", and then we remove the last
    # slash.
    rel=$(dirname $l | sed 's,[^/],,g;s,/,../,g;s,/$,,')
    ln -sf $rel$t $l
    ;;
  esac
done

