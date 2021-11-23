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
use folley_format::{device_to_server::PanTiltStatus, DeviceToServer, ServerToDevice};
use hal::{
    gpio::{
        p0::{Parts, P0_03, P0_04, P0_28, P0_29},
        Disconnected, Level,
    },
    pac::{TIMER0, TIMER1, TWIM0, UARTE0},
    ppi::{self, Ppi0},
    saadc::SaadcConfig,
    timer::Periodic,
    twim::Pins as TwimPins,
    Timer, Twim,
};
use postcard::CobsAccumulator;
use pwm_pca9685::Pca9685;

#[rtic::app(
    device=nrf52840_hal::pac,
    peripherals=true,
    monotonic=rtic::cyccnt::CYCCNT
)]
const APP: () = {
    struct Resources {
        pan_tilt_status: PanTiltStatus,
        accumulator: CobsAccumulator<32>,
        uarte0: Uarte<UARTE0>,
        timer0: Timer<TIMER0, Periodic>,
        pan_tilt: PanTilt<Pca9685<Twim<TWIM0>>>,
        mic_array: MicArray<
            P0_03<Disconnected>,
            P0_04<Disconnected>,
            P0_28<Disconnected>,
            P0_29<Disconnected>,
            TIMER1,
            Ppi0,
        >,
    }

    // Initialize peripherals, before interrupts are unmasked
    // Returns all resources that need to be dynamically instantiated
    #[init(spawn = [read_uarte0])]
    fn init(ctx: init::Context) -> init::LateResources {
        // Initialize UARTE0
        // Initialize port0
        let port0 = Parts::new(ctx.device.P0);

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

        // Initialize UARTE0 peripheral with standard configuration
        let uarte0 = Uarte::init(
            ctx.device.UARTE0, // Take peripheral handle by value
            uart_pins,         // Take pins by value
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        );

        let scl = port0.p0_27.into_floating_input().degrade();
        let sda = port0.p0_26.into_floating_input().degrade();

        let twim0_pins = TwimPins { scl, sda };
        let mut pan_tilt = PanTilt::new(ctx.device.TWIM0, twim0_pins);

        let ppi = ppi::Parts::new(ctx.device.PPI);

        let mic_pins = MicArrayPins {
            mic1: port0.p0_03,
            mic2: port0.p0_04,
            mic3: port0.p0_28,
            mic4: port0.p0_29,
        };
        let saadc_config = SaadcConfig {
            ..SaadcConfig::default()
        };

        let mut timer1 = Timer::periodic(ctx.device.TIMER1);
        timer1.enable_interrupt();
        timer1.start(3_000_000u32);

        let mut mic_array =
            MicArray::new(ctx.device.SAADC, mic_pins, saadc_config, timer1, ppi.ppi0);

        let mut timer0 = Timer::periodic(ctx.device.TIMER0);
        timer0.enable_interrupt();
        timer0.start(500_000u32); // 100 ms

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
            timer0,
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

    #[task(capacity = 5, priority = 10, resources = [pan_tilt, pan_tilt_status], spawn = [send_message])]
    fn handle_message(ctx: handle_message::Context, msg: ServerToDevice) {
        let pan_tilt = ctx.resources.pan_tilt;
        let pan_tilt_status = ctx.resources.pan_tilt_status;
        let ServerToDevice {
            pan_degrees,
            tilt_degrees,
        } = msg;

        if let Some(deg) = pan_degrees {
            defmt::println!("Pan to {} degrees", deg);
            pan_tilt_status.pan_deg = deg;
            pan_tilt.pan_deg(deg);
        }
        if let Some(deg) = tilt_degrees {
            defmt::println!("Tilt to {} degrees", deg);
            pan_tilt_status.tilt_deg = deg;
            pan_tilt.tilt_deg(deg);
        }

        ctx.spawn
            .send_message(DeviceToServer {
                pan_tilt: *pan_tilt_status,
            })
            .ok();
    }

    #[task(capacity = 10, resources = [uarte0], priority  = 1)]
    fn send_message(mut ctx: send_message::Context, msg: DeviceToServer) {
        defmt::info!("Sending message: {:?}", &msg);
        let mut buf = [0; 32];
        if let Ok(bytes) = postcard::to_slice_cobs(&msg, &mut buf) {
            while let Err(_) = ctx
                .resources
                .uarte0
                .lock(|uarte0| uarte0.try_start_tx(&bytes))
            {
                defmt::debug!("Waiting for currently running tx task to finish");
                // Go to sleep to avoid busy waiting
                cortex_m::asm::wfi();
            }
        } else {
            defmt::error!(
                "Could not serialize message {}. Please increase buffer size.",
                msg
            )
        }
        defmt::debug!("Done sending message");
    }

    #[task(
        binds = TIMER0,
        priority = 99,
        resources = [timer0, uarte0],
    )]
    fn on_timer0(mut ctx: on_timer0::Context) {
        let timer0 = ctx.resources.timer0;
        defmt::debug!("Running task on_timer 0");
        if timer0.event_compare_cc0().read().bits() != 0x00u32 {
            timer0.event_compare_cc0().write(|w| unsafe { w.bits(0) });
            // We need to lock here, because the on_uarte0 task could also access
            // uarte0_rx, and as that task has a higher priority, it could pre-empt
            // the current task.
            ctx.resources.uarte0.lock(|uarte0| {
                // Stop current read transaction. Fluhes the Uarte FIFO,
                // and triggers and ENDRX event, which triggers on_uarte0
                uarte0.stop_rx_task();
            });
        }
    }

    #[task(
        binds = UARTE0_UART0,
        priority = 100,
        resources = [uarte0],
        spawn = [read_uarte0],
    )]
    fn on_uarte0(mut ctx: on_uarte0::Context) {
        use firmware::uarte::UarteEvent::*;
        defmt::debug!("Running task on_uarte0");

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

    #[task(binds = SAADC, priority = 255, resources = [mic_array])]
    fn on_saadc(ctx: on_saadc::Context) {
        let mic_array = ctx.resources.mic_array;
        mic_array.clear_interrupt();

        defmt::println!("Sample ready!");
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
