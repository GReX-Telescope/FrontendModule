//! Logic for the HMC624A attenuator

use embedded_hal::{digital::v2::OutputPin, spi::FullDuplex};
use micromath::F32Ext;

#[derive(Debug)]
pub enum Error<LEE> {
    SpiError,
    LeError(LEE),
}

#[derive(Debug)]
pub struct DualHMC624A<LE1, LE2, SPI> {
    le1: LE1,
    le2: LE2,
    spi: SPI,
}

impl<LE1, LE2, SPI, LEE> DualHMC624A<LE1, LE2, SPI>
where
    SPI: FullDuplex<u8>,
    LE1: OutputPin<Error = LEE>,
    LE2: OutputPin<Error = LEE>,
{
    pub fn new(spi: SPI, le1: LE1, le2: LE2) -> Self {
        Self { le1, le2, spi }
    }

    // Set attenuation in dB (steps of 0.5) with ranges from 0 to 31.5
    pub fn set_attenuation(&mut self, atten: f32) -> Result<(), Error<LEE>> {
        // Find the closest 0.5 interval
        let closest = (atten * 2.0).round() / 2.0;
        // Count how many LSBs that is
        let half_steps = (closest / 0.5) as u8;
        // And find distance from the maximum of 63 steps
        let setting = (63 - half_steps) & 0b111111;
        // Set both latch enable pins to low to start clocking in data
        self.le1.set_low().map_err(|e| Error::LeError(e))?;
        self.le2.set_low().map_err(|e| Error::LeError(e))?;
        // Shift out the bits
        self.spi.send(setting).map_err(|_| Error::SpiError)?;
        // LEs to high to latch the state
        self.le1.set_high().map_err(|e| Error::LeError(e))?;
        self.le2.set_high().map_err(|e| Error::LeError(e))?;
        // We're done!
        Ok(())
    }
}
