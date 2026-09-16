// SPDX-License-Identifier: GPL-2.0-or-later

use crate::tonewheel::TONEWHEEL_COUNT;

pub const PEDAL_DRAWBAR_COUNT: usize = 2;
pub const PEDAL_FIRST_NOTE: u8 = 24;
pub const PEDAL_KEY_COUNT: usize = 25;
const PEDAL_LAST_NOTE: u8 = PEDAL_FIRST_NOTE + PEDAL_KEY_COUNT as u8 - 1;
const DRAWBAR_LEVELS: [f32; 9] = [0.0, 0.089, 0.126, 0.178, 0.251, 0.355, 0.501, 0.708, 1.0];

#[derive(Clone, Copy)]
struct PedalKey {
    active: bool,
    gate: f32,
    target: f32,
}

impl PedalKey {
    const OFF: Self = Self {
        active: false,
        gate: 0.0,
        target: 0.0,
    };
}

/// Twenty-five-note console pedal clavier. Both drawbars address the shared
/// generator one octave apart; the bounded gate slew represents the pedal
/// contact and suppresses impossible sample discontinuities.
pub struct Pedalboard {
    keys: [PedalKey; PEDAL_KEY_COUNT],
    drawbars: [u8; PEDAL_DRAWBAR_COUNT],
    gate_step: f32,
}

impl Pedalboard {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            keys: [PedalKey::OFF; PEDAL_KEY_COUNT],
            drawbars: [8, 0],
            gate_step: 1.0 / (sample_rate * 0.0015).max(1.0),
        }
    }

    pub fn set_drawbar(&mut self, index: usize, position: u8) -> bool {
        if index >= PEDAL_DRAWBAR_COUNT || position > 8 {
            return false;
        }
        self.drawbars[index] = position;
        true
    }

    pub fn drawbar(&self, index: usize) -> Option<u8> {
        self.drawbars.get(index).copied()
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) -> bool {
        let Some(key) = key_index(note) else {
            return false;
        };
        if !unit(velocity) || velocity == 0.0 {
            return false;
        }
        self.keys[key].active = true;
        self.keys[key].target = 1.0;
        true
    }

    pub fn note_off(&mut self, note: u8, velocity: f32) -> bool {
        let Some(key) = key_index(note) else {
            return false;
        };
        if !unit(velocity) {
            return false;
        }
        self.keys[key].active = false;
        self.keys[key].target = 0.0;
        true
    }

    pub fn tick(&mut self) {
        for key in &mut self.keys {
            if key.gate < key.target {
                key.gate = (key.gate + self.gate_step).min(key.target);
            } else if key.gate > key.target {
                key.gate = (key.gate - self.gate_step).max(key.target);
            }
        }
    }

    pub fn sample(&self, wheels: &[f32; TONEWHEEL_COUNT]) -> f32 {
        let mut output = 0.0;
        for (key_index, key) in self.keys.iter().enumerate() {
            if key.gate == 0.0 {
                continue;
            }
            for (drawbar, octave) in [0_usize, 12].into_iter().enumerate() {
                let wheel = key_index + octave;
                let level = DRAWBAR_LEVELS[self.drawbars[drawbar] as usize];
                output += key.gate * level * wheels[wheel];
            }
        }
        output * 0.07
    }

    pub fn reset(&mut self) {
        self.keys.fill(PedalKey::OFF);
    }
}

fn key_index(note: u8) -> Option<usize> {
    (PEDAL_FIRST_NOTE..=PEDAL_LAST_NOTE)
        .contains(&note)
        .then(|| usize::from(note - PEDAL_FIRST_NOTE))
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pedal_range_and_drawbars_are_bounded() {
        let mut pedals = Pedalboard::new(48_000.0);
        assert!(!pedals.note_on(23, 1.0));
        assert!(pedals.note_on(24, 1.0));
        assert!(pedals.note_on(48, 1.0));
        assert!(!pedals.note_on(49, 1.0));
        assert!(pedals.set_drawbar(1, 8));
        assert!(!pedals.set_drawbar(2, 8));
        assert!(!pedals.set_drawbar(0, 9));
    }
}
