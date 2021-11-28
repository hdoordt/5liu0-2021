#![no_std]
#![no_main]

use folley_firmware as firmware;
use nrf52840_hal as hal;

use firmware::{
    mic_array::{MicArray, Pins as MicArrayPins},
    pan_tilt::PanTilt,
    uarte::{Baudrate, Parity, Pins as UartePins, Uarte},
};

#[allow(unused_imports)]
use hal::prelude::*;

use embedded_hal::timer::CountDown;
use folley_format::{
    device_to_server::{PanTiltStatus, SampleBuffer},
    DeviceToServer, ServerToDevice,
};
use hal::{
    gpio::{
        p0::{self, P0_03, P0_04, P0_28, P0_29},
        Disconnected, Level,
    },
    pac::{TIMER0, TIMER1, TWIM0, UARTE0},
    ppi::{self, Ppi0, Ppi1},
    saadc::{Oversample, SaadcConfig},
    twim::Pins as TwimPins,
    Timer, Twim,
};
use postcard::CobsAccumulator;
use pwm_pca9685::Pca9685;

type MicArrayInstance = MicArray<
    P0_03<Disconnected>,
    P0_04<Disconnected>,
    P0_28<Disconnected>,
    P0_29<Disconnected>,
    TIMER1,
    Ppi1,
>;

