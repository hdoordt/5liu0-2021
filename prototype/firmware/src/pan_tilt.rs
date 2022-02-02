use folley_format::device_to_server::PanTiltStatus;
use nrf52840_hal::{
    twim::{Frequency, Instance, Pins},
    Twim,
};

use pwm_pca9685::{Address, Channel, Pca9685};

pub struct PanTilt<TWIM> {
    pwm: Pca9685<TWIM>,
    pan_deg_goal: i32,
    tilt_deg_goal: i32,
    tilt_deg: i32,
    pan_deg: i32,
}

const TILT_LIMIT_DEG: i32 = 90;
const TILT_0_DEG: u16 = 760;
const TILT_180_DEG: u16 = 2900;

fn tilt_deg_to_off_val(degrees: i32) -> u16 {
    (TILT_180_DEG - TILT_0_DEG) / 180 * (degrees.min(180) as u16) + TILT_0_DEG
}

const PAN_LIMIT_DEG: i32 = 180;
const PAN_0_DEG: u16 = 760;
const PAN_360_DEG: u16 = 2930;

fn pan_deg_to_off_val(degrees: i32) -> u16 {
    (PAN_360_DEG - PAN_0_DEG) / 180 * (degrees.min(180) as u16) + PAN_0_DEG
}

impl<T: Instance> PanTilt<Twim<T>> {
    pub fn new(twim: T, pins: Pins, pan_deg: i32, tilt_deg: i32) -> Self {
        let twim0 = Twim::new(twim, pins, Frequency::K400);

        let mut pwm =
            Pca9685::new(twim0, Address::default()).expect("Error initializing PWM controller");
        pwm.enable().unwrap();
        pwm.set_prescale(20).unwrap();
        pwm.set_channel_on(Channel::C14, 0).unwrap();
        pwm.set_channel_off(Channel::C14, 0).unwrap();
        pwm.set_channel_on(Channel::C15, 0).unwrap();
        pwm.set_channel_off(Channel::C15, 0).unwrap();

        let mut pan_tilt = Self {
            pwm,
            pan_deg_goal: pan_deg,
            tilt_deg_goal: tilt_deg,
            pan_deg: 0,
            tilt_deg: 0,
        };

        pan_tilt.pan_to_deg(pan_deg);
        pan_tilt.tilt_to_deg(tilt_deg);

        pan_tilt
    }

    pub fn status(&self) -> PanTiltStatus {
        PanTiltStatus {
            pan_deg: self.pan_deg_goal,
            tilt_deg: self.tilt_deg_goal,
        }
    }

    pub fn tilt_to_deg(&mut self, degrees: i32) {
        let degrees = degrees.min(TILT_LIMIT_DEG);
        self.tilt_deg_goal = degrees;
        defmt::trace!("Tilt goal: {} degrees", degrees);   
    }

    pub fn pan_to_deg(&mut self, degrees: i32) {
        let degrees = degrees.min(PAN_LIMIT_DEG);
        self.pan_deg_goal =degrees;
        defmt::trace!("Pan goal: {} degrees", degrees);
        
    }

    pub fn tilt_with_deg(&mut self, degrees: i32) {
        let degrees = (degrees + self.tilt_deg as i32).max(0);
        self.tilt_to_deg(degrees as i32);
    }

    pub fn pan_with_deg(&mut self, degrees: i32) {
        let degrees = (degrees + self.pan_deg as i32).max(0);
        self.pan_to_deg(degrees as i32);
    }

    pub fn step(&mut self) {
        fn next_angle(current: i32, goal: i32) -> i32 {
            use core::cmp::Ordering::*;
            match current.cmp(&goal) {
                Greater => current -1,
                Less => current + 1,
                Equal => current,
            }
        }

        let tilt_deg = next_angle(self.tilt_deg, self.tilt_deg_goal);
        let pan_deg = next_angle(self.pan_deg, self.pan_deg_goal);

        let tilt_val = tilt_deg_to_off_val(tilt_deg);
        self.pwm.set_channel_off(Channel::C14, tilt_val as u16).unwrap();
        let pan_val = pan_deg_to_off_val(pan_deg);
        self.pwm.set_channel_off(Channel::C15, pan_val as u16).unwrap();
        self.tilt_deg = tilt_deg;
        self.pan_deg = pan_deg;
        defmt::debug!("Pan: {} ({}), tilt: {} ({})", pan_deg, self.pan_deg_goal, tilt_deg,  self.tilt_deg_goal);
    }
}
