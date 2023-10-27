use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use postcard::{
    accumulator::{CobsAccumulator, FeedResult},
    to_slice_cobs,
};
use serialport::SerialPort;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Serial port for the FEM
    port: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Gets monitor data from the FEM
    Mon,
    /// Controls the power of the LNA
    Lna {
        /// LNA Channel
        channel: Lna,
        /// LNA power setting
        setting: Setting,
    },
    /// Sets the IF "power good" threshold
    If { level: f32 },
    /// Sets the attenuation level in dB (0 to 31.5)
    Atten { level: f32 },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Setting {
    Enabled,
    Disabled,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Lna {
    Ch1,
    Ch2,
}

impl Setting {
    fn en(&self) -> bool {
        match self {
            Setting::Enabled => true,
            Setting::Disabled => false,
        }
    }
}

/// Write a command out on the serial port (COBS) and wait for the response
fn write_read(
    cmd: &transport::Command,
    mut port: Box<dyn SerialPort>,
) -> Option<transport::Response> {
    let mut buf = [0u8; 256];
    let s = to_slice_cobs(cmd, &mut buf).unwrap();
    port.write_all(s).expect("Serial write failed");

    // Bytes per read
    let mut raw_buf = [0u8; 256];
    // Bytes in the accumulator (COBS)
    let mut cobs_buf: CobsAccumulator<256> = CobsAccumulator::new();
    // Keep truckin until we've got a response
    while let Ok(n) = port.read(&mut raw_buf) {
        if n == 0 {
            // We're done reading
            break;
        }
        let buf = &raw_buf[..n];
        let mut window = buf;
        'cobs: while !window.is_empty() {
            window = match cobs_buf.feed::<transport::Response>(window) {
                FeedResult::Consumed => break 'cobs,
                FeedResult::OverFull(new_wind) => new_wind,
                FeedResult::DeserError(new_wind) => new_wind,
                FeedResult::Success { data, remaining: _ } => return Some(data),
            };
        }
    }
    None
}

fn monitor(port: Box<dyn SerialPort>) {
    dbg!(write_read(&transport::Command::Monitor, port));
}

fn lna_power(port: Box<dyn SerialPort>, channel: Lna, setting: Setting) {
    let cmd = match channel {
        Lna::Ch1 => transport::Command::Control(transport::Action::Lna1Power(setting.en())),
        Lna::Ch2 => transport::Command::Control(transport::Action::Lna2Power(setting.en())),
    };
    dbg!(write_read(&cmd, port));
}

fn if_level(port: Box<dyn SerialPort>, level: f32) {
    dbg!(write_read(
        &transport::Command::Control(transport::Action::SetIfLevel(level)),
        port,
    ));
}

fn attenuation(port: Box<dyn SerialPort>, level: f32) {
    assert!(
        (0.0..=31.5).contains(&level),
        "Attenuation level must be between 0 and 31.5"
    );
    dbg!(write_read(
        &transport::Command::Control(transport::Action::SetAtten(level)),
        port,
    ));
}

const FEM_BAUD: u32 = 115_200;

fn main() {
    // Parse the CLI
    let cli = Cli::parse();
    // Try to open the serial port
    let port = serialport::new(cli.port, FEM_BAUD)
        .timeout(Duration::from_millis(1000))
        .open()
        .expect("Failed to open serial port");
    // Dispath on action
    match cli.command {
        Command::Mon => monitor(port),
        Command::If { level } => if_level(port, level),
        Command::Atten { level } => attenuation(port, level),
        Command::Lna { channel, setting } => lna_power(port, channel, setting),
    }
}
