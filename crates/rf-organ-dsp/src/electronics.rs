// SPDX-License-Identifier: GPL-2.0-or-later

use core::f32::consts::TAU;

const EXPRESSION_SECTION_CAPACITANCE_PF: f32 = 60.0;
const EXPRESSION_GRID_RESISTANCE_OHM: f32 = 15_000_000.0;
const EXPRESSION_LOW_HZ: f32 =
    1.0 / (TAU * EXPRESSION_GRID_RESISTANCE_OHM * EXPRESSION_SECTION_CAPACITANCE_PF * 1.0e-12);
const EXPRESSION_HIGH_HZ: f32 = 4_000.0;
const TONE_CONTROL_HZ: f32 = 200.0;
/// How asymmetric each single-ended stage is. The stages differ in operating
/// point, so they differ here; all three values are provisional and the
/// laboratory reports the harmonic structure they produce.
///
/// They are scaled together by a character, the same arrangement the matching
/// transformers use: what a bench can measure from outside the preamplifier is
/// how asymmetric the chain is, not how the three stages divide it between
/// them. Telling them apart needs an injection at each stage, and until there
/// is one the ratios between these three stay where they are and only their
/// common size moves.
const V4A_ASYMMETRY: f32 = 0.45;
const V4B_ASYMMETRY: f32 = 0.30;
const V3B_ASYMMETRY: f32 = 0.22;
/// The range that character may take, with one meaning the values above.
pub const STAGE_CHARACTER_RANGE: (f32, f32) = (0.0, 3.0);
pub const STAGE_CHARACTER_DEFAULT: f32 = 1.0;

/// One of the preamplifier's three single-ended stages.
///
/// They are named here so that a bench can be asked about one of them at a
/// time. A probe at a stage's grid and another at its plate sees that stage
/// alone, which is the only way to learn how the three divide the chain's
/// asymmetry between them; a tone into the input and a reading at the output
/// passes through all three and comes out once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConsoleStage {
    V4A = 0,
    V4B = 1,
    V3B = 2,
}

impl ConsoleStage {
    pub const ALL: [Self; 3] = [Self::V4A, Self::V4B, Self::V3B];

    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::V4A),
            1 => Some(Self::V4B),
            2 => Some(Self::V3B),
            _ => None,
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::V4A => "v4a",
            Self::V4B => "v4b",
            Self::V3B => "v3b",
        }
    }

    /// How lopsided this stage is before the character and its trim, and how
    /// hard the drive control pushes it.
    const fn asymmetry(self) -> f32 {
        match self {
            Self::V4A => V4A_ASYMMETRY,
            Self::V4B => V4B_ASYMMETRY,
            Self::V3B => V3B_ASYMMETRY,
        }
    }

    const fn drive_share(self) -> f32 {
        match self {
            Self::V4A => 0.45,
            Self::V4B => 0.35,
            Self::V3B => 0.20,
        }
    }
}

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
    stage_character: f32,
    /// Per-stage offsets from that shared character, in the order of
    /// [`ConsoleStage::ALL`]. Zero leaves a stage where the character puts it.
    stage_trims: [f32; 3],
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
            stage_character: STAGE_CHARACTER_DEFAULT,
            stage_trims: [0.0; 3],
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

    /// Scales all three stage asymmetries together. One is the provisional
    /// set the model ships with; the laboratory's console fit reports what a
    /// reference recording asks for instead.
    pub fn set_stage_character(&mut self, character: f32) -> bool {
        let (low, high) = STAGE_CHARACTER_RANGE;
        if !character.is_finite() || !(low..=high).contains(&character) {
            return false;
        }
        self.stage_character = character;
        true
    }

    pub const fn stage_character(&self) -> f32 {
        self.stage_character
    }

    /// Moves one stage off the shared character. A bench that has injected at
    /// that stage alone can say what it should be; without one, the three
    /// stay where they are.
    pub fn set_stage_trim(&mut self, stage: ConsoleStage, trim: f32) -> bool {
        if !trim.is_finite() || !(-1.0..=1.0).contains(&trim) {
            return false;
        }
        self.stage_trims[stage.index()] = trim;
        true
    }

    pub const fn stage_trim(&self, stage: ConsoleStage) -> f32 {
        self.stage_trims[stage.index()]
    }

    /// How lopsided one stage is as it currently stands.
    fn stage_asymmetry(&self, stage: ConsoleStage) -> f32 {
        stage.asymmetry() * self.stage_character * trim_gain(self.stage_trims[stage.index()])
    }

    /// One stage on its own, as a probe at its grid and another at its plate
    /// would see it. Nothing before or after it is in the way, which is what
    /// makes the reading about that stage and not about the chain.
    pub fn stage_sample(&self, stage: ConsoleStage, input: f32) -> f32 {
        tube_stage(
            input,
            self.drive * stage.drive_share(),
            self.stage_asymmetry(stage),
        )
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
        let pre_expression = self.stage_sample(ConsoleStage::V4A, equalized);

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
        let post_expression = self.stage_sample(ConsoleStage::V4B, expressed);
        let tone_coefficient = one_pole(TONE_CONTROL_HZ, self.sample_rate);
        self.tone_state += tone_coefficient * (post_expression - self.tone_state);
        let tone_high = post_expression - self.tone_state;
        let toned = post_expression + (tone_gain(self.tone_control) - 1.0) * tone_high;

        // V3B/12BH7 is the final active stage before output transformer T3.
        let amplified = self.stage_sample(ConsoleStage::V3B, toned);

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

/// One single-ended triode stage. A triode's plate current follows roughly a
/// three-halves power of its grid voltage, so the transfer curve is not
/// symmetric: one half of the waveform compresses harder than the other, and
/// the distortion that results is dominated by the second harmonic rather
/// than the third. That is the character the AO-28's single-ended stages
/// contribute, and it is why a symmetric soft clipper is the wrong shape for
/// them. `asymmetry` is how much harder the positive half compresses, and is
/// provisional per stage.
fn tube_stage(input: f32, drive: f32, asymmetry: f32) -> f32 {
    let driven = input * (1.0 + 7.0 * drive);
    // Two shapes in series, and the order matters for which harmonic leads.
    // The symmetric part is a cubic soft clip, whose lowest distortion term is
    // third order and therefore grows with the square of level. The asymmetric
    // part is a rational curve whose expansion is x - a x^2 + a^2 x^3: second
    // order to first power of level, third order to the square of it. That is
    // the single-ended triode relationship, where the third harmonic sits
    // roughly as far below the second as the second is below the fundamental,
    // and it is what makes a console sound like one stage rather than like a
    // clipper.
    // Unit slope at the origin, so the stage neither gains nor loses level
    // until it is actually being driven.
    let clipped = if driven >= 1.0 {
        2.0 / 3.0
    } else if driven <= -1.0 {
        -2.0 / 3.0
    } else {
        driven - driven * driven * driven / 3.0
    };
    let saturated = clipped / (1.0 + asymmetry * clipped);
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
