#![no_std]
#![no_main]

use firmware::mic_array;
use folley_calc::{calc_angle, expected_lags_size, Channels};
use folley_firmware as firmware;
use nrf52840_hal as hal;

#[allow(unused_imports)]
use hal::prelude::*;

use folley_format::{device_to_server::PanTiltStatus, DeviceToServer, ServerToDevice};
use hal::{
    gpio::{
        p0::{self, P0_03, P0_04, P0_28, P0_29},
        Disconnected,
    },
    gpiote::Gpiote,
    pac::{TIMER0, TIMER1, TWIM0, UARTE0},
    ppi::{self, Ppi0, Ppi3},
    saadc::{Resolution, Time},
    Clocks, Twim,
};

#[cfg(feature = "pan_tilt")]
use firmware::pan_tilt::PanTilt;
#[cfg(not(feature = "pan_tilt"))]
use firmware::stubs::PanTilt;

#[cfg(not(feature = "uart"))]
use firmware::stubs::{CobsAccumulator, Uarte};
#[cfg(feature = "uart")]
use firmware::uarte::{Baudrate, Parity, Pins as UartePins, Uarte};
#[cfg(feature = "uart")]
use postcard::CobsAccumulator;

#[cfg(feature = "mic_array")]
use firmware::mic_array::{MicArray, Pins as MicArrayPins};
#[cfg(not(feature = "mic_array"))]
use firmware::stubs::MicArray;

use firmware::consts::*;

type MicArrayInstance = MicArray<
    P0_03<Disconnected>,
    P0_04<Disconnected>,
    P0_28<Disconnected>,
    P0_29<Disconnected>,
    TIMER1,
    Ppi3,
>;

