// SPDX-License-Identifier: GPL-2.0-or-later
//! The rotary cabinet's own amplifier.
//!
//! The cabinet modelled is a 122: a balanced input, a 12AU7 pair driving two
//! 6550 beam tetrodes in push-pull, no feedback around the output transformer,
//! and a passive crossover after it. The figures below are the ones on Electro
//! Music's schematic of the 122 main amplifier as redrawn in 1995 ("117V Type
//! 122 rotor and reverb amplifiers"), where a figure is printed there, and are
//! marked provisional where it is not. What the amplifier does to the organ is
//! three things, in this order:
//!
//! - it rolls the top off where the input's 0.001 uF meets the series 10 k
//!   resistors and the volume rheostat;
//! - each output tube's grid is fed back from its own plate through 390 k and
//!   0.0047 uF, so the stage's gain falls a few decibels above a few tens of
//!   hertz and the bass stands that far above the rest;
//! - driven past the 25 V its cathodes bias the 6550 grids at, the grids
//!   conduct, the push-pull pair clips symmetrically, and the 0.1 uF coupling
//!   capacitors charge, pushing both tubes further towards cutoff until the
//!   220 k grid leaks discharge them again - the "blocking" of an overdriven
//!   valve amplifier, with the recovery time those two parts set.
//!
//! Below clipping the amplifier is level-neutral at the top of the audio
//! band, so a console that never drives it hard sounds as it did. How hard it
//! is driven is a control, because on a real cabinet it is: the volume
//! rheostat on the amplifier chassis and the connector kit set it, and no
//! console's setting is published.

use crate::model::{Knob, Model};

/// R1 + R3, in series with the two input lines.
pub(crate) const INPUT_SERIES_OHM: f32 = 20_000.0;
/// R2, the volume rheostat across the lines, at the full setting the input
/// filter is taken at.
pub(crate) const VOLUME_RHEOSTAT_OHM: f32 = 100_000.0;
/// C7, across the two grids.
pub(crate) const INPUT_SHUNT_F: f32 = 1.0e-9;
/// R8 and R9, the 12AU7 plate loads.
pub(crate) const DRIVER_PLATE_LOAD_OHM: f32 = 56_000.0;
/// The 12AU7 pair's shared cathode stands at 6 V, so its grids conduct when
/// either is driven 6 V positive.
pub(crate) const DRIVER_BIAS_V: f32 = 6.0;
/// C9 and C10, from the 12AU7 plates to the 6550 grids, and R10 and R11, the
/// grid leaks.
pub(crate) const COUPLING_F: f32 = 0.1e-6;
pub(crate) const GRID_LEAK_OHM: f32 = 220_000.0;
/// R14/C21 and R15/C13, from each 6550 plate back to its own grid.
pub(crate) const FEEDBACK_OHM: f32 = 390_000.0;
pub(crate) const FEEDBACK_F: f32 = 4.7e-9;
/// The 6550 cathodes stand at 25 V across R12, so their grids conduct 25 V
/// above rest.
pub(crate) const OUTPUT_BIAS_V: f32 = 25.0;

/// One 12AU7 half's voltage gain and plate resistance at the 2.5 mA the 6 V
/// across 1.2 k gives each. Provisional: from the valve's published curves
/// rather than a measurement of this stage.
pub(crate) const DRIVER_GAIN: f32 = 14.2;
pub(crate) const DRIVER_PLATE_OHM: f32 = 11_000.0;
/// How many volts the 6550's plate swings per volt at its grid, which is what
/// the plate-to-grid network feeds back. Provisional, from the valve's
/// curves and a plate load the schematic does not give.
pub(crate) const OUTPUT_STAGE_GAIN: f32 = 22.0;
/// How far below rest a 6550 grid goes before the tube stops conducting, at
/// the 310 V its screens are held to. Provisional, from the valve's curves.
pub(crate) const OUTPUT_CUTOFF_V: f32 = 38.0;
/// What a conducting 6550 grid presents to the coupling capacitor charging
/// through it. Provisional.
pub(crate) const GRID_CONDUCTION_OHM: f32 = 1_500.0;
/// What drives the amplifier's input: the console's output through its
/// connector kit. Provisional; no kit's figure is published.
pub(crate) const ORGAN_SOURCE_OHM: f32 = 600.0;
/// Grid volts per unit of the engine's signal at the middle of the drive
/// control: an 888 chord at full expression just reaches the 6550 grids'
/// conduction there. Provisional - the point a cabinet starts to break up is
/// a setting, not a property - and the laboratory holds this to that anchor.
pub(crate) const GRID_VOLTS_PER_UNIT: f32 = 122.05;
/// How far the drive control moves that, in decibels, end to end.
pub(crate) const DRIVE_SPAN_DB: f32 = 24.0;
/// Where the drive control starts.
pub const AMP_DRIVE_DEFAULT: f32 = 0.5;

