// SPDX-License-Identifier: GPL-2.0-or-later

use crate::wav::{self, Audio};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const SAMPLE_RATE: u32 = 48_000;
const CAPTURES: [&str; 7] = [
    "01-direct-888",
    "02-third-percussion",
    "03-scanner-c3",
    "04-leslie-chorale",
    "05-leslie-tremolo",
    "06-full-console",
    "leslie-cabinet-impulse",
];

#[derive(Clone, Copy, Debug)]
struct Comparison {
    common_seconds: f64,
    onset_delta_ms: f64,
    model_rms_dbfs: f64,
    reference_rms_dbfs: f64,
    level_delta_db: f64,
    peak_delta_db: f64,
    crest_delta_db: f64,
    envelope_correlation: f64,
    model_stereo_width_db: f64,
    reference_stereo_width_db: f64,
    stereo_width_delta_db: f64,
}

pub fn compare_directories(model: &Path, reference: &Path) -> Result<String, String> {
    validate_reference_directory(reference)?;
    let mut report = String::from(
        "capture,common_seconds,reference_minus_model_onset_ms,model_rms_dbfs,reference_rms_dbfs,model_minus_reference_level_db,model_minus_reference_peak_db,model_minus_reference_crest_db,envelope_correlation,model_stereo_width_db,reference_stereo_width_db,model_minus_reference_stereo_width_db\n",
    );
    let mut compared = 0;
    for capture in CAPTURES {
        let reference_path = reference.join(format!("{capture}.wav"));
        if !reference_path.is_file() {
            continue;
        }
        let model_path = model.join(format!("{capture}.wav"));
        if !model_path.is_file() {
            return Err(format!(
                "model capture is missing: {}",
                model_path.display()
            ));
        }
        let model_audio = read_audio(&model_path)?;
        let reference_audio = read_audio(&reference_path)?;
        let result = compare_audio(&model_audio, &reference_audio)?;
        writeln!(
            &mut report,
            "{capture},{:.9},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.9},{:.6},{:.6},{:.6}",
            result.common_seconds,
            result.onset_delta_ms,
            result.model_rms_dbfs,
            result.reference_rms_dbfs,
            result.level_delta_db,
            result.peak_delta_db,
            result.crest_delta_db,
            result.envelope_correlation,
            result.model_stereo_width_db,
            result.reference_stereo_width_db,
            result.stereo_width_delta_db,
        )
        .expect("string write cannot fail");
        compared += 1;
    }
    if compared == 0 {
        return Err(format!(
            "no named reference captures were found in {}",
            reference.display()
        ));
    }
    Ok(report)
}

fn validate_reference_directory(reference: &Path) -> Result<(), String> {
    let external = reference.join("reference-manifest.txt");
    if external.is_file() {
        let text = fs::read_to_string(&external)
            .map_err(|error| format!("reading {}: {error}", external.display()))?;
        return validate_reference_manifest(&text);
    }
    let generated = reference.join("manifest.txt");
    if generated.is_file() {
        let text = fs::read_to_string(&generated)
            .map_err(|error| format!("reading {}: {error}", generated.display()))?;
        if text.starts_with("RF-Organ deterministic calibration suite\n") {
            return Ok(());
        }
    }
    Err(format!(
        "reference directory needs reference-manifest.txt: {}",
        reference.display()
    ))
}

fn validate_reference_manifest(text: &str) -> Result<(), String> {
    if !text.starts_with("RF-Organ reference capture\n") {
        return Err("reference manifest has an invalid header".into());
    }
    let fields = text
        .lines()
        .skip(1)
        .filter_map(|line| {
            let line = line.trim();
            (!line.is_empty() && !line.starts_with('#'))
                .then(|| line.split_once('='))
                .flatten()
                .map(|(key, value)| (key.trim(), value.trim()))
        })
        .collect::<BTreeMap<_, _>>();
    for required in ["capture_id", "source", "rights", "console", "signal_chain"] {
        if fields.get(required).is_none_or(|value| value.is_empty()) {
            return Err(format!("reference manifest needs a non-empty {required}"));
        }
    }
    if fields.get("sample_rate") != Some(&"48000") {
        return Err("reference manifest sample_rate must be 48000".into());
    }
    if fields.get("normalization") != Some(&"none") {
        return Err("reference manifest normalization must be none".into());
    }
    Ok(())
}

fn read_audio(path: &Path) -> Result<Audio, String> {
    let bytes = fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
    wav::decode(&bytes).map_err(|error| format!("decoding {}: {error}", path.display()))
}

fn compare_audio(model: &Audio, reference: &Audio) -> Result<Comparison, String> {
    if model.sample_rate != SAMPLE_RATE || reference.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "captures must be {SAMPLE_RATE} Hz; model={} reference={}",
            model.sample_rate, reference.sample_rate
        ));
    }
    let model_onset = onset(model)?;
    let reference_onset = onset(reference)?;
    let common = (model.frames.len() - model_onset).min(reference.frames.len() - reference_onset);
    if common < usize::try_from(SAMPLE_RATE / 20).expect("bounded sample rate") {
        return Err("captures have less than 50 ms in common after alignment".into());
    }
    let model_frames = &model.frames[model_onset..model_onset + common];
    let reference_frames = &reference.frames[reference_onset..reference_onset + common];
    let model_rms = rms(model_frames);
    let reference_rms = rms(reference_frames);
    if model_rms <= 1.0e-12 || reference_rms <= 1.0e-12 {
        return Err("aligned capture is silent".into());
    }
    let model_peak = peak(model_frames);
    let reference_peak = peak(reference_frames);
    let model_width = stereo_width(model_frames);
    let reference_width = stereo_width(reference_frames);
    Ok(Comparison {
        common_seconds: common as f64 / f64::from(SAMPLE_RATE),
        onset_delta_ms: (reference_onset as f64 - model_onset as f64) * 1_000.0
            / f64::from(SAMPLE_RATE),
        model_rms_dbfs: decibels(model_rms),
        reference_rms_dbfs: decibels(reference_rms),
        level_delta_db: decibels(model_rms / reference_rms),
        peak_delta_db: decibels(model_peak / reference_peak),
        crest_delta_db: decibels(model_peak / model_rms) - decibels(reference_peak / reference_rms),
        envelope_correlation: envelope_correlation(model_frames, reference_frames),
        model_stereo_width_db: model_width,
        reference_stereo_width_db: reference_width,
        stereo_width_delta_db: model_width - reference_width,
    })
}

fn onset(audio: &Audio) -> Result<usize, String> {
    if audio.frames.is_empty() {
        return Err("capture contains no audio frames".into());
    }
    let peak = peak(&audio.frames);
    if peak <= 1.0e-12 {
        return Err("capture is silent".into());
    }
    let noise_frames = audio
        .frames
        .len()
        .min(usize::try_from(audio.sample_rate / 10).expect("bounded sample rate"));
    let noise = rms(&audio.frames[..noise_frames]);
    let threshold = (peak * 0.01).max(noise * 8.0).max(1.0e-9);
    audio
        .frames
        .iter()
        .position(|frame| frame[0].abs().max(frame[1].abs()) >= threshold)
        .ok_or_else(|| "capture has no onset above its noise floor".into())
}

fn rms(frames: &[[f64; 2]]) -> f64 {
    let energy = frames
        .iter()
        .map(|frame| frame[0] * frame[0] + frame[1] * frame[1])
        .sum::<f64>();
    (energy / (frames.len() * 2) as f64).sqrt()
}

fn peak(frames: &[[f64; 2]]) -> f64 {
    frames
        .iter()
        .flat_map(|frame| frame.iter())
        .map(|sample| sample.abs())
        .fold(0.0, f64::max)
}

