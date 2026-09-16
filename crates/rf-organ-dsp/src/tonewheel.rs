// SPDX-License-Identifier: GPL-2.0-or-later
use core::f32::consts::{PI, TAU};

pub const TONEWHEEL_COUNT: usize = 91;

// Translated from the 60 Hz generator table in setBfree, which in turn models
// the tooth counts and gear train of the physical tone generator.
const GEAR_RATIOS_60_HZ: [(u16, u16); 12] = [
    (85, 104),
    (71, 82),
    (67, 73),
    (35, 36),
    (69, 67),
    (12, 11),
    (37, 32),
    (49, 40),
    (48, 37),
    (11, 8),
    (67, 46),
    (54, 35),
];

/// Mechanical 60 Hz generator frequency for terminal `index + 1`.
pub fn gear_frequency(index: usize) -> Option<f32> {
    if index >= TONEWHEEL_COUNT {
        return None;
    }
    let note = index % 12;
    let octave = index / 12;
    let (ratio_index, teeth) = if index >= 84 {
        (note + 5, 192_u16)
    } else {
        (note, 2_u16 << octave)
    };
    let (driven, driving) = GEAR_RATIOS_60_HZ[ratio_index];
    Some(20.0 * f32::from(teeth) * f32::from(driven) / f32::from(driving))
}

#[derive(Clone, Copy)]
struct Tonewheel {
    sine: f32,
    cosine: f32,
    rotation_sine: f32,
    rotation_cosine: f32,
    frequency: f32,
}

impl Tonewheel {
    const EMPTY: Self = Self {
        sine: 0.0,
        cosine: 1.0,
        rotation_sine: 0.0,
        rotation_cosine: 1.0,
        frequency: 0.0,
    };

    fn tick(&mut self, sample_rate: f32, index: usize) -> f32 {
        let sine = self.sine * self.rotation_cosine + self.cosine * self.rotation_sine;
        let cosine = self.cosine * self.rotation_cosine - self.sine * self.rotation_sine;
        let correction = 1.5 - 0.5 * (sine * sine + cosine * cosine);
        self.sine = sine * correction;
        self.cosine = cosine * correction;

        // Low wheels in measured generators are visibly less sinusoidal. This
        // compact band-limited profile is provisional pending wheel recordings.
        let third = sine * (3.0 - 4.0 * sine * sine);
        let squared = sine * sine;
        let fifth = sine * (16.0 * squared * squared - 20.0 * squared + 5.0);
        let third_gain = if index < 24 && self.frequency * 3.0 < sample_rate * 0.48 {
            if index < 12 { 0.12 } else { 0.045 }
        } else {
            0.0
        };
        let fifth_gain = if index < 12 && self.frequency * 5.0 < sample_rate * 0.48 {
            0.035
        } else {
            0.0
        };
        (sine + third_gain * third + fifth_gain * fifth) / (1.0 + third_gain + fifth_gain)
    }
}

pub struct TonewheelBank {
    wheels: [Tonewheel; TONEWHEEL_COUNT],
    samples: [f32; TONEWHEEL_COUNT],
    sample_rate: f32,
}

impl TonewheelBank {
    pub fn new(sample_rate: f32) -> Self {
        let mut wheels = [Tonewheel::EMPTY; TONEWHEEL_COUNT];
        for (index, wheel) in wheels.iter_mut().enumerate() {
            let frequency = gear_frequency(index).expect("bounded wheel index");
            let increment = TAU * frequency / sample_rate;
            let (rotation_sine, rotation_cosine) = sin_cos(increment);
            let initial_phase =
                -PI + TAU * ((index * 37 % TONEWHEEL_COUNT) as f32) / TONEWHEEL_COUNT as f32;
            let (sine, cosine) = sin_cos(initial_phase);
            *wheel = Tonewheel {
                sine,
                cosine,
                rotation_sine,
                rotation_cosine,
                frequency,
            };
        }
        Self {
            wheels,
            samples: [0.0; TONEWHEEL_COUNT],
            sample_rate,
        }
    }

    pub fn tick(&mut self) {
        for (index, (wheel, sample)) in self
            .wheels
            .iter_mut()
            .zip(self.samples.iter_mut())
            .enumerate()
        {
            *sample = wheel.tick(self.sample_rate, index);
        }
    }

    pub const fn samples(&self) -> &[f32; TONEWHEEL_COUNT] {
        &self.samples
    }
}

fn sin_cos(angle: f32) -> (f32, f32) {
    let x2 = angle * angle;
    let sine = angle * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))));
    let cosine = 1.0 - x2 / 2.0 * (1.0 - x2 / 12.0 * (1.0 - x2 / 30.0 * (1.0 - x2 / 56.0)));
    (sine, cosine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gear_table_spans_the_physical_generator() {
        let low = gear_frequency(0).expect("low wheel");
        let high = gear_frequency(90).expect("high wheel");
        assert!((low - 32.69).abs() < 0.05);
        assert!((high - 5925.7).abs() < 2.0);
    }

    #[test]
    fn table_is_strictly_ascending() {
        for index in 1..TONEWHEEL_COUNT {
            assert!(gear_frequency(index) > gear_frequency(index - 1));
        }
    }
}
