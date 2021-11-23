#[cfg(feature = "defmt")]
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct ServerToDevice {
    pub pan_degrees: Option<f32>,
    pub tilt_degrees: Option<f32>,
}
