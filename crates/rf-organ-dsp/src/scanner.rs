// SPDX-License-Identifier: GPL-2.0-or-later

const DELAY_CAPACITY: usize = 1024;
const TAP_COUNT: usize = 16;
// One mechanical scanner revolution traverses the delay line out and back.
// These raised-cosine positions make the sixteenth-to-first transition cyclic.
const TAP_POSITIONS: [f32; TAP_COUNT] = [
    0.0, 0.038, 0.146, 0.309, 0.5, 0.691, 0.854, 0.962, 1.0, 0.962, 0.854, 0.691, 0.5, 0.309,
    0.146, 0.038,
];

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

    const fn depth(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::Vibrato1 | Self::Chorus1 => 0.34,
            Self::Vibrato2 | Self::Chorus2 => 0.67,
            Self::Vibrato3 | Self::Chorus3 => 1.0,
        }
    }

    const fn chorus(self) -> bool {
        matches!(self, Self::Chorus1 | Self::Chorus2 | Self::Chorus3)
    }
}

/// Reduced real-time model of the LC vibrato line and rotating capacitive
/// scanner. Sixteen delay taps are selected continuously at scanner speed.
pub struct ScannerVibrato {
    data: [f32; DELAY_CAPACITY],
    write: usize,
    phase: f32,
    sample_rate: f32,
    mode: ScannerMode,
}

impl ScannerVibrato {
    pub const fn new(sample_rate: f32) -> Self {
        Self {
            data: [0.0; DELAY_CAPACITY],
            write: 0,
            phase: 0.0,
            sample_rate,
            mode: ScannerMode::Off,
        }
    }

    pub fn set_mode(&mut self, mode: ScannerMode) {
        self.mode = mode;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.data[self.write] = input;
        self.write = (self.write + 1) % DELAY_CAPACITY;

        // The physical scanner rotates at approximately 6.9 Hz and bridges
        // adjacent stator contacts. A parabolic bridge avoids switching edges.
        self.phase += 6.9 * TAP_COUNT as f32 / self.sample_rate;
        if self.phase >= TAP_COUNT as f32 {
            self.phase -= TAP_COUNT as f32;
        }

        if self.mode == ScannerMode::Off {
            return input;
        }
        let tap = self.phase as usize;
        let fraction = self.phase - tap as f32;
        let blend = fraction * fraction * (3.0 - 2.0 * fraction);
        let depth = self.mode.depth();
        let maximum_delay = self.sample_rate * 0.00165 * depth;
        let first = self.tap(tap, maximum_delay);
        let second = self.tap((tap + 1) % TAP_COUNT, maximum_delay);
        let scanned = first + blend * (second - first);
        if self.mode.chorus() {
            0.5 * (input + scanned)
        } else {
            scanned
        }
    }

    pub fn reset(&mut self) {
        self.data.fill(0.0);
        self.write = 0;
    }

    fn tap(&self, tap: usize, maximum_delay: f32) -> f32 {
        let position = TAP_POSITIONS[tap];
        let delay = (2.0 + maximum_delay * position).clamp(1.0, (DELAY_CAPACITY - 2) as f32);
        let whole = delay as usize;
        let fraction = delay - whole as f32;
        let newer = (self.write + DELAY_CAPACITY - whole - 1) % DELAY_CAPACITY;
        let older = (newer + DELAY_CAPACITY - 1) % DELAY_CAPACITY;
        self.data[newer] * (1.0 - fraction) + self.data[older] * fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_is_exact_while_the_line_keeps_running() {
        let mut scanner = ScannerVibrato::new(48_000.0);
        for index in 0..128 {
            let input = index as f32 / 128.0;
            assert_eq!(scanner.process(input), input);
        }
    }

    #[test]
    fn chorus_retains_a_direct_component() {
        let mut vibrato = ScannerVibrato::new(48_000.0);
        let mut chorus = ScannerVibrato::new(48_000.0);
        vibrato.set_mode(ScannerMode::Vibrato3);
        chorus.set_mode(ScannerMode::Chorus3);
        let mut difference = 0.0;
        for index in 0..4096 {
            let input = if index == 0 { 1.0 } else { 0.0 };
            difference += (vibrato.process(input) - chorus.process(input)).abs();
        }
        assert!(difference > 0.1);
    }
}
