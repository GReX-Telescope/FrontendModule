use crate::{
    bsp::{read_temp, Rf1IfPow, Rf2IfPow},
    log_det::read_power,
};
use rp2040_hal::{adc::TempSense, Adc};
use transport::MonitorPayload;

pub fn update_monitor_payload(
    payload: &mut MonitorPayload,
    adc: &mut Adc,
    rf1_if_pow: &mut Rf1IfPow,
    rf2_if_pow: &mut Rf2IfPow,
    internal_temp: &mut TempSense,
) {
    // Update IF Powers
    payload.if1_power = read_power(adc, rf1_if_pow).unwrap();
    payload.if2_power = read_power(adc, rf2_if_pow).unwrap();
    // Update internal temp
    payload.ic_temp = read_temp(adc, internal_temp).unwrap();
    // TODO rest of payload
}