fn stereo_width(frames: &[[f64; 2]]) -> f64 {
    let (mid, side) = frames.iter().fold((0.0, 0.0), |(mid, side), frame| {
        let next_mid = 0.5 * (frame[0] + frame[1]);
        let next_side = 0.5 * (frame[0] - frame[1]);
        (mid + next_mid * next_mid, side + next_side * next_side)
    });
    decibels((side / mid.max(1.0e-30)).sqrt())
}

fn envelope_correlation(model: &[[f64; 2]], reference: &[[f64; 2]]) -> f64 {
    const WINDOW: usize = SAMPLE_RATE as usize / 100;
    let model = envelope(model, WINDOW);
    let reference = envelope(reference, WINDOW);
    pearson(&model, &reference)
}

fn envelope(frames: &[[f64; 2]], window: usize) -> Vec<f64> {
    frames.chunks_exact(window).map(rms).collect::<Vec<_>>()
}

fn pearson(left: &[f64], right: &[f64]) -> f64 {
    let count = left.len().min(right.len());
    if count == 0 {
        return 0.0;
    }
    let left = &left[..count];
    let right = &right[..count];
    let left_mean = left.iter().sum::<f64>() / count as f64;
    let right_mean = right.iter().sum::<f64>() / count as f64;
    let (numerator, left_energy, right_energy) = left.iter().zip(right).fold(
        (0.0, 0.0, 0.0),
        |(numerator, left_energy, right_energy), (left, right)| {
            let left = left - left_mean;
            let right = right - right_mean;
            (
                numerator + left * right,
                left_energy + left * left,
                right_energy + right * right,
            )
        },
    );
    let denominator = (left_energy * right_energy).sqrt();
    if denominator <= 1.0e-30 {
        if left == right { 1.0 } else { 0.0 }
    } else {
        (numerator / denominator).clamp(-1.0, 1.0)
    }
}

fn decibels(value: f64) -> f64 {
    20.0 * value.max(1.0e-15).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(delay: usize, gain: f64, stereo: f64) -> Audio {
        let mut frames = vec![[0.0, 0.0]; delay];
        for index in 0..4_800 {
            let sample = gain
                * (std::f64::consts::TAU * 440.0 * index as f64 / f64::from(SAMPLE_RATE)).sin();
            frames.push([sample, sample * stereo]);
        }
        Audio {
            sample_rate: SAMPLE_RATE,
            frames,
        }
    }

    #[test]
    fn comparison_aligns_onsets_without_normalizing_level() {
        let model = signal(6_000, 0.5, 1.0);
        let reference = signal(6_100, 0.25, 1.0);
        let result = compare_audio(&model, &reference).unwrap();
        assert!((result.onset_delta_ms - 100.0 / 48.0).abs() < 0.001);
        assert!((result.level_delta_db - 6.020_599_913).abs() < 0.001);
        assert!((result.envelope_correlation - 1.0).abs() < 1.0e-9);
    }

    #[test]
    fn comparison_reports_stereo_width_difference() {
        let model = signal(6_000, 0.5, -0.5);
        let reference = signal(6_000, 0.5, 0.5);
        let result = compare_audio(&model, &reference).unwrap();
        assert!(result.stereo_width_delta_db > 10.0);
    }

    #[test]
    fn comparison_rejects_implicit_resampling() {
        let model = signal(100, 0.5, 1.0);
        let mut reference = signal(100, 0.5, 1.0);
        reference.sample_rate = 44_100;
        assert!(compare_audio(&model, &reference).is_err());
    }

    #[test]
    fn external_reference_manifest_requires_provenance_and_unchanged_level() {
        let valid = "RF-Organ reference capture\ncapture_id=b3-2026-01\nsource=private session\nrights=private measurement\nconsole=Hammond B-3\nsignal_chain=Leslie 122, pair at 1 m\nsample_rate=48000\nnormalization=none\n";
        assert!(validate_reference_manifest(valid).is_ok());
        assert!(
            validate_reference_manifest(&valid.replace("normalization=none", "normalization=peak"))
                .is_err()
        );
        assert!(
            validate_reference_manifest(&valid.replace("rights=private measurement\n", ""))
                .is_err()
        );
    }
}
