#!/usr/bin/env bash

TARGET=armv7-unknown-linux-musleabihf # Pi 3 and 4
TARGET_HOST=snap-pi # Configured in .ssh/config
TARGET_PATH=/home/pi/fem_cli

export CROSS_CONTAINER_ENGINE=podman # Or docker

cross build +nightly --release --target $TARGET --bin mnc-cli
scp target/$TARGET/release/mnc-cli ${TARGET_HOST}:${TARGET_PATH}