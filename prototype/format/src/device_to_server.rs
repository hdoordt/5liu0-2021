#[cfg(feature = "defmt")]
use defmt::Format;
use serde::{Deserialize, Serialize};

use serde_big_array::big_array;
big_array! { BigArray; }

#[derive(Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub enum DeviceToServer {
    Sync,
    PanTilt(PanTiltStatus),
    #[serde(with = "BigArray")]
    Samples([MicArraySample; 1024]),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct PanTiltStatus {
    pub pan_deg: i32,
    pub tilt_deg: i32,
}

pub type MicArraySample = [i16; 4];
