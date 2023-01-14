use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use heapless::Vec;
use postcard::{from_bytes, to_vec};
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
    Monitor,
    /// Controls the power and calibration state of the LNA
    #[command(subcommand)]
    Lna(LnaCommand),
    /// Sets the IF "power good" threshold
    GoodIf { level: f32 },
    /// Sets the attenuation level in dB (0 to 31.5)
    Attenuation { level: f32 },
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

#[derive(Subcommand)]
enum LnaCommand {
    /// Sets the power state of a given LNA
    Power { channel: Lna, setting: Setting },
    /// Sets the calibration state of a given LNA
    Cal { channel: Lna, setting: Setting },
}

fn monitor(mut port: Box<dyn SerialPort>) {
    // Prepare the command payload
    let com: Vec<u8, 8> = to_vec(&transport::Command::Monitor).unwrap();
    // Transmit the payload
    port.write_all(&com).expect("Serial write failed");
    // Wait for the response
    let mut buf = [0u8; 1024];
    match port.read(&mut buf) {
        Ok(t) => {
            // Deserialize the response
            let resp: transport::MonitorPayload =
                from_bytes(&buf[..t]).expect("Couldn't deserialize response payload");
            println!("The monitor payload is currently {t} bytes");
            // And print
            dbg!(resp);
        }
        Err(e) => {
            eprintln!("{e:?}");
        }
    }
}

fn lna_power(mut port: Box<dyn SerialPort>, channel: Lna, setting: Setting) {
    let com: Vec<u8, 8> = to_vec(
        &(match channel {
            Lna::Ch1 => transport::Command::Control(transport::Action::Lna1Power(setting.en())),
            Lna::Ch2 => transport::Command::Control(transport::Action::Lna2Power(setting.en())),
        }),
    )
    .unwrap();
    // Transmit the payload
    port.write_all(&com).expect("Serial write failed");
}

fn lna_cal(mut port: Box<dyn SerialPort>, channel: Lna, setting: Setting) {
    let com: Vec<u8, 8> = to_vec(
        &(match channel {
            Lna::Ch1 => transport::Command::Control(transport::Action::SetCal1(setting.en())),
            Lna::Ch2 => transport::Command::Control(transport::Action::SetCal2(setting.en())),
        }),
    )
    .unwrap();
    // Transmit the payload
    port.write_all(&com).expect("Serial write failed");
}

fn if_level(mut port: Box<dyn SerialPort>, level: f32) {
    let com: Vec<u8, 8> = to_vec(&transport::Command::Control(transport::Action::SetIfLevel(
        level,
    )))
    .unwrap();
    port.write_all(&com).expect("Serial write failed");
}

fn attenuation(mut port: Box<dyn SerialPort>, level: f32) {
    assert!(
        (0.0..=31.5).contains(&level),
        "Attenuation level must be between 0 and 31.5"
    );
    let com: Vec<u8, 8> = to_vec(&transport::Command::Control(transport::Action::SetAtten(
        level,
    )))
    .unwrap();
    port.write_all(&com).expect("Serial write failed");
}

const FEM_BAUD: u32 = 115_200;

fn main() {
    // Parse the CLI
    let cli = Cli::parse();
    // Try to open the serial port
    let port = serialport::new(cli.port, FEM_BAUD)
        .timeout(Duration::from_millis(100))
        .open()
        .expect("Failed to open serial port");
    // Dispath on action
    match cli.command {
        Command::Monitor => monitor(port),
        Command::GoodIf { level } => if_level(port, level),
        Command::Attenuation { level } => attenuation(port, level),
        Command::Lna(c) => match c {
            LnaCommand::Power { channel, setting } => lna_power(port, channel, setting),
            LnaCommand::Cal { channel, setting } => lna_cal(port, channel, setting),
        },
    }
}
