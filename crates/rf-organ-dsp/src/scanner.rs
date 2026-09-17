// SPDX-License-Identifier: GPL-2.0-or-later

use crate::vibrato_line::VibratoLine;

/// Six positions of the console vibrato/chorus selector plus bypass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ScannerMode {
    #[default]
    Off = 0,
    Vibrato1 = 1,
    Vibrato2 = 2,
    Vibrato3 = 3,
    Chorus1 = 4,
    Chorus2 = 5,
    Chorus3 = 6,
}

impl ScannerMode {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Off),
            1 => Some(Self::Vibrato1),
            2 => Some(Self::Vibrato2),
            3 => Some(Self::Vibrato3),
            4 => Some(Self::Chorus1),
            5 => Some(Self::Chorus2),
            6 => Some(Self::Chorus3),
            _ => None,
        }
    }

    /// Tap set of the depth selector: V1 and C1 share one, V2 and C2 the next
    /// and V3 and C3 the widest.
    const fn depth(self) -> usize {
        match self {
            Self::Off | Self::Vibrato1 | Self::Chorus1 => 0,
            Self::Vibrato2 | Self::Chorus2 => 1,
            Self::Vibrato3 | Self::Chorus3 => 2,
        }
    }

    /// Whether the switch leaves the 22 kΩ source resistor in circuit, which
    /// is the whole electrical difference between chorus and vibrato.
    const fn chorus(self) -> bool {
        matches!(self, Self::Chorus1 | Self::Chorus2 | Self::Chorus3)
    }
}

/// The console vibrato/chorus: an LC ladder scanned by a rotating capacitive
/// pickup. See `vibrato_line` for the circuit itself.
pub struct ScannerVibrato {
    line: VibratoLine,
    mode: ScannerMode,
}

impl ScannerVibrato {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            line: VibratoLine::new(sample_rate),
            mode: ScannerMode::Off,
        }
    }

    pub fn set_mode(&mut self, mode: ScannerMode) {
        self.mode = mode;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        if self.mode == ScannerMode::Off {
            // The physical line keeps running with the console switched to
            // bypass. Only the scanner angle is kept here: the ladder settles
            // in about a millisecond, so restarting it on a switch costs less
            // than solving it for a signal nothing listens to.
            self.line.advance_scanner();
            return input;
        }
        self.line
            .process(input, self.mode.depth(), self.mode.chorus())
    }

    pub fn reset(&mut self) {
        self.line.reset();
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn bypass_is_exact_while_the_scanner_keeps_turning() {
        let mut scanner = ScannerVibrato::new(48_000.0);
        let start = scanner.line.phase();
        for index in 0..128 {
            let input = index as f32 / 128.0;
            assert_eq!(scanner.process(input), input);
        }
        assert!(scanner.line.phase() > start);
    }

    /// Chorus keeps the source resistor in circuit, so the first tap carries a
    /// large direct component that vibrato does not have.
    #[test]
    fn chorus_and_vibrato_differ_beyond_a_gain() {
        let mut vibrato = ScannerVibrato::new(48_000.0);
        let mut chorus = ScannerVibrato::new(48_000.0);
        vibrato.set_mode(ScannerMode::Vibrato3);
        chorus.set_mode(ScannerMode::Chorus3);
        let mut vibrato_energy = 0.0;
        let mut chorus_energy = 0.0;
        let mut difference = 0.0;
        for index in 0..4_096 {
            let input = if index == 0 { 1.0 } else { 0.0 };
            let left = vibrato.process(input);
            let right = chorus.process(input);
            vibrato_energy += left * left;
            chorus_energy += right * right;
            difference += (left - right).abs();
        }
        assert!(vibrato_energy > 0.0 && chorus_energy > 0.0);
        assert!(difference > 0.1);
        // The two impulse responses are not scalar multiples of each other.
        let ratio = (chorus_energy / vibrato_energy).sqrt();
        let mut shape_difference = 0.0;
        let mut vibrato = ScannerVibrato::new(48_000.0);
        let mut chorus = ScannerVibrato::new(48_000.0);
        vibrato.set_mode(ScannerMode::Vibrato3);
        chorus.set_mode(ScannerMode::Chorus3);
        for index in 0..4_096 {
            let input = if index == 0 { 1.0 } else { 0.0 };
            shape_difference += (chorus.process(input) - ratio * vibrato.process(input)).abs();
        }
        assert!(shape_difference > 0.05, "{shape_difference}");
    }

    /// Wider depth settings reach further down the ladder, so they delay more.
    #[test]
    fn depth_settings_order_their_delay() {
        let centroid = |mode| {
            let mut scanner = ScannerVibrato::new(48_000.0);
            scanner.set_mode(mode);
            let mut weighted = 0.0;
            let mut energy = 0.0;
            for index in 0..2_048 {
                let input = if index == 0 { 1.0 } else { 0.0 };
                let sample = scanner.process(input);
                let power = sample * sample;
                weighted += power * index as f32;
                energy += power;
            }
            weighted / energy
        };
        let narrow = centroid(ScannerMode::Vibrato1);
        let wide = centroid(ScannerMode::Vibrato3);
        assert!(wide > narrow, "narrow={narrow} wide={wide}");
    }
}
