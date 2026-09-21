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

/// How much of the bend the lean may take before the weak half of the curve
/// would stop bending at all. See `process`.
const LEAN_CEILING: f32 = 0.9;

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
        // The lean is a perturbation on the bend and may not cancel it. Left
        // unbounded it could: `asymmetry` reaches one from the host, and
        // there the weak half's curve came out at exactly zero, its divisor
        // at one, and the transformer stopped saturating on that half
        // altogether -- a linear gain that hands back more than it is given.
        // No core does that; a magnetised one saturates unevenly on both
        // halves, not on one. The ceiling only binds past 0.9, so every
        // setting anyone has used, the default among them, is untouched.
        let lean = (self.asymmetry * bend).min(bend * LEAN_CEILING);
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

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn run(transformer: &mut MatchingTransformer, input: f32, samples: usize) -> f32 {
        let mut last = 0.0;
        for _ in 0..samples {
            last = transformer.process(input);
        }
        last
    }

    /// With no drive the transformer is out of the circuit, exactly.
    ///
    /// `process` blends a clean path against a saturated one by the drive,
    /// so at zero the saturated half is multiplied away and what comes out
    /// is what went in -- not nearly, but bit for bit. Worth pinning
    /// because it is the property that lets the control be turned off.
    #[test]
    fn no_drive_is_an_exact_bypass() {
        let mut transformer = MatchingTransformer::new(RATE);
        assert!(transformer.set(0.0, 0.25));
        for step in -400..=400 {
            let input = step as f32 / 100.0;
            assert_eq!(transformer.process(input).to_bits(), input.to_bits());
        }
    }

    /// An odd curve has no second harmonic to give.
    ///
    /// The module says exactly this about `asymmetry`, and it is the reason
    /// the field exists: a core that leans produces a second-order
    /// difference product and a symmetric one cannot. At zero the curve
    /// must therefore be odd -- feed it the negative and the output is the
    /// negative -- and the magnetisation has to be cleared between the two
    /// halves or its memory, not the curve, decides the answer.
    #[test]
    fn a_core_that_does_not_lean_has_an_odd_curve() {
        let mut transformer = MatchingTransformer::new(RATE);
        assert!(transformer.set(0.8, 0.25));
        assert!(transformer.set_asymmetry(0.0));
        for step in 1..=200 {
            let input = step as f32 / 50.0;
            transformer.reset();
            let positive = run(&mut transformer, input, 64);
            transformer.reset();
            let negative = run(&mut transformer, -input, 64);
            assert!(
                (positive + negative).abs() <= 1.0e-6,
                "at {input}: {positive} against {negative}"
            );
        }
    }

    /// And a core that leans does not.
    ///
    /// The converse, so that the test above cannot be satisfied by a curve
    /// that ignores the control altogether.
    #[test]
    fn a_leaning_core_bends_its_two_halves_differently() {
        let mut transformer = MatchingTransformer::new(RATE);
        assert!(transformer.set(0.8, 0.25));
        assert!(transformer.set_asymmetry(ASYMMETRY_DEFAULT));
        transformer.reset();
        let positive = run(&mut transformer, 3.0, 64);
        transformer.reset();
        let negative = run(&mut transformer, -3.0, 64);
        assert!(
            (positive + negative).abs() > 1.0e-3,
            "the lean did nothing: {positive} against {negative}"
        );
    }

    /// The magnetisation is a memory, and `reset` is what forgets it.
    ///
    /// It follows the input through a one-pole at roughly 900 Hz, so it
    /// lags a step rather than following it, and two runs from a cleared
    /// core must agree. A transformer that carried its state across a reset
    /// would make every capture depend on the one before it.
    #[test]
    fn the_core_remembers_until_it_is_reset() {
        let mut transformer = MatchingTransformer::new(RATE);
        assert!(transformer.set(0.5, 0.5));

        assert_eq!(transformer.diagnostics().magnetization, 0.0);
        let _ = transformer.process(1.0);
        let after_one = transformer.diagnostics().magnetization;
        assert!(after_one > 0.0 && after_one < 1.0, "{after_one}");

        let settled = {
            let _ = run(&mut transformer, 1.0, 4_800);
            transformer.diagnostics().magnetization
        };
        assert!(settled > after_one, "it never charged: {settled}");

        transformer.reset();
        assert_eq!(transformer.diagnostics().magnetization, 0.0);
        let again = transformer.process(1.0);
        transformer.reset();
        let once_more = transformer.process(1.0);
        assert_eq!(again.to_bits(), once_more.to_bits());
    }

    /// However hard it is driven, what comes out is bounded.
    ///
    /// That is what saturation means, and it is the property the core is
    /// there for: a console drive can be wound a long way up and the
    /// transformer is downstream of it.
    ///
    /// Each input starts from a cleared core, because the bound is on the
    /// curve and not on the memory. With history the output can exceed the
    /// instantaneous input quite legitimately -- the magnetisation carries
    /// what came before, which is the whole point of modelling it -- so a
    /// test that walked a ramp and compared against the sample in hand
    /// would be asserting something untrue about a hysteresis model. It
    /// was, and this is the repair.
    ///
    /// It is also what found `LEAN_CEILING`. At an asymmetry of one, which
    /// the host can ask for, the weak half's curve reached exactly zero,
    /// its divisor reached one, and the output grew with the input without
    /// any bound at all.
    #[test]
    fn the_output_stays_bounded_however_hard_it_is_driven() {
        let mut transformer = MatchingTransformer::new(RATE);
        assert!(transformer.set(1.0, 1.0));
        assert!(transformer.set_asymmetry(1.0));
        for step in -20..=20 {
            let input = step as f32 * 500.0;
            transformer.reset();
            let output = transformer.process(input);
            assert!(output.is_finite(), "at {input}: {output}");
            assert!(
                output.abs() <= 10.0,
                "at {input}: {output} -- the curve stopped saturating"
            );
        }
    }

    /// The controls refuse what they cannot mean.
    #[test]
    fn the_controls_take_only_a_unit() {
        let mut transformer = MatchingTransformer::new(RATE);
        assert!(transformer.set(0.0, 0.0));
        assert!(transformer.set(1.0, 1.0));
        assert!(!transformer.set(-0.1, 0.5));
        assert!(!transformer.set(0.5, 1.1));
        assert!(!transformer.set(f32::NAN, 0.5));
        assert!(transformer.set_asymmetry(0.0));
        assert!(transformer.set_asymmetry(1.0));
        assert!(!transformer.set_asymmetry(-0.01));
        assert!(!transformer.set_asymmetry(f32::INFINITY));
    }

    /// The three units are distinct slots and stay in their own rows.
    #[test]
    fn each_unit_keeps_its_own_slot() {
        let mut seen = [false; 3];
        for unit in TransformerUnit::ALL {
            assert!(!seen[unit.index()], "{} repeats a slot", unit.label());
            seen[unit.index()] = true;
        }
        assert!(seen.iter().all(|taken| *taken));
    }
}
