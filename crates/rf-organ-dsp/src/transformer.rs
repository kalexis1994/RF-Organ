// SPDX-License-Identifier: GPL-2.0-or-later

/// Reduced magnetic model shared by the AO-28 T1/T2 input matching
/// transformers and the T3 output transformer. Each instance owns its own
/// magnetization state and its own calibration.
pub struct MatchingTransformer {
    sample_rate: f32,
    drive: f32,
    hysteresis: f32,
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
            magnetization: 0.0,
        }
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
        // Provisional reduced model: a slow magnetic state biases a smooth,
        // asymmetric saturation. It is shared by all notes and therefore also
        // produces the expected inter-note interaction.
        let memory_rate = (900.0 / self.sample_rate).clamp(0.001, 0.1);
        self.magnetization += memory_rate * (input - self.magnetization);
        let driven =
            input * (1.0 + 5.0 * self.drive) + self.magnetization * (0.8 * self.hysteresis);
        let positive = 1.0 + driven.abs() * (0.55 + 0.35 * self.hysteresis);
        let saturated = driven / positive;
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
