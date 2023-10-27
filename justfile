pi-arch := 'armv7-unknown-linux-musleabihf'
fem-arch := 'thumbv6m-none-eabi'
chip := 'RP2040'
debugger := '1366:1008'

# Build the CLI app for whatever this platform is
build-cli:
    cargo build --release --bin cli

# Build the CLI app for the RPi
build-cli-pi:
    cross build --release --target {{pi-arch}} --bin cli

# Build the release firmware for the FEM
build-firmware:
    cargo build --release --target {{fem-arch}} --bin firmware

# Build the debug firmware for the FEM
build-debug-firmware:
    cargo build --target {{fem-arch}} --bin firmware

# Program the FEM (assuming it's connected via SWD with a JLink in CMSIS-DAP mode)
flash-firmware:
    cargo flash --release --chip {{chip}} --bin firmware --target {{fem-arch}} --probe {{debugger}}

# Run the FEM program interactivley with the debug probe
run-firmware: build-debug-firmware
    probe-rs run --chip {{chip}} --probe {{debugger}} {{justfile_directory()}}/target/{{fem-arch}}/debug/firmware

# Attach to running FEM program
attach-firmware:
    probe-rs attach --chip {{chip}} --probe {{debugger}} {{justfile_directory()}}/target/{{fem-arch}}/debug/firmware

# Run the CLI app (with args)
run-cli +ARGS: build-cli
    {{justfile_directory()}}/target/release/cli {{ARGS}}