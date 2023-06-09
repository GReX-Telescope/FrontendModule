//! Types that facilitate transport between the FEM firmware and MnC software
#![no_std]

use serde::{Deserialize, Serialize};

/// Actions that can be performed
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Action {
    /// Set the IF "Good" power threshold in dBm
    SetIfLevel(f32),
    /// Control the power state of the LNA1 regulator
    Lna1Power(bool),
    /// Control the power state of the LNA2 regulator
    Lna2Power(bool),
    // Set attenuation
    SetAtten(f32),
}

/// Monitor data sent in response to a [`Command::Monitor`] call
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct MonitorPayload {
    /// IF1 power in dBm
    pub if1_power: f32,
    /// IF2 power in dBm
    pub if2_power: f32,
    /// RP2040 internal temperature in C
    pub ic_temp: f32,
    /// Voltage and current of LNA1
    pub lna1_power: Power,
    /// Voltage and current of LNA2
    pub lna2_power: Power,
    /// Voltage and current of the analog rail
    pub analog_power: Power,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Power {
    /// Votlage in volts
    pub voltage: f32,
    /// Current in amps
    pub current: f32,
}

/// Payloads from MnC software to the FEM
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Command {
    Monitor,
    Control(Action),
}
