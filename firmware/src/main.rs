#![no_std]
#![no_main]

mod atten;
mod bsp;
mod log_det;
mod mnc;

use bsp::*;
use defmt::*;
use defmt_rtt as _;
use fugit::RateExtU32;
use panic_probe as _;
use postcard::{
    accumulator::{CobsAccumulator, FeedResult},
    to_slice_cobs,
};

// Embedded Hal traits
use embedded_hal::digital::v2::OutputPin;

use hal::{
    adc::{Adc, AdcPin},
    clocks::{init_clocks_and_plls, Clock},
    entry, pac,
    sio::Sio,
    uart::{DataBits, StopBits, UartConfig, UartPeripheral},
    watchdog::Watchdog,
    I2C,
};
use rp2040_hal as hal;

#[entry]
fn main() -> ! {
    info!("FEM Booting!");
    // Setup peripherals, core clocks, etc.
    let mut pac = pac::Peripherals::take().unwrap();
    let _core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    let clocks = init_clocks_and_plls(
        bsp::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // Setup the pins
    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    info!("Setting up ADC");
    // Enable the ADC peripheral and internal temperature sensor
    let mut adc = Adc::new(pac.ADC, &mut pac.RESETS);
    let mut temp_sense = adc.take_temp_sensor().unwrap();

    // Setup RF power monitor chip
    let mut rf1_if_pow = AdcPin::new(pins.rf1_if_pow.into_floating_input());
    let mut rf2_if_pow = AdcPin::new(pins.rf2_if_pow.into_floating_input());

    // Grab the UART pins and setup the peripheral (115200 baud)
    info!("Setting up UART");
    let uart_pins: UartPins = (pins.txd.into_function(), pins.rxd.into_function());
    let uart: Uart = UartPeripheral::new(pac.UART1, uart_pins, &mut pac.RESETS)
        .enable(
            UartConfig::new(115200.Hz(), DataBits::Eight, None, StopBits::One),
            clocks.peripheral_clock.freq(),
        )
        .unwrap();
    info!("Setting up GPIO");
    // Set the LNA outputs to ON by default
    let mut lna_1 = pins.rf1_lna_en.into_push_pull_output();
    let mut lna_2 = pins.rf2_lna_en.into_push_pull_output();
    lna_1.set_high().unwrap();
    lna_2.set_high().unwrap();

    // Set the RF status LEDs to off
    let mut rf1_status_led = pins.rf1_stat_led.into_push_pull_output();
    let mut rf2_status_led = pins.rf2_stat_led.into_push_pull_output();
    rf1_status_led.set_low().unwrap();
    rf2_status_led.set_low().unwrap();

    // Setup the SPI pins and initial state of the latch enable pins (high)
    // pins are implicitly used by the SPI driver
    info!("Setting up SPI");
    let mut atten1_le = pins.atten1_le.into_push_pull_output();
    atten1_le.set_low().unwrap();
    let mut atten2_le = pins.atten2_le.into_push_pull_output();
    atten2_le.set_low().unwrap();

    let spi = hal::Spi::<_, _, _, 6>::new(
        pac.SPI0,
        (pins.sdo.into_function(), pins.clk.into_function()),
    )
    .init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        1.MHz(),
        embedded_hal::spi::MODE_0,
    );

    // And setup the two attenuators
    let mut atten = atten::DualHMC624A::new(spi, atten1_le, atten2_le);
    // and set the initial state to 0
    atten.set_attenuation(0.0).unwrap();

    info!("Setting up I2C");
    // Setup I2C for the TMP100 and INA3221
    let i2c = I2C::i2c0(
        pac.I2C0,
        pins.sda.into_function(),
        pins.scl.into_function(),
        400.kHz(),
        &mut pac.RESETS,
        &clocks.system_clock,
    );

    // Setup the INA3221
    let mut ina3221 = ina3221::INA3221::new(i2c, ina3221::AddressPin::Gnd);
    match ina3221.reset() {
        Ok(_) => (),
        Err(_) => error!("INA3221 failed to reset"),
    };
    match ina3221.set_averaging(ina3221::registers::Averages::_256) {
        Ok(_) => (),
        Err(_) => error!("INA3221 failed to set averages"),
    };

    // Setup state for if good and monitor
    let mut state = mnc::State::default();

    // Setup the state for the COBS input message accumulator
    let mut in_buf = [0u8; 256];
    let mut out_buf = [0u8; 256];
    let mut cobs_buf: CobsAccumulator<256> = CobsAccumulator::new();

    info!("FEM Booted, starting main thread!");

    loop {
        // Update monitor payload in state
        mnc::update_monitor_payload(
            &mut state.last_monitor,
            &mut adc,
            &mut rf1_if_pow,
            &mut rf2_if_pow,
            &mut temp_sense,
            &mut ina3221,
        );
        // If there are bytes for us, push them to the accumulator
        if uart.uart_is_readable() {
            while let Ok(n) = uart.read_raw(&mut in_buf) {
                let buf = &in_buf[..n];
                let mut window = buf;
                'cobs: while !window.is_empty() {
                    window = match cobs_buf.feed::<transport::Command>(window) {
                        FeedResult::Consumed => break 'cobs,
                        FeedResult::OverFull(new_wind) => new_wind,
                        FeedResult::DeserError(new_wind) => new_wind,
                        FeedResult::Success {
                            data: cmd,
                            remaining,
                        } => {
                            // Handle command
                            info!("New incoming payload - {}", cmd);

                            match cmd {
                                transport::Command::Monitor => {
                                    // Serialize the last monitor payload
                                    let resp =
                                        transport::Response::Monitor(state.last_monitor.clone());
                                    info!("Sending monitor data - {}", resp);
                                    let s = to_slice_cobs(&resp, &mut out_buf).unwrap();
                                    uart.write_full_blocking(s);
                                }
                                transport::Command::Control(action) => {
                                    // Do the control thing
                                    match action {
                                        transport::Action::SetIfLevel(level) => {
                                            state.if_good_threshold = level
                                        }
                                        transport::Action::Lna1Power(en) => {
                                            if en {
                                                lna_1.set_high().unwrap();
                                            } else {
                                                lna_1.set_low().unwrap();
                                            }
                                        }
                                        transport::Action::Lna2Power(en) => {
                                            if en {
                                                lna_2.set_high().unwrap();
                                            } else {
                                                lna_2.set_low().unwrap();
                                            }
                                        }
                                        transport::Action::SetAtten(a) => {
                                            match atten.set_attenuation(a) {
                                                Ok(_) => (),
                                                Err(_) => error!("Failed to set attenuation"),
                                            }
                                        }
                                    }
                                    // Then send an ack
                                    let resp = transport::Response::Ack;
                                    let s = to_slice_cobs(&resp, &mut out_buf).unwrap();
                                    uart.write_full_blocking(s);
                                }
                            }
                            remaining
                        }
                    };
                }
            }
        }
        // Set the RF Good LEDs
        if state.last_monitor.if1_power >= state.if_good_threshold {
            rf1_status_led.set_high().unwrap();
        } else {
            rf1_status_led.set_low().unwrap();
        }
        if state.last_monitor.if2_power >= state.if_good_threshold {
            rf2_status_led.set_high().unwrap();
        } else {
            rf2_status_led.set_low().unwrap();
        }
    }
}