#[rtic::app(
    device=nrf52840_hal::pac,
    peripherals=true,
    monotonic=rtic::cyccnt::CYCCNT
)]
const APP: () = {
    struct Resources {
        pan_tilt_status: PanTiltStatus,
        accumulator: CobsAccumulator<32>,
        uarte0: Uarte<UARTE0, TIMER0, Ppi0>,
        pan_tilt: PanTilt<Pca9685<Twim<TWIM0>>>,
        mic_array: MicArrayInstance,
    }

    // Initialize peripherals, before interrupts are unmasked
    // Returns all resources that need to be dynamically instantiated
    #[init(spawn = [read_uarte0])]
    fn init(ctx: init::Context) -> init::LateResources {
        // Initialize UARTE0
        // Initialize port0
        let port0 = p0::Parts::new(ctx.device.P0);
        let ppi = ppi::Parts::new(ctx.device.PPI);

        // Receiving pin, initialize as input
        let rxd = port0.p0_08.into_floating_input().degrade();

        // Transmitting pin, initialize as output
        let txd = port0.p0_06.into_push_pull_output(Level::Low).degrade(); // Erase the type, creating a generic pin

        // let cts = port0.p0_07.into_floating_input().degrade();
        // let rts = port0.p0_05.into_push_pull_output(Level::High).degrade();
        // Create Pins struct to pass to Uarte
        let uart_pins = UartePins {
            rxd,
            txd,
            // We don't use cts/rts
            cts: None, // Clear to send pin
            rts: None, // Request to send pin
        };

        let mut timer0 = Timer::periodic(ctx.device.TIMER0);
        timer0.start(500_000u32); // 100 ms

        // Initialize UARTE0 peripheral with standard configuration
        let uarte0 = Uarte::init(
            ctx.device.UARTE0, // Take peripheral handle by value
            uart_pins,         // Take pins by value
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
            timer0,
            ppi.ppi0,
        );

        let scl = port0.p0_30.into_floating_input().degrade();
        let sda = port0.p0_31.into_floating_input().degrade();

        let twim0_pins = TwimPins { scl, sda };
        let mut pan_tilt = PanTilt::new(ctx.device.TWIM0, twim0_pins);

        let mic_pins = MicArrayPins {
            mic1: port0.p0_03,
            mic2: port0.p0_04,
            mic3: port0.p0_28,
            mic4: port0.p0_29,
        };
        let saadc_config = SaadcConfig {
            oversample: Oversample::BYPASS,
            ..SaadcConfig::default()
        };

        let mut timer1 = Timer::periodic(ctx.device.TIMER1);
        timer1.start(1_000u32);

        let mut mic_array =
            MicArray::new(ctx.device.SAADC, mic_pins, saadc_config, timer1, ppi.ppi1);

        let accumulator = CobsAccumulator::new();

        let pan_tilt_status = PanTiltStatus {
            pan_deg: 90.,
            tilt_deg: 90.,
        };

        pan_tilt.pan_deg(pan_tilt_status.pan_deg);
        pan_tilt.tilt_deg(pan_tilt_status.tilt_deg);

        mic_array.start_sampling_task();

        init::LateResources {
            uarte0,
            accumulator,
            pan_tilt,
            pan_tilt_status,
            mic_array,
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
        let pan_tilt = ctx.resources.pan_tilt;
        let pan_tilt_status = ctx.resources.pan_tilt_status;
        let mut mic_array = ctx.resources.mic_array;
        let ServerToDevice {
            pan_degrees,
            tilt_degrees,
            set_sampling_enabled,
        } = msg;

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
        match set_sampling_enabled {
            Some(true) => mic_array.lock(|m| m.start_sampling_task()),
            Some(false) => mic_array.lock(|m| m.stop_sampling_task()),
            None => {}
        }

        ctx.spawn
            .send_message(DeviceToServer {
                pan_tilt: Some(*pan_tilt_status),
                ..DeviceToServer::default()
            })
            .ok();
    }

    #[task(capacity = 10, resources = [uarte0], priority  = 1)]
    fn send_message(mut ctx: send_message::Context, msg: DeviceToServer) {
        defmt::info!("Sending message: {:?}", &msg);
        let mut buf = [0; 1024];
        match postcard::to_slice_cobs(&msg, &mut buf) {
            Ok(bytes) => {
                while let Err(_) = ctx
                    .resources
                    .uarte0
                    .lock(|uarte0| uarte0.try_start_tx(&bytes))
                {
                    defmt::debug!("Waiting for currently running tx task to finish");
                    // Go to sleep to avoid busy waiting
                    cortex_m::asm::wfi();
                }
            }
            Err(e) => {
                defmt::error!("Could not serialize message {}. Error: {}", msg, defmt::Debug2Format(&e))
            }
        }
        defmt::debug!("Done sending message");
    }

    #[task(
        binds = UARTE0_UART0,
        priority = 100,
        resources = [uarte0],
        spawn = [read_uarte0],
    )]
    fn on_uarte0(mut ctx: on_uarte0::Context) {
        use firmware::uarte::UarteEvent::*;
        defmt::trace!("Running task on_uarte0");

        ctx.resources
            .uarte0
            .lock(|uarte0| match uarte0.get_clear_event() {
                Some(EndRx) => {
                    ctx.spawn.read_uarte0().ok();
                }
                _ => (),
            });
    }

    #[task(
        priority = 101,
        resources = [uarte0, accumulator],
        spawn = [handle_message],
    )]
    fn read_uarte0(ctx: read_uarte0::Context) {
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

    #[task(binds = SAADC, priority = 255, resources = [mic_array], spawn = [send_message])]
    fn on_saadc(ctx: on_saadc::Context) {
        let mic_array = ctx.resources.mic_array;
        let mut buf = SampleBuffer::default();
        mic_array.clear_interrupt();
        let count = mic_array.copy_samples(&mut buf);

        defmt::debug!("Sample ready! {}, {:?}", count, &buf);
        let msg = DeviceToServer {
            samples: Some(buf),
            ..DeviceToServer::default()
        };
        ctx.spawn
            .send_message(msg)
            .expect("Error spawning send_message task");
    }

    // RTIC requires that unused interrupts are declared in an extern block when
    // using software tasks; these interrupts will be used to dispatch the
    // software tasks.
    // See https://rtic.rs/0.5/book/en/by-example/tasks.html;
    extern "C" {
        // Software interrupt 0 / Event generator unit 0
        fn SWI0_EGU0();
        // Software interrupt 1 / Event generator unit 1
        fn SWI1_EGU1();
        // Software interrupt 2 / Event generator unit 2
        fn SWI2_EGU2();
    }
};
