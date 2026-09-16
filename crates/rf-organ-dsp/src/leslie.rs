// SPDX-License-Identifier: GPL-2.0-or-later
use core::f32::consts::TAU;

const DELAY_CAPACITY: usize = 8192;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum LeslieMode {
    #[default]
    Off = 0,
    Brake = 1,
    Chorale = 2,
    Tremolo = 3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeslieDiagnostics {
    pub horn_speed_hz: f32,
    pub horn_target_hz: f32,
    pub drum_speed_hz: f32,
    pub drum_target_hz: f32,
}

impl LeslieMode {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Off),
            1 => Some(Self::Brake),
            2 => Some(Self::Chorale),
            3 => Some(Self::Tremolo),
            _ => None,
        }
    }
}

struct DelayLine {
    data: [f32; DELAY_CAPACITY],
    write: usize,
}

impl DelayLine {
    const fn new() -> Self {
        Self {
            data: [0.0; DELAY_CAPACITY],
            write: 0,
        }
    }

    fn push(&mut self, value: f32) {
        self.data[self.write] = value;
        self.write = (self.write + 1) % DELAY_CAPACITY;
    }

    fn read(&self, delay: f32) -> f32 {
        let delay = delay.clamp(1.0, (DELAY_CAPACITY - 2) as f32);
        let whole = delay as usize;
        let fraction = delay - whole as f32;
        let newer = (self.write + DELAY_CAPACITY - whole - 1) % DELAY_CAPACITY;
        let older = (newer + DELAY_CAPACITY - 1) % DELAY_CAPACITY;
        self.data[newer] * (1.0 - fraction) + self.data[older] * fraction
    }

    fn clear(&mut self) {
        self.data.fill(0.0);
        self.write = 0;
    }
}

#[derive(Clone, Copy)]
struct Rotor {
    sine: f32,
    cosine: f32,
    speed_hz: f32,
    target_hz: f32,
    acceleration_seconds: f32,
    direction: f32,
}

impl Rotor {
    const fn new(direction: f32) -> Self {
        Self {
            sine: 0.0,
            cosine: 1.0,
            speed_hz: 0.0,
            target_hz: 0.0,
            acceleration_seconds: 1.0,
            direction,
        }
    }

    fn advance(&mut self, sample_rate: f32) {
        let smoothing = 1.0 / (self.acceleration_seconds * sample_rate).max(1.0);
        self.speed_hz += smoothing * (self.target_hz - self.speed_hz);
        let angle = self.direction * TAU * self.speed_hz / sample_rate;
        let (rotation_sine, rotation_cosine) = small_rotation(angle);
        let sine = self.sine * rotation_cosine + self.cosine * rotation_sine;
        let cosine = self.cosine * rotation_cosine - self.sine * rotation_sine;
        let correction = 1.5 - 0.5 * (sine * sine + cosine * cosine);
        self.sine = sine * correction;
        self.cosine = cosine * correction;
    }
}

pub struct Leslie {
    sample_rate: f32,
    mode: LeslieMode,
    mix: f32,
    crossover: f32,
    low_state: f32,
    horn: Rotor,
    drum: Rotor,
    horn_delay: DelayLine,
    drum_delay: DelayLine,
    cabinet_left: DelayLine,
    cabinet_right: DelayLine,
    horn_tone_left: f32,
    horn_tone_right: f32,
    mic_distance: f32,
    stereo_width: f32,
    reflections: f32,
    horn_drum_balance: f32,
}

impl Leslie {
    pub fn new(sample_rate: f32) -> Self {
        let x = TAU * 800.0 / sample_rate;
        Self {
            sample_rate,
            mode: LeslieMode::Off,
            mix: 0.82,
            crossover: x / (1.0 + x),
            low_state: 0.0,
            horn: Rotor::new(1.0),
            drum: Rotor::new(-1.0),
            horn_delay: DelayLine::new(),
            drum_delay: DelayLine::new(),
            cabinet_left: DelayLine::new(),
            cabinet_right: DelayLine::new(),
            horn_tone_left: 0.0,
            horn_tone_right: 0.0,
            mic_distance: 0.35,
            stereo_width: 0.75,
            reflections: 0.22,
            horn_drum_balance: 0.0,
        }
    }

