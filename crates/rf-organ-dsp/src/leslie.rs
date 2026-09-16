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

    pub fn process(&mut self, input: f32) -> [f32; 2] {
        self.low_state += self.crossover * (input - self.low_state);
        let low = self.low_state;
        let high = input - low;
        self.horn_delay.push(high);
        self.drum_delay.push(low);

        self.horn.advance(self.sample_rate);
        self.drum.advance(self.sample_rate);

        let horn_base = self.sample_rate * 0.0014;
        let horn_depth = self.sample_rate * 0.00032;
        let drum_base = self.sample_rate * 0.0020;
        let drum_depth = self.sample_rate * 0.00011;
        let horn_left = self
            .horn_delay
            .read(horn_base + horn_depth * self.horn.sine);
        let horn_right = self
            .horn_delay
            .read(horn_base - horn_depth * self.horn.sine);
        let drum_left = self
            .drum_delay
            .read(drum_base + drum_depth * self.drum.sine);
        let drum_right = self
            .drum_delay
            .read(drum_base - drum_depth * self.drum.sine);

        let horn_l_gain = 0.58 + 0.42 * self.horn.cosine;
        let horn_r_gain = 0.58 - 0.42 * self.horn.cosine;
        let drum_l_gain = 0.72 + 0.28 * self.drum.cosine;
        let drum_r_gain = 0.72 - 0.28 * self.drum.cosine;
        let wet_left = horn_left * horn_l_gain + drum_left * drum_l_gain;
        let wet_right = horn_right * horn_r_gain + drum_right * drum_r_gain;
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

    pub fn reset(&mut self) {
        self.low_state = 0.0;
        self.horn_delay.clear();
        self.drum_delay.clear();
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
