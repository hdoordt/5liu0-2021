#![cfg_attr(not(feature = "std"), no_std)]

use folley_format::device_to_server::MicArraySample;
const V_SOUND: i32 = 343;

/// Calculate the cross-correlation of real-valued signals x and y. The result is put in the output buffer.
/// Make sure x and y are of the same length M, and the output buffer is of length N = 2* M -1.M
#[allow(non_snake_case)]
pub fn xcorr_real<const N: usize, const M: usize>(x: &[i16; M], y: &[i16; M], out: &mut [u32; N]) {
    debug_assert_eq!(N, 2 * M - 1);
    // This method may be improved by taking the Fourier transform X and Y of each of the signals x and Y,
    // multiplying the output of X with the complex conjugate of Y, and reverse-transform the product.
    for n in 0..N {
        for m in 0..M {
            let y_val = y[m] as u32;
            let x_index = (n + m) as isize - (N as isize) / 2;
            let x_val = if x_index >= 0 {
                *x.get(x_index as usize).unwrap_or(&0)
            } else {
                0
            } as u32;
            out[n] += x_val * y_val;
        }
    }
}

/// Calculate the lag in sample numbers of signals x and y using cross-correlation. The buffer is used
/// to store the cross-correlation output.
#[allow(non_snake_case)]
pub fn calc_lag<const N: usize, const M: usize>(
    x: &[i16; M],
    y: &[i16; M],
    buf: &mut [u32; N],
) -> isize {
    use core::cmp::Ordering;
    xcorr_real(x, y, buf);
    let lag_offset = buf.len() as isize / 2;
    buf.iter()
        .map(|v| *v as u32)
        .enumerate()
        .max_by(|(i, curr), (_, prev)| curr.cmp(prev))
        .map(|(argmax, _)| argmax as isize - lag_offset)
        .unwrap()
}

/// Calculate the angle of an audio source, using the cross correlation of two signals.
pub fn calc_angle<
    const T_S_US: u32,
    const D_MICS_MM: u32,
    const N: usize,
    const M: usize,
    const K: usize,
>(
    x: &[i16; M],
    y: &[i16; M],
    buf: &mut [u32; N],
    lag_table: &[u32; K],
) -> u32 {
    let lag = calc_lag(x, y, buf) as i32;
    let lag = reduce_lag::<K>(lag);
    lag_to_angle::<T_S_US, D_MICS_MM, K>(lag, lag_table)
}

pub fn lag_to_angle<const T_S_US: u32, const D_MICS_MM: u32, const K: usize>(
    lag: i32,
    table: &[u32; K],
) -> u32 {
    let i = lag + K as i32 / 2;
    table[i as usize]
}

pub fn gen_lag_table<const T_S_US: u32, const D_MICS_MM: u32, const K: usize>() -> [u32; K] {
    debug_assert_eq!(K, expected_lags_size(T_S_US, D_MICS_MM));
    let mut table = [0u32; K];

    table.iter_mut().enumerate().for_each(|(lag, angle)| {
        let lag = lag as i32 - K as i32 / 2;
        let cos_theta = (lag * T_S_US as i32 * V_SOUND) / D_MICS_MM as i32;
        let theta = ACOS_TABLE
            .iter()
            .find(|(cos, deg)| *cos <= cos_theta)
            .map(|(_, deg)| *deg)
            .unwrap_or(0);

        *angle = theta as u32;
    });
    table
}

pub const fn expected_lags_size(sample_period_us: u32, mic_distance_mm: u32) -> usize {
    (mic_distance_mm * 1000 / (sample_period_us * V_SOUND as u32)) as usize * 2 + 1
}

pub fn reduce_lag<const K: usize>(lag: i32) -> i32 {
    let MAX_LAG = ((K - 1) / 2) as i32;
    let MIN_LAG: i32 = -MAX_LAG;

    if lag > MAX_LAG {
        MIN_LAG + (lag % MAX_LAG)
        // MAX_LAG
    } else if lag < MIN_LAG {
        MAX_LAG + (lag % MIN_LAG)
        // MIN_LAG
    } else {
        lag
    }
}

pub struct Channels<const N: usize> {
    pub ch1: [i16; N],
    pub ch2: [i16; N],
    pub ch3: [i16; N],
    pub ch4: [i16; N],
}

impl<const N: usize> Channels<N> {
    pub fn from_samples(samples: [MicArraySample; N]) -> Self {
        let mut chans = Channels {
            ch1: [0i16; N],
            ch2: [0i16; N],
            ch3: [0i16; N],
            ch4: [0i16; N],
        };
        samples.into_iter().enumerate().for_each(|(i, s)| {
            chans.ch1[i] = s[0];
            chans.ch2[i] = s[1];
            chans.ch3[i] = s[2];
            chans.ch4[i] = s[3];
        });
        chans
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

        const EXPECTED: [u32; N] = [
            2653233, 5429466, 8293409, 11057978, 13463163, 15577371, 17567748, 19514665, 21620069,
            24154372, 21615116, 18974750, 16217227, 13470869, 10999032, 8760016, 6633330, 4574504,
            2452032,
        ];

        let samples: [_; M] = read_samples::<M>().try_into().unwrap();
        let channels = Channels::from_samples(samples);

        let mut out = [0u32; N];
        xcorr_real(&channels.ch1, &channels.ch2, &mut out);

        let _: Vec<_> = (0..M).map(|i| assert_eq!(out[i], EXPECTED[i])).collect();
    }

    #[test]
    pub fn test_calc_lag() {
        const M: usize = 1024;
        const N: usize = 2 * M - 1;
        let samples: [_; M] = read_samples::<M>().try_into().unwrap();
        let channels = Channels::from_samples(samples);
        let mut buf = [0u32; N];
        let lag = calc_lag(&channels.ch1, &channels.ch2, &mut buf);
        assert_eq!(lag, 3);
    }

    #[test]
    pub fn test_calc_angle() {
        const M: usize = 1024;
        const N: usize = 2 * M - 1;
        let samples: [_; M] = read_samples::<M>().try_into().unwrap();
        let channels = Channels::from_samples(samples);
        let mut buf = [0u32; N];
        let lag_table = gen_lag_table::<74, 125, 9>();
        let theta =
            calc_angle::<74, 125, N, M, 9>(&channels.ch1, &channels.ch2, &mut buf, &lag_table);
        assert_eq!(theta, 53);
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

    #[test]
    fn test_reduce_lag() {
        const K: usize = expected_lags_size(14, 125);
        const LAGS: [(i32, i32); 4] = [(-27, 25), (26, 26), (-26, -26), (27, -25)];
        LAGS.iter().for_each(|(lag, ref expected)| {
            assert_eq!(reduce_lag::<K>(*lag), *expected);
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
