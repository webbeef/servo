#!/bin/bash

PROFILE=release

./build-arm64.sh --profile ${PROFILE} && \
     llvm-strip target/aarch64-unknown-linux-gnu/${PROFILE}/servo && \
     ./package-arm64.sh --profile ${PROFILE} && \
     scp ./target/aarch64-unknown-linux-gnu/${PROFILE}/servo-tech-demo.tar.gz mobian@mobian:/home/mobian/