/// A soft knee, in grid volts, on either side of where a grid starts to
/// conduct; a numerical smoothing rather than a valve property.
const KNEE_V: f32 = 1.0;

pub struct LeslieAmp {
    /// The input's low pass, bilinear with its corner held where the parts
    /// put it: feed-forward gain and feedback coefficient.
    input: [f32; 2],
    input_state: f32,
    /// The feedback shelf, as a first-order section in transposed form.
    shelf: [f32; 3],
    shelf_state: f32,
    driver_swing: f32,
    /// How far each output tube's coupling capacitor has charged past rest.
    shift: [f32; 2],
    charge_rate: f32,
    discharge_rate: f32,
    cutoff_margin: f32,
    small_signal_slope: f32,
    anchor_volts: f32,
    drive_span_db: f32,
    grid_gain: f32,
}

impl LeslieAmp {
    pub fn new(sample_rate: f32) -> Self {
        Self::with_model(sample_rate, &Model::BASELINE)
    }

    pub fn with_model(sample_rate: f32, model: &Model) -> Self {
        let driver_source = parallel(
            parallel(model[Knob::AmpDriverPlateOhm], DRIVER_PLATE_LOAD_OHM),
            GRID_LEAK_OHM,
        );
        let input_source = parallel(
            INPUT_SERIES_OHM + model[Knob::AmpOrganSourceOhm],
            VOLUME_RHEOSTAT_OHM,
        );
        let input_corner = 1.0 / (core::f32::consts::TAU * input_source * INPUT_SHUNT_F);
        let warped = tan(core::f32::consts::PI * input_corner / sample_rate);

        // The plate-to-grid network: with the driver's source impedance it
        // feeds back A Rs / (Rs + R + 1/sC) of the plate, so the stage runs at
        // (1 + s tau) / (1 + s (tau + A Rs C)) of its open gain, tau being
        // (Rs + R) C. Taken at unity at the top of the band, where the
        // capacitor is a short, that is a shelf lifting the bass by 1 + A Rs
        // / (Rs + R).
        let tau = (driver_source + FEEDBACK_OHM) * FEEDBACK_F;
        let slow = tau + model[Knob::AmpOutputStageGain] * driver_source * FEEDBACK_F;
        let top_gain = slow / tau;
        let k = 2.0 * sample_rate;
        let norm = 1.0 / (1.0 + k * slow);
        let shelf = [
            top_gain * (1.0 + k * tau) * norm,
            top_gain * (1.0 - k * tau) * norm,
            (1.0 - k * slow) * norm,
        ];

        let charge_time = COUPLING_F * (driver_source + model[Knob::AmpGridConductionOhm]);
        let discharge_time = COUPLING_F * (GRID_LEAK_OHM + driver_source);
        let cutoff_margin = model[Knob::AmpOutputCutoffV] - OUTPUT_BIAS_V;
        let mut amp = Self {
            input: [warped / (1.0 + warped), (warped - 1.0) / (1.0 + warped)],
            input_state: 0.0,
            shelf,
            shelf_state: 0.0,
            driver_swing: DRIVER_BIAS_V * model[Knob::AmpDriverGain],
            shift: [0.0; 2],
            charge_rate: 1.0 / (charge_time * sample_rate),
            discharge_rate: 1.0 / (discharge_time * sample_rate),
            cutoff_margin,
            small_signal_slope: 3.0 * sqrt(cutoff_margin),
            anchor_volts: model[Knob::AmpGridVoltsPerUnit],
            drive_span_db: model[Knob::AmpDriveSpanDb],
            grid_gain: 0.0,
        };
        amp.set_drive(AMP_DRIVE_DEFAULT);
        amp
    }

