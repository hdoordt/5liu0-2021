use nrf52840_hal as hal;

pub use hal::uarte::{Baudrate, Instance, Parity, Pins, Uarte as HalUarte};

use self::rx_buffer::UarteRxBuffer;

pub enum UarteEvent {
    EndRx,
    EndTx,
    // Add more variants as you expect more to occur
}

pub struct Uarte<U> {
    uarte: U,
    buffer: UarteRxBuffer,
    endtx_raised: bool,
}

impl<U: Instance> Uarte<U> {
    pub fn init(uarte: U, pins: Pins, parity: Parity, baudrate: Baudrate) -> Self {
        let buffer = UarteRxBuffer::take().expect("UarteRxBuffer is already taken");

        // We want to use advanced features that the HAL sadly does not implement.
        // Therefore, we destruct the Uarte object just created, regaining the UARTE0 peripheral
        // This way, we can still use the HAL for the initalization code.
        let (uarte, pins) = HalUarte::new(uarte, pins, parity, baudrate).free();

        // We don't want the pins to be de-initialized on drop,
        // so we just forget about them.
        core::mem::forget(pins);

        // Now we set up the uarte0 peripheral.
        let buffer_slice = buffer.as_slice();

        uarte
            .rxd
            .ptr
            .write(|w| unsafe { w.ptr().bits(buffer_slice.as_ptr() as u32) });

        uarte
            .rxd
            .maxcnt
            .write(|w| unsafe { w.maxcnt().bits(buffer_slice.len() as u16) });
        uarte
            .intenset
            .write(|w| w.endrx().set_bit().endtx().set_bit());
        uarte.tasks_startrx.write(|w| w.tasks_startrx().set_bit());

        Self {
            uarte,
            buffer,
            endtx_raised: false,
        }
    }

    pub fn try_start_tx(&mut self, bytes: &[u8]) -> Result<(), ()> {
        if self
            .uarte
            .events_txstarted
            .read()
            .events_txstarted()
            .bit_is_set()
        {
            if !self.endtx_raised {
                // There's a write transaction started, and it's not done yet.
                return Err(());
            }
            self.endtx_raised = false;
            // Clear event flags
            self.uarte.events_endtx.reset();
            self.uarte.events_txstarted.reset();
        }

        defmt::trace!(
            "TX contents: {:?}. chunk_len: {}",
            bytes,
            bytes.len()
        );

        // Setup transaction parameters
        self.uarte
            .txd
            .ptr // Where to find the data
            .write(|w| unsafe { w.ptr().bits(bytes.as_ptr() as u32) });
        self.uarte
            .txd
            .maxcnt // The length of the data
            .write(|w| unsafe { w.maxcnt().bits(bytes.len() as u16) });
        // Start write transaction
        self.uarte
            .tasks_starttx
            .write(|w| w.tasks_starttx().set_bit());
        Ok(())
    }

    pub fn stop_rx_task(&mut self) {
        self.uarte
            .tasks_stoprx
            .write(|w| w.tasks_stoprx().set_bit());
    }

    pub fn get_clear_event(&mut self) -> Option<UarteEvent> {
        if self.uarte.events_endrx.read().events_endrx().bit_is_set() {
            // Start a new read transaction
            self.uarte
                .tasks_startrx
                .write(|w| w.tasks_startrx().set_bit());
            // Clear interrupt flag
            self.uarte.events_endrx.reset();
            return Some(UarteEvent::EndRx);
        }
        if self.uarte.events_endtx.read().events_endtx().bit_is_set() {
            self.uarte.events_endtx.reset();
            self.endtx_raised = true;
            return Some(UarteEvent::EndTx);
        }

        None
    }

    pub fn get_rx_chunk(&mut self) -> &'static [u8] {
        let chunk_len = self.uarte.rxd.amount.read().amount().bits() as usize;
        // defmt::trace!(
        //     "UARTE0_BUFFER contents: {:?}. chunk_len: {}",
        //     self.buffer.as_slice(),
        //     chunk_len
        // );

        &self.buffer.as_slice()[0..chunk_len]
    }
}

mod rx_buffer {
    use core::{
        marker::PhantomData,
        sync::atomic::{AtomicBool, Ordering},
    };

    const BUFFER_SIZE: usize = 255;

    // Don't use a buffer bigger than 255 bytes,
    // as the nRF52832 can't handle them
    static mut UARTE_RX_BUFFER: [u8; BUFFER_SIZE] = [0u8; BUFFER_SIZE];
    static BUFFER_TAKEN: AtomicBool = AtomicBool::new(false);

    pub struct UarteRxBuffer {
        _marker: PhantomData<bool>,
    }

    impl UarteRxBuffer {
        pub fn take() -> Option<Self> {
            if BUFFER_TAKEN.swap(true, Ordering::Relaxed) {
                return None;
            }
            Some(Self {
                _marker: PhantomData,
            })
        }

        pub fn as_slice(&self) -> &'static [u8] {
            unsafe { &UARTE_RX_BUFFER }
        }
    }
}
