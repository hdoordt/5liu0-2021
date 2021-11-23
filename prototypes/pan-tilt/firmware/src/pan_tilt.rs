use nrf52840_hal::{
    twim::{Frequency, Instance, Pins},
    Twim,
};
use pwm_pca9685::{Address, Channel, Pca9685};

pub struct PanTilt<PWM> {
    pwm: PWM,
}

const TILT_LIMIT_DEG: f32 = 150.;
const TILT_0_DEG: f32 = 760.;
const TILT_180_DEG: f32 = 2900.;

fn tilt_deg_to_off_val(degrees: f32) -> f32 {
    (TILT_180_DEG - TILT_0_DEG) / 180. * degrees + TILT_0_DEG
}

const PAN_LIMIT_DEG: f32 = 180.;
const PAN_0_DEG: f32 = 760.;
const PAN_360_DEG: f32 = 2930.;

fn pan_deg_to_off_val(degrees: f32) -> f32 {
    (PAN_360_DEG - PAN_0_DEG) / 180. * degrees + PAN_0_DEG
}

fn rad_to_deg(rad: f32) -> f32 {
    let pi = core::f32::consts::PI;

    (rad * pi) / 180.
}

impl<T: Instance> PanTilt<Pca9685<Twim<T>>> {
    pub fn new(twim: T, pins: Pins) -> Self {
        let twim0 = Twim::new(twim, pins, Frequency::K400);

        let mut pwm =
            Pca9685::new(twim0, Address::default()).expect("Error initializing PWM controller");
        pwm.enable().unwrap();
        pwm.set_prescale(20).unwrap();
        pwm.set_channel_on(Channel::C14, 0).unwrap();
        pwm.set_channel_off(Channel::C14, 0).unwrap();
        pwm.set_channel_on(Channel::C15, 0).unwrap();
        pwm.set_channel_off(Channel::C15, 0).unwrap();

        Self { pwm }
    }

    pub fn tilt_deg(&mut self, degrees: f32) {
        let degrees = degrees.min(TILT_LIMIT_DEG).max(0.);
        let val = tilt_deg_to_off_val(degrees);
        defmt::debug!("Tilt value: {}; degrees: {}", val, degrees);
        self.pwm.set_channel_off(Channel::C14, val as u16).unwrap();
    }

    pub fn pan_deg(&mut self, degrees: f32) {
        let degrees = degrees.min(PAN_LIMIT_DEG).max(0.);
        let val = pan_deg_to_off_val(degrees);
        defmt::debug!("Pan value: {}; degrees: {}", val, degrees);
        self.pwm.set_channel_off(Channel::C15, val as u16).unwrap();
    }

    pub fn tilt_rad(&mut self, rad: f32) {
        self.tilt_deg(rad_to_deg(rad));
    }

    pub fn pan_rad(&mut self, rad: f32) {
        self.pan_deg(rad_to_deg(rad));
    }
}
