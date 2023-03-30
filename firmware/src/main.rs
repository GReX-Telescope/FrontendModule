#![no_std]
#![no_main]

mod atten;
mod bsp;
mod log_det;
mod mnc;
mod tmp100;

use bsp::*;
use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::{digital::v2::OutputPin, PwmPin};
use fugit::RateExtU32;
use hal::{
    adc::{Adc, TempSense},
    clocks::Clock,
    i2c::I2C,
    pac,
    pwm::Slices,
    uart::{UartConfig, UartPeripheral},
};
use heapless::Vec;
use panic_probe as _;
use rp2040_hal as hal;
use rp2040_monotonic::Rp2040Monotonic;
use shared_bus::I2cProxy;

// Boy this is a thicc type
type I2cBus = I2cProxy<'static, Mutex<RefCell<bsp::I2c>>>;

// Bind software tasks to SIO_IRQ_PROC0, we're not using it
#[rtic::app(device = pac, peripherals = true, dispatchers = [SIO_IRQ_PROC0])]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        rf1_if_pow: Rf1IfPow,
        rf2_if_pow: Rf2IfPow,
        adc: Adc,
        state: mnc::State,
    }

    #[local]
    struct Local {
        rf1_status_led: Rf1StatusLed,
        rf2_status_led: Rf2StatusLed,
        lna_1: Rf1LnaEn,
        lna_2: Rf2LnaEn,
        temp_sense: TempSense,
        uart: bsp::Uart,
        monitor_payload: transport::MonitorPayload,
        tmp100: tmp100::TMP100<I2cBus>,
        ina3221: ina3221::INA3221<I2cBus>,
        atten: atten::DualHMC624A<Att1Le, Att2Le, Spi>,
        rf1_cal: Rf1CalPwm,
        rf2_cal: Rf2CalPwm,
    }

    #[monotonic(binds = TIMER_IRQ_0, default = true)]
    type Tonic = Rp2040Monotonic;

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        // Soft-reset does not release the hardware spinlocks
        // Release them now to avoid a deadlock after debug or watchdog reset
        // This is normally done in the custom #[entry]
        // Safety: As stated in the docs, this is the first thing that will
        // run in the entry point of the firmware
        unsafe {
            hal::sio::spinlock_reset();
        }

        // Create the RTIC timer
        let mono = Rp2040Monotonic::new(cx.device.TIMER);

        // Grab the global RESETS
        let mut resets = cx.device.RESETS;

        // Setup the clocks
        let mut watchdog = hal::Watchdog::new(cx.device.WATCHDOG);
        let clocks = hal::clocks::init_clocks_and_plls(
            XOSC_CRYSTAL_FREQ,
            cx.device.XOSC,
            cx.device.CLOCKS,
            cx.device.PLL_SYS,
            cx.device.PLL_USB,
            &mut resets,
            &mut watchdog,
        )
        .ok()
        .unwrap();

        // Grab the pins and set them up
        let sio = hal::Sio::new(cx.device.SIO);
        let pins = bsp::Pins::new(
            cx.device.IO_BANK0,
            cx.device.PADS_BANK0,
            sio.gpio_bank0,
            &mut resets,
        );

        // Enable the ADC peripheral and internal temperature sensor
        let mut adc = Adc::new(cx.device.ADC, &mut resets);
        let temp_sense = adc.enable_temp_sensor();
        let rf1_if_pow: Rf1IfPow = pins.rf1_if_pow.into_mode();
        let rf2_if_pow: Rf2IfPow = pins.rf2_if_pow.into_mode();

        // Grab the UART pins and setup the peripheral (115200 baud)
        let uart_pins: (Txd, Rxd) = (pins.txd.into_mode(), pins.rxd.into_mode());
        let mut uart = UartPeripheral::new(cx.device.UART1, uart_pins, &mut resets)
            .enable(UartConfig::default(), clocks.peripheral_clock.freq())
            .unwrap();

        // Enable the UART interrupt
        uart.enable_rx_interrupt();

        // Set the LNA outputs to ON by default
        let mut lna_1: Rf1LnaEn = pins.rf1_lna_en.into_mode();
        let mut lna_2: Rf2LnaEn = pins.rf2_lna_en.into_mode();
        lna_1.set_high().unwrap();
        lna_2.set_high().unwrap();

        // Set the RF status LEDs to off
        let mut rf1_status_led: Rf1StatusLed = pins.rf1_status_led.into_mode();
        let mut rf2_status_led: Rf2StatusLed = pins.rf2_status_led.into_mode();
        rf1_status_led.set_low().unwrap();
        rf2_status_led.set_low().unwrap();

        // Setup I2C for the TMP100 and INA3221
        let sda: Sda = pins.sda.into_mode();
        let scl: Scl = pins.scl.into_mode();
        let i2c = I2C::i2c0(
            cx.device.I2C0,
            sda,
            scl,
            400.kHz(),
            &mut resets,
            &clocks.system_clock,
        );
        let i2c_bus = shared_bus::new_cortexm!(I2c = i2c).unwrap();

        // Initialize the TMP100
        let mut tmp100 = tmp100::TMP100::new(i2c_bus.acquire_i2c(), 0b1001000);
        tmp100.init().unwrap();

        // Setup the INA3221
        let mut ina3221 = ina3221::INA3221::new(i2c_bus.acquire_i2c(), ina3221::AddressPin::Gnd);
        match ina3221.reset() {
            Ok(_) => (),
            Err(_) => error!("INA3221 failed to reset"),
        };
        match ina3221.set_averaging(ina3221::registers::Averages::_256) {
            Ok(_) => (),
            Err(_) => error!("INA3221 failed to set averages"),
        };

        // Setup the SPI pins and initial state of the latch enable pins (high)
        // pins are implicitly used by the SPI driver
        let _clk: Clk = pins.clk.into_mode();
        let _mosi: Mosi = pins.mosi.into_mode();
        let mut atten1_le: Att1Le = pins.atten1_le.into_mode();
        //atten1_le.set_high().unwrap();
        let mut atten2_le: Att2Le = pins.atten2_le.into_mode();
        //atten2_le.set_high().unwrap();

        let spi = hal::Spi::<_, _, 6>::new(cx.device.SPI0);
        let spi: Spi = spi.init(
            &mut resets,
            clocks.peripheral_clock.freq(),
            1.MHz(),
            &embedded_hal::spi::MODE_0,
        );

        // And setup the two attenuators
        let mut atten = atten::DualHMC624A::new(spi, atten1_le, atten2_le);
        // and set the initial state to 0
        //atten.set_attenuation(0.0).unwrap();

        // Setup the PWMs for the calibration output
        let pwm_slices = Slices::new(cx.device.PWM, &mut resets);
        let mut pwm = pwm_slices.pwm1;

        // Configure PWM frequency to close to 32 kHz
        pwm.set_div_int(1);
        pwm.set_div_frac(15);
        pwm.set_top(1024);
        pwm.set_ph_correct();

        // Attach PWM pins to outputs
        let mut rf1_cal = pwm.channel_b;
        let _channel_pin_1 = rf1_cal.output_to(pins.rf1_cal);

        let mut rf2_cal = pwm.channel_a;
        let _channel_pin_2 = rf2_cal.output_to(pins.rf2_cal);

        // Set the duty cycle to 50%
        rf1_cal.set_duty(512);
        rf2_cal.set_duty(512);

        // And boot disabled
        rf1_cal.disable();
        rf2_cal.disable();

        info!("Booted!");

        (
            Shared {
                rf1_if_pow,
                rf2_if_pow,
                adc,
                state: Default::default(),
            },
            Local {
                monitor_payload: Default::default(),
                lna_1,
                lna_2,
                rf1_status_led,
                rf2_status_led,
                temp_sense,
                uart,
                tmp100,
                ina3221,
                atten,
                rf1_cal,
                rf2_cal,
            },
            init::Monotonics(mono),
        )
    }

    // Idle task just updates the IF Good LEDs
    #[idle(local = [rf1_status_led, rf2_status_led], shared = [rf1_if_pow, rf2_if_pow, adc, state])]
    fn idle(cx: idle::Context) -> ! {
        // Locals
        let rf1 = cx.local.rf1_status_led;
        let rf2 = cx.local.rf2_status_led;
        // Shared
        let mut rf1_if_pow = cx.shared.rf1_if_pow;
        let mut rf2_if_pow = cx.shared.rf2_if_pow;
        let mut adc = cx.shared.adc;
        let mut state = cx.shared.state;
        loop {
            (&mut rf1_if_pow, &mut rf2_if_pow, &mut adc, &mut state).lock(
                |rf1_if_pow, rf2_if_pow, adc, state| {
                    if read_adc(adc, rf1_if_pow).unwrap() >= state.if_good_threshold {
                        rf1.set_high().unwrap();
                    } else {
                        rf1.set_low().unwrap();
                    }
                    if read_adc(adc, rf2_if_pow).unwrap() >= state.if_good_threshold {
                        rf2.set_high().unwrap();
                    } else {
                        rf2.set_low().unwrap();
                    }
                },
            )
        }
    }

    // Only one task - we're just going to react to requests for either monitor data or control
    #[task(binds = UART1_IRQ, local = [uart, temp_sense, monitor_payload, tmp100, ina3221, lna_1, lna_2, atten, rf1_cal, rf2_cal], shared = [rf1_if_pow, rf2_if_pow, adc, state])]
    fn on_rx(cx: on_rx::Context) {
        // Grab all the locals
        let uart = cx.local.uart;
        let temp_sense = cx.local.temp_sense;
        let tmp100 = cx.local.tmp100;
        let monitor_payload = cx.local.monitor_payload;
        let ina3221 = cx.local.ina3221;
        let lna1 = cx.local.lna_1;
        let lna2 = cx.local.lna_2;
        let atten = cx.local.atten;
        let rf1_cal = cx.local.rf1_cal;
        let rf2_cal = cx.local.rf2_cal;
        // And shared
        let rf1_if_pow = cx.shared.rf1_if_pow;
        let rf2_if_pow = cx.shared.rf2_if_pow;
        let adc = cx.shared.adc;
        let mut state = cx.shared.state;

        // Unsure yet how big this payload will be
        let mut buf = [0u8; 8];
        let bytes_read;
        // Check to make sure the UART is readable
        if uart.uart_is_readable() {
            let res = uart.read_raw(&mut buf);
            match res {
                Err(_) => {
                    error!("Error on UART read");
                    return;
                }
                Ok(bytes) => {
                    bytes_read = bytes;
                }
            }
        } else {
            return;
        }
        // Deserialize command
        let command: transport::Command = match postcard::from_bytes(&buf[..bytes_read]) {
            Ok(t) => t,
            Err(_) => {
                error!("Error deserializing incoming command payload");
                return;
            }
        };
        // Dispatch command
        match command {
            transport::Command::Monitor => {
                info!("Request for monitor payload - sending");
                // Update monitor payload and send
                (adc, rf1_if_pow, rf2_if_pow).lock(|adc, rf1_if_pow, rf2_if_pow| {
                    mnc::update_monitor_payload(
                        monitor_payload,
                        adc,
                        rf1_if_pow,
                        rf2_if_pow,
                        temp_sense,
                        tmp100,
                        ina3221,
                    );
                });
                let bytes: Vec<u8, 40> = match postcard::to_vec(&monitor_payload) {
                    Ok(v) => v,
                    Err(_) => {
                        error!("Error packing monitor payload");
                        return;
                    }
                };
                if uart.uart_is_writable() {
                    uart.write_full_blocking(&bytes);
                } else {
                    error!("Couldn't write to UART")
                }
            }
            transport::Command::Control(a) => match a {
                transport::Action::SetIfLevel(level) => state.lock(|s| s.if_good_threshold = level),
                transport::Action::Lna1Power(en) => {
                    if en {
                        lna1.set_high().unwrap();
                    } else {
                        lna1.set_low().unwrap();
                    }
                }
                transport::Action::Lna2Power(en) => {
                    if en {
                        lna2.set_high().unwrap();
                    } else {
                        lna2.set_low().unwrap();
                    }
                }
                transport::Action::SetAtten(a) => match atten.set_attenuation(a) {
                    Ok(_) => (),
                    Err(_) => error!("Failed to set attenuation"),
                },
                transport::Action::SetCal1(en) => {
                    if en {
                        rf1_cal.enable();
                    } else {
                        rf1_cal.disable();
                    }
                }
                transport::Action::SetCal2(en) => {
                    if en {
                        rf2_cal.enable();
                    } else {
                        rf2_cal.disable();
                    }
                }
            },
        }
    }
}
