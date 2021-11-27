use core::{
    marker::PhantomData,
    mem,
    sync::atomic::{compiler_fence, Ordering},
};

use embedded_hal::adc::Channel;
use nrf52840_hal::{
    pac::SAADC,
    ppi::ConfigurablePpi,
    saadc::SaadcConfig,
    timer::{Instance, Periodic},
    Saadc, Timer,
};

use self::saadc_buffer::SaadcBuffer;
pub use saadc_buffer::buffer_size;
pub type RawSample = [i16; 4];

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

pub struct MicArray<M1, M2, M3, M4, T, P>
where
    M1: Channel<Saadc, ID = u8>,
    M2: Channel<Saadc, ID = u8>,
    M3: Channel<Saadc, ID = u8>,
    M4: Channel<Saadc, ID = u8>,
{
    saadc: SAADC,
    pins: PhantomData<Pins<M1, M2, M3, M4>>,
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

        for (chan, &ain_id) in pins.channels().iter().enumerate() {
            defmt::debug!("Configuring channel {} for AIN{}", chan, ain_id);

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
            assert!((1..=8).contains(&ain_id));
            saadc.ch[chan]
                .pselp
                .write(|w| unsafe { w.pselp().bits(ain_id) });
            saadc.ch[chan].pseln.write(|w| w.pseln().nc());
        }

        // Set up DMA
        let buffer_slice = buffer.as_slice();
        saadc
            .result
            .ptr
            .write(|w| unsafe { w.bits(buffer_slice.as_ptr() as u32) });
        saadc.result.maxcnt.write(|w| unsafe {
            w.bits((mem::size_of::<RawSample>() / 2 * buffer_slice.len()) as u32)
        });

        let timer = timer.free();
        let timer_block = timer.as_timer0();

        // Connect PPI channel
        ppi_channel.set_task_endpoint(&saadc.tasks_sample);
        ppi_channel.set_event_endpoint(&timer_block.events_compare[0]);
        ppi_channel.enable();

        saadc.intenset.write(|w| w.end().set_bit());
        compiler_fence(Ordering::SeqCst);
        // Calibrate
        saadc.events_calibratedone.reset();
        saadc
            .tasks_calibrateoffset
            .write(|w| w.tasks_calibrateoffset().set_bit());
        while saadc
            .events_calibratedone
            .read()
            .events_calibratedone()
            .bit_is_clear()
        {}

        // Only start after all initalization is done.
        compiler_fence(Ordering::SeqCst);
        saadc.tasks_start.write(|w| w.tasks_start().set_bit());

        Self {
            saadc,
            pins: PhantomData,
            buffer,
            timer,
            ppi_channel: PhantomData,
        }
    }

    pub fn clear_interrupt(&mut self) {
        self.saadc.events_end.reset();
    }

    pub fn copy_samples(&self, buf: &mut [RawSample]) -> usize {
        // The amount of samples read by the SAADC
        let amount = self.saadc.result.amount.read().bits() as usize;

        let slice = self.buffer.as_slice();

        let count = slice.len().min(amount).min(buf.len());
        // Note(unsafe): We have made sure count is no longer than the source and the dest length
        unsafe { core::ptr::copy_nonoverlapping(slice.as_ptr(), buf.as_mut_ptr(), count) }
        // Only start conversion after copy is complete.
        compiler_fence(Ordering::SeqCst);
        self.saadc.tasks_start.write(|w| w.tasks_start().set_bit());
        count
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

    pub const fn buffer_size() -> usize {
        32
    }

    static mut SAADC_BUFFER: [RawSample; buffer_size()] = [[0i16; 4]; buffer_size()];
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
