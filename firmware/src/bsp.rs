use embedded_hal::adc::{Channel, OneShot};
use hal::{
    i2c::I2C,
    pac::{I2C0, SPI0, UART1},
    Adc,
};
use rp2040_hal as hal;

// Crystal freq
pub const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;

/// Reference voltage for ADC conversions
pub const ADC_REF_VOLT: f32 = 3.3;

// Don't forget the second stage bootloader
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_IS25LP080;

// And add all of our pins!
hal::bsp_pins! {
    Gpio20 {
        name: rf1_status_led,
        aliases: { PushPullOutput: Rf1StatusLed }
    },
    Gpio21 {
        name: rf2_status_led,
        aliases: {PushPullOutput: Rf2StatusLed }
    },
    Gpio22 {
        name: rf1_lna_en,
        aliases: {PushPullOutput: Rf1LnaEn }
    },
    Gpio23 {
        name: rf2_lna_en,
        aliases: {PushPullOutput: Rf2LnaEn }
    },
    Gpio26 {
        name: rf1_if_pow,
        aliases: {FloatingInput: Rf1IfPow}
    },
    Gpio27 {
        name: rf2_if_pow,
        aliases: {FloatingInput: Rf2IfPow}
    },
    Gpio8 {
        name: txd,
        aliases: {FunctionUart: Txd}
    }
    Gpio9 {
        name: rxd,
        aliases: {FunctionUart: Rxd}
    }
    Gpio24 {
        name: sda,
        aliases: {FunctionI2C: Sda}
    }
    Gpio25 {
        name: scl,
        aliases: {FunctionI2C: Scl}
    }
    Gpio2 {
        name: clk
        aliases: {FunctionSpi: Clk}
    }
    Gpio3 {
        name: mosi
        aliases: {FunctionSpi: Mosi}
    }
    Gpio0 {
        name: atten2_le
        aliases: {PushPullOutput: Att2Le}
    }
    Gpio10 {
        name: atten1_le
        aliases: {PushPullOutput: Att1Le}
    }
    Gpio18 {
        name: rf2_cal,
    }
    Gpio19 {
        name: rf1_cal,
    }
}

// Some more type aliases
pub type Uart = hal::uart::UartPeripheral<hal::uart::Enabled, UART1, (Txd, Rxd)>;
pub type I2c = I2C<I2C0, (Sda, Scl)>;
pub type Spi = hal::Spi<hal::spi::Enabled, SPI0, 6>;

/// Get the 0..1 scaled floating point number representing the 12 bit ADC value
pub fn read_adc<PIN>(adc: &mut Adc, pin: &mut PIN) -> Result<f32, ()>
where
    PIN: Channel<Adc, ID = u8>,
{
    // Read the counts
    let counts: u16 = adc.read(pin).map_err(|_| ())?;
    // Scale raw 12-bit format to 0 .. 1
    let scaled = f32::from(counts) / f32::from(1u16 << 12);
    Ok(scaled.clamp(0.0, 1.0))
}

/// Read the internal temperature sensor in degrees C
pub fn read_temp<PIN>(adc: &mut Adc, pin: &mut PIN) -> Result<f32, ()>
where
    PIN: Channel<Adc, ID = u8>,
{
    // Get the raw voltage
    let v = read_adc(adc, pin)? * ADC_REF_VOLT;
    // RP2040 Datasheet 4.9.5
    Ok(27.0 - (v - 0.706) / 0.001721)
}
