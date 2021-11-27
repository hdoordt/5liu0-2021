#[cfg(feature = "defmt")]
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct DeviceToServer {
    pub pan_tilt: Option<PanTiltStatus>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct PanTiltStatus {
    pub pan_deg: f32,
    pub tilt_deg: f32,
}
