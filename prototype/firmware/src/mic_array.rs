use core::marker::PhantomData;

use embedded_hal::adc::Channel;
use nrf52840_hal::{
    pac::{saadc, SAADC},
    ppi::ConfigurablePpi,
    saadc::SaadcConfig,
    timer::{Instance, Periodic},
    Saadc, Timer,
};

use self::saadc_buffer::SaadcBuffer;

pub struct Pins<MIC1, MIC2, MIC3, MIC4>
where
    MIC1: Channel<Saadc, ID = u8>,
    MIC2: Channel<Saadc, ID = u8>,
    MIC3: Channel<Saadc, ID = u8>,
    MIC4: Channel<Saadc, ID = u8>,
{
    pub mic1: MIC1,
    pub mic2: MIC2,
    pub mic3: MIC3,
    pub mic4: MIC4,
}

impl<MIC1, MIC2, MIC3, MIC4> Pins<MIC1, MIC2, MIC3, MIC4>
where
    MIC1: Channel<Saadc, ID = u8>,
    MIC2: Channel<Saadc, ID = u8>,
    MIC3: Channel<Saadc, ID = u8>,
    MIC4: Channel<Saadc, ID = u8>,
{
    fn channels(&self) -> [u8; 4] {
        [
            <MIC1 as Channel<Saadc>>::channel(),
            <MIC2 as Channel<Saadc>>::channel(),
            <MIC3 as Channel<Saadc>>::channel(),
            <MIC4 as Channel<Saadc>>::channel(),
        ]
    }
}

type RawSample = [i16; 4];

pub struct MicArray<MIC1, MIC2, MIC3, MIC4, TIM, PPICH>
where
    MIC1: Channel<Saadc, ID = u8>,
    MIC2: Channel<Saadc, ID = u8>,
    MIC3: Channel<Saadc, ID = u8>,
    MIC4: Channel<Saadc, ID = u8>,
{
    saadc: SAADC,
    pins: Pins<MIC1, MIC2, MIC3, MIC4>,
    buffer: SaadcBuffer,
    timer: TIM,
    ppi_channel: PPICH,
}

impl<MIC1, MIC2, MIC3, MIC4, TIM, PPICH> MicArray<MIC1, MIC2, MIC3, MIC4, TIM, PPICH>
where
    MIC1: Channel<Saadc, ID = u8>,
    MIC2: Channel<Saadc, ID = u8>,
    MIC3: Channel<Saadc, ID = u8>,
    MIC4: Channel<Saadc, ID = u8>,
    TIM: Instance,
    PPICH: ConfigurablePpi,
{
    pub fn new(
        saadc: SAADC,
        pins: Pins<MIC1, MIC2, MIC3, MIC4>,
        config: SaadcConfig,
        mut timer: Timer<TIM, Periodic>,
        mut ppi_channel: PPICH,
    ) -> Self {
        let buffer = SaadcBuffer::take().expect("SaadcBuffer is already taken");

        // Taken from nrf52840_hal
        let SaadcConfig {
            resolution,
            oversample,
            reference,
            gain,
            resistor,
            time,
        } = config;

        saadc.enable.write(|w| w.enable().enabled());
        saadc.resolution.write(|w| w.val().variant(resolution));
        saadc
            .oversample
            .write(|w| w.oversample().variant(oversample));
        saadc.samplerate.write(|w| w.mode().task());

        for chan in pins.channels().map(|ch| ch as usize) {
            saadc.ch[chan].config.write(|w| {
                w.refsel().variant(reference);
                w.gain().variant(gain);
                w.tacq().variant(time);
                w.mode().se();
                w.resp().variant(resistor);
                w.resn().bypass();
                w.burst().enabled();
                w
            });
            saadc.ch[chan].pseln.write(|w| w.pseln().nc());
        }

        let buffer_slice = buffer.as_slice();
        saadc
            .result
            .ptr
            .write(|w| unsafe { w.bits(buffer_slice.as_ptr() as u32) });
        saadc
            .result
            .maxcnt
            .write(|w| unsafe { w.bits(buffer_slice.len() as u32) });

        let timer = timer.free();
        let timer_block = timer.as_timer0();

        ppi_channel.set_task_endpoint(&saadc.tasks_sample);
        ppi_channel.set_event_endpoint(&timer_block.events_compare[0]);
        ppi_channel.enable();

        saadc.intenset.write(|w| w.resultdone().set_bit());


        // Calibrate
        saadc.events_calibratedone.reset();
        saadc.tasks_calibrateoffset.write(|w| unsafe { w.bits(1) });
        while saadc.events_calibratedone.read().bits() == 0 {}

        Self {
            saadc,
            pins,
            buffer,
            timer,
            ppi_channel,
        }
    }

    pub fn clear_interrupt(&mut self) {
        self.saadc.intenset.write(|w| w.resultdone().set_bit());
    }

    pub fn sample_raw() -> RawSample {
        todo!();
    }

    pub fn start_sampling_task(&mut self) {
        self.timer.as_timer0().tasks_start.write(|w| w.tasks_start().set_bit());
    }

    pub fn stop_sampling_task(&mut self) {
        self.timer.as_timer0().tasks_stop.write(|w| w.tasks_stop().set_bit());
    }
}

mod saadc_buffer {
    use core::{
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    };

    use super::RawSample;

    const BUFFER_SIZE: usize = 32;

    static mut SAADC_BUFFER: [RawSample; BUFFER_SIZE] = [[0i16; 4]; BUFFER_SIZE];
    static BUFFER_TAKEN: AtomicBool = AtomicBool::new(false);

    pub struct SaadcBuffer {
        _marker: PhantomData<bool>,
    }

    impl SaadcBuffer {
        pub fn take() -> Option<Self> {
            if BUFFER_TAKEN.swap(true, Ordering::Relaxed) {
                return None;
            }
            Some(Self {
                _marker: PhantomData,
            })
        }

        pub fn as_slice(&self) -> &'static [RawSample] {
            unsafe { &SAADC_BUFFER }
        }
    }
}
