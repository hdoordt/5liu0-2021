#![cfg_attr(not(feature = "std"), no_std)]

use folley_format::device_to_server::MicArraySample;
const V_SOUND: i32 = 343;

/// Calculate the cross-correlation of real-valued signals x and y. The result is put in the output buffer.
/// Make sure x and y are of the same length M, and the output buffer is of length N <= 2* M -1.M
#[allow(non_snake_case)]
pub fn xcorr_real<const XCORR_LEN: usize, const SIGNAL_LEN: usize>(
    x: &[i16; SIGNAL_LEN],
    y: &[i16; SIGNAL_LEN],
    out: &mut [i64; XCORR_LEN],
) -> usize {
    debug_assert!(XCORR_LEN <= 2 * SIGNAL_LEN - 1);
    // This method may be improved by taking the Fourier transform X and Y of each of the signals x and Y,
    // multiplying the output of X with the complex conjugate of Y, and reverse-transform the product.
    let mut argmax = 0;
    let mut max = 0;
    for n in 0..XCORR_LEN {
        for m in 0..SIGNAL_LEN {
            let x_val = x[m] as i64;
            let y_index = (n + m) as isize - (XCORR_LEN as isize) / 2;
            let y_val = if y_index >= 0 {
                *y.get(y_index as usize).unwrap_or(&0)
            } else {
                0
            } as i64;
            out[n] += x_val * y_val;
        }
        if out[n] > max {
            max = out[n];
            argmax = n;
        }
    }
    argmax
}

/// Calculate the lag in sample numbers of signals x and y using cross-correlation. The buffer is used
/// to store the cross-correlation output.
#[allow(non_snake_case)]
pub fn calc_lag<const XCORR_LEN: usize, const SIGNAL_LEN: usize>(
    x: &[i16; SIGNAL_LEN],
    y: &[i16; SIGNAL_LEN],
    buf: &mut [i64; XCORR_LEN],
) -> isize {
    let argmax = xcorr_real(x, y, buf) as isize;
    let lag_offset = XCORR_LEN as isize / 2;
    argmax - lag_offset
}

/// Calculate the angle of an audio source, using the cross correlation of two signals.
pub fn calc_angle<
    const T_S_US: u32,
    const D_MICS_MM: u32,
    const XCORR_LEN: usize,
    const SIGNAL_LEN: usize,
>(
    x: &[i16; SIGNAL_LEN],
    y: &[i16; SIGNAL_LEN],
    buf: &mut [i64; XCORR_LEN],
    lag_table: &[u32; XCORR_LEN],
) -> u32 {
    let lag = calc_lag(x, y, buf) as i32;
    lag_to_angle::<T_S_US, D_MICS_MM, XCORR_LEN>(lag, lag_table)
}

pub fn lag_to_angle<const T_S_US: u32, const D_MICS_MM: u32, const LAGS_SIZE: usize>(
    lag: i32,
    table: &[u32; LAGS_SIZE],
) -> u32 {
    let i = lag + LAGS_SIZE as i32 / 2;
    table[i as usize]
}

pub fn gen_lag_table<const T_S_US: u32, const D_MICS_MM: u32, const SIZE: usize>() -> [u32; SIZE] {
    debug_assert_eq!(SIZE, max_lags_size(T_S_US, D_MICS_MM));
    let mut table = [0u32; SIZE];

    table.iter_mut().enumerate().for_each(|(lag, angle)| {
        let lag = lag as i32 - SIZE as i32 / 2;
        let cos_theta = (lag * T_S_US as i32 * V_SOUND) / D_MICS_MM as i32;
        let theta = ACOS_TABLE
            .iter()
            .find(|(cos, _)| *cos <= cos_theta)
            .map(|(_, deg)| *deg)
            .unwrap_or(0);

        *angle = theta as u32;
    });
    table
}

/// Given the distance between two microphones in millimeters, the sample period in microseconds,
/// and the speed of sound in m/s, calculates the maximum number of samples possible between
/// the moment the signal hits the first microphone and the moment it reaches the second.
pub const fn max_lags_size(sample_period_us: u32, mic_distance_mm: u32) -> usize {
    (mic_distance_mm * 1000 / (sample_period_us * V_SOUND as u32)) as usize * 2 + 1
}

/// Representation of samples of 4 separate channels
pub struct Channels<const SIGNAL_LEN: usize> {
    pub ch1: [i16; SIGNAL_LEN],
    pub ch2: [i16; SIGNAL_LEN],
    pub ch3: [i16; SIGNAL_LEN],
    pub ch4: [i16; SIGNAL_LEN],
}

