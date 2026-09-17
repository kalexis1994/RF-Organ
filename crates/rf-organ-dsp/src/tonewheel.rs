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

/// The speed of the one shaft everything hangs off.
///
/// Hammond gives the synchronous run motor a 2-pole field and a 6-pole
/// armature turning at 1200 rpm on sixty cycles, and a 4-pole armature at
/// 1500 rpm on fifty. The wheels are geared to it so that the organ plays at
/// pitch either way, which is why the supply moves what follows and not the
/// tuning.
const SHAFT_RPM_SIXTY: f32 = 1200.0;
const SHAFT_RPM_FIFTY: f32 = 1500.0;

/// How steady that shaft is not.
///
/// The service manual describes the drive as resilient at every joint: the
/// shaft is "resiliently coupled to the synchronous running motor", it "is
/// divided into several sections connected by flexible couplings", and each
/// wheel assembly is "coupled resiliently to the drive shaft" by a bakelite
/// gear that "rotate[s] freely on the shafts with the tone wheels" and is held
/// to its assembly "by a pair of coil springs". A drive built that way cannot
/// turn perfectly evenly, and because the 91 wheels hang off that one shaft,
/// what it does it does to all of them together.
///
/// Two motions are modelled. The motor's own coupling lets the shaft swing
/// slowly about the position the supply holds it to, which is wow; and the
/// shaft turns once per revolution past gears that are not perfect, which is
/// flutter at the speed above. How far it strays in each, and at what rate it
/// swings, are not numbers Hammond publishes, so they are provisional and the
/// laboratory reports the pitch deviation they come to.
const WOW_RATE_HZ: f32 = 2.7;
const WOW_DEPTH: f32 = 6.0e-4;
const FLUTTER_DEPTH: f32 = 4.0e-4;

/// The coil springs between each bakelite gear and its assembly, as one
/// resonance. Every assembly is built the same way, so they are modelled as
/// one filter rather than 48 identical ones; what the springs do to the shaft
/// before it reaches the wheels is a low pass with a rise at its corner, and
/// where that corner sits is provisional.
const COUPLING_HZ: f32 = 34.0;
/// Samples between readings of the drive's speed.
const RETIME_INTERVAL: u32 = 64;
const COUPLING_DAMPING: f32 = 0.6;

/// The drive shaft, and the springs between it and the wheels.
#[derive(Clone, Copy)]
struct Driveshaft {
    wow: (f32, f32),
    wow_step: (f32, f32),
    flutter: (f32, f32),
    flutter_step: (f32, f32),
    /// State-variable resonator standing for the coil-spring coupling.
    low: f32,
    band: f32,
    coupling_f: f32,
    depth: f32,
}

impl Driveshaft {
    fn new(sample_rate: f32) -> Self {
        let mut shaft = Self {
            wow: (0.0, 1.0),
            wow_step: (0.0, 1.0),
            flutter: (0.0, 1.0),
            flutter_step: (0.0, 1.0),
            low: 0.0,
            band: 0.0,
            coupling_f: 2.0 * (PI * COUPLING_HZ / sample_rate).min(0.4),
            depth: 1.0,
        };
        shaft.retune(sample_rate, SHAFT_RPM_SIXTY);
        shaft
    }

    fn retune(&mut self, sample_rate: f32, shaft_rpm: f32) {
        self.wow_step = sin_cos_pair(TAU * WOW_RATE_HZ / sample_rate);
        self.flutter_step = sin_cos_pair(TAU * (shaft_rpm / 60.0) / sample_rate);
    }

    /// How much faster or slower than nominal the wheels are turning now, as
    /// a fraction of their speed.
    fn tick(&mut self) -> f32 {
        advance(&mut self.wow, self.wow_step);
        advance(&mut self.flutter, self.flutter_step);
        let drive = WOW_DEPTH * self.wow.0 + FLUTTER_DEPTH * self.flutter.0;
        // One state-variable step: the springs pass the slow part and ring a
        // little where they resonate.
        self.low += self.coupling_f * self.band;
        let high = drive - self.low - COUPLING_DAMPING * self.band;
        self.band += self.coupling_f * high;
        self.depth * self.low
    }
}

/// Advances a unit phasor by a step, keeping it on the circle.
fn advance(phasor: &mut (f32, f32), step: (f32, f32)) {
    let sine = phasor.0 * step.1 + phasor.1 * step.0;
    let cosine = phasor.1 * step.1 - phasor.0 * step.0;
    let correction = 1.5 - 0.5 * (sine * sine + cosine * cosine);
    *phasor = (sine * correction, cosine * correction);
}

fn sin_cos_pair(angle: f32) -> (f32, f32) {
    sin_cos(angle)
}

