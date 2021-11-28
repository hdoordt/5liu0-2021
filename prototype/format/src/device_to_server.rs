use core::ops::{Deref, DerefMut};

#[cfg(feature = "defmt")]
use defmt::Format;
use serde::{Deserialize, Serialize};

use serde_big_array::big_array;
big_array! { BigArray; }

#[derive(Serialize, Deserialize, Debug, Default)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct DeviceToServer {
    pub pan_tilt: Option<PanTiltStatus>,
    pub samples: Option<SampleBuffer>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct PanTiltStatus {
    pub pan_deg: f32,
    pub tilt_deg: f32,
}



pub type MicArraySample = [i16; 4];
#[derive(Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "defmt", derive(Format))]
#[repr(transparent)]
pub struct SampleBuffer(#[serde(with = "BigArray")] pub [MicArraySample; Self::size()]);

impl SampleBuffer {
    const SIZE: usize = 64;

    pub const fn size() -> usize {
        Self::SIZE
    }
}

impl Default for SampleBuffer {
    fn default() -> Self {
        Self([[0i16; 4]; Self::size()])
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
