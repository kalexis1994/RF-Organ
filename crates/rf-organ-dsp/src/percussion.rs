// SPDX-License-Identifier: GPL-2.0-or-later

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PercussionHarmonic {
    #[default]
    Second = 0,
    Third = 1,
}

impl PercussionHarmonic {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Second),
            1 => Some(Self::Third),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PercussionVolume {
    Soft = 0,
    #[default]
    Normal = 1,
}

impl PercussionVolume {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Soft),
            1 => Some(Self::Normal),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PercussionDecay {
    Slow = 0,
    #[default]
    Fast = 1,
}

impl PercussionDecay {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Slow),
            1 => Some(Self::Fast),
            _ => None,
        }
    }
}

pub struct Percussion {
    sample_rate: f32,
    enabled: bool,
    harmonic: PercussionHarmonic,
    volume: PercussionVolume,
    decay: PercussionDecay,
    envelope: f32,
}

impl Percussion {
    pub const fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            enabled: false,
            harmonic: PercussionHarmonic::Second,
            volume: PercussionVolume::Normal,
            decay: PercussionDecay::Fast,
            envelope: 0.0,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.envelope = 0.0;
        }
    }

    pub fn set_harmonic(&mut self, harmonic: PercussionHarmonic) {
        self.harmonic = harmonic;
    }

    pub fn set_volume(&mut self, volume: PercussionVolume) {
        self.volume = volume;
    }

    pub fn set_decay(&mut self, decay: PercussionDecay) {
        self.decay = decay;
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn harmonic(&self) -> PercussionHarmonic {
        self.harmonic
    }

    /// The console percussion is single-trigger: a new envelope starts only
    /// after every key has first been released.
    pub fn trigger(&mut self) {
        if self.enabled {
            self.envelope = match self.volume {
                PercussionVolume::Soft => 0.34,
                PercussionVolume::Normal => 0.72,
            };
        }
    }

    pub fn process(&mut self, harmonic_signal: f32) -> f32 {
        let output = harmonic_signal * self.envelope;
        let t60 = match self.decay {
            PercussionDecay::Slow => 1.25,
            PercussionDecay::Fast => 0.28,
        };
        let decrement = (6.907_755 / (t60 * self.sample_rate)).min(1.0);
        self.envelope *= 1.0 - decrement;
        output
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_envelope_reaches_minus_sixty_db_near_its_nominal_time() {
        let mut percussion = Percussion::new(48_000.0);
        percussion.set_enabled(true);
        percussion.trigger();
        let initial = percussion.process(1.0);
        let mut final_level = initial;
        for _ in 1..13_440 {
            final_level = percussion.process(1.0);
        }
        assert!((final_level / initial - 0.001).abs() < 0.0001);
    }
}