#[derive(Clone, Copy)]
struct Tonewheel {
    sine: f32,
    cosine: f32,
    rotation_sine: f32,
    rotation_cosine: f32,
    /// The same pair at nominal speed, which the shaft's unsteadiness is
    /// applied to rather than accumulated into.
    base_rotation_sine: f32,
    base_rotation_cosine: f32,
    /// Once-per-revolution phasor, for wheel eccentricity.
    eccentric_sine: f32,
    eccentric_cosine: f32,
    eccentric_rotation_sine: f32,
    eccentric_rotation_cosine: f32,
    frequency: f32,
    /// Radians the wheel turns per sample at nominal speed, which is what the
    /// shaft's unsteadiness is a fraction of.
    increment: f32,
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
        base_rotation_sine: 0.0,
        base_rotation_cosine: 1.0,
        eccentric_sine: 0.0,
        eccentric_cosine: 1.0,
        eccentric_rotation_sine: 0.0,
        eccentric_rotation_cosine: 1.0,
        frequency: 0.0,
        increment: 0.0,
        level: 1.0,
    };

    /// Sets how far this wheel turns per sample, given how fast the shaft is
    /// going just now as a fraction of nominal. Whatever the shaft is doing,
    /// every wheel does a proportional amount of it, because they are all
    /// geared to that one shaft; and the difference is far too small to need
    /// a sine of its own.
    fn retime(&mut self, wobble: f32) {
        let extra = self.increment * wobble;
        let sine = self.base_rotation_sine + self.base_rotation_cosine * extra;
        let cosine = self.base_rotation_cosine - self.base_rotation_sine * extra;
        let correction = 1.5 - 0.5 * (sine * sine + cosine * cosine);
        self.rotation_sine = sine * correction;
        self.rotation_cosine = cosine * correction;
    }

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
    shaft: Driveshaft,
    since_retime: u32,
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
                base_rotation_sine: rotation_sine,
                base_rotation_cosine: rotation_cosine,
                eccentric_sine,
                eccentric_cosine,
                eccentric_rotation_sine,
                eccentric_rotation_cosine,
                frequency,
                increment,
                // Whatever the generator's own filters leave of this wheel.
                level: crate::taper::TAPER[index],
            };
        }
        Self {
            wheels,
            samples: [0.0; TONEWHEEL_COUNT],
            sample_rate,
            eccentricity: ECCENTRICITY_DEPTH,
            shaft: Driveshaft::new(sample_rate),
            since_retime: 0,
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
        // The drive is retimed on a slower clock than the wheels turn on. The
        // fastest thing it does is once per shaft revolution, twenty-five
        // times a second at most, so reading it several hundred times a second
        // loses nothing and keeps 91 wheels from doing the arithmetic 48,000
        // times each.
        let wobble = self.shaft.tick();
        if self.since_retime == 0 {
            for wheel in &mut self.wheels {
                wheel.retime(wobble);
            }
        }
        self.since_retime = (self.since_retime + 1) % RETIME_INTERVAL;

        for (index, (wheel, sample)) in self
            .wheels
            .iter_mut()
            .zip(self.samples.iter_mut())
            .enumerate()
        {
            *sample = wheel.tick(self.sample_rate, index, self.eccentricity);
        }
    }

    /// How far the drive is allowed to stray from its nominal speed, from a
    /// perfectly steady shaft at zero to the whole of what is modelled at one.
    pub fn set_drive_wobble(&mut self, depth: f32) -> bool {
        if !depth.is_finite() || !(0.0..=1.0).contains(&depth) {
            return false;
        }
        self.shaft.depth = depth;
        true
    }

    /// Which supply the run motor is on. The wheels are geared to it so that
    /// the organ plays at pitch either way, so this moves the speed the shaft
    /// turns at and nothing else.
    pub fn set_shaft_rpm_for_fifty(&mut self, fifty: bool) {
        let rpm = if fifty {
            SHAFT_RPM_FIFTY
        } else {
            SHAFT_RPM_SIXTY
        };
        self.shaft.retune(self.sample_rate, rpm);
    }

    /// Where the drive is now, as a fraction of nominal speed, for the
    /// laboratory.
    pub const fn drive_deviation(&self) -> f32 {
        self.shaft.depth * self.shaft.low
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

    /// What the resilient drive comes to, in cents, and that switching it off
    /// leaves a shaft that does not stray at all.
    #[test]
    fn the_drive_strays_by_a_documented_structure_and_an_undocumented_amount() {
        let mut bank = TonewheelBank::new(48_000.0);
        let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
        for _ in 0..48_000 * 3 {
            bank.tick();
            let deviation = bank.drive_deviation();
            lowest = lowest.min(deviation);
            highest = highest.max(deviation);
        }
        let cents = |ratio: f32| 1200.0 * log2_approx(1.0 + ratio);
        let swing = cents(highest) - cents(lowest);
        assert!(
            (0.5..8.0).contains(&swing),
            "the drive swings {swing} cents, which is not a generator"
        );

        assert!(bank.set_drive_wobble(0.0));
        let mut steady = TonewheelBank::new(48_000.0);
        assert!(steady.set_drive_wobble(0.0));
        for _ in 0..4_800 {
            steady.tick();
            assert_eq!(steady.drive_deviation(), 0.0);
        }
        assert!(!bank.set_drive_wobble(1.2));
        assert!(!bank.set_drive_wobble(-0.1));
    }

    /// The supply moves how fast the shaft turns and leaves the tuning where
    /// it is, because Hammond geared the two markets to agree on pitch.
    #[test]
    fn the_supply_moves_the_shaft_and_not_the_pitch() {
        let mut sixty = TonewheelBank::new(48_000.0);
        let mut fifty = TonewheelBank::new(48_000.0);
        fifty.set_shaft_rpm_for_fifty(true);
        // Count how often each drive crosses zero: the faster shaft strays
        // back and forth more often in the same time.
        let crossings = |bank: &mut TonewheelBank| {
            let mut previous = 0.0_f32;
            let mut count = 0_u32;
            for _ in 0..48_000 * 4 {
                bank.tick();
                let now = bank.drive_deviation();
                if previous <= 0.0 && now > 0.0 {
                    count += 1;
                }
                previous = now;
            }
            count
        };
        let slow = crossings(&mut sixty);
        let fast = crossings(&mut fifty);
        assert!(fast > slow, "fifty cycles gave {fast} against {slow}");
    }

    /// Base-two logarithm, near one, for reporting cents in a crate without
    /// one.
    fn log2_approx(value: f32) -> f32 {
        let x = (value - 1.0) / (value + 1.0);
        let squared = x * x;
        2.0 * x * (1.0 + squared / 3.0 + squared * squared / 5.0) / core::f32::consts::LN_2
    }

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