impl<const SIGNAL_LEN: usize> Channels<SIGNAL_LEN> {
    /// Put samples into four separate arrays and bundle them in a Channels object.
    /// In this method, the channel mean is subtracted from each sample,
    /// in order to make the DC value ~ zero. This improves cross correlation.
    pub fn from_samples(samples: [MicArraySample; SIGNAL_LEN]) -> Self {
        let mut chans = Channels {
            ch1: [0i16; SIGNAL_LEN],
            ch2: [0i16; SIGNAL_LEN],
            ch3: [0i16; SIGNAL_LEN],
            ch4: [0i16; SIGNAL_LEN],
        };
        let mut totals = [0i64; 4];
        samples.into_iter().enumerate().for_each(|(i, s)| {
            chans.ch1[i] = s[0];
            totals[0] += s[0] as i64;
            chans.ch2[i] = s[1];
            totals[1] += s[1] as i64;
            chans.ch3[i] = s[2];
            totals[2] += s[2] as i64;
            chans.ch4[i] = s[3];
            totals[3] += s[3] as i64;
        });

        chans
            .channels_mut()
            .iter_mut()
            .enumerate()
            .for_each(|(i, ch)| {
                let mean = totals[i] / SIGNAL_LEN as i64;
                let mean = mean as i16;
                ch.iter_mut().for_each(|s| *s = s.saturating_sub(mean));
            });

        chans
    }