    pub fn set_mode(&mut self, mode: LeslieMode) {
        self.mode = mode;
        let (horn, drum) = match mode {
            LeslieMode::Off | LeslieMode::Brake => (0.0, 0.0),
            LeslieMode::Chorale => (0.8, 0.7),
            LeslieMode::Tremolo => (6.8, 5.6),
        };
        self.horn.target_hz = horn;
        self.drum.target_hz = drum;
    }

    pub fn set_mix(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        self.mix = value;
        true
    }

    pub fn set_acceleration(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        let scale = 0.35 + 1.8 * value;
        self.horn.acceleration_seconds = 0.35 * scale;
        self.drum.acceleration_seconds = 0.9 * scale;
        true
    }

    pub fn set_cabinet(
        &mut self,
        mic_distance: f32,
        stereo_width: f32,
        reflections: f32,
        horn_drum_balance: f32,
    ) -> bool {
        if !unit(mic_distance)
            || !unit(stereo_width)
            || !unit(reflections)
            || !bipolar(horn_drum_balance)
        {
            return false;
        }
        self.mic_distance = mic_distance;
        self.stereo_width = stereo_width;
        self.reflections = reflections;
        self.horn_drum_balance = horn_drum_balance;
        true
    }

    pub fn process(&mut self, input: f32) -> [f32; 2] {
        self.low_state += self.crossover * (input - self.low_state);
        let low = self.low_state;
        let high = input - low;
        self.horn_delay.push(high);
        self.drum_delay.push(low);

        self.horn.advance(self.sample_rate);
        self.drum.advance(self.sample_rate);

        let horn_base = self.sample_rate * (0.00075 + 0.0015 * self.mic_distance);
        let horn_depth = self.sample_rate * 0.00042 * (1.0 - 0.55 * self.mic_distance);
        let drum_base = self.sample_rate * (0.0011 + 0.0020 * self.mic_distance);
        let drum_depth = self.sample_rate * 0.00017 * (1.0 - 0.5 * self.mic_distance);
        let raw_horn_left = self
            .horn_delay
            .read(horn_base + horn_depth * self.horn.sine);
        let raw_horn_right = self
            .horn_delay
            .read(horn_base - horn_depth * self.horn.sine);
        let drum_left = self
            .drum_delay
            .read(drum_base + drum_depth * self.drum.sine);
        let drum_right = self
            .drum_delay
            .read(drum_base - drum_depth * self.drum.sine);

        let horn_filter = one_pole(2_200.0, self.sample_rate);
        self.horn_tone_left += horn_filter * (raw_horn_left - self.horn_tone_left);
        self.horn_tone_right += horn_filter * (raw_horn_right - self.horn_tone_right);
        let horn_high_left = raw_horn_left - self.horn_tone_left;
        let horn_high_right = raw_horn_right - self.horn_tone_right;
        let horn_facing_left = 0.5 + 0.5 * self.horn.cosine;
        let horn_facing_right = 1.0 - horn_facing_left;
        let horn_left = self.horn_tone_left * (0.55 + 0.35 * horn_facing_left)
            + horn_high_left * (0.20 + 0.80 * horn_facing_left);
        let horn_right = self.horn_tone_right * (0.55 + 0.35 * horn_facing_right)
            + horn_high_right * (0.20 + 0.80 * horn_facing_right);

        let drum_facing_left = 0.5 + 0.5 * self.drum.cosine;
        let drum_facing_right = 1.0 - drum_facing_left;
        let horn_gain = if self.horn_drum_balance < 0.0 {
            1.0 + self.horn_drum_balance
        } else {
            1.0
        };
        let drum_gain = if self.horn_drum_balance > 0.0 {
            1.0 - self.horn_drum_balance
        } else {
            1.0
        };
        let mut wet_left =
            horn_gain * horn_left + drum_gain * drum_left * (0.66 + 0.34 * drum_facing_left);
        let mut wet_right =
            horn_gain * horn_right + drum_gain * drum_right * (0.66 + 0.34 * drum_facing_right);

        let mid = 0.5 * (wet_left + wet_right);
        let side = 0.5 * (wet_left - wet_right) * (0.35 + 1.3 * self.stereo_width);
        wet_left = mid + side;
        wet_right = mid - side;

        self.cabinet_left.push(wet_left);
        self.cabinet_right.push(wet_right);
        let reflection_left = 0.42 * self.cabinet_left.read(self.sample_rate * 0.0037)
            + 0.28 * self.cabinet_right.read(self.sample_rate * 0.0063)
            - 0.18 * self.cabinet_left.read(self.sample_rate * 0.0109);
        let reflection_right = 0.42 * self.cabinet_right.read(self.sample_rate * 0.0041)
            + 0.28 * self.cabinet_left.read(self.sample_rate * 0.0069)
            - 0.18 * self.cabinet_right.read(self.sample_rate * 0.0117);
        wet_left += 0.65 * self.reflections * reflection_left;
        wet_right += 0.65 * self.reflections * reflection_right;
        let wet = if self.mode == LeslieMode::Off {
            0.0
        } else {
            self.mix
        };
        [
            input * (1.0 - wet) + wet_left * wet,
            input * (1.0 - wet) + wet_right * wet,
        ]
    }