#[rtic::app(
    device=nrf52840_hal::pac,
    peripherals=true,
    monotonic=rtic::cyccnt::CYCCNT
)]
const APP: () = {
    struct Resources {
        #[cfg(feature = "uart")]
        accumulator: CobsAccumulator<32>,
        #[cfg(feature = "uart")]
        uarte0: Uarte<UARTE0, TIMER0, Ppi0>,
        #[cfg(feature = "mic_array")]
        mic_array: MicArrayInstance,
        #[cfg(feature = "pan_tilt")]
        pan_tilt: PanTilt<Twim<TWIM0>>,
        #[cfg(feature = "pan_tilt")]
        pan_tilt_status: PanTiltStatus,
        #[cfg(feature = "analyze")]
        lag_table: [u32; LAG_TABLE_SIZE],
    }

    // Initialize peripherals, before interrupts are unmasked
    // Returns all resources that need to be dynamically instantiated
    #[init(spawn = [read_uarte0, send_message])]
    #[allow(unused_variables)]
    fn init(ctx: init::Context) -> init::LateResources {
        // Initialize UARTE0
        // Initialize port0
        let port0 = p0::Parts::new(ctx.device.P0);
        let mut ppi = ppi::Parts::new(ctx.device.PPI);

        let clocks = Clocks::new(ctx.device.CLOCK);
        clocks.enable_ext_hfosc();

        #[cfg(feature = "uart")]
        let (uarte0, accumulator) = {
            use hal::gpio::Level;
            use hal::timer::Timer;

            // Receiving pin, initialize as input
            let rxd = port0.p0_08.into_floating_input().degrade();

            // Transmitting pin, initialize as output
            let txd = port0.p0_06.into_push_pull_output(Level::Low).degrade(); // Erase the type, creating a generic pin

            let rts = port0.p0_05.into_push_pull_output(Level::High).degrade();
            let cts = port0.p0_07.into_floating_input().degrade();
            // Create Pins struct to pass to Uarte
            let uart_pins = UartePins {
                rxd,
                txd,
                // We don't use cts/rts
                cts: Some(cts), // Clear to send pin
                rts: Some(rts), // Request to send pin
            };

            let mut timer0 = Timer::periodic(ctx.device.TIMER0);
            timer0.start(100_000u32); // 100 ms

            // Initialize UARTE0 peripheral with standard configuration
            let uarte0 = Uarte::init(
                ctx.device.UARTE0, // Take peripheral handle by value
                uart_pins,         // Take pins by value
                Parity::EXCLUDED,
                Baudrate::BAUD1M,
                timer0,
                ppi.ppi0,
            );
            ctx.spawn.send_message(DeviceToServer::Sync);
            let accumulator = CobsAccumulator::new();
            (uarte0, accumulator)
        };

        #[cfg(feature = "pan_tilt")]
        let (pan_tilt, pan_tilt_status) = {
            use hal::twim::Pins as TwimPins;
            let pan_tilt_status = PanTiltStatus {
                pan_deg: 90.,
                tilt_deg: 90.,
            };

            let scl = port0.p0_30.into_floating_input().degrade();
            let sda = port0.p0_31.into_floating_input().degrade();

            let twim0_pins = TwimPins { scl, sda };
            let mut pan_tilt = PanTilt::new(ctx.device.TWIM0, twim0_pins);

            pan_tilt.pan_deg(pan_tilt_status.pan_deg);
            pan_tilt.tilt_deg(pan_tilt_status.tilt_deg);
            (pan_tilt, pan_tilt_status)
        };

        #[cfg(feature = "mic_array")]
        let mic_array = {
            use embedded_hal::timer::CountDown;
            use hal::saadc::{Gain, Oversample, Resistor, SaadcConfig};
            use hal::timer::Timer;

            let mic_pins = MicArrayPins {
                mic1: port0.p0_03,
                mic2: port0.p0_04,
                mic3: port0.p0_28,
                mic4: port0.p0_29,
            };
            let saadc_config = SaadcConfig {
                resolution: Resolution::_12BIT,
                oversample: Oversample::BYPASS,
                resistor: Resistor::PULLDOWN,
                gain: Gain::GAIN1_6,
                time: Time::_5US,
                ..SaadcConfig::default()
            };

            let gpiote = Gpiote::new(ctx.device.GPIOTE);
            let mut timer1 = Timer::periodic(ctx.device.TIMER1);

            let btn1_pin = port0.p0_11.into_pullup_input().degrade();
            gpiote.channel1().input_pin(&btn1_pin).hi_to_lo();
            ppi.ppi1.set_task_endpoint(timer1.task_stop());
            ppi.ppi1.set_event_endpoint(gpiote.channel1().event());
            ppi.ppi1.enable();

            let btn2_pin = port0.p0_12.into_pullup_input().degrade();
            gpiote.channel2().input_pin(&btn2_pin).hi_to_lo();
            ppi.ppi2.set_task_endpoint(timer1.task_start());
            ppi.ppi2.set_event_endpoint(gpiote.channel2().event());
            ppi.ppi2.enable();

            timer1.start(T_S_US);

            let mut mic_array =
                MicArray::new(ctx.device.SAADC, mic_pins, saadc_config, timer1, ppi.ppi3);

            mic_array.start_sampling_task();
            mic_array
        };

        init::LateResources {
            #[cfg(feature = "uart")]
            uarte0,
            #[cfg(feature = "uart")]
            accumulator,
            #[cfg(feature = "mic_array")]
            mic_array,
            #[cfg(feature = "pan_tilt")]
            pan_tilt,
            #[cfg(feature = "pan_tilt")]
            pan_tilt_status,
            #[cfg(feature = "analyze")]
            lag_table: folley_calc::gen_lag_table::<T_S_US, D_MICS_MM, LAG_TABLE_SIZE>(),
        }
    }

    // Defines what happens when there's nothing left to do
    #[idle]
    fn idle(_ctx: idle::Context) -> ! {
        loop {
            // Go to sleep, waiting for an interrupt
            cortex_m::asm::wfi();
        }
    }

    #[task(capacity = 5, priority = 10, resources = [pan_tilt, pan_tilt_status, mic_array], spawn = [send_message])]
    fn handle_message(ctx: handle_message::Context, msg: ServerToDevice) {
        let ServerToDevice {
            #[cfg(feature = "pan_tilt")]
            pan_degrees,
            #[cfg(feature = "pan_tilt")]
            tilt_degrees,
            #[cfg(feature = "mic_array")]
            set_sampling_enabled,
            ..
        } = msg;

        #[cfg(feature = "pan_tilt")]
        let pan_tilt_status = {
            let pan_tilt = ctx.resources.pan_tilt;
            let pan_tilt_status = ctx.resources.pan_tilt_status;
            if let Some(deg) = pan_degrees {
                defmt::debug!("Pan to {} degrees", deg);
                pan_tilt_status.pan_deg = deg;
                pan_tilt.pan_deg(deg);
            }
            if let Some(deg) = tilt_degrees {
                defmt::debug!("Tilt to {} degrees", deg);
                pan_tilt_status.tilt_deg = deg;
                pan_tilt.tilt_deg(deg);
            }
            ctx.spawn
                .send_message(DeviceToServer::PanTilt(pan_tilt_status))
                .ok();
        };
        #[cfg(feature = "mic_array")]
        {
            let mut mic_array = ctx.resources.mic_array;
            match set_sampling_enabled {
                Some(true) => mic_array.lock(|m| m.start_sampling_task()),
                Some(false) => mic_array.lock(|m| m.stop_sampling_task()),
                None => {}
            }
        }
    }

    #[task(capacity = 10, resources = [uarte0], priority  = 99)]
    #[cfg_attr(not(feature = "uart"), allow(unused_variables, unused_mut))]
    fn send_message(mut ctx: send_message::Context, msg: DeviceToServer) {
        #[cfg(feature = "uart")]
        {
            use firmware::uarte::StartTxResult::Busy;

            while let Busy = ctx
                .resources
                .uarte0
                .lock(|uarte0| uarte0.try_start_tx(&msg))
            {
                // while let Busy = ctx.resources.uarte0.try_start_tx(bytes){
                defmt::trace!("Waiting for currently running tx task to finish");
                // Go to sleep to avoid busy waiting
                cortex_m::asm::wfi();
            }
            defmt::debug!("Sent!");
        }
    }

    #[task(
        binds = UARTE0_UART0,
        priority = 100,
        resources = [uarte0],
        spawn = [read_uarte0],
    )]
    #[cfg_attr(not(feature = "uart"), allow(unused_variables, unused_mut))]
    fn on_uarte0(mut ctx: on_uarte0::Context) {
        #[cfg(feature = "uart")]
        {
            use firmware::uarte::UarteEvent::*;
            defmt::trace!("Running task on_uarte0");

            ctx.resources.uarte0.lock(|uarte0| {
                if let Some(EndRx) = uarte0.get_clear_event() {
                    ctx.spawn.read_uarte0().ok();
                }
            });
        }
    }

    #[task(
        priority = 101,
        resources = [uarte0, accumulator],
        spawn = [handle_message],
    )]
    #[cfg_attr(not(feature = "uart"), allow(unused_variables))]
    fn read_uarte0(mut ctx: read_uarte0::Context) {
        #[cfg(feature = "uart")]
        {
            use postcard::FeedResult::*;

            // We have ownership declared in the resources
            let chunk = ctx.resources.uarte0.get_rx_chunk();
            match ctx.resources.accumulator.feed(chunk) {
                Consumed => {}
                OverFull(_) => defmt::warn!("Accumulator full, dropping contents"),
                DeserError(_) => defmt::error!("Deserialize error, throwing away message"),
                Success { data, .. } => ctx
                    .spawn
                    .handle_message(data)
                    .expect("Could not start handle_message task, please increase its capacity."),
            }
        }
    }

    #[task(binds = SAADC, priority = 255, resources = [mic_array], spawn = [on_samples, send_message])]
    #[cfg_attr(not(feature = "mic_array"), allow(unused_variables))]
    fn on_saadc(ctx: on_saadc::Context) {
        #[cfg(feature = "mic_array")]
        {
            let mic_array = ctx.resources.mic_array;

            mic_array.stop_sampling_task();

            let channels = {
                let samples = mic_array.get_newest_samples();

                for c in samples.chunks(SAMPLE_BUF_SIZE) {
                    let msg = DeviceToServer::Samples(c.try_into().unwrap());
                    ctx.spawn.send_message(DeviceToServer::Sync).ok();
                    if let Err(e) = ctx.spawn.send_message(msg) {
                        defmt::warn!("Error spawning send_message task");
                    }
                    ctx.spawn.send_message(DeviceToServer::Sync).ok();
                }
                Channels::<SAMPLE_BUF_SIZE>::from_samples((*samples).try_into().unwrap())
            };
            defmt::debug!("{:?}", &channels.ch1);
            defmt::debug!("{:?}", &channels.ch2);

            // mic_array.start_sampling_task();
            if let Err(_) = ctx.spawn.on_samples(channels) {
                defmt::warn!("Could not spawn on_samples task");
            };

            // defmt::println!("angle: {}", angle);
            // defmt::trace!("Sample ready!");
        }
    }

    #[task(priority = 10, resources = [mic_array, lag_table])]
    fn on_samples(mut ctx: on_samples::Context, channels: Channels<SAMPLE_BUF_SIZE>) {
        let mut buf = [0i32; XCORR_SIZE];
        let angle = folley_calc::calc_angle::<
            T_S_US,
            D_MICS_MM,
            XCORR_SIZE,
            SAMPLE_BUF_SIZE,
            LAG_TABLE_SIZE,
        >(
            &channels.ch1,
            &channels.ch2,
            &mut buf,
            ctx.resources.lag_table,
        );
        ctx.resources.mic_array.lock(|mic_array| {
            mic_array.start_sampling_task();
        });
        defmt::println!("angle: {}", angle);
    }

    extern "C" {
        fn SWI0_EGU0();
        fn SWI1_EGU1();
        fn SWI2_EGU2();
        fn SWI3_EGU3();
    }
};