    fn channels_mut(&mut self) -> [&mut [i16; SIGNAL_LEN]; 4] {
        [&mut self.ch1, &mut self.ch2, &mut self.ch3, &mut self.ch4]
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod test {
    use std::{
        cmp::Ordering,
        collections::{HashMap, HashSet},
    };

    use folley_format::device_to_server::MicArraySample;

    use crate::*;

    fn read_samples<const N: usize>() -> Vec<MicArraySample> {
        include_str!("../samples")
            .lines()
            .take(N)
            .map(|l| {
                let mut chans = l.split(',').map(|ch| ch.parse());
                [
                    chans.next().unwrap().unwrap(),
                    chans.next().unwrap().unwrap(),
                    chans.next().unwrap().unwrap(),
                    chans.next().unwrap().unwrap(),
                ]
            })
            .collect()
    }

    #[test]
    pub fn test_xcorr() {
        const M: usize = 10;
        const N: usize = 2 * M - 1;

        const EXPECTED: [i64; N] = [
            -38192, -78724, -49321, 24901, 87948, 159501, 168362, 80307, -28044, -148461, -192540,
            -134940, -52434, 9653, 71414, 66513, 39441, 21076, -6440,
        ];

        let samples: [_; M] = read_samples::<M>().try_into().unwrap();
        let channels = Channels::from_samples(samples);

        let mut out = [0i64; N];
        xcorr_real(&channels.ch1, &channels.ch2, &mut out);

        (0..N).for_each(|i| assert_eq!(out[i], EXPECTED[i]));
    }

    #[test]
    pub fn test_calc_lag() {
        const M: usize = 1024;
        const N: usize = 2 * M - 1;
        let samples: [_; M] = read_samples::<M>().try_into().unwrap();
        let channels = Channels::from_samples(samples);
        let mut buf = [0i64; N];
        let lag = calc_lag(&channels.ch1, &channels.ch2, &mut buf);
        assert_eq!(lag, -4);
    }

    #[test]
    pub fn test_calc_angle() {
        const M: usize = 1024;
        const N: usize = 9;
        let samples: [_; M] = read_samples::<M>().try_into().unwrap();
        let channels = Channels::from_samples(samples);
        let mut buf = [0i64; N];
        let lag_table = gen_lag_table::<74, 125, N>();
        let theta = calc_angle::<74, 125, N, M>(&channels.ch1, &channels.ch2, &mut buf, &lag_table);
        assert_eq!(theta, 145);
    }

    #[test]
    pub fn test_lag_to_angle() {
        const FLOORED_LAG_ANGLES: [(i32, u32); 53] = [
            (-26, 177),
            (-25, 164),
            (-24, 158),
            (-23, 153),
            (-22, 148),
            (-21, 144),
            (-20, 141),
            (-19, 137),
            (-18, 134),
            (-17, 131),
            (-16, 128),
            (-15, 126),
            (-14, 123),
            (-13, 120),
            (-12, 118),
            (-11, 115),
            (-10, 113),
            (-9, 111),
            (-8, 108),
            (-7, 106),
            (-6, 104),
            (-5, 102),
            (-4, 99),
            (-3, 97),
            (-2, 95),
            (-1, 93),
            (0, 90),
            (1, 88),
            (2, 86),
            (3, 84),
            (4, 82),
            (5, 79),
            (6, 77),
            (7, 75),
            (8, 73),
            (9, 70),
            (10, 68),
            (11, 65),
            (12, 63),
            (13, 61),
            (14, 58),
            (15, 55),
            (16, 53),
            (17, 50),
            (18, 47),
            (19, 44),
            (20, 40),
            (21, 37),
            (22, 33),
            (23, 28),
            (24, 23),
            (25, 17),
            (26, 3),
        ];
        let lag_table = gen_lag_table::<14, 125, 53>();
        FLOORED_LAG_ANGLES.iter().for_each(|(lag, angle)| {
            assert_eq!(lag_to_angle::<14, 125, 53>(*lag, &lag_table), *angle)
        });
    }
}

const ACOS_TABLE: [(i32, i32); 181] = [
    (1000, 0),
    (999, 1),
    (999, 2),
    (998, 3),
    (997, 4),
    (996, 5),
    (994, 6),
    (992, 7),
    (990, 8),
    (987, 9),
    (984, 10),
    (981, 11),
    (978, 12),
    (974, 13),
    (970, 14),
    (965, 15),
    (961, 16),
    (956, 17),
    (951, 18),
    (945, 19),
    (939, 20),
    (933, 21),
    (927, 22),
    (920, 23),
    (913, 24),
    (906, 25),
    (898, 26),
    (891, 27),
    (882, 28),
    (874, 29),
    (866, 30),
    (857, 31),
    (848, 32),
    (838, 33),
    (829, 34),
    (819, 35),
    (809, 36),
    (798, 37),
    (788, 38),
    (777, 39),
    (766, 40),
    (754, 41),
    (743, 42),
    (731, 43),
    (719, 44),
    (707, 45),
    (694, 46),
    (682, 47),
    (669, 48),
    (656, 49),
    (642, 50),
    (629, 51),
    (615, 52),
    (601, 53),
    (587, 54),
    (573, 55),
    (559, 56),
    (544, 57),
    (529, 58),
    (515, 59),
    (500, 60),
    (484, 61),
    (469, 62),
    (454, 63),
    (438, 64),
    (422, 65),
    (406, 66),
    (390, 67),
    (374, 68),
    (358, 69),
    (342, 70),
    (325, 71),
    (309, 72),
    (292, 73),
    (275, 74),
    (258, 75),
    (241, 76),
    (225, 77),
    (207, 78),
    (190, 79),
    (173, 80),
    (156, 81),
    (139, 82),
    (121, 83),
    (104, 84),
    (087, 85),
    (069, 86),
    (052, 87),
    (034, 88),
    (017, 89),
    (000, 90),
    (-017, 91),
    (-034, 92),
    (-052, 93),
    (-069, 94),
    (-087, 95),
    (-104, 96),
    (-121, 97),
    (-139, 98),
    (-156, 99),
    (-173, 100),
    (-190, 101),
    (-207, 102),
    (-225, 103),
    (-241, 104),
    (-258, 105),
    (-275, 106),
    (-292, 107),
    (-309, 108),
    (-325, 109),
    (-342, 110),
    (-358, 111),
    (-374, 112),
    (-390, 113),
    (-406, 114),
    (-422, 115),
    (-438, 116),
    (-454, 117),
    (-469, 118),
    (-484, 119),
    (-500, 120),
    (-515, 121),
    (-529, 122),
    (-544, 123),
    (-559, 124),
    (-573, 125),
    (-587, 126),
    (-601, 127),
    (-615, 128),
    (-629, 129),
    (-642, 130),
    (-656, 131),
    (-669, 132),
    (-682, 133),
    (-694, 134),
    (-707, 135),
    (-719, 136),
    (-731, 137),
    (-743, 138),
    (-754, 139),
    (-766, 140),
    (-777, 141),
    (-788, 142),
    (-798, 143),
    (-809, 144),
    (-819, 145),
    (-829, 146),
    (-838, 147),
    (-848, 148),
    (-857, 149),
    (-866, 150),
    (-874, 151),
    (-882, 152),
    (-891, 153),
    (-898, 154),
    (-906, 155),
    (-913, 156),
    (-920, 157),
    (-927, 158),
    (-933, 159),
    (-939, 160),
    (-945, 161),
    (-951, 162),
    (-956, 163),
    (-961, 164),
    (-965, 165),
    (-970, 166),
    (-974, 167),
    (-978, 168),
    (-981, 169),
    (-984, 170),
    (-987, 171),
    (-990, 172),
    (-992, 173),
    (-994, 174),
    (-996, 175),
    (-997, 176),
    (-998, 177),
    (-999, 178),
    (-999, 179),
    (-1000, 180),
];