    /// Read-only mechanical state for deterministic calibration tools.
    pub const fn diagnostics(&self) -> LeslieDiagnostics {
        LeslieDiagnostics {
            horn_speed_hz: self.horn.speed_hz,
            horn_target_hz: self.horn.target_hz,
            drum_speed_hz: self.drum.speed_hz,
            drum_target_hz: self.drum.target_hz,
        }
    }

    pub fn reset(&mut self) {
        self.low_state = 0.0;
        self.horn_delay.clear();
        self.drum_delay.clear();
        self.cabinet_left.clear();
        self.cabinet_right.clear();
        self.horn_tone_left = 0.0;
        self.horn_tone_right = 0.0;
    }
}

fn small_rotation(angle: f32) -> (f32, f32) {
    let squared = angle * angle;
    (
        angle * (1.0 - squared / 6.0),
        1.0 - squared / 2.0 * (1.0 - squared / 12.0),
    )
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn bipolar(value: f32) -> bool {
    value.is_finite() && (-1.0..=1.0).contains(&value)
}

fn one_pole(frequency: f32, sample_rate: f32) -> f32 {
    let normalized = TAU * frequency / sample_rate;
    normalized / (1.0 + normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_is_an_exact_bypass_while_rotors_keep_state() {
        let mut leslie = Leslie::new(48_000.0);
        for index in 0..1024 {
            let input = index as f32 / 1024.0 - 0.5;
            assert_eq!(leslie.process(input), [input, input]);
        }
    }

    #[test]
    fn cabinet_reflections_create_a_distinct_tail() {
        let mut dry_cabinet = Leslie::new(48_000.0);
        let mut live_cabinet = Leslie::new(48_000.0);
        dry_cabinet.set_mode(LeslieMode::Brake);
        live_cabinet.set_mode(LeslieMode::Brake);
        assert!(dry_cabinet.set_mix(1.0));
        assert!(live_cabinet.set_mix(1.0));
        assert!(dry_cabinet.set_cabinet(0.35, 0.75, 0.0, 0.0));
        assert!(live_cabinet.set_cabinet(0.35, 0.75, 1.0, 0.0));
        let mut difference = 0.0;
        for index in 0..2048 {
            let input = if index == 0 { 1.0 } else { 0.0 };
            let dry = dry_cabinet.process(input);
            let live = live_cabinet.process(input);
            if index > 256 {
                difference += (dry[0] - live[0]).abs() + (dry[1] - live[1]).abs();
            }
        }
        assert!(difference > 0.01);
    }

    #[test]
    fn diagnostics_report_rotor_targets_and_motion() {
        let mut leslie = Leslie::new(48_000.0);
        leslie.set_mode(LeslieMode::Tremolo);
        let initial = leslie.diagnostics();
        assert_eq!(initial.horn_target_hz, 6.8);
        assert_eq!(initial.drum_target_hz, 5.6);
        assert_eq!(initial.horn_speed_hz, 0.0);
        for _ in 0..48_000 {
            leslie.process(0.0);
        }
        let moving = leslie.diagnostics();
        assert!(moving.horn_speed_hz > moving.drum_speed_hz);
        assert!(moving.horn_speed_hz < moving.horn_target_hz);
    }
}
