//! Types that facilitate transport between the FEM firmware and MnC software
use serde::{Deserialize, Serialize};

/// Actions that can be performed
#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum Action {
    /// Set the IF1 "Good" power threshold in dBm
    SetIf1Level(f32),
    /// Set the IF2 "Good" power threshold in dBm
    SetIf2Level(f32),
    /// Control the power state of the LNA1 regulator
    Lna1Power(bool),
    /// Control the power state of the LNA2 regulator
    Lna2Power(bool),
    // TODO attenuation control
}

/// Monitor data sent in response to a [`Command::Monitor`] call
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Payload {
    /// IF1 power in dBm
    if1_power: f32,
    /// IF2 power in dBm
    if2_power: f32,
    /// PCB surface temperature in C
    surface_temp: f32,
    /// RP2040 internal temperature in C
    ic_temp: f32,
    // TODO voltages and currents
}

/// Payloads from MnC software to the FEM
#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum Command {
    Monitor,
    Control(Action),
}
