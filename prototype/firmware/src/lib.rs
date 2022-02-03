#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering};

use defmt_rtt as _; // global logger

use panic_probe as _;

pub mod consts {
    use folley_calc::max_lags_size;

    /// Sample period in microseconds
    pub const T_S_US: u32 = 37;
    /// Distance between two mics in millimeters
    pub const D_MICS_MM: u32 = 125;

    /// Size of a set of samples
    pub const SAMPLE_BUF_SIZE: usize = 1024;
    /// Amount of lags evaluated in the cross correlation
    pub const XCORR_LEN: usize = max_lags_size(T_S_US, D_MICS_MM);
}

#[cfg(feature = "mic_array")]
pub mod mic_array;
#[cfg(feature = "pan_tilt")]
pub mod pan_tilt;
#[cfg(feature = "uart")]
pub mod uarte;

/// Workaround for RTIC not being able to
/// conditionally compile resources
pub mod stubs {
    use core::marker::PhantomData;

    pub struct Uarte<U, T, P>(PhantomData<U>, PhantomData<T>, PhantomData<P>);
    pub struct CobsAccumulator<const N: usize>;

    pub struct MicArray<M1, M2, M3, M4, T, P>(
        PhantomData<M1>,
        PhantomData<M2>,
        PhantomData<M3>,
        PhantomData<M4>,
        PhantomData<T>,
        PhantomData<P>,
    );
    pub struct PanTilt<T>(PhantomData<T>);
}

// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf()
}

static COUNT: AtomicUsize = AtomicUsize::new(0);
defmt::timestamp!("{=usize}", {
    // NOTE(no-CAS) `timestamps` runs with interrupts disabled
    let n = COUNT.load(Ordering::Relaxed);
    COUNT.store(n + 1, Ordering::Relaxed);
    n
});

/// Terminates the application and makes `probe-run` exit with exit-code = 0
pub fn exit() -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}
