use std::time::Duration;

use clap::{Parser, Subcommand};
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
}

fn monitor(mut port: Box<dyn SerialPort>) {
    // Prepare the command payload
    let com: Vec<u8, 4> = to_vec(&transport::Command::Monitor).unwrap();
    // Transmit the payload
    port.write_all(&com).expect("Serial write failed");
    // Wait for the response
    let mut buf = [0u8; 1024];
    match port.read(&mut buf) {
        Ok(t) => {
            // Deserialize the response
            let resp: transport::MonitorPayload =
                from_bytes(&buf[..t]).expect("Couldn't deserialize response payload");
            // And print
            dbg!(resp);
        }
        Err(e) => {
            eprintln!("{e:?}");
            return;
        }
    }

    todo!()
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
    }
}
