use crate::{
    bsp::{read_temp, Rf1IfPow, Rf2IfPow},
    log_det::read_power,
    tmp100::TMP100,
};
use defmt::error;
use embedded_hal::blocking::i2c;
use ina3221::INA3221;
use rp2040_hal::{adc::TempSense, Adc};
use transport::MonitorPayload;

#[derive(Debug)]
pub struct State {
    pub if_good_threshold: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            if_good_threshold: -10.0,
        }
    }
}

fn bus_volt_log<I2C, E>(ina: &mut INA3221<I2C>, ch: ina3221::Channel) -> f32
where
    I2C: i2c::Read<Error = E> + i2c::Write<Error = E> + i2c::WriteRead<Error = E>,
{
    match ina.bus_voltage(ch) {
        Ok(v) => v,
        Err(_) => {
            error!("Error getting bus voltage");
            Default::default()
        }
    }
}

fn shunt_volt_log<I2C, E>(ina: &mut INA3221<I2C>, ch: ina3221::Channel) -> f32
where
    I2C: i2c::Read<Error = E> + i2c::Write<Error = E> + i2c::WriteRead<Error = E>,
{
    match ina.shunt_voltage(ch) {
        Ok(v) => v,
        Err(_) => {
            error!("Error getting shunt voltage");
            Default::default()
        }
    }
}

pub fn update_monitor_payload<I2C, E>(
    payload: &mut MonitorPayload,
    adc: &mut Adc,
    rf1_if_pow: &mut Rf1IfPow,
    rf2_if_pow: &mut Rf2IfPow,
    internal_temp: &mut TempSense,
    tmp100: &mut TMP100<I2C>,
    ina3221: &mut INA3221<I2C>,
) where
    I2C: i2c::Read<Error = E> + i2c::Write<Error = E> + i2c::WriteRead<Error = E>,
{
    // Update IF Powers
    payload.if1_power = read_power(adc, rf1_if_pow).unwrap();
    payload.if2_power = read_power(adc, rf2_if_pow).unwrap();
    // Update internal temp
    payload.ic_temp = read_temp(adc, internal_temp).unwrap();
    // Update surface temp
    payload.surface_temp = match tmp100.temp_c() {
        Ok(t) => t,
        Err(_) => {
            error!("Error reading TMP100");
            return;
        }
    };
    // Voltages and currents - LNAs have Rsense of 1, Analog has Rsense of 0.2
    payload.lna1_power.voltage = bus_volt_log(ina3221, ina3221::Channel::Ch1);
    payload.lna1_power.current = shunt_volt_log(ina3221, ina3221::Channel::Ch1);
    payload.lna2_power.voltage = bus_volt_log(ina3221, ina3221::Channel::Ch2);
    payload.lna2_power.current = shunt_volt_log(ina3221, ina3221::Channel::Ch2);
    payload.analog_power.voltage = bus_volt_log(ina3221, ina3221::Channel::Ch3);
    payload.analog_power.current = shunt_volt_log(ina3221, ina3221::Channel::Ch3) / 0.2;
}
