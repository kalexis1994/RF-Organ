// SPDX-License-Identifier: GPL-2.0-or-later

pub struct MatchingTransformer {
    sample_rate: f32,
    drive: f32,
    hysteresis: f32,
    magnetization: f32,
}

impl MatchingTransformer {
    pub const fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            drive: 0.35,
            hysteresis: 0.25,
            magnetization: 0.0,
        }
    }

    pub fn set(&mut self, drive: f32, hysteresis: f32) -> bool {
        if !unit(drive) || !unit(hysteresis) {
            return false;
        }
        self.drive = drive;
        self.hysteresis = hysteresis;
        true
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // Provisional reduced model: a slow magnetic state biases a smooth,
        // asymmetric saturation. It is shared by all notes and therefore also
        // produces the expected inter-note interaction.
        let memory_rate = (900.0 / self.sample_rate).clamp(0.001, 0.1);
        self.magnetization += memory_rate * (input - self.magnetization);
        let driven =
            input * (1.0 + 5.0 * self.drive) + self.magnetization * (0.8 * self.hysteresis);
        let positive = 1.0 + driven.abs() * (0.55 + 0.35 * self.hysteresis);
        let saturated = driven / positive;
        let clean_gain = 1.0 / (1.0 + 5.0 * self.drive);
        input * (1.0 - self.drive) + saturated * clean_gain * self.drive
    }

    pub fn reset(&mut self) {
        self.magnetization = 0.0;
    }
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
