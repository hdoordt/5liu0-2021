use core::{
    marker::PhantomData,
    mem,
    sync::atomic::{compiler_fence, Ordering}, ops::Deref,
};

use embedded_hal::adc::Channel;
use embedded_hal::timer::Cancel;
use folley_format::device_to_server::MicArraySample;
use nrf52840_hal::{
    pac::SAADC,
    ppi::ConfigurablePpi,
    saadc::SaadcConfig,
    timer::{Instance, Periodic},
    Saadc, Timer,
};

use self::saadc_buffer::{SaadcBuffer, SampleBuffer};

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
        mut timer: Timer<T, Periodic>,
        mut ppi_channel: P,
    ) -> Self {
        let mut buffer = SaadcBuffer::take().expect("SaadcBuffer is already taken");

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
            defmt::trace!("Configuring channel {} for AIN{}", chan, ain_id);

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
            assert!((0..=7).contains(&ain_id));
            saadc.ch[chan]
                .pselp
                .write(|w| unsafe { w.pselp().bits(ain_id + 1) });
            saadc.ch[chan].pseln.write(|w| w.pseln().nc());
        }

        // Set up DMA
        let buffer_slice = buffer.write_buf();
        saadc
            .result
            .ptr
            .write(|w| unsafe { w.bits(buffer_slice.as_ptr() as u32) });
        saadc.result.maxcnt.write(|w| unsafe {
            w.bits((mem::size_of::<MicArraySample>() / 2 * buffer_slice.len()) as u32)
        });
        timer.cancel().unwrap();
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

        Self {
            saadc,
            pins: PhantomData,
            buffer,
            timer,
            ppi_channel: PhantomData,
        }
    }

    pub fn get_newest_samples(&mut self) -> &mut <SampleBuffer as Deref>::Target {
        self.buffer.swap();
        self.saadc
            .result
            .ptr
            .write(|w| unsafe { w.bits(self.buffer.write_buf().as_ptr() as u32) });
        &mut self.buffer.read_buf().0
    }

    pub fn start_sampling_task(&mut self) {
        self.saadc.events_end.reset();
        self.saadc.tasks_start.write(|w| w.tasks_start().set_bit());
        self.timer
            .as_timer0()
            .tasks_start
            .write(|w| w.tasks_start().set_bit());
    }

    pub fn stop_sampling_task(&mut self) {
        self.saadc.events_end.reset();
        // self.saadc.tasks_stop.write(|w| w.tasks_stop().set_bit());
        self.timer
            .as_timer0()
            .tasks_stop
            .write(|w| w.tasks_stop().set_bit());
    }
}

mod saadc_buffer {

    use core::{
        ops::{Deref, DerefMut},
        sync::atomic::{AtomicBool, Ordering},
    };

    use folley_format::device_to_server::MicArraySample;

    static mut SAADC_BUFFER_A: SampleBuffer = SampleBuffer::new_empty();
    static mut SAADC_BUFFER_B: SampleBuffer = SampleBuffer::new_empty();
    static BUFFER_TAKEN: AtomicBool = AtomicBool::new(false);

    pub struct SaadcBuffer {
        read: &'static mut SampleBuffer,
        write: &'static mut SampleBuffer,
    }

    impl SaadcBuffer {
        pub fn take() -> Option<Self> {
            if BUFFER_TAKEN.swap(true, Ordering::Relaxed) {
                return None;
            }

            Some(Self {
                read: unsafe { &mut SAADC_BUFFER_A },
                write: unsafe { &mut SAADC_BUFFER_B },
            })
        }

        pub fn write_buf(&mut self) -> &mut SampleBuffer {
            &mut self.write
        }

        pub fn read_buf(&mut self) -> &mut SampleBuffer {
            &mut self.read
        }

        pub fn swap(&mut self) {
            let write = self.read as *mut _;
            let read = self.write as *mut _;

            self.write = unsafe { &mut *write };
            self.read = unsafe { &mut *read };
        }
    }

    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "defmt", derive(Format))]
    #[repr(transparent)]
    pub struct SampleBuffer(pub [MicArraySample; Self::size()]);

    impl SampleBuffer {
        const SIZE: usize = crate::consts::SAMPLE_BUF_SIZE;

        pub const fn size() -> usize {
            // assert_eq!(Self::SIZE % 4, 0, "SampleBuffer size must be a multiple of 4");
            Self::SIZE
        }

        pub const fn new_empty() -> Self {
            Self([[0; 4]; Self::size()])
        }
    }
    impl Deref for SampleBuffer {
        type Target = [MicArraySample; Self::size()];

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for SampleBuffer {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
}
