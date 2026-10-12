// SPDX-License-Identifier: GPL-2.0-or-later
//! Which of the model's unmeasured constants the sound actually depends on.
//!
//! Every knob is moved to each end of the span the provenance table gives it,
//! one at a time, and the calibration phrases are rendered again. What comes
//! back is how far each render moved from the shipped one, in decibels: of
//! level, of spectral shape in third-octave bands, and of envelope in ten
//! millisecond windows. A decibel is roughly where a careful listener starts
//! to hear a level change, so the figures read as "about that many steps a
//! listener could notice", and no more than that: this is a ranking of where
//! to look first, not a model of hearing.
//!
//! Two things it cannot see. A constant no phrase reaches moves nothing, and
//! is reported as not exercised rather than as harmless. And what changes
//! pitch or phase rather than level - the drive's wow and flutter, a rotor's
//! Doppler - barely registers in these three measures.

use crate::captures::{PHRASE_SECONDS, SAMPLE_RATE};
use crate::signal::{decibels, rms, welch_power};
use crate::{Scenario, render_phrase_with};
use rf_organ_dsp::provenance::{Basis, Span, find};
use rf_organ_dsp::{Knob, Model};
use std::fmt::Write as _;
use std::thread;

/// The phrases a knob is judged by: the generator and keying alone, then
/// each of the stages that can be switched into the path.
const SCENARIOS: [Scenario; 6] = [
    Scenario::Direct,
    Scenario::Percussion,
    Scenario::Scanner,
    Scenario::Chorale,
    Scenario::Tremolo,
    Scenario::PedalBoth,
];

/// Third-octave centres from about 31.5 Hz to 16 kHz.
const BANDS: usize = 28;
const FFT_SIZE: usize = 8192;
/// Envelope windows, and how far under the loudest one a window may sit and
/// still be compared. Below that the phrase is silence and its decibels are
/// noise.
const WINDOW_SECONDS: f64 = 0.01;
const ENVELOPE_FLOOR_DB: f64 = 50.0;
/// Bands this far under the phrase's level hold too little to compare.
const BAND_FLOOR_DB: f64 = 50.0;
/// No single window or band is allowed to say more than this, so that one
/// note ending a few milliseconds sooner cannot rank a knob on its own.
const CAP_DB: f64 = 20.0;

/// What one channel of one render is reduced to.
struct Channel {
    level_db: f64,
    bands_db: [f64; BANDS],
    envelope_db: Vec<f64>,
}

/// The largest difference one render showed from the shipped one, and where.
#[derive(Clone, Copy, Debug)]
pub struct Finding {
    pub decibels: f64,
    pub scenario: &'static str,
    pub metric: &'static str,
}

pub struct Row {
    pub knob: Knob,
    pub basis: Basis,
    pub span: Span,
    pub baseline: f32,
    pub low: f32,
    pub high: f32,
    pub at_low: Finding,
    pub at_high: Finding,
}

impl Row {
    pub fn worst(&self) -> f64 {
        self.at_low.decibels.max(self.at_high.decibels)
    }
}

pub fn survey() -> Result<Vec<Row>, String> {
    // An engine is too large for a spawned thread's default stack.
    thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(survey_inner)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "the survey panicked".to_owned())?
}

