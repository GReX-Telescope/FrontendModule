#![no_std]
#![no_main]

mod bsp;
mod log_det;
mod mnc;

use bsp::*;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::v2::{OutputPin, ToggleableOutputPin};
use hal::pac;
use hal::{
    adc::{Adc, TempSense},
    gpio::{Output, Pin, PushPull},
    uart::{UartConfig, UartPeripheral},
};
use heapless::Vec;
use panic_probe as _;
use rp2040_hal as hal;
use rp2040_hal::clocks::Clock;
use rp2040_monotonic::{ExtU64, Rp2040Monotonic};

// Bind software tasks to SIO_IRQ_PROC0, we're not using it
#[rtic::app(device = pac, peripherals = true, dispatchers = [SIO_IRQ_PROC0])]
mod app {
    use super::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        rf1_status_led: Rf1StatusLed,
        rf2_status_led: Rf2StatusLed,
        lna_1: Rf1LnaEn,
        lna_2: Rf2LnaEn,
        rf1_if_pow: Rf1IfPow,
        rf2_if_pow: Rf2IfPow,
        temp_sense: TempSense,
        uart: bsp::Uart,
        monitor_payload: transport::MonitorPayload,
        adc: Adc,
    }

    #[monotonic(binds = TIMER_IRQ_0, default = true)]
    type Tonic = Rp2040Monotonic;

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

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
        let rf1_if_pow = pins.rf1_if_pow.into_floating_input();
        let rf2_if_pow = pins.rf2_if_pow.into_floating_input();

        // Grab the UART pins and setup the peripheral (115200 baud)
        let uart_pins = (
            pins.txd.into_mode::<hal::gpio::FunctionUart>(),
            pins.rxd.into_mode::<hal::gpio::FunctionUart>(),
        );
        let mut uart = UartPeripheral::new(cx.device.UART1, uart_pins, &mut resets)
            .enable(UartConfig::default(), clocks.peripheral_clock.freq())
            .unwrap();

        // Enable the UART interrupt
        uart.enable_rx_interrupt();

        // Set the LNA outputs to ON by default
        let mut lna_1 = pins.rf1_lna_en.into_push_pull_output();
        let mut lna_2 = pins.rf2_lna_en.into_push_pull_output();
        lna_1.set_high().unwrap();
        lna_2.set_high().unwrap();

        // Set the RF status LEDs to off
        let mut rf1_status_led = pins.rf1_status_led.into_push_pull_output();
        let mut rf2_status_led = pins.rf2_status_led.into_push_pull_output();
        rf1_status_led.set_low().unwrap();
        rf2_status_led.set_low().unwrap();

        (
            Shared {},
            Local {
                monitor_payload: Default::default(),
                lna_1,
                lna_2,
                rf1_status_led,
                rf2_status_led,
                rf1_if_pow,
                rf2_if_pow,
                temp_sense,
                uart,
                adc,
            },
            init::Monotonics(mono),
        )
    }

    // Only one task - we're just going to react to requests for either monitor data or control
    #[task(binds = UART1_IRQ, local = [uart, temp_sense, rf1_if_pow, rf2_if_pow, monitor_payload, adc])]
    fn on_rx(cx: on_rx::Context) {
        // Grab all the locals
        let uart = cx.local.uart;
        let temp_sense = cx.local.temp_sense;
        let rf1_if_pow = cx.local.rf1_if_pow;
        let rf2_if_pow = cx.local.rf2_if_pow;
        let monitor_payload = cx.local.monitor_payload;
        let adc = cx.local.adc;

        // Unsure yet how big this payload will be
        let mut buf = [0u8; 128];
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
            Err(e) => {
                error!("Error deserializing incoming command payload");
                return;
            }
        };
        // Dispatch command
        match command {
            transport::Command::Monitor => {
                // Update monitor payload and send
                mnc::update_monitor_payload(
                    monitor_payload,
                    adc,
                    rf1_if_pow,
                    rf2_if_pow,
                    temp_sense,
                );
                let bytes: Vec<u8, 32> = match postcard::to_vec(&monitor_payload) {
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
            transport::Command::Control(_) => {
                // TODO
            }
        }
    }
}