    /// How hard the console drives the amplifier: the middle is the anchor,
    /// and each end is half the span away from it.
    pub fn set_drive(&mut self, value: f32) -> bool {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return false;
        }
        let decibels = (value - 0.5) * self.drive_span_db;
        self.grid_gain = self.anchor_volts * from_decibels(decibels);
        true
    }

    pub fn reset(&mut self) {
        self.input_state = 0.0;
        self.shelf_state = 0.0;
        self.shift = [0.0; 2];
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let [gain, feedback] = self.input;
        let filtered = gain * input + self.input_state;
        self.input_state = gain * input - feedback * filtered;
        let linear = filtered * self.grid_gain;
        let [b0, b1, a1] = self.shelf;
        let shaped = b0 * linear + self.shelf_state;
        self.shelf_state = b1 * linear - a1 * shaped;

        // The driver pair is balanced, so its own curvature is odd; its grids
        // conduct a bias's worth past rest, which at its gain is far past
        // where the output grids already have.
        let drive = self.driver_swing * tanh(shaped / self.driver_swing);

        let mut current = [0.0; 2];
        for (side, (shift, current)) in self.shift.iter_mut().zip(&mut current).enumerate() {
            let grid = if side == 0 { drive } else { -drive } - *shift;
            let excess = grid - OUTPUT_BIAS_V;
            if excess > 0.0 {
                *shift += excess * self.charge_rate;
            }
            *shift -= *shift * self.discharge_rate;
            let grid = soft_ceiling(grid, OUTPUT_BIAS_V);
            let conducting = (grid + self.cutoff_margin).max(0.0);
            *current = conducting * sqrt(conducting);
        }
        (current[0] - current[1]) / (self.small_signal_slope * self.grid_gain)
    }
}

fn parallel(a: f32, b: f32) -> f32 {
    a * b / (a + b)
}

/// `value`, bent smoothly onto `ceiling` across a knee either side of it.
fn soft_ceiling(value: f32, ceiling: f32) -> f32 {
    if value <= ceiling - KNEE_V {
        value
    } else if value >= ceiling + KNEE_V {
        ceiling
    } else {
        let over = value - ceiling + KNEE_V;
        value - over * over / (4.0 * KNEE_V)
    }
}

/// Square root without a maths library: a bit-level first guess at the
/// reciprocal root, three Newton steps, and a multiply.
pub(crate) fn sqrt(value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }
    let mut root = f32::from_bits(0x5f37_59df - (value.to_bits() >> 1));
    let half = 0.5 * value;
    root *= 1.5 - half * root * root;
    root *= 1.5 - half * root * root;
    root *= 1.5 - half * root * root;
    value * root
}

/// Tangent of an angle up to about a radian, from the sine and cosine
/// series; a corner at a third of the lowest supported rate asks for 0.95.
fn tan(angle: f32) -> f32 {
    let square = angle * angle;
    let mut sine = angle;
    let mut cosine = 1.0;
    let mut term_sine = angle;
    let mut term_cosine = 1.0;
    for n in 1..8 {
        let n = n as f32;
        term_sine *= -square / ((2.0 * n) * (2.0 * n + 1.0));
        term_cosine *= -square / ((2.0 * n - 1.0) * (2.0 * n));
        sine += term_sine;
        cosine += term_cosine;
    }
    sine / cosine
}

/// Hyperbolic tangent as a [3/2] Pade approximant, held at one past where
/// it reaches it.
fn tanh(value: f32) -> f32 {
    if value >= 3.0 {
        return 1.0;
    }
    if value <= -3.0 {
        return -1.0;
    }
    let square = value * value;
    value * (27.0 + square) / (27.0 + 9.0 * square)
}

/// Decibels to gain without a maths library, through a [3/3] Pade
/// approximant of the exponential on a range split into halvings.
fn from_decibels(decibels: f32) -> f32 {
    let mut exponent = decibels * (core::f32::consts::LN_10 / 20.0);
    let mut halvings = 0;
    while exponent.abs() > 0.5 && halvings < 16 {
        exponent *= 0.5;
        halvings += 1;
    }
    let square = exponent * exponent;
    let cube = square * exponent;
    let mut gain = (120.0 + 60.0 * exponent + 12.0 * square + cube)
        / (120.0 - 60.0 * exponent + 12.0 * square - cube);
    for _ in 0..halvings {
        gain *= gain;
    }
    gain
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    #[test]
    fn the_maths_helpers_hold_their_ground() {
        for value in [1.0e-4_f32, 0.25, 2.0, 13.0, 38.0, 400.0] {
            assert!(
                (sqrt(value) - std::primitive::f32::sqrt(value)).abs() < 1.0e-5 * value.max(1.0)
            );
        }
        for value in [-2.5_f32, -1.0, -0.1, 0.0, 0.3, 1.0, 2.9] {
            assert!((tanh(value) - std::primitive::f32::tanh(value)).abs() < 0.025);
        }
        for angle in [0.01_f32, 0.3, 0.6, 0.95] {
            assert!((tan(angle) - std::primitive::f32::tan(angle)).abs() < 1.0e-5);
        }
        for decibels in [-12.0_f32, -3.0, 0.0, 6.0, 12.0] {
            let exact = 10.0_f32.powf(decibels / 20.0);
            assert!((from_decibels(decibels) - exact).abs() < 1.0e-4 * exact);
        }
    }

    fn sine(amp: &mut LeslieAmp, frequency: f32, level: f32, frames: usize) -> Vec<f32> {
        let step = core::f32::consts::TAU * frequency / 48_000.0;
        (0..frames)
            .map(|frame| amp.process(level * std::primitive::f32::sin(step * frame as f32)))
            .collect()
    }

    fn peak(samples: &[f32]) -> f32 {
        samples
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
    }

    /// Well below clipping, a tone at the top of the band comes out at the
    /// level it went in.
    #[test]
    fn a_quiet_tone_passes_at_its_own_level() {
        let mut amp = LeslieAmp::with_model(48_000.0, &Model::BASELINE);
        let out = sine(&mut amp, 2_000.0, 0.01, 24_000);
        let level = peak(&out[12_000..]) / 0.01;
        assert!((level - 1.0).abs() < 0.03, "{level}");
    }

    /// The plate-to-grid network lifts the bass by the amount its parts set.
    #[test]
    fn the_feedback_network_lifts_the_bass() {
        let mut amp = LeslieAmp::with_model(48_000.0, &Model::BASELINE);
        let low = sine(&mut amp, 20.0, 0.01, 96_000);
        let mut amp = LeslieAmp::with_model(48_000.0, &Model::BASELINE);
        let high = sine(&mut amp, 2_000.0, 0.01, 24_000);
        let lift = 20.0 * std::primitive::f32::log10(peak(&low[48_000..]) / peak(&high[12_000..]));
        assert!((2.0..5.0).contains(&lift), "{lift} dB");
    }

    /// Driven well past the grids' conduction, the output stops growing.
    #[test]
    fn driven_hard_it_clips() {
        let mut amp = LeslieAmp::with_model(48_000.0, &Model::BASELINE);
        let edge = OUTPUT_BIAS_V / GRID_VOLTS_PER_UNIT;
        let soft = peak(&sine(&mut amp, 1_000.0, 0.5 * edge, 24_000)[12_000..]);
        let mut amp = LeslieAmp::with_model(48_000.0, &Model::BASELINE);
        let hard = peak(&sine(&mut amp, 1_000.0, 4.0 * edge, 24_000)[12_000..]);
        assert!(hard < 3.0 * soft, "{soft} then {hard}");
    }
}
