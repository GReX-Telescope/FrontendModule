use crate::bsp::{read_adc, ADC_REF_VOLT};
use embedded_hal::adc::Channel;
use rp2040_hal::Adc;

/// Get the power corresponding to the read ADC value - returns an empty error on ADC failures
pub fn read_power<PIN>(adc: &mut Adc, pin: &mut PIN) -> Result<f32, ()>
where
    PIN: Channel<Adc, ID = u8>,
{
    // Get the ADC value and convert to true voltage
    let vx = read_adc(adc, pin)? * ADC_REF_VOLT;
    // And apply slope and intercept and account for the 20 dB tap
    Ok(vx / 0.0215 - 47.0 + 20.0)
}
