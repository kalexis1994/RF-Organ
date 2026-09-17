// SPDX-License-Identifier: GPL-2.0-or-later

use core::f32::consts::TAU;

const EXPRESSION_SECTION_CAPACITANCE_PF: f32 = 60.0;
const EXPRESSION_GRID_RESISTANCE_OHM: f32 = 15_000_000.0;
const EXPRESSION_LOW_HZ: f32 =
    1.0 / (TAU * EXPRESSION_GRID_RESISTANCE_OHM * EXPRESSION_SECTION_CAPACITANCE_PF * 1.0e-12);
const EXPRESSION_HIGH_HZ: f32 = 4_000.0;
const TONE_CONTROL_HZ: f32 = 200.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConsoleElectronicsDiagnostics {
    pub expression: f32,
    pub section_capacitance_pf: f32,
    pub low_corner_hz: f32,
    pub tone_control: f32,
    pub tone_corner_hz: f32,
}

/// Reduced console preamplifier and swell-pedal model. The topology is kept
/// separate from the generator and rotary cabinet so measured coefficients can
/// replace these provisional values without changing the instrument contract.
pub struct ConsoleElectronics {
    sample_rate: f32,
    drive: f32,
    bass_trim: f32,
    tone_control: f32,
    expression_character: f32,
    expression_state: f32,
    expression_low_state: f32,
    expression_high_state: f32,
    bass_state: f32,
    tone_state: f32,
    dc_input: f32,
    dc_output: f32,
}

impl ConsoleElectronics {
    pub const fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            drive: 0.32,
            bass_trim: 0.0,
            tone_control: 0.0,
            expression_character: 0.55,
            expression_state: 1.0,
            expression_low_state: 0.0,
            expression_high_state: 0.0,
            bass_state: 0.0,
            tone_state: 0.0,
            dc_input: 0.0,
            dc_output: 0.0,
        }
    }

    pub fn set(&mut self, drive: f32, bass: f32, tone: f32) -> bool {
        if !unit(drive) || !bipolar(bass) || !bipolar(tone) {
            return false;
        }
        self.drive = drive;
        self.bass_trim = bass;
        self.tone_control = tone;
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

        let bass_coefficient = one_pole(310.0, self.sample_rate);
        self.bass_state += bass_coefficient * (input - self.bass_state);
        let bass_gain = trim_gain(self.bass_trim);
        let equalized = input + (bass_gain - 1.0) * self.bass_state;

        // V4A drives the passive swell network. Keeping this stage before the
        // pedal lets expression alter how strongly V4B and the output stage
        // are driven, instead of applying a final digital volume multiplier.
        let pre_expression = tube_stage(equalized, self.drive * 0.45, 0.08);

        // Reduced three-band form of the capacitive expression network. The
        // two expression-control sections use the documented 60 pF/section
        // value and R34 (15 MOhm) sets a provisional low-band corner. The
        // transfer between the moving and fixed plates, and the C17/C22/C24
        // high shoulder, remain calibration targets.
        let low_coefficient = one_pole(EXPRESSION_LOW_HZ, self.sample_rate);
        let high_coefficient = one_pole(EXPRESSION_HIGH_HZ, self.sample_rate);
        self.expression_low_state += low_coefficient * (pre_expression - self.expression_low_state);
        self.expression_high_state +=
            high_coefficient * (pre_expression - self.expression_high_state);
        let low = self.expression_low_state;
        let high = pre_expression - self.expression_high_state;
        let mid = pre_expression - low - high;

        let linear_gain = self.expression_state;
        let mid_gain =
            linear_gain * (1.0 - self.expression_character * (1.0 - self.expression_state));
        let compensation = linear_gain - mid_gain;
        let low_gain = mid_gain + 0.72 * compensation;
        let high_gain = mid_gain + 0.48 * compensation;
        let expressed = low * low_gain + mid * mid_gain + high * high_gain;

        // V4B follows the passive expression network. The AO-28 tone control
        // then applies a broad shelf above roughly 200 Hz before V3B. The
        // original control only cut; the RackForge calibration parameter also
        // permits the documented modern +9 dB extension around its neutral.
        let post_expression = tube_stage(expressed, self.drive * 0.35, -0.05);
        let tone_coefficient = one_pole(TONE_CONTROL_HZ, self.sample_rate);
        self.tone_state += tone_coefficient * (post_expression - self.tone_state);
        let tone_high = post_expression - self.tone_state;
        let toned = post_expression + (tone_gain(self.tone_control) - 1.0) * tone_high;

        // V3B/12BH7 is the final active stage before output transformer T3.
        let amplified = tube_stage(toned, self.drive * 0.20, 0.03);

        // Coupling capacitors remove the small asymmetric-stage bias.
        let dc_coefficient = 1.0 - one_pole(18.0, self.sample_rate);
        let high_pass = dc_coefficient * (self.dc_output + amplified - self.dc_input);
        self.dc_input = amplified;
        self.dc_output = high_pass;

        high_pass
    }

    pub fn diagnostics(&self) -> ConsoleElectronicsDiagnostics {
        ConsoleElectronicsDiagnostics {
            expression: self.expression_state,
            section_capacitance_pf: EXPRESSION_SECTION_CAPACITANCE_PF,
            low_corner_hz: EXPRESSION_LOW_HZ,
            tone_control: self.tone_control,
            tone_corner_hz: TONE_CONTROL_HZ,
        }
    }

    pub fn reset(&mut self, expression: f32) {
        self.expression_state = expression;
        self.expression_low_state = 0.0;
        self.expression_high_state = 0.0;
        self.bass_state = 0.0;
        self.tone_state = 0.0;
        self.dc_input = 0.0;
        self.dc_output = 0.0;
    }
}

fn tube_stage(input: f32, drive: f32, bias_scale: f32) -> f32 {
    let driven = input * (1.0 + 7.0 * drive);
    let bias = bias_scale * drive;
    let biased = driven + bias;
    let saturated = biased / (1.0 + biased.abs()) - bias / (1.0 + bias.abs());
    let makeup = 1.0 / (1.0 + 3.2 * drive);
    (input * (1.0 - drive) + saturated * drive) * makeup
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

fn tone_gain(value: f32) -> f32 {
    // [3/3] Pade approximation of exp(ln(10) * dB / 20). This keeps the
    // control linear in decibels without requiring libm in the no_std engine.
    let exponent = 1.036_163_3 * value;
    let square = exponent * exponent;
    let cube = square * exponent;
    (120.0 + 60.0 * exponent + 12.0 * square + cube)
        / (120.0 - 60.0 * exponent + 12.0 * square - cube)
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

    #[test]
    fn documented_expression_section_sets_the_low_corner() {
        let mut electronics = ConsoleElectronics::new(48_000.0);
        electronics.reset(0.0);
        let heel = electronics.diagnostics();
        electronics.reset(1.0);
        let toe = electronics.diagnostics();
        assert_eq!(heel.section_capacitance_pf, 60.0);
        assert_eq!(toe.section_capacitance_pf, 60.0);
        assert_eq!(heel.low_corner_hz, toe.low_corner_hz);
        assert!((170.0..180.0).contains(&heel.low_corner_hz));
    }

    #[test]
    fn tone_control_spans_nine_decibels_at_high_frequency() {
        assert!((20.0 * tone_gain(-1.0).log10() + 9.0).abs() < 0.001);
        assert!((20.0 * tone_gain(1.0).log10() - 9.0).abs() < 0.001);
        let electronics = ConsoleElectronics::new(48_000.0);
        assert_eq!(electronics.diagnostics().tone_corner_hz, 200.0);
    }
}
