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

/// Level the upper drawbars lose while percussion is on at Normal volume.
/// Hammond documents the vintage console as reducing them by about 6 dB, and
/// as leaving them alone at Soft.
const NORMAL_DRAWBAR_ATTENUATION: f32 = 0.501_187_2;
/// One-pole rise of the percussion envelope. The console charges its
/// percussion capacitor through the key contact rather than switching a
/// level, so the attack is fast but not instantaneous. Provisional.
const ATTACK_SECONDS: f32 = 0.0015;
/// Time constant with which the percussion recovers once every key is up.
/// A console played fast and detached does not deliver full percussion on
/// every note; this is where that comes from. Provisional.
const RECOVERY_SECONDS: f32 = 0.12;

pub struct Percussion {
    sample_rate: f32,
    enabled: bool,
    harmonic: PercussionHarmonic,
    volume: PercussionVolume,
    decay: PercussionDecay,
    /// Peak this strike was triggered at, after the available charge.
    peak: f32,
    /// Rising gate, and the falling one it multiplies.
    attack: f32,
    envelope: f32,
    /// How much of the percussion supply is available, 0 to 1.
    charge: f32,
}

impl Percussion {
    pub const fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            enabled: false,
            harmonic: PercussionHarmonic::Second,
            volume: PercussionVolume::Normal,
            decay: PercussionDecay::Fast,
            peak: 0.0,
            attack: 0.0,
            envelope: 0.0,
            charge: 1.0,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.envelope = 0.0;
            self.attack = 0.0;
            self.peak = 0.0;
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

    /// Level the upper manual keeps while this tablet is set. Percussion at
    /// Normal volume takes about 6 dB out of the drawbars; Soft does not.
    pub const fn drawbar_attenuation(&self) -> f32 {
        match (self.enabled, self.volume) {
            (true, PercussionVolume::Normal) => NORMAL_DRAWBAR_ATTENUATION,
            _ => 1.0,
        }
    }

    /// The console percussion is single-trigger: a new envelope starts only
    /// after every key has first been released, and only with whatever charge
    /// has recovered since.
    pub fn trigger(&mut self) {
        if self.enabled {
            let level = match self.volume {
                PercussionVolume::Soft => 0.34,
                PercussionVolume::Normal => 0.72,
            };
            self.peak = level * self.charge;
            self.charge = 0.0;
            self.attack = 0.0;
            self.envelope = 1.0;
        }
    }

    /// `keys_held` reports whether any upper-manual key is down. The supply
    /// only recovers once they are all up.
    pub fn process(&mut self, harmonic_signal: f32, keys_held: bool) -> f32 {
        let output = harmonic_signal * self.peak * self.attack * self.envelope;
        let attack_rate = (1.0 / (ATTACK_SECONDS * self.sample_rate)).min(1.0);
        self.attack += attack_rate * (1.0 - self.attack);
        let t60 = match self.decay {
            PercussionDecay::Slow => 1.25,
            PercussionDecay::Fast => 0.28,
        };
        let decrement = (6.907_755 / (t60 * self.sample_rate)).min(1.0);
        self.envelope *= 1.0 - decrement;
        if !keys_held {
            let recovery = (1.0 / (RECOVERY_SECONDS * self.sample_rate)).min(1.0);
            self.charge += recovery * (1.0 - self.charge);
        }
        output
    }

    pub fn reset(&mut self) {
        self.peak = 0.0;
        self.attack = 0.0;
        self.envelope = 0.0;
        self.charge = 1.0;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    /// Peak of one strike, and the sample it arrives at.
    fn strike(percussion: &mut Percussion, frames: usize) -> (f32, usize) {
        let mut peak = 0.0;
        let mut at = 0;
        for frame in 0..frames {
            let sample = percussion.process(1.0, true);
            if sample > peak {
                peak = sample;
                at = frame;
            }
        }
        (peak, at)
    }

    #[test]
    fn fast_envelope_reaches_minus_sixty_db_near_its_nominal_time() {
        let mut percussion = Percussion::new(48_000.0);
        percussion.set_enabled(true);
        percussion.trigger();
        let (peak, _) = strike(&mut percussion, 240);
        let mut level = 0.0;
        for _ in 240..13_440 {
            level = percussion.process(1.0, true);
        }
        // The envelope starts decaying from the trigger, so the attack costs a
        // little of the nominal T60 window.
        assert!((level / peak).abs() < 0.0015, "{}", level / peak);
    }

    /// The envelope rises through the contact instead of stepping.
    #[test]
    fn the_attack_is_fast_but_not_instantaneous() {
        let mut percussion = Percussion::new(48_000.0);
        percussion.set_enabled(true);
        percussion.trigger();
        let first = percussion.process(1.0, true);
        assert_eq!(first, 0.0);
        let (peak, at) = strike(&mut percussion, 480);
        assert!(peak > 0.5);
        let seconds = at as f32 / 48_000.0;
        assert!((0.001..0.010).contains(&seconds), "peak at {seconds} s");
    }

    /// Re-keying before the supply recovers gives a weaker strike, and holding
    /// keys down keeps it from recovering at all.
    #[test]
    fn percussion_recovers_only_once_every_key_is_up() {
        let mut percussion = Percussion::new(48_000.0);
        percussion.set_enabled(true);
        percussion.trigger();
        let (first, _) = strike(&mut percussion, 4_800);

        percussion.trigger();
        let (immediate, _) = strike(&mut percussion, 480);
        assert!(immediate < first * 0.1, "{immediate} against {first}");

        // Half a second of keys up is most of the way back; a second is all
        // of it.
        for _ in 0..24_000 {
            percussion.process(0.0, false);
        }
        percussion.trigger();
        let (half_second, _) = strike(&mut percussion, 4_800);
        assert!(
            (0.9 * first..first).contains(&half_second),
            "{half_second} against {first}"
        );

        for _ in 0..48_000 {
            percussion.process(0.0, false);
        }
        percussion.trigger();
        let (recovered, _) = strike(&mut percussion, 4_800);
        assert!(recovered > first * 0.99, "{recovered} against {first}");
    }

    #[test]
    fn only_normal_volume_takes_level_out_of_the_drawbars() {
        let mut percussion = Percussion::new(48_000.0);
        assert_eq!(percussion.drawbar_attenuation(), 1.0);
        percussion.set_enabled(true);
        let decibels = 20.0 * percussion.drawbar_attenuation().log10();
        assert!((decibels + 6.0).abs() < 0.05, "{decibels} dB");
        percussion.set_volume(PercussionVolume::Soft);
        assert_eq!(percussion.drawbar_attenuation(), 1.0);
    }
}
