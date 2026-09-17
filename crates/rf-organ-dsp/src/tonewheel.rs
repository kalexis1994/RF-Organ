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
    let (teeth, driven, driving) = gearing(index)?;
    Some(20.0 * f32::from(teeth) * f32::from(driven) / f32::from(driving))
}

/// Teeth on the wheel at terminal `index + 1`. The wheel turns once per that
/// many cycles of its tone, which is the rate at which an off-centre wheel
/// changes its distance to the pickup.
pub const fn gear_teeth(index: usize) -> Option<u16> {
    match gearing(index) {
        Some((teeth, _, _)) => Some(teeth),
        None => None,
    }
}

const fn gearing(index: usize) -> Option<(u16, u16, u16)> {
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
    Some((teeth, driven, driving))
}

/// Hammond describes an off-centre wheel as moving its high spots nearer to
/// and further from the pickup once per revolution, so the tone becomes
/// slightly louder and softer at that rate. The depth of it differs from
/// organ to organ; this default is provisional and the laboratory measures
/// the sidebands it produces.
const ECCENTRICITY_DEPTH: f32 = 0.015;

#[derive(Clone, Copy)]
struct Tonewheel {
    sine: f32,
    cosine: f32,
    rotation_sine: f32,
    rotation_cosine: f32,
    /// Once-per-revolution phasor, for wheel eccentricity.
    eccentric_sine: f32,
    eccentric_cosine: f32,
    eccentric_rotation_sine: f32,
    eccentric_rotation_cosine: f32,
    frequency: f32,
    /// Output level of this wheel, which differs from wheel to wheel on a real
    /// generator. Flat until a console is measured.
    level: f32,
}

impl Tonewheel {
    const EMPTY: Self = Self {
        sine: 0.0,
        cosine: 1.0,
        rotation_sine: 0.0,
        rotation_cosine: 1.0,
        eccentric_sine: 0.0,
        eccentric_cosine: 1.0,
        eccentric_rotation_sine: 0.0,
        eccentric_rotation_cosine: 1.0,
        frequency: 0.0,
        level: 1.0,
    };

    fn tick(&mut self, sample_rate: f32, index: usize, eccentricity: f32) -> f32 {
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
        let shape =
            (sine + third_gain * third + fifth_gain * fifth) / (1.0 + third_gain + fifth_gain);

        let eccentric_sine = self.eccentric_sine * self.eccentric_rotation_cosine
            + self.eccentric_cosine * self.eccentric_rotation_sine;
        let eccentric_cosine = self.eccentric_cosine * self.eccentric_rotation_cosine
            - self.eccentric_sine * self.eccentric_rotation_sine;
        let eccentric_correction =
            1.5 - 0.5 * (eccentric_sine * eccentric_sine + eccentric_cosine * eccentric_cosine);
        self.eccentric_sine = eccentric_sine * eccentric_correction;
        self.eccentric_cosine = eccentric_cosine * eccentric_correction;

        shape * self.level * (1.0 + eccentricity * self.eccentric_sine)
    }
}

pub struct TonewheelBank {
    wheels: [Tonewheel; TONEWHEEL_COUNT],
    samples: [f32; TONEWHEEL_COUNT],
    sample_rate: f32,
    eccentricity: f32,
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
            // One revolution per `teeth` cycles of the tone, and each wheel
            // was stamped and mounted on its own, so they do not start
            // together.
            let teeth = f32::from(gear_teeth(index).expect("bounded wheel index"));
            let (eccentric_rotation_sine, eccentric_rotation_cosine) =
                sin_cos(TAU * frequency / (teeth * sample_rate));
            // The series in `sin_cos` is only good near zero, so the starting
            // angle stays inside a half turn either way, like the tone phase.
            let (eccentric_sine, eccentric_cosine) = sin_cos(
                -PI + TAU * ((index * 53 % TONEWHEEL_COUNT) as f32) / TONEWHEEL_COUNT as f32,
            );
            *wheel = Tonewheel {
                sine,
                cosine,
                rotation_sine,
                rotation_cosine,
                eccentric_sine,
                eccentric_cosine,
                eccentric_rotation_sine,
                eccentric_rotation_cosine,
                frequency,
                level: 1.0,
            };
        }
        Self {
            wheels,
            samples: [0.0; TONEWHEEL_COUNT],
            sample_rate,
            eccentricity: ECCENTRICITY_DEPTH,
        }
    }

    /// Depth of the once-per-revolution level change, shared by every wheel
    /// until per-wheel measurements exist.
    pub fn set_eccentricity(&mut self, depth: f32) -> bool {
        if !depth.is_finite() || !(0.0..=0.5).contains(&depth) {
            return false;
        }
        self.eccentricity = depth;
        true
    }

    /// Output level of one wheel, as a generator's own taper would set it.
    pub fn set_level(&mut self, index: usize, level: f32) -> bool {
        if !level.is_finite() || !(0.0..=4.0).contains(&level) {
            return false;
        }
        match self.wheels.get_mut(index) {
            Some(wheel) => {
                wheel.level = level;
                true
            }
            None => false,
        }
    }

    pub fn level(&self, index: usize) -> Option<f32> {
        self.wheels.get(index).map(|wheel| wheel.level)
    }

    pub const fn eccentricity(&self) -> f32 {
        self.eccentricity
    }

    pub fn tick(&mut self) {
        for (index, (wheel, sample)) in self
            .wheels
            .iter_mut()
            .zip(self.samples.iter_mut())
            .enumerate()
        {
            *sample = wheel.tick(self.sample_rate, index, self.eccentricity);
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

    /// A wheel turns once per tooth-count cycles of its tone, so the
    /// eccentricity of the lowest wheel beats at about 16 Hz and that of the
    /// highest at about 31 Hz.
    #[test]
    fn revolution_rates_follow_the_tooth_counts() {
        let revolutions = |index: usize| {
            gear_frequency(index).expect("wheel") / f32::from(gear_teeth(index).expect("teeth"))
        };
        assert_eq!(gear_teeth(0), Some(2));
        assert_eq!(gear_teeth(83), Some(128));
        assert_eq!(gear_teeth(90), Some(192));
        assert!((revolutions(0) - 16.35).abs() < 0.1, "{}", revolutions(0));
        assert!((revolutions(90) - 30.9).abs() < 0.2, "{}", revolutions(90));
        assert_eq!(gear_teeth(TONEWHEEL_COUNT), None);
    }

    #[test]
    fn eccentricity_modulates_level_once_per_revolution() {
        let mut bank = TonewheelBank::new(48_000.0);
        assert!(bank.set_eccentricity(0.25));
        for index in 0..TONEWHEEL_COUNT {
            assert!(bank.set_level(index, 0.0));
        }
        assert!(bank.set_level(0, 1.0));
        let revolutions = gear_frequency(0).expect("wheel") / 2.0;
        let frames = (48_000.0 / revolutions) as usize;
        let mut peak: f32 = 0.0;
        let mut trough = f32::MAX;
        let mut envelope: f32 = 0.0;
        for frame in 0..frames * 3 {
            bank.tick();
            envelope = envelope.max(bank.samples()[0].abs());
            if frame % 64 == 63 {
                if frame > frames {
                    peak = peak.max(envelope);
                    trough = trough.min(envelope);
                }
                envelope = 0.0;
            }
        }
        // A quarter of modulation depth has to show up as a level swing.
        assert!(peak > trough * 1.2, "peak {peak} against trough {trough}");
        assert!(!bank.set_eccentricity(0.9));
        assert!(!bank.set_level(TONEWHEEL_COUNT, 1.0));
        assert_eq!(bank.level(0), Some(1.0));
    }

    #[test]
    fn table_is_strictly_ascending() {
        for index in 1..TONEWHEEL_COUNT {
            assert!(gear_frequency(index) > gear_frequency(index - 1));
        }
    }
}
