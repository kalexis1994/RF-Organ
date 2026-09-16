// SPDX-License-Identifier: GPL-2.0-or-later

use core::f32::consts::TAU;

/// Reduced console preamplifier and swell-pedal model. The topology is kept
/// separate from the generator and rotary cabinet so measured coefficients can
/// replace these provisional values without changing the instrument contract.
pub struct ConsoleElectronics {
    sample_rate: f32,
    drive: f32,
    bass_trim: f32,
    treble_trim: f32,
    expression_character: f32,
    expression_state: f32,
    bass_state: f32,
    treble_state: f32,
    dc_input: f32,
    dc_output: f32,
}

impl ConsoleElectronics {
    pub const fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            drive: 0.32,
            bass_trim: 0.0,
            treble_trim: 0.0,
            expression_character: 0.55,
            expression_state: 1.0,
            bass_state: 0.0,
            treble_state: 0.0,
            dc_input: 0.0,
            dc_output: 0.0,
        }
    }

    pub fn set(&mut self, drive: f32, bass: f32, treble: f32) -> bool {
        if !unit(drive) || !bipolar(bass) || !bipolar(treble) {
            return false;
        }
        self.drive = drive;
        self.bass_trim = bass;
        self.treble_trim = treble;
        true
    }

    pub fn set_expression_character(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        self.expression_character = value;
        true
    }

    pub fn process(&mut self, input: f32, expression: f32) -> f32 {
        let expression_rate = 1.0 / (0.008 * self.sample_rate).max(1.0);
        self.expression_state += expression_rate * (expression - self.expression_state);
        let expression_gain = self.expression_state
            * (1.0 - self.expression_character * (1.0 - self.expression_state));

        let bass_coefficient = one_pole(310.0, self.sample_rate);
        let treble_coefficient = one_pole(2_450.0, self.sample_rate);
        self.bass_state += bass_coefficient * (input - self.bass_state);
        self.treble_state += treble_coefficient * (input - self.treble_state);
        let high = input - self.treble_state;
        let bass_gain = trim_gain(self.bass_trim);
        let treble_gain = trim_gain(self.treble_trim);
        let equalized = input + (bass_gain - 1.0) * self.bass_state + (treble_gain - 1.0) * high;

        let driven = equalized * (1.0 + 7.0 * self.drive);
        let bias = 0.08 * self.drive;
        let biased = driven + bias;
        let saturated = biased / (1.0 + biased.abs()) - bias / (1.0 + bias.abs());
        let makeup = 1.0 / (1.0 + 3.2 * self.drive);
        let amplified = (equalized * (1.0 - self.drive) + saturated * self.drive) * makeup;

        // Coupling capacitors remove the small asymmetric-stage bias.
        let dc_coefficient = 1.0 - one_pole(18.0, self.sample_rate);
        let high_pass = dc_coefficient * (self.dc_output + amplified - self.dc_input);
        self.dc_input = amplified;
        self.dc_output = high_pass;

        // The provisional swell network becomes slightly darker toward its
        // heel position, while remaining exactly silent at expression zero.
        high_pass * expression_gain * (0.78 + 0.22 * self.expression_state)
    }

    pub fn reset(&mut self, expression: f32) {
        self.expression_state = expression;
        self.bass_state = 0.0;
        self.treble_state = 0.0;
        self.dc_input = 0.0;
        self.dc_output = 0.0;
    }
}

fn one_pole(frequency: f32, sample_rate: f32) -> f32 {
    let normalized = TAU * frequency / sample_rate;
    normalized / (1.0 + normalized)
}

fn trim_gain(value: f32) -> f32 {
    if value >= 0.0 {
        1.0 + 3.0 * value
    } else {
        1.0 / (1.0 - 3.0 * value)
    }
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn bipolar(value: f32) -> bool {
    value.is_finite() && (-1.0..=1.0).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heel_position_reaches_silence_after_the_physical_slew() {
        let mut electronics = ConsoleElectronics::new(48_000.0);
        let mut maximum: f32 = 0.0;
        for index in 0..48_000 {
            let input = if index & 1 == 0 { 0.5 } else { -0.5 };
            let output = electronics.process(input, 0.0);
            if index > 47_000 {
                maximum = maximum.max(output.abs());
            }
        }
        assert!(maximum < 1.0e-6);
    }

    #[test]
    fn console_controls_reject_non_physical_ranges() {
        let mut electronics = ConsoleElectronics::new(48_000.0);
        assert!(electronics.set(0.5, -1.0, 1.0));
        assert!(!electronics.set(1.1, 0.0, 0.0));
        assert!(!electronics.set(0.5, -1.1, 0.0));
        assert!(!electronics.set_expression_character(f32::NAN));
    }
}
