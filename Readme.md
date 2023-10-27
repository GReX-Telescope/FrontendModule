# GReX Frontend Module

You'll need:
- [Rust](https://rustup.rs/)
- ARM Rust toolchain
    - `rustup target add thumbv7em-none-eabihf`
- [just](https://github.com/casey/just)
    - `cargo install just`
- [cross](https://github.com/cross-rs/cross) (which requires either Docker or Podman)
    - `cargo install cross --git https://github.com/cross-rs/cross`
- [probe-rs](https://probe.rs/)
    - `cargo install probe-rs --features cli,ftdi`

## Hardware

For assembly, access the interactive BOM [here](https://grex-telescope.github.io/FrontendModule/hardware/bom/ibom.html)

## Software

### Building

Build for the current architecture

`just build-cli`

### For the RPi in the box

`just build-cli-pi`

The resulting binary is in `target/armv7-unknown-linux-musleabihf/release/cli` which you can copy to the Pi, `chmod +x` if you need, and run.

## Firmware

### Building

`just build-firmware`

The resulting binary (elf) is in target/thumbv6m-none-eabi/release/firmware

### Flashing

Using a JLink in CMSIS-DAP mode

`just flash-firmware`