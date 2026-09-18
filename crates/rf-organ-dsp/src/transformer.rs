// SPDX-License-Identifier: GPL-2.0-or-later

/// How lopsided a magnetised core makes its own curve, before the control
/// that scales it.
///
/// Provisional: what a real core does depends on its material and its
/// history, and neither is published for these. This value puts the
/// second-order difference product about eleven decibels under the third at
/// the baseline character, which is a core whose symmetric saturation still
/// leads and whose lean is a perturbation on it. A bench that measures both
/// products on a real transformer contradicts or confirms exactly that
/// spacing.
pub const ASYMMETRY_DEFAULT: f32 = 0.10;

/// Reduced magnetic model shared by the AO-28 T1/T2 input matching
/// transformers and the T3 output transformer. Each instance owns its own
/// magnetization state and its own calibration.
pub struct MatchingTransformer {
    sample_rate: f32,
    drive: f32,
    hysteresis: f32,
    asymmetry: f32,
    magnetization: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformerDiagnostics {
    pub drive: f32,
    pub hysteresis: f32,
    pub magnetization: f32,
}

impl MatchingTransformer {
    pub const fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            drive: 0.35,
            hysteresis: 0.25,
            asymmetry: ASYMMETRY_DEFAULT,
            magnetization: 0.0,
        }
    }

    /// How lopsided a magnetised core makes the saturation, from an odd
    /// curve at zero to the whole of what is modelled at one. An odd curve
    /// cannot produce a second-order difference product; a real transformer
    /// does, so this exists and the laboratory reports what it comes to.
    pub fn set_asymmetry(&mut self, amount: f32) -> bool {
        if !unit(amount) {
            return false;
        }
        self.asymmetry = amount;
        true
    }

    pub fn set(&mut self, drive: f32, hysteresis: f32) -> bool {
        if !unit(drive) || !unit(hysteresis) {
            return false;
        }
        self.drive = drive;
        self.hysteresis = hysteresis;
        true
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // Provisional reduced model: a slow magnetic state biases a smooth
        // saturation. It is shared by all notes and therefore also produces
        // the expected inter-note interaction.
        let memory_rate = (900.0 / self.sample_rate).clamp(0.001, 0.1);
        self.magnetization += memory_rate * (input - self.magnetization);
        let driven =
            input * (1.0 + 5.0 * self.drive) + self.magnetization * (0.8 * self.hysteresis);
        // A core does not sit at the middle of its own curve: it keeps some of
        // the magnetisation it has been given, and the two halves of a wave
        // then saturate differently. The lean is a property of the core rather
        // than of the signal in it - the first attempt here scaled it by the
        // magnetisation state, which above that state's own corner is nearly
        // nothing, so it changed the measured second-order product by four
        // hundredths of a decibel and was no model at all. How far a real core
        // leans is provisional; that an odd curve has no even harmonics to
        // give is not.
        let bend = 0.55 + 0.35 * self.hysteresis;
        let lean = self.asymmetry * bend;
        let curve = if driven >= 0.0 {
            bend + lean
        } else {
            bend - lean
        };
        let saturated = driven / (1.0 + driven.abs() * curve);
        let clean_gain = 1.0 / (1.0 + 5.0 * self.drive);
        input * (1.0 - self.drive) + saturated * clean_gain * self.drive
    }

    pub fn reset(&mut self) {
        self.magnetization = 0.0;
    }

    pub const fn diagnostics(&self) -> TransformerDiagnostics {
        TransformerDiagnostics {
            drive: self.drive,
            hysteresis: self.hysteresis,
            magnetization: self.magnetization,
        }
    }
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

/// The three AO-28 transformers modelled by RF-Organ. They share one musical
/// character control but keep independent calibration and magnetic state:
/// T1 carries the lower manual plus pedals, T2 the upper manual, and T3 the
/// output stage after V3B.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformerUnit {
    T1,
    T2,
    T3,
}

impl TransformerUnit {
    pub const ALL: [Self; 3] = [Self::T1, Self::T2, Self::T3];

    pub const fn label(self) -> &'static str {
        match self {
            Self::T1 => "t1",
            Self::T2 => "t2",
            Self::T3 => "t3",
        }
    }

    /// Stable slot for per-unit calibration tables.
    pub const fn index(self) -> usize {
        match self {
            Self::T1 => 0,
            Self::T2 => 1,
            Self::T3 => 2,
        }
    }
}
