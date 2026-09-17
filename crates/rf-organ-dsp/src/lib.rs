// SPDX-License-Identifier: GPL-2.0-or-later
#![no_std]

//! Real-time RF-Organ engine.
//!
//! The implementation is intentionally allocation-free. The current constants
//! establish a physically informed baseline and remain subject to measurement.

mod electronics;
mod manual;
mod pedal;
mod percussion;
mod rotary;
mod scanner;
mod taper;
mod tonewheel;
mod transformer;
mod vibrato_line;

pub use electronics::{
    ConsoleElectronics, ConsoleElectronicsDiagnostics, ConsoleStage, STAGE_CHARACTER_DEFAULT,
    STAGE_CHARACTER_RANGE,
};
pub use manual::{
    DRAWBAR_COUNT, LEAKAGE_BOOST_DEFAULT, MANUAL_FIRST_NOTE, MANUAL_KEY_COUNT,
    compartment_companions, drawbar_wheel,
};
pub use pedal::{PEDAL_DRAWBAR_COUNT, PEDAL_FIRST_NOTE, PEDAL_KEY_COUNT};
pub use percussion::{PercussionDecay, PercussionHarmonic, PercussionVolume};
pub use rotary::{
    DRUM_RADIUS_DEFAULT_M, DRUM_RADIUS_RANGE_M, HORN_RADIUS_DEFAULT_M, HORN_RADIUS_RANGE_M,
    LEVEL_RANGE_DB, LEVEL_SILENT_DB, MIC_DISTANCE_DEFAULT_M, MIC_DISTANCE_RANGE_M,
    MIC_OFFSET_MAX_M, MIC_PATTERN_DEFAULT, MIC_SPACING_DEFAULT_M, MIC_SPACING_MAX_M,
    MainsFrequency, MicrophoneArray, MicrophonePair, MicrophoneType, Rotary, RotaryDiagnostics,
    RotaryGeometry, RotaryMode, RotaryPlacement, SUB_LEVEL_DEFAULT_DB, StopAngle,
};
pub use scanner::{ScannerMode, ScannerVibrato};
pub use taper::TAPER;
pub use tonewheel::{TONEWHEEL_COUNT, gear_frequency, gear_teeth};
pub use transformer::{MatchingTransformer, TransformerDiagnostics, TransformerUnit};
pub use vibrato_line::{CUTOFF_HZ as SCANNER_LINE_CUTOFF_HZ, ROTOR_HZ as SCANNER_ROTOR_HZ};

use manual::Manual;
use pedal::Pedalboard;
use percussion::Percussion;
use tonewheel::TonewheelBank;

pub const SAMPLE_RATE_MIN: f32 = 32_000.0;
pub const SAMPLE_RATE_MAX: f32 = 192_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelError(pub &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrganPart {
    Upper,
    Lower,
    Pedal,
}

/// One physical organ. Tonewheels are shared; MIDI notes only operate the
/// manual contacts. The two matching-transformer paths follow the AO-28
/// console wiring: upper on T2, and lower plus pedals on T1.
pub struct OrganEngine {
    tonewheels: TonewheelBank,
    upper: Manual,
    lower: Manual,
    pedals: Pedalboard,
    upper_transformer: MatchingTransformer,
    lower_pedal_transformer: MatchingTransformer,
    electronics: ConsoleElectronics,
    output_transformer: MatchingTransformer,
    scanner: ScannerVibrato,
    percussion: Percussion,
    rotary: Rotary,
    transformer_character: (f32, f32),
    transformer_trims: [(f32, f32); 3],
    output_level: f32,
    expression: f32,
    leakage: f32,
    held_notes: u8,
    upper_scanner: bool,
    lower_scanner: bool,
}

impl OrganEngine {
    pub fn new(sample_rate: f32) -> Result<Self, ModelError> {
        if !sample_rate.is_finite() || !(SAMPLE_RATE_MIN..=SAMPLE_RATE_MAX).contains(&sample_rate) {
            return Err(ModelError(
                "sample rate must be finite and between 32 and 192 kHz",
            ));
        }
        Ok(Self {
            tonewheels: TonewheelBank::new(sample_rate),
            upper: Manual::new(sample_rate),
            lower: Manual::new(sample_rate),
            pedals: Pedalboard::new(sample_rate),
            upper_transformer: MatchingTransformer::new(sample_rate),
            lower_pedal_transformer: MatchingTransformer::new(sample_rate),
            electronics: ConsoleElectronics::new(sample_rate),
            output_transformer: MatchingTransformer::new(sample_rate),
            scanner: ScannerVibrato::new(sample_rate),
            percussion: Percussion::new(sample_rate),
            rotary: Rotary::new(sample_rate),
            transformer_character: (0.35, 0.25),
            transformer_trims: [(0.0, 0.0); 3],
            output_level: 0.72,
            expression: 1.0,
            leakage: 0.025,
            held_notes: 0,
            upper_scanner: true,
            lower_scanner: false,
        })
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) -> bool {
        self.note_on_part(OrganPart::Upper, note, velocity)
    }

    pub fn note_on_part(&mut self, part: OrganPart, note: u8, velocity: f32) -> bool {
        if part == OrganPart::Lower {
            return self.lower.note_on(note, velocity);
        }
        if part == OrganPart::Pedal {
            return self.pedals.note_on(note, velocity);
        }
        let Some(was_active) = self.upper.is_active(note) else {
            return false;
        };
        if !self.upper.note_on(note, velocity) {
            return false;
        }
        if !was_active {
            if self.held_notes == 0 {
                self.percussion.trigger();
            }
            self.held_notes = self.held_notes.saturating_add(1);
        }
        true
    }

    pub fn note_off(&mut self, note: u8, velocity: f32) -> bool {
        self.note_off_part(OrganPart::Upper, note, velocity)
    }

    pub fn note_off_part(&mut self, part: OrganPart, note: u8, velocity: f32) -> bool {
        if part == OrganPart::Lower {
            return self.lower.note_off(note, velocity);
        }
        if part == OrganPart::Pedal {
            return self.pedals.note_off(note, velocity);
        }
        let Some(was_active) = self.upper.is_active(note) else {
            return false;
        };
        if !self.upper.note_off(note, velocity) {
            return false;
        }
        if was_active {
            self.held_notes = self.held_notes.saturating_sub(1);
        }
        true
    }

    pub fn all_notes_off(&mut self) {
        self.upper.reset();
        self.lower.reset();
        self.pedals.reset();
        self.held_notes = 0;
    }

    pub fn all_notes_off_part(&mut self, part: OrganPart) {
        match part {
            OrganPart::Upper => {
                self.upper.reset();
                self.held_notes = 0;
            }
            OrganPart::Lower => self.lower.reset(),
            OrganPart::Pedal => self.pedals.reset(),
        }
    }

    pub fn set_drawbar(&mut self, index: usize, position: u8) -> bool {
        self.upper.set_drawbar(index, position)
    }

    pub fn drawbar(&self, index: usize) -> Option<u8> {
        self.upper.drawbar(index)
    }

    pub fn set_manual_drawbar(&mut self, part: OrganPart, index: usize, position: u8) -> bool {
        match part {
            OrganPart::Upper => self.upper.set_drawbar(index, position),
            OrganPart::Lower => self.lower.set_drawbar(index, position),
            OrganPart::Pedal => self.pedals.set_drawbar(index, position),
        }
    }

    pub fn manual_drawbar(&self, part: OrganPart, index: usize) -> Option<u8> {
        match part {
            OrganPart::Upper => self.upper.drawbar(index),
            OrganPart::Lower => self.lower.drawbar(index),
            OrganPart::Pedal => self.pedals.drawbar(index),
        }
    }

    pub fn set_output_level(&mut self, value: f32) -> bool {
        if !value.is_finite() || !(0.0..=1.5).contains(&value) {
            return false;
        }
        self.output_level = value;
        true
    }

    pub fn set_expression(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        self.expression = value;
        true
    }

    pub fn set_contact_spread(&mut self, value: f32) -> bool {
        self.upper.set_contact_spread(value)
            && self.lower.set_contact_spread(value)
            && self.pedals.set_contact_spread(value)
    }

    pub fn set_contact_bounce(&mut self, value: f32) -> bool {
        self.upper.set_contact_bounce(value)
            && self.lower.set_contact_bounce(value)
            && self.pedals.set_contact_bounce(value)
    }

    /// Depth of the once-per-revolution level change of an off-centre wheel.
    pub fn set_eccentricity(&mut self, depth: f32) -> bool {
        self.tonewheels.set_eccentricity(depth)
    }

    /// How fast the leakage grows as more keys go down, on both manuals.
    /// Scales the three preamplifier stage asymmetries together. See
    /// `crates/rf-organ-dsp/src/electronics.rs` for why it is one control.
    pub fn set_console_stage_character(&mut self, character: f32) -> bool {
        self.electronics.set_stage_character(character)
    }

    /// Moves one preamplifier stage off the shared character. Only a bench
    /// that injected at that stage can say what belongs here.
    pub fn set_console_stage_trim(&mut self, stage: ConsoleStage, trim: f32) -> bool {
        self.electronics.set_stage_trim(stage, trim)
    }

    pub fn set_leakage_boost(&mut self, rate: f32) -> bool {
        self.upper.set_leakage_boost(rate) && self.lower.set_leakage_boost(rate)
    }

    pub fn set_drive_wobble(&mut self, depth: f32) -> bool {
        self.tonewheels.set_drive_wobble(depth)
    }

    /// Where the drive is now, as a fraction of its nominal speed. This is the
    /// shaft's own state, so a tool can hold what it measures in the audio
    /// against what the mechanism says it should be.
    pub const fn drive_deviation(&self) -> f32 {
        self.tonewheels.drive_deviation()
    }

    pub const fn eccentricity(&self) -> f32 {
        self.tonewheels.eccentricity()
    }

    /// Output level of one generator wheel, one-based terminal `index + 1`.
    pub fn set_wheel_level(&mut self, index: usize, level: f32) -> bool {
        self.tonewheels.set_level(index, level)
    }

    pub fn wheel_level(&self, index: usize) -> Option<f32> {
        self.tonewheels.level(index)
    }

    pub fn set_leakage(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        self.leakage = value * 0.12;
        true
    }

    /// Sets the shared musical character of the three transformers. Each unit
    /// keeps its own calibration trim on top of this value.
    pub fn set_transformer(&mut self, drive: f32, hysteresis: f32) -> bool {
        if !unit(drive) || !unit(hysteresis) {
            return false;
        }
        self.transformer_character = (drive, hysteresis);
        for which in TransformerUnit::ALL {
            self.calibrate(which);
        }
        true
    }

    /// Offsets one transformer from the shared character. Trims are bipolar
    /// and the resulting coefficients are clamped to the model range, so a
    /// measurement of T1, T2 or T3 can be applied without moving the others.
    pub fn set_transformer_trim(
        &mut self,
        which: TransformerUnit,
        drive: f32,
        hysteresis: f32,
    ) -> bool {
        if !bipolar(drive) || !bipolar(hysteresis) {
            return false;
        }
        self.transformer_trims[which.index()] = (drive, hysteresis);
        self.calibrate(which);
        true
    }

    pub const fn transformer_trim(&self, which: TransformerUnit) -> (f32, f32) {
        self.transformer_trims[which.index()]
    }

    pub const fn transformer_diagnostics(&self, which: TransformerUnit) -> TransformerDiagnostics {
        match which {
            TransformerUnit::T1 => self.lower_pedal_transformer.diagnostics(),
            TransformerUnit::T2 => self.upper_transformer.diagnostics(),
            TransformerUnit::T3 => self.output_transformer.diagnostics(),
        }
    }

    fn calibrate(&mut self, which: TransformerUnit) {
        let (character_drive, character_hysteresis) = self.transformer_character;
        let (drive_trim, hysteresis_trim) = self.transformer_trims[which.index()];
        let drive = (character_drive + drive_trim).clamp(0.0, 1.0);
        let hysteresis = (character_hysteresis + hysteresis_trim).clamp(0.0, 1.0);
        let transformer = match which {
            TransformerUnit::T1 => &mut self.lower_pedal_transformer,
            TransformerUnit::T2 => &mut self.upper_transformer,
            TransformerUnit::T3 => &mut self.output_transformer,
        };
        let applied = transformer.set(drive, hysteresis);
        debug_assert!(applied);
    }

    pub fn set_console(&mut self, drive: f32, bass: f32, tone: f32) -> bool {
        self.electronics.set(drive, bass, tone)
    }

    pub fn set_expression_character(&mut self, value: f32) -> bool {
        self.electronics.set_expression_character(value)
    }

    pub fn set_scanner_mode(&mut self, mode: ScannerMode) {
        self.scanner.set_mode(mode);
    }

    pub fn set_scanner_manuals(&mut self, upper: bool, lower: bool) {
        self.upper_scanner = upper;
        self.lower_scanner = lower;
    }

    pub fn set_percussion_enabled(&mut self, enabled: bool) {
        self.percussion.set_enabled(enabled);
    }

    pub fn set_percussion_harmonic(&mut self, harmonic: PercussionHarmonic) {
        self.percussion.set_harmonic(harmonic);
    }

    pub fn set_percussion_volume(&mut self, volume: PercussionVolume) {
        self.percussion.set_volume(volume);
    }

    pub fn set_percussion_decay(&mut self, decay: PercussionDecay) {
        self.percussion.set_decay(decay);
    }

    pub fn set_rotary_mode(&mut self, mode: RotaryMode) {
        self.rotary.set_mode(mode);
    }

    pub const fn rotary_mode(&self) -> RotaryMode {
        self.rotary.mode()
    }

    pub fn set_rotary_mix(&mut self, value: f32) -> bool {
        self.rotary.set_mix(value)
    }

    pub fn set_rotary_acceleration(&mut self, value: f32) -> bool {
        self.rotary.set_acceleration(value)
    }

    pub fn set_rotary_cabinet(&mut self, reflections: f32) -> bool {
        self.rotary.set_cabinet(reflections)
    }

    /// The three microphone volumes, in decibels.
    pub fn set_rotary_levels(&mut self, horn_db: f32, drum_db: f32, sub_db: f32) -> bool {
        self.rotary.set_levels(horn_db, drum_db, sub_db)
    }

    pub fn set_rotary_microphones(&mut self, array: MicrophoneArray) -> bool {
        self.rotary.set_microphones(array)
    }

    pub fn set_rotary_rotor_radii(&mut self, horn_m: f32, drum_m: f32) -> bool {
        self.rotary.set_rotor_radii(horn_m, drum_m)
    }

    pub fn set_rotary_stop_angles(&mut self, horn: StopAngle, drum: StopAngle) -> bool {
        self.rotary.set_stop_angles(horn, drum)
    }

    /// The supply the installation runs from. It reaches two machines, and
    /// does a different documented thing to each: the cabinet's motors turn
    /// in proportion to it, while the console's run motor turns at whichever
    /// synchronous speed its market's armature gives, geared back to concert
    /// pitch either way.
    pub fn set_mains(&mut self, mains: MainsFrequency) {
        self.rotary.set_mains(mains);
        self.tonewheels
            .set_shaft_rpm_for_fifty(mains == MainsFrequency::Fifty);
    }

    pub fn set_rotary_microphone_type(&mut self, capsule: MicrophoneType) {
        self.rotary.set_microphone_type(capsule);
    }

    pub fn next_sample(&mut self) -> [f32; 2] {
        self.tonewheels.tick();
        self.upper.tick_contacts();
        self.lower.tick_contacts();
        self.pedals.tick_contacts();
        let wheels = self.tonewheels.samples();
        let upper = self
            .upper
            .sample(wheels, self.leakage, self.percussion.enabled())
            * self.percussion.drawbar_attenuation();
        let lower = self.lower.sample(wheels, self.leakage, false);
        let pedals = self.pedals.sample(wheels);
        let percussion_bus = match self.percussion.harmonic() {
            PercussionHarmonic::Second => 3,
            PercussionHarmonic::Third => 4,
        };
        let percussion = self.percussion.process(
            self.upper.harmonic_sample(wheels, percussion_bus),
            self.held_notes > 0,
        );
        let console = self.route_ao28_inputs(upper, lower, pedals, percussion);
        let console = self.electronics.process(console, self.expression);
        let organ = self.output_transformer.process(console) * self.output_level;
        self.rotary.process(organ)
    }

    /// Routes the generator buses into the AO-28 input channels. T2 receives
    /// the upper manual, T1 receives the combined lower/pedal bus, and the
    /// percussion amplifier joins the signal at V4A after the scanner return.
    fn route_ao28_inputs(&mut self, upper: f32, lower: f32, pedals: f32, percussion: f32) -> f32 {
        let upper = self.upper_transformer.process(upper);
        let lower_pedal = self.lower_pedal_transformer.process(lower + pedals);
        let scanner_input = if self.upper_scanner { upper } else { 0.0 }
            + if self.lower_scanner { lower_pedal } else { 0.0 };
        let direct = if self.upper_scanner { 0.0 } else { upper }
            + if self.lower_scanner { 0.0 } else { lower_pedal };
        direct + self.scanner.process(scanner_input) + percussion
    }

    /// Clears keyed and downstream state without rephasing the continuously
    /// rotating generator.
    pub fn reset(&mut self) {
        self.upper.reset();
        self.lower.reset();
        self.pedals.reset();
        self.upper_transformer.reset();
        self.lower_pedal_transformer.reset();
        self.electronics.reset(1.0);
        self.output_transformer.reset();
        self.scanner.reset();
        self.percussion.reset();
        self.rotary.reset();
        self.expression = 1.0;
        self.held_notes = 0;
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
    extern crate std;

    use super::*;

    #[test]
    fn silence_does_not_stop_the_generator() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        let before = engine.tonewheels.samples()[12];
        for _ in 0..64 {
            assert_eq!(engine.next_sample(), [0.0, 0.0]);
        }
        assert_ne!(before, engine.tonewheels.samples()[12]);
    }

    #[test]
    fn a_key_operates_nine_shared_wheels() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        assert!(engine.note_on(60, 0.8));
        let mut energy = 0.0;
        for _ in 0..4096 {
            let [left, right] = engine.next_sample();
            energy += left * left + right * right;
        }
        assert!(energy > 0.01);
    }

    #[test]
    fn rotary_is_inside_the_instrument_path() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        assert!(engine.note_on(60, 1.0));
        engine.set_rotary_mode(RotaryMode::Tremolo);
        assert!(engine.set_rotary_mix(1.0));
        let mut stereo_difference = 0.0;
        for _ in 0..24_000 {
            let [left, right] = engine.next_sample();
            stereo_difference += (left - right).abs();
        }
        assert!(stereo_difference > 0.1);
    }

    #[test]
    fn percussion_sounds_with_the_drawbars_closed() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        for drawbar in 0..DRAWBAR_COUNT {
            assert!(engine.set_drawbar(drawbar, 0));
        }
        engine.set_percussion_enabled(true);
        engine.set_percussion_harmonic(PercussionHarmonic::Third);
        assert!(engine.note_on(60, 1.0));
        let mut energy = 0.0;
        for _ in 0..4096 {
            let [left, right] = engine.next_sample();
            energy += left * left + right * right;
        }
        assert!(energy > 0.001);
    }

    #[test]
    fn lower_manual_and_pedals_share_the_generator() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        for drawbar in 0..DRAWBAR_COUNT {
            assert!(engine.set_manual_drawbar(OrganPart::Upper, drawbar, 0));
        }
        assert!(engine.note_on_part(OrganPart::Lower, 60, 0.8));
        assert!(engine.note_on_part(OrganPart::Pedal, 24, 1.0));
        let mut energy = 0.0;
        for _ in 0..4096 {
            let [left, right] = engine.next_sample();
            energy += left * left + right * right;
        }
        assert!(energy > 0.01);
    }

    #[test]
    fn percussion_joins_after_the_scanner_return() {
        let mut scanned = OrganEngine::new(48_000.0).expect("valid engine");
        let mut silent = OrganEngine::new(48_000.0).expect("valid engine");
        for engine in [&mut scanned, &mut silent] {
            // A transformer drive high enough that anything passing through
            // T1 or T2 would come out visibly compressed.
            assert!(engine.set_transformer(0.9, 0.8));
            engine.set_scanner_mode(ScannerMode::Vibrato3);
            engine.set_scanner_manuals(true, true);
        }

        // Whatever the matching transformers and the vibrato line do with the
        // manuals, the percussion channel reaches the V4A sum untouched: two
        // identical consoles that differ only in percussion differ by exactly
        // that much.
        let with_percussion = scanned.route_ao28_inputs(0.5, 0.4, 0.3, 0.25);
        let without_percussion = silent.route_ao28_inputs(0.5, 0.4, 0.3, 0.0);
        assert!((with_percussion - without_percussion - 0.25).abs() < 1.0e-6);
    }

    #[test]
    fn lower_and_pedal_share_t1_before_the_vibrato_switch() {
        let mut lower = OrganEngine::new(48_000.0).expect("valid engine");
        let mut pedal = OrganEngine::new(48_000.0).expect("valid engine");
        assert!(lower.set_transformer(0.8, 0.7));
        assert!(pedal.set_transformer(0.8, 0.7));
        lower.set_scanner_manuals(false, false);
        pedal.set_scanner_manuals(false, false);

        let from_lower = lower.route_ao28_inputs(0.0, 0.4, 0.0, 0.0);
        let from_pedal = pedal.route_ao28_inputs(0.0, 0.0, 0.4, 0.0);
        assert_eq!(from_lower, from_pedal);
    }

    #[test]
    fn upper_t2_and_lower_t1_have_independent_magnetic_states() {
        let mut combined = OrganEngine::new(48_000.0).expect("valid engine");
        let mut upper = OrganEngine::new(48_000.0).expect("valid engine");
        let mut lower = OrganEngine::new(48_000.0).expect("valid engine");
        for engine in [&mut combined, &mut upper, &mut lower] {
            assert!(engine.set_transformer(0.9, 0.8));
            engine.set_scanner_manuals(false, false);
        }

        let both = combined.route_ao28_inputs(0.4, 0.4, 0.0, 0.0);
        let separate = upper.route_ao28_inputs(0.4, 0.0, 0.0, 0.0)
            + lower.route_ao28_inputs(0.0, 0.4, 0.0, 0.0);
        assert_eq!(both, separate);
    }

    #[test]
    fn transformer_trims_offset_one_unit_at_a_time() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        assert!(engine.set_transformer(0.4, 0.3));
        for which in TransformerUnit::ALL {
            let diagnostics = engine.transformer_diagnostics(which);
            assert_eq!(diagnostics.drive, 0.4, "{}", which.label());
            assert_eq!(diagnostics.hysteresis, 0.3, "{}", which.label());
        }

        assert!(engine.set_transformer_trim(TransformerUnit::T3, 0.25, -0.1));
        assert_eq!(
            engine.transformer_diagnostics(TransformerUnit::T3).drive,
            0.65
        );
        assert!(
            (engine
                .transformer_diagnostics(TransformerUnit::T3)
                .hysteresis
                - 0.2)
                .abs()
                < 1.0e-6
        );
        assert_eq!(
            engine.transformer_diagnostics(TransformerUnit::T1).drive,
            0.4
        );
        assert_eq!(
            engine.transformer_diagnostics(TransformerUnit::T2).drive,
            0.4
        );
        assert_eq!(engine.transformer_trim(TransformerUnit::T3), (0.25, -0.1));
    }

    #[test]
    fn transformer_trims_follow_the_character_control_and_stay_in_range() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        assert!(engine.set_transformer_trim(TransformerUnit::T1, -0.5, 1.0));
        assert!(engine.set_transformer(0.2, 0.5));
        let t1 = engine.transformer_diagnostics(TransformerUnit::T1);
        assert_eq!(t1.drive, 0.0);
        assert_eq!(t1.hysteresis, 1.0);
        assert_eq!(
            engine.transformer_diagnostics(TransformerUnit::T2).drive,
            0.2
        );
        assert!(!engine.set_transformer_trim(TransformerUnit::T1, 1.5, 0.0));
        assert!(!engine.set_transformer_trim(TransformerUnit::T1, 0.0, f32::NAN));
        assert_eq!(engine.transformer_trim(TransformerUnit::T1), (-0.5, 1.0));
    }

    #[test]
    fn an_output_transformer_trim_changes_only_the_output_stage() {
        let mut shared = OrganEngine::new(48_000.0).expect("valid engine");
        let mut trimmed = OrganEngine::new(48_000.0).expect("valid engine");
        for engine in [&mut shared, &mut trimmed] {
            assert!(engine.set_transformer(0.4, 0.3));
            engine.set_scanner_manuals(false, false);
            assert!(engine.note_on(72, 1.0));
        }
        assert!(trimmed.set_transformer_trim(TransformerUnit::T3, 0.4, 0.0));

        let mut difference = 0.0;
        for _ in 0..4096 {
            let [left, _] = shared.next_sample();
            let [trimmed_left, _] = trimmed.next_sample();
            difference += (left - trimmed_left).abs();
        }
        assert!(difference > 0.01);
        assert_eq!(
            shared.transformer_diagnostics(TransformerUnit::T2).drive,
            trimmed.transformer_diagnostics(TransformerUnit::T2).drive
        );
    }

    #[test]
    fn t3_receives_the_summed_console_output() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        assert!(engine.note_on(60, 1.0));
        for _ in 0..4096 {
            engine.next_sample();
        }
        assert!(engine.output_transformer.diagnostics().magnetization.abs() > 1.0e-6);
        engine.reset();
        assert_eq!(engine.output_transformer.diagnostics().magnetization, 0.0);
    }
}
