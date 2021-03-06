#[cfg(feature = "defmt")]
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct ServerToDevice {
    pub pan_degrees: Option<i32>,
    pub tilt_degrees: Option<i32>,
    pub set_sampling_enabled: Option<bool>,
}