fn survey_inner() -> Result<Vec<Row>, String> {
    let baseline = signatures(&Model::BASELINE)?;
    let workers = thread::available_parallelism().map_or(4, usize::from);
    let chunks = Knob::ALL.chunks(Knob::ALL.len().div_ceil(workers));
    let rows = thread::scope(|scope| {
        let handles = chunks
            .map(|knobs| {
                let baseline = &baseline;
                thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn_scoped(scope, move || {
                        knobs
                            .iter()
                            .map(|knob| survey_knob(*knob, baseline))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut rows = Vec::new();
        for handle in handles {
            rows.extend(
                handle
                    .join()
                    .map_err(|_| "a survey worker panicked".to_owned())??,
            );
        }
        Ok::<_, String>(rows)
    })?;
    let mut rows = rows;
    rows.sort_by(|left, right| right.worst().total_cmp(&left.worst()));
    Ok(rows)
}

fn survey_knob(knob: Knob, baseline: &[[Channel; 2]]) -> Result<Row, String> {
    let constant = find(knob.name()).ok_or_else(|| format!("{} is not registered", knob.name()))?;
    let (low, high) = constant
        .span
        .bounds()
        .ok_or_else(|| format!("{} has no span to survey", knob.name()))?;
    let (low, high) = (low as f32, high as f32);
    let at = |value: f32| -> Result<Finding, String> {
        let moved = signatures(&Model::BASELINE.with(knob, value))?;
        Ok(difference(baseline, &moved))
    };
    Ok(Row {
        knob,
        basis: constant.basis,
        span: constant.span,
        baseline: Model::BASELINE[knob],
        low,
        high,
        at_low: at(low)?,
        at_high: at(high)?,
    })
}

fn signatures(model: &Model) -> Result<Vec<[Channel; 2]>, String> {
    SCENARIOS
        .iter()
        .map(|scenario| {
            let stereo = render_phrase_with(*scenario, model).map_err(|error| error.to_string())?;
            Ok([channel(&stereo, 0), channel(&stereo, 1)])
        })
        .collect()
}

fn channel(stereo: &[f32], side: usize) -> Channel {
    let rate = f64::from(SAMPLE_RATE);
    let samples = stereo
        .iter()
        .skip(side)
        .step_by(2)
        .map(|sample| f64::from(*sample))
        .collect::<Vec<_>>();
    // The chord sounds from a quarter second to three; the held part leaves
    // the attack and the release to the envelope.
    let held = &samples[(0.30 * rate) as usize..(2.95 * rate) as usize];
    let level_db = decibels(rms(held));
    let power = welch_power(held, FFT_SIZE);
    let mut bands_db = [0.0; BANDS];
    for (band, slot) in bands_db.iter_mut().enumerate() {
        let centre = 10.0_f64.powf(0.1 * (15 + band) as f64);
        let (from, to) = (
            centre / 2.0_f64.powf(1.0 / 6.0),
            centre * 2.0_f64.powf(1.0 / 6.0),
        );
        let bin = |frequency: f64| (frequency * FFT_SIZE as f64 / rate).round() as usize;
        let energy = power[bin(from)..bin(to).max(bin(from) + 1)]
            .iter()
            .sum::<f64>();
        *slot = 10.0 * energy.max(1.0e-30).log10();
    }
    let window = (WINDOW_SECONDS * rate) as usize;
    let envelope_db = samples
        .chunks(window)
        .take(PHRASE_SECONDS * SAMPLE_RATE as usize / window)
        .map(|chunk| decibels(rms(chunk)))
        .collect();
    Channel {
        level_db,
        bands_db,
        envelope_db,
    }
}

fn difference(baseline: &[[Channel; 2]], moved: &[[Channel; 2]]) -> Finding {
    let mut worst = Finding {
        decibels: 0.0,
        scenario: "",
        metric: "",
    };
    for ((scenario, base), moved) in SCENARIOS.iter().zip(baseline).zip(moved) {
        for (base, moved) in base.iter().zip(moved) {
            let level = (moved.level_db - base.level_db).abs().min(CAP_DB);
            let spectrum = shape_difference(base, moved);
            let envelope = envelope_difference(base, moved);
            for (decibels, metric) in [
                (level, "level"),
                (spectrum, "spectrum"),
                (envelope, "envelope"),
            ] {
                if decibels.is_nan() || decibels > worst.decibels {
                    worst = Finding {
                        decibels: if decibels.is_nan() { CAP_DB } else { decibels },
                        scenario: scenario.id(),
                        metric,
                    };
                }
            }
        }
    }
    worst
}

/// Root mean square, over the bands that carry anything, of how far each
/// band moved once the overall level is taken out.
fn shape_difference(base: &Channel, moved: &Channel) -> f64 {
    let loudest = base.bands_db.iter().copied().fold(f64::MIN, f64::max);
    let base_level = 10.0
        * base
            .bands_db
            .iter()
            .map(|db| 10.0_f64.powf(db / 10.0))
            .sum::<f64>()
            .log10();
    let moved_level = 10.0
        * moved
            .bands_db
            .iter()
            .map(|db| 10.0_f64.powf(db / 10.0))
            .sum::<f64>()
            .log10();
    let differences = base
        .bands_db
        .iter()
        .zip(&moved.bands_db)
        .filter(|(base, _)| **base > loudest - BAND_FLOOR_DB)
        .map(|(base, moved)| {
            ((moved - moved_level) - (base - base_level))
                .abs()
                .min(CAP_DB)
        })
        .collect::<Vec<_>>();
    root_mean_square(&differences)
}

fn envelope_difference(base: &Channel, moved: &Channel) -> f64 {
    let loudest = base.envelope_db.iter().copied().fold(f64::MIN, f64::max);
    let differences = base
        .envelope_db
        .iter()
        .zip(&moved.envelope_db)
        .filter(|(base, _)| **base > loudest - ENVELOPE_FLOOR_DB)
        .map(|(base, moved)| (moved - base).abs().min(CAP_DB))
        .collect::<Vec<_>>();
    root_mean_square(&differences)
}

fn root_mean_square(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt()
}

pub fn report(rows: &[Row]) -> String {
    let mut csv = String::from(
        "rank,constant,basis,span,baseline,low,high,low_db,low_scenario,low_metric,high_db,high_scenario,high_metric,worst_db,exercised\n",
    );
    for (rank, row) in rows.iter().enumerate() {
        let span = match row.span {
            Span::Exact => "exact",
            Span::Sourced(..) => "sourced",
            Span::Assumed(..) => "assumed",
        };
        let _ = writeln!(
            csv,
            "{},{},{},{span},{},{},{},{:.2},{},{},{:.2},{},{},{:.2},{}",
            rank + 1,
            row.knob.name(),
            row.basis.name(),
            row.baseline,
            row.low,
            row.high,
            row.at_low.decibels,
            row.at_low.scenario,
            row.at_low.metric,
            row.at_high.decibels,
            row.at_high.scenario,
            row.at_high.metric,
            row.worst(),
            // Exactly nothing moved: no phrase reaches this constant, which
            // says the phrases are incomplete, not that it does not matter.
            if row.worst() > 0.0 { "yes" } else { "no" }
        );
    }
    csv
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A channel compared with itself moved nothing, and one played louder
    /// moved its level and nothing else.
    #[test]
    fn differences_measure_what_moved_and_nothing_else() {
        let tone = (0..PHRASE_SECONDS * SAMPLE_RATE as usize)
            .flat_map(|index| {
                let value =
                    (std::f32::consts::TAU * 440.0 * index as f32 / SAMPLE_RATE as f32).sin() * 0.1;
                [value, value]
            })
            .collect::<Vec<_>>();
        let louder = tone.iter().map(|sample| sample * 2.0).collect::<Vec<_>>();
        let base = [[channel(&tone, 0), channel(&tone, 1)]];
        let same = [[channel(&tone, 0), channel(&tone, 1)]];
        let moved = [[channel(&louder, 0), channel(&louder, 1)]];
        assert_eq!(difference(&base, &same).decibels, 0.0);

        let finding = difference(&base, &moved);
        assert!((finding.decibels - 6.02).abs() < 0.01, "{finding:?}");
        assert!(matches!(finding.metric, "level" | "envelope"));
        assert!(shape_difference(&base[0][0], &moved[0][0]) < 1.0e-9);
    }
}
