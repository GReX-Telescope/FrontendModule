# GReX Frontend Module

## Firmware

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