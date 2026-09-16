// SPDX-License-Identifier: GPL-2.0-or-later
#![no_std]

//! Real-time RF-Organ engine.
//!
//! The implementation is intentionally allocation-free. The current constants
//! establish a physically informed baseline and remain subject to measurement.

mod leslie;
mod manual;
mod percussion;
mod scanner;
mod tonewheel;
mod transformer;

pub use leslie::LeslieMode;
pub use manual::{DRAWBAR_COUNT, MANUAL_FIRST_NOTE, MANUAL_KEY_COUNT, drawbar_wheel};
pub use percussion::{PercussionDecay, PercussionHarmonic, PercussionVolume};
pub use scanner::ScannerMode;
pub use tonewheel::{TONEWHEEL_COUNT, gear_frequency};

use leslie::Leslie;
use manual::Manual;
use percussion::Percussion;
use scanner::ScannerVibrato;
use tonewheel::TonewheelBank;
use transformer::MatchingTransformer;

pub const SAMPLE_RATE_MIN: f32 = 32_000.0;
pub const SAMPLE_RATE_MAX: f32 = 192_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelError(pub &'static str);

/// One physical organ. Tonewheels and the matching transformer are shared;
/// MIDI notes only operate the manual contacts.
pub struct OrganEngine {
    tonewheels: TonewheelBank,
    upper: Manual,
    transformer: MatchingTransformer,
    scanner: ScannerVibrato,
    percussion: Percussion,
    leslie: Leslie,
    output_level: f32,
    expression: f32,
    leakage: f32,
    held_notes: u8,
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
            transformer: MatchingTransformer::new(sample_rate),
            scanner: ScannerVibrato::new(sample_rate),
            percussion: Percussion::new(sample_rate),
            leslie: Leslie::new(sample_rate),
            output_level: 0.72,
            expression: 1.0,
            leakage: 0.025,
            held_notes: 0,
        })
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) -> bool {
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
        self.held_notes = 0;
    }

    pub fn set_drawbar(&mut self, index: usize, position: u8) -> bool {
        self.upper.set_drawbar(index, position)
    }

    pub fn drawbar(&self, index: usize) -> Option<u8> {
        self.upper.drawbar(index)
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
    }

    pub fn set_contact_bounce(&mut self, value: f32) -> bool {
        self.upper.set_contact_bounce(value)
    }

    pub fn set_leakage(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        self.leakage = value * 0.12;
        true
    }

    pub fn set_transformer(&mut self, drive: f32, hysteresis: f32) -> bool {
        self.transformer.set(drive, hysteresis)
    }

    pub fn set_scanner_mode(&mut self, mode: ScannerMode) {
        self.scanner.set_mode(mode);
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

    pub fn set_leslie_mode(&mut self, mode: LeslieMode) {
        self.leslie.set_mode(mode);
    }

    pub fn set_leslie_mix(&mut self, value: f32) -> bool {
        self.leslie.set_mix(value)
    }

    pub fn set_leslie_acceleration(&mut self, value: f32) -> bool {
        self.leslie.set_acceleration(value)
    }

    pub fn next_sample(&mut self) -> [f32; 2] {
        self.tonewheels.tick();
        self.upper.tick_contacts();
        let wheels = self.tonewheels.samples();
        let buses = self
            .upper
            .sample(wheels, self.leakage, self.percussion.enabled());
        let percussion_bus = match self.percussion.harmonic() {
            PercussionHarmonic::Second => 3,
            PercussionHarmonic::Third => 4,
        };
        let percussion = self
            .percussion
            .process(self.upper.harmonic_sample(wheels, percussion_bus));
        let transformed = self.transformer.process(buses + percussion);
        let organ = self.scanner.process(transformed) * self.expression * self.output_level;
        self.leslie.process(organ)
    }

    /// Clears keyed and downstream state without rephasing the continuously
    /// rotating generator.
    pub fn reset(&mut self) {
        self.upper.reset();
        self.transformer.reset();
        self.scanner.reset();
        self.percussion.reset();
        self.leslie.reset();
        self.expression = 1.0;
        self.held_notes = 0;
    }
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
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
    fn leslie_is_inside_the_instrument_path() {
        let mut engine = OrganEngine::new(48_000.0).expect("valid engine");
        assert!(engine.note_on(60, 1.0));
        engine.set_leslie_mode(LeslieMode::Tremolo);
        assert!(engine.set_leslie_mix(1.0));
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
}
