use core::marker::PhantomData;

use embedded_hal::adc::Channel;
use nrf52840_hal::{
    pac::SAADC,
    ppi::ConfigurablePpi,
    saadc::SaadcConfig,
    timer::{Instance, Periodic},
    Saadc, Timer,
};

use self::saadc_buffer::SaadcBuffer;

pub struct Pins<M1, M2, M3, M4>
where
    M1: Channel<Saadc, ID = u8>,
    M2: Channel<Saadc, ID = u8>,
    M3: Channel<Saadc, ID = u8>,
    M4: Channel<Saadc, ID = u8>,
{
    pub mic1: M1,
    pub mic2: M2,
    pub mic3: M3,
    pub mic4: M4,
}

impl<M1, M2, M3, M4> Pins<M1, M2, M3, M4>
where
    M1: Channel<Saadc, ID = u8>,
    M2: Channel<Saadc, ID = u8>,
    M3: Channel<Saadc, ID = u8>,
    M4: Channel<Saadc, ID = u8>,
{
    fn channels(&self) -> [u8; 4] {
        [
            <M1 as Channel<Saadc>>::channel(),
            <M2 as Channel<Saadc>>::channel(),
            <M3 as Channel<Saadc>>::channel(),
            <M4 as Channel<Saadc>>::channel(),
        ]
    }
}

type RawSample = [i16; 4];

pub struct MicArray<M1, M2, M3, M4, T, P>
where
    M1: Channel<Saadc, ID = u8>,
    M2: Channel<Saadc, ID = u8>,
    M3: Channel<Saadc, ID = u8>,
    M4: Channel<Saadc, ID = u8>,
{
    saadc: SAADC,
    pins: Pins<M1, M2, M3, M4>,
    buffer: SaadcBuffer,
    timer: T,
    ppi_channel: PhantomData<P>,
}

impl<M1, M2, M3, M4, T, P> MicArray<M1, M2, M3, M4, T, P>
where
    M1: Channel<Saadc, ID = u8>,
    M2: Channel<Saadc, ID = u8>,
    M3: Channel<Saadc, ID = u8>,
    M4: Channel<Saadc, ID = u8>,
    T: Instance,
    P: ConfigurablePpi,
{
    pub fn new(
        saadc: SAADC,
        pins: Pins<M1, M2, M3, M4>,
        config: SaadcConfig,
        timer: Timer<T, Periodic>,
        mut ppi_channel: P,
    ) -> Self {
        let buffer = SaadcBuffer::take().expect("SaadcBuffer is already taken");

        // Heavily based on nrf52840_hal::saadc
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
        
        // Set up DMA
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

        // Connect PPI channel
        ppi_channel.set_task_endpoint(&saadc.tasks_sample);
        ppi_channel.set_event_endpoint(&timer_block.events_compare[0]);
        ppi_channel.enable();

        saadc.intenset.write(|w| w.resultdone().set_bit());

        // Calibrate
        saadc.events_calibratedone.reset();
        saadc.tasks_calibrateoffset.write(|w| unsafe { w.bits(1) });
        while saadc.events_calibratedone.read().events_calibratedone().bit_is_clear() {}

        Self {
            saadc,
            pins,
            buffer,
            timer,
            ppi_channel: PhantomData,
        }
    }

    pub fn clear_interrupt(&mut self) {
        self.saadc.events_resultdone.reset();
    }

    pub fn sample_raw() -> RawSample {
        todo!();
    }

    pub fn start_sampling_task(&mut self) {
        self.timer
            .as_timer0()
            .tasks_start
            .write(|w| w.tasks_start().set_bit());
    }

    pub fn stop_sampling_task(&mut self) {
        self.timer
            .as_timer0()
            .tasks_stop
            .write(|w| w.tasks_stop().set_bit());
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
