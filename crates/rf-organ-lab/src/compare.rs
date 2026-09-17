// SPDX-License-Identifier: GPL-2.0-or-later

use crate::captures::{
    C_NOTE, CHARACTER, F_NOTE, Level, NO_TRIM, Notes, TransformerPath, Trims, capture_names,
    note_frequency, transformer_capture, transformer_steady_frames, trim_for,
};
use crate::wav::{self, Audio};
use rf_organ_dsp::gear_frequency;
use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const SAMPLE_RATE: u32 = 48_000;
const PEDAL_HARMONICS: [(usize, usize); 8] = [
    (1, 0),
    (2, 12),
    (3, 19),
    (4, 24),
    (6, 31),
    (8, 36),
    (10, 40),
    (12, 43),
];

pub struct Reports {
    pub capture_comparison: String,
    pub pedal_spectrum_comparison: String,
    pub pedal_fit_candidates: String,
    pub transformer_intermodulation_comparison: String,
    pub transformer_fit_candidates: String,
    pub reference_quality: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum QualityStatus {
    Pass,
    Warning,
    Fail,
}

impl QualityStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warning => "warning",
            Self::Fail => "fail",
        }
    }
}

#[derive(Clone, Debug)]
struct CaptureQuality {
    status: QualityStatus,
    qualification: &'static str,
}

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

pub fn compare_directories(model: &Path, reference: &Path) -> Result<Reports, String> {
    validate_reference_directory(reference)?;
    let (reference_quality, quality) = analyze_reference_quality(reference)?;
    let mut report = String::from(
        "capture,common_seconds,reference_minus_model_onset_ms,model_rms_dbfs,reference_rms_dbfs,model_minus_reference_level_db,model_minus_reference_peak_db,model_minus_reference_crest_db,envelope_correlation,model_stereo_width_db,reference_stereo_width_db,model_minus_reference_stereo_width_db\n",
    );
    let mut compared = 0;
    for capture in capture_names() {
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
    let (pedal_spectrum_comparison, pedal_fit_candidates) =
        compare_pedal_captures(model, reference, &quality)?;
    let (transformer_intermodulation_comparison, transformer_fit_candidates) =
        compare_transformer_captures(model, reference, &quality)?;
    Ok(Reports {
        capture_comparison: report,
        pedal_spectrum_comparison,
        pedal_fit_candidates,
        transformer_intermodulation_comparison,
        transformer_fit_candidates,
        reference_quality,
    })
}

fn compare_pedal_captures(
    model: &Path,
    reference: &Path,
    quality: &BTreeMap<String, CaptureQuality>,
) -> Result<(String, String), String> {
    let registrations = [("07-pedal-16ft", 0, 3), ("08-pedal-8ft", 1, 2)];
    let mut spectrum = String::from(
        "capture,harmonic,wheel,frequency_hz,model_dbc,reference_dbc,reference_minus_model_db\n",
    );
    let mut fit = String::from(
        "parameter,capture,bus,harmonics,documented_resistance_ohm,current_value,candidate_value,unit,reference_minus_model_db,qualification\n",
    );
    for (capture, reference_harmonic, anchor_bus) in registrations {
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
        let model_levels = pedal_spectrum(&model_audio, reference_harmonic)?;
        let reference_levels = pedal_spectrum(&reference_audio, reference_harmonic)?;
        for (index, (harmonic, wheel)) in PEDAL_HARMONICS.into_iter().enumerate() {
            let frequency =
                gear_frequency(wheel).ok_or_else(|| "missing pedal wheel".to_owned())?;
            writeln!(
                &mut spectrum,
                "{capture},{harmonic},{},{frequency:.9},{:.9},{:.9},{:.9}",
                wheel + 1,
                model_levels[index],
                reference_levels[index],
                reference_levels[index] - model_levels[index]
            )
            .expect("string write cannot fail");
        }
        let capture_quality = quality
            .get(capture)
            .ok_or_else(|| format!("missing quality result for {capture}"))?;
        if capture_quality.status != QualityStatus::Fail {
            write_bus_candidates(
                &mut fit,
                capture,
                &model_levels,
                &reference_levels,
                anchor_bus,
                capture_quality.qualification,
            );
        }

        if capture == "07-pedal-16ft" && capture_quality.status != QualityStatus::Fail {
            let model_t999 = key_off_energy_t999(&model_audio)?;
            let reference_t999 = key_off_energy_t999(&reference_audio)?;
            let multiplier = model_t999 / reference_t999;
            writeln!(
                &mut fit,
                "l20-effective-cutoff,{capture},3,1+3,,700.000000,{:.6},Hz,,joint-end-to-end-starting-point-{}",
                700.0 * multiplier,
                capture_quality.qualification
            )
            .expect("string write cannot fail");
        }
    }
    Ok((spectrum, fit))
}

fn pedal_spectrum(audio: &Audio, reference_harmonic: usize) -> Result<[f64; 8], String> {
    if audio.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "pedal captures must be {SAMPLE_RATE} Hz; got {}",
            audio.sample_rate
        ));
    }
    let onset = onset(audio)?;
    let start = onset + SAMPLE_RATE as usize / 2;
    let end = start + SAMPLE_RATE as usize;
    let frames = audio
        .frames
        .get(start..end)
        .ok_or_else(|| "pedal capture needs one steady second after its attack".to_owned())?;
    let amplitudes = PEDAL_HARMONICS.map(|(_, wheel)| {
        spectral_amplitude(
            frames,
            f64::from(gear_frequency(wheel).expect("bounded pedal wheel")),
        )
    });
    let reference = amplitudes[reference_harmonic];
    if reference <= 1.0e-12 {
        return Err("pedal reference harmonic is below the analysis floor".to_owned());
    }
    Ok(amplitudes.map(|amplitude| decibels(amplitude / reference)))
}

fn write_bus_candidates(
    fit: &mut String,
    capture: &str,
    model: &[f64; 8],
    reference: &[f64; 8],
    anchor_bus: usize,
    qualification: &str,
) {
    let (prefix, candidates): (&str, &[(usize, &str, f64)]) = if capture == "07-pedal-16ft" {
        (
            "sixteen",
            &[(0, "10+12", 470.0), (1, "6+8", 47.0), (2, "2+4", 10.0)],
        )
    } else {
        ("eight", &[(0, "10+12", 20.0), (1, "6+8", 5.0)])
    };
    let anchor_delta = bus_level(reference, anchor_bus) - bus_level(model, anchor_bus);
    for (bus, harmonics, resistance) in candidates {
        let delta = bus_level(reference, *bus) - bus_level(model, *bus) - anchor_delta;
        let multiplier = 10.0_f64.powf(delta / 20.0);
        writeln!(
            fit,
            "{prefix}-bus-{bus}-gain,{capture},{bus},{harmonics},{resistance:.6},1.000000,{multiplier:.6},multiplier,{delta:.6},relative-to-anchor-bus-{anchor_bus}-{qualification}"
        )
        .expect("string write cannot fail");
    }
}

/// One measured path/level point: the noise-corrected difference product of a
/// C+F dyad, in both the model and the reference.
#[derive(Clone, Copy)]
struct Observation {
    path: TransformerPath,
    level: Level,
    /// Third-order product, the metric the reduced model actually produces.
    model_dbc: f64,
    reference_dbc: f64,
    status: QualityStatus,
    qualification: &'static str,
}

fn compare_transformer_captures(
    model: &Path,
    reference: &Path,
    quality: &BTreeMap<String, CaptureQuality>,
) -> Result<(String, String), String> {
    let mut csv = String::from(
        "path,unit,level,c_hz,f_hz,difference_hz,third_order_hz,model_difference_dbfs,reference_difference_dbfs,model_difference_dbc,reference_difference_dbc,reference_minus_model_difference_db,model_third_order_dbfs,reference_third_order_dbfs,model_third_order_dbc,reference_third_order_dbc,reference_minus_model_third_order_db,qualification\n",
    );
    let mut observations = Vec::new();
    for path in TransformerPath::ALL {
        for level in Level::ALL {
            let triplet = Notes::ALL.map(|notes| transformer_capture(path, level, notes));
            if !reference
                .join(format!("{}.wav", triplet[2].id))
                .is_file()
            {
                continue;
            }
            let load = |directory: &Path| -> Result<[Audio; 3], String> {
                let mut loaded = Vec::with_capacity(3);
                for capture in triplet {
                    let file = directory.join(format!("{}.wav", capture.id));
                    if !file.is_file() {
                        return Err(format!(
                            "the {} {} triplet needs its {} capture: {}",
                            path.label(),
                            level.label(),
                            capture.notes.label(),
                            file.display()
                        ));
                    }
                    loaded.push(read_audio(&file)?);
                }
                Ok([
                    loaded.remove(0),
                    loaded.remove(0),
                    loaded.remove(0),
                ])
            };
            let model_audio = load(model)?;
            let reference_audio = load(reference)?;
            let model_levels = capture_intermodulation(&model_audio)?;
            let reference_levels = capture_intermodulation(&reference_audio)?;
            let mut status = QualityStatus::Pass;
            let mut qualification = "quality-pass";
            for capture in triplet {
                let measured = quality
                    .get(capture.id)
                    .ok_or_else(|| format!("missing quality result for {}", capture.id))?;
                if measured.status > status {
                    status = measured.status;
                    qualification = measured.qualification;
                }
            }
            writeln!(
                &mut csv,
                "{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{qualification}",
                path.label(),
                path.unit().label(),
                level.label(),
                model_levels.c_hz,
                model_levels.f_hz,
                model_levels.difference_hz,
                model_levels.third_order_hz,
                model_levels.difference_dbfs,
                reference_levels.difference_dbfs,
                model_levels.difference_dbc,
                reference_levels.difference_dbc,
                reference_levels.difference_dbc - model_levels.difference_dbc,
                model_levels.third_order_dbfs,
                reference_levels.third_order_dbfs,
                model_levels.third_order_dbc,
                reference_levels.third_order_dbc,
                reference_levels.third_order_dbc - model_levels.third_order_dbc,
            )
            .expect("string write cannot fail");
            observations.push(Observation {
                path,
                level,
                model_dbc: model_levels.third_order_dbc,
                reference_dbc: reference_levels.third_order_dbc,
                status,
                qualification,
            });
        }
    }
    Ok((csv, transformer_fit_candidates(&observations)))
}

#[derive(Clone)]
struct TransformerFit {
    drive_trim: f32,
    effective_drive: f32,
    rms_error_db: f64,
    worst_residual_db: f64,
    self_check_db: f64,
    levels: String,
}

/// Fits the difference product of one path against its reference captures.
///
/// Only the drive trim is searched. The magnetic memory coefficient barely
/// moves this metric, so it stays where the character control puts it until a
/// measurement that separates it exists.
fn transformer_fit_candidates(observations: &[Observation]) -> String {
    let mut csv = String::from(
        "parameter,path,unit,levels,current_drive_trim,candidate_drive_trim,candidate_effective_drive,rms_error_db,worst_residual_db,sweep_self_check_db,qualification\n",
    );
    let usable = |path: TransformerPath| -> Vec<Observation> {
        observations
            .iter()
            .copied()
            .filter(|observation| {
                observation.path == path
                    && observation.status != QualityStatus::Fail
                    && observation.reference_dbc.is_finite()
            })
            .collect()
    };
    let present = |path: TransformerPath| -> bool {
        observations
            .iter()
            .any(|observation| observation.path == path)
    };

    let injection = usable(TransformerPath::Injection);
    let output_fit = (!injection.is_empty()).then(|| fit_path(TransformerPath::Injection, &injection, NO_TRIM));
    if let Some(fit) = &output_fit {
        write_fit_row(
            &mut csv,
            TransformerPath::Injection,
            fit,
            &qualification_of(&injection),
        );
    } else if present(TransformerPath::Injection) {
        write_blocked_row(
            &mut csv,
            TransformerPath::Injection,
            "injection-captures-rejected-by-quality-gate",
        );
    }

    for path in [TransformerPath::Upper, TransformerPath::Lower] {
        let rows = usable(path);
        if rows.is_empty() {
            if present(path) {
                write_blocked_row(&mut csv, path, "captures-rejected-by-quality-gate");
            }
            continue;
        }
        match &output_fit {
            Some(output) => {
                let base = trim_for(NO_TRIM, TransformerPath::Injection.unit(), output.drive_trim, 0.0);
                let fit = fit_path(path, &rows, base);
                write_fit_row(
                    &mut csv,
                    path,
                    &fit,
                    &format!(
                        "{}-with-t3-trim-{:.3}",
                        qualification_of(&rows),
                        output.drive_trim
                    ),
                );
            }
            None => write_blocked_row(&mut csv, path, "underdetermined-requires-t3-injection"),
        }
    }
    csv
}

fn qualification_of(observations: &[Observation]) -> String {
    let worst = observations
        .iter()
        .max_by_key(|observation| observation.status)
        .expect("non-empty fit input");
    worst.qualification.to_owned()
}

fn write_fit_row(csv: &mut String, path: TransformerPath, fit: &TransformerFit, qualification: &str) {
    writeln!(
        csv,
        "{}-drive-trim,{},{},{},0.000000,{:.6},{:.6},{:.6},{:.6},{:.6},{qualification}",
        path.unit().label(),
        path.label(),
        path.unit().label(),
        fit.levels,
        fit.drive_trim,
        fit.effective_drive,
        fit.rms_error_db,
        fit.worst_residual_db,
        fit.self_check_db,
    )
    .expect("string write cannot fail");
}

fn write_blocked_row(csv: &mut String, path: TransformerPath, qualification: &str) {
    writeln!(
        csv,
        "{}-drive-trim,{},{},,0.000000,,,,,,{qualification}",
        path.unit().label(),
        path.label(),
        path.unit().label(),
    )
    .expect("string write cannot fail");
}

const COARSE_FIRST: f32 = -0.35;
const COARSE_STEP: f32 = 0.05;
const COARSE_POINTS: usize = 20;
const FINE_STEP: f32 = 0.01;
const FINE_POINTS: usize = 9;

fn fit_path(path: TransformerPath, observations: &[Observation], base: Trims) -> TransformerFit {
    let mut best: Option<(f32, f64)> = None;
    let evaluate = |trim: f32, best: &mut Option<(f32, f64)>| {
        let error = fit_error(path, observations, base, trim);
        if best.is_none_or(|(_, previous)| error < previous) {
            *best = Some((trim, error));
        }
    };
    for step in 0..COARSE_POINTS {
        evaluate(COARSE_FIRST + COARSE_STEP * step as f32, &mut best);
    }
    let (coarse, _) = best.expect("non-empty coarse grid");
    for step in 0..FINE_POINTS {
        let trim = coarse + FINE_STEP * (step as f32 - (FINE_POINTS / 2) as f32);
        if (-1.0..=1.0).contains(&trim) {
            evaluate(trim, &mut best);
        }
    }
    let (drive_trim, rms_error_db) = best.expect("non-empty search");
    let worst_residual_db = observations
        .iter()
        .map(|observation| {
            (predicted_third_order_dbc(
                path,
                observation.level,
                trim_for(base, path.unit(), drive_trim, 0.0),
            ) - observation.reference_dbc)
                .abs()
        })
        .fold(0.0_f64, f64::max);
    let mut levels = observations
        .iter()
        .map(|observation| observation.level.label())
        .collect::<Vec<_>>();
    levels.dedup();
    TransformerFit {
        drive_trim,
        effective_drive: (CHARACTER.0 + drive_trim).clamp(0.0, 1.0),
        rms_error_db,
        worst_residual_db,
        self_check_db: sweep_self_check(path, observations),
        levels: levels.join("+"),
    }
}

/// Largest disagreement between the shortened fitting sweep and the model's
/// own four-second captures at the untrimmed calibration. It bounds how much
/// of a residual belongs to the probe window rather than to the reference.
fn sweep_self_check(path: TransformerPath, observations: &[Observation]) -> f64 {
    observations
        .iter()
        .map(|observation| {
            (predicted_third_order_dbc(path, observation.level, NO_TRIM) - observation.model_dbc)
                .abs()
        })
        .fold(0.0_f64, f64::max)
}

fn fit_error(
    path: TransformerPath,
    observations: &[Observation],
    base: Trims,
    trim: f32,
) -> f64 {
    let trims = trim_for(base, path.unit(), trim, 0.0);
    let total = observations
        .iter()
        .map(|observation| {
            let predicted = predicted_third_order_dbc(path, observation.level, trims);
            let error = predicted - observation.reference_dbc;
            error * error
        })
        .sum::<f64>();
    (total / observations.len() as f64).sqrt()
}

/// Steady-state third-order product the model produces for one candidate
/// trim, measured like a reference capture: the two single notes are
/// subtracted from the dyad in power.
fn predicted_third_order_dbc(path: TransformerPath, level: Level, trims: Trims) -> f64 {
    const SETTLE: usize = SAMPLE_RATE as usize / 4;
    const MEASURE: usize = SAMPLE_RATE as usize / 2;
    let frames = Notes::ALL
        .map(|notes| transformer_steady_frames(transformer_capture(path, level, notes), trims, SETTLE, MEASURE));
    intermodulation(&frames[0], &frames[1], &frames[2]).third_order_dbc
}

#[derive(Clone, Copy)]
struct TransformerIntermodulation {
    c_hz: f64,
    f_hz: f64,
    difference_hz: f64,
    difference_dbfs: f64,
    difference_dbc: f64,
    third_order_hz: f64,
    third_order_dbfs: f64,
    third_order_dbc: f64,
}

fn capture_intermodulation(audio: &[Audio; 3]) -> Result<TransformerIntermodulation, String> {
    Ok(intermodulation(
        transformer_steady(&audio[0])?,
        transformer_steady(&audio[1])?,
        transformer_steady(&audio[2])?,
    ))
}

fn intermodulation(
    c_only: &[[f64; 2]],
    f_only: &[[f64; 2]],
    dyad: &[[f64; 2]],
) -> TransformerIntermodulation {
    let c_hz = note_frequency(C_NOTE);
    let f_hz = note_frequency(F_NOTE);
    let difference_hz = f_hz - c_hz;
    let third_order_hz = 2.0 * c_hz - f_hz;
    let c = spectral_amplitude(dyad, c_hz);
    let f = spectral_amplitude(dyad, f_hz);
    let carrier = 0.5 * (c + f);
    let product = |frequency: f64| {
        let dyad_amplitude = spectral_amplitude(dyad, frequency);
        let c_single = spectral_amplitude(c_only, frequency);
        let f_single = spectral_amplitude(f_only, frequency);
        (dyad_amplitude * dyad_amplitude - c_single * c_single - f_single * f_single)
            .max(1.0e-30)
            .sqrt()
    };
    let difference = product(difference_hz);
    let third_order = product(third_order_hz);
    TransformerIntermodulation {
        c_hz,
        f_hz,
        difference_hz,
        difference_dbfs: decibels(difference),
        difference_dbc: decibels(difference / carrier),
        third_order_hz,
        third_order_dbfs: decibels(third_order),
        third_order_dbc: decibels(third_order / carrier),
    }
}

/// Two seconds of held tone, starting half a second after the contact. The
/// long window keeps the rectangular-window skirts of the two carriers from
/// reaching the intermodulation bins.
fn transformer_steady(audio: &Audio) -> Result<&[[f64; 2]], String> {
    let onset = onset(audio)?;
    let start = onset + SAMPLE_RATE as usize / 2;
    audio
        .frames
        .get(start..start + 2 * SAMPLE_RATE as usize)
        .ok_or_else(|| "transformer capture needs two steady seconds after its contact".to_owned())
}

fn analyze_reference_quality(
    reference: &Path,
) -> Result<(String, BTreeMap<String, CaptureQuality>), String> {
    let mut csv = String::from(
        "capture,status,duration_seconds,peak_dbfs,clipped_samples,noise_dbfs,steady_dbfs,snr_db,onset_seconds,onset_error_ms,key_off_seconds,key_off_error_ms,early_tuning_cents,late_tuning_cents,drift_cents,issues\n",
    );
    let mut results = BTreeMap::new();
    for capture in capture_names() {
        let path = reference.join(format!("{capture}.wav"));
        if !path.is_file() {
            continue;
        }
        let audio = read_audio(&path)?;
        let measured = capture_quality(capture, &audio)?;
        writeln!(
            &mut csv,
            "{capture},{},{:.9},{:.6},{},{:.6},{:.6},{:.6},{:.9},{:.6},{},{},{},{},{},{}",
            measured.status.label(),
            measured.duration_seconds,
            measured.peak_dbfs,
            measured.clipped_samples,
            measured.noise_dbfs,
            measured.steady_dbfs,
            measured.snr_db,
            measured.onset_seconds,
            measured.onset_error_ms,
            optional(measured.key_off_seconds, 9),
            optional(measured.key_off_error_ms, 6),
            optional(measured.early_tuning_cents, 6),
            optional(measured.late_tuning_cents, 6),
            optional(measured.drift_cents, 6),
            measured.issues
        )
        .expect("string write cannot fail");
        results.insert(
            capture.to_owned(),
            CaptureQuality {
                status: measured.status,
                qualification: match measured.status {
                    QualityStatus::Pass => "quality-pass",
                    QualityStatus::Warning => "quality-warning",
                    QualityStatus::Fail => "quality-fail",
                },
            },
        );
    }
    Ok((csv, results))
}

struct QualityMeasurement {
    status: QualityStatus,
    duration_seconds: f64,
    peak_dbfs: f64,
    clipped_samples: usize,
    noise_dbfs: f64,
    steady_dbfs: f64,
    snr_db: f64,
    onset_seconds: f64,
    onset_error_ms: f64,
    key_off_seconds: Option<f64>,
    key_off_error_ms: Option<f64>,
    early_tuning_cents: Option<f64>,
    late_tuning_cents: Option<f64>,
    drift_cents: Option<f64>,
    issues: String,
}

fn capture_quality(capture: &str, audio: &Audio) -> Result<QualityMeasurement, String> {
    if audio.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "quality analysis requires {SAMPLE_RATE} Hz; got {} for {capture}",
            audio.sample_rate
        ));
    }
    let onset = onset(audio)?;
    let duration_seconds = audio.frames.len() as f64 / f64::from(SAMPLE_RATE);
    let peak_value = peak(&audio.frames);
    let clipped_samples = audio
        .frames
        .iter()
        .flat_map(|frame| frame.iter())
        .filter(|sample| sample.abs() >= 0.999_9)
        .count();
    let noise_end = onset.min(SAMPLE_RATE as usize / 10);
    let noise = if noise_end == 0 {
        0.0
    } else {
        rms(&audio.frames[..noise_end])
    };
    let impulse = capture == "leslie-cabinet-impulse";
    let steady_start = if impulse {
        onset
    } else {
        onset + SAMPLE_RATE as usize / 2
    };
    let steady_end = if impulse {
        audio.frames.len()
    } else {
        steady_start + SAMPLE_RATE as usize
    };
    let steady = audio
        .frames
        .get(steady_start..steady_end)
        .map(rms)
        .ok_or_else(|| format!("{capture} needs one steady second after onset"))?;
    let snr_db = decibels(steady / noise.max(1.0e-15));
    let onset_seconds = onset as f64 / f64::from(SAMPLE_RATE);
    let expected_onset = if impulse { 0.0 } else { 0.25 };
    let onset_error_ms = (onset_seconds - expected_onset) * 1_000.0;
    let timed = !impulse;
    let key_off = timed.then(|| detect_key_off(audio, onset)).transpose()?;
    let key_off_seconds = key_off.map(|frame| frame as f64 / f64::from(SAMPLE_RATE));
    let key_off_error_ms = key_off_seconds.map(|time| (time - 3.0) * 1_000.0);

    let tuning_target = match capture {
        "07-pedal-16ft" | "09-pedal-16ft-8ft" => gear_frequency(0).map(f64::from),
        "08-pedal-8ft" => gear_frequency(12).map(f64::from),
        _ => crate::captures::TRANSFORMER_CAPTURES
            .iter()
            .find(|entry| entry.id == capture)
            .and_then(|entry| match entry.notes {
                Notes::C => Some(note_frequency(C_NOTE)),
                Notes::F => Some(note_frequency(F_NOTE)),
                Notes::Dyad => None,
            }),
    };
    let (early_tuning_cents, late_tuning_cents, drift_cents) = if let Some(target) = tuning_target {
        let window = SAMPLE_RATE as usize / 2;
        let early_start = onset + SAMPLE_RATE as usize / 2;
        let off = key_off.ok_or_else(|| format!("{capture} has no key-off for tuning analysis"))?;
        let late_start = off
            .checked_sub(window + SAMPLE_RATE as usize / 4)
            .ok_or_else(|| format!("{capture} is too short for late tuning analysis"))?;
        let early = estimate_frequency(
            audio
                .frames
                .get(early_start..early_start + window)
                .ok_or_else(|| format!("{capture} has no early tuning window"))?,
            target,
        );
        let late = estimate_frequency(
            audio
                .frames
                .get(late_start..late_start + window)
                .ok_or_else(|| format!("{capture} has no late tuning window"))?,
            target,
        );
        let early_cents = 1_200.0 * (early / target).log2();
        let late_cents = 1_200.0 * (late / target).log2();
        (
            Some(early_cents),
            Some(late_cents),
            Some(late_cents - early_cents),
        )
    } else {
        (None, None, None)
    };

    let mut status = QualityStatus::Pass;
    let mut issues = Vec::new();
    quality_gate(
        &mut status,
        &mut issues,
        duration_seconds < if impulse { 0.45 } else { 3.2 },
        QualityStatus::Fail,
        "short-duration",
    );
    quality_gate(
        &mut status,
        &mut issues,
        clipped_samples > 0,
        QualityStatus::Fail,
        "clipping",
    );
    quality_gate(
        &mut status,
        &mut issues,
        snr_db < 20.0,
        QualityStatus::Fail,
        "snr-below-20db",
    );
    quality_gate(
        &mut status,
        &mut issues,
        (20.0..40.0).contains(&snr_db),
        QualityStatus::Warning,
        "snr-below-40db",
    );
    timing_gate(&mut status, &mut issues, onset_error_ms, "onset");
    if let Some(error) = key_off_error_ms {
        timing_gate(&mut status, &mut issues, error, "key-off");
    }
    if let (Some(absolute), Some(drift)) = (early_tuning_cents, drift_cents) {
        quality_gate(
            &mut status,
            &mut issues,
            absolute.abs() > 50.0 || drift.abs() > 5.0,
            QualityStatus::Fail,
            "unstable-tuning",
        );
        quality_gate(
            &mut status,
            &mut issues,
            absolute.abs() > 10.0 || drift.abs() > 1.0,
            QualityStatus::Warning,
            "tuning-warning",
        );
    }
    Ok(QualityMeasurement {
        status,
        duration_seconds,
        peak_dbfs: decibels(peak_value),
        clipped_samples,
        noise_dbfs: decibels(noise),
        steady_dbfs: decibels(steady),
        snr_db,
        onset_seconds,
        onset_error_ms,
        key_off_seconds,
        key_off_error_ms,
        early_tuning_cents,
        late_tuning_cents,
        drift_cents,
        issues: if issues.is_empty() {
            "none".to_owned()
        } else {
            issues.join("|")
        },
    })
}

fn quality_gate(
    status: &mut QualityStatus,
    issues: &mut Vec<&'static str>,
    condition: bool,
    severity: QualityStatus,
    issue: &'static str,
) {
    if condition {
        *status = (*status).max(severity);
        issues.push(issue);
    }
}

fn timing_gate(
    status: &mut QualityStatus,
    issues: &mut Vec<&'static str>,
    error_ms: f64,
    name: &'static str,
) {
    let (fail, warning) = match name {
        "onset" => ("onset-outside-100ms", "onset-outside-20ms"),
        "key-off" => ("key-off-outside-100ms", "key-off-outside-20ms"),
        _ => unreachable!(),
    };
    quality_gate(
        status,
        issues,
        error_ms.abs() > 100.0,
        QualityStatus::Fail,
        fail,
    );
    quality_gate(
        status,
        issues,
        error_ms.abs() > 20.0,
        QualityStatus::Warning,
        warning,
    );
}

fn detect_key_off(audio: &Audio, onset: usize) -> Result<usize, String> {
    let window = SAMPLE_RATE as usize / 20;
    let step = SAMPLE_RATE as usize / 200;
    let expected = onset + (2.75 * f64::from(SAMPLE_RATE)) as usize;
    let search_radius = SAMPLE_RATE as usize / 10;
    let baseline_start = expected.saturating_sub(SAMPLE_RATE as usize / 2);
    let baseline_end = expected.saturating_sub(search_radius);
    let baseline = audio
        .frames
        .get(baseline_start..baseline_end)
        .map(frame_power)
        .filter(|power| *power > 1.0e-30)
        .ok_or_else(|| "capture has no stable pre-release level".to_owned())?;
    let threshold = baseline * 0.5;
    let half_window = window / 2;
    let start = expected.saturating_sub(search_radius).max(half_window);
    let end = (expected + search_radius).min(audio.frames.len().saturating_sub(half_window));
    let mut previous: Option<(usize, f64)> = None;
    for center in (start..end).step_by(step) {
        let power = frame_power(&audio.frames[center - half_window..center + half_window]);
        if let Some((previous_center, previous_power)) = previous
            && previous_power > threshold
            && power <= threshold
        {
            let fraction = (previous_power - threshold) / (previous_power - power);
            return Ok(
                previous_center + (fraction * (center - previous_center) as f64).round() as usize
            );
        }
        previous = Some((center, power));
    }
    Err("capture has no key-off energy crossing near the protocol time".to_owned())
}

fn estimate_frequency(frames: &[[f64; 2]], expected: f64) -> f64 {
    let half = frames.len() / 2;
    let first = complex_projection(&frames[..half], expected, 0);
    let second = complex_projection(&frames[half..half * 2], expected, half);
    let channel = if magnitude(first[0]) + magnitude(second[0])
        >= magnitude(first[1]) + magnitude(second[1])
    {
        0
    } else {
        1
    };
    let first_phase = first[channel].1.atan2(first[channel].0);
    let second_phase = second[channel].1.atan2(second[channel].0);
    let phase_advance =
        (second_phase - first_phase + std::f64::consts::PI).rem_euclid(TAU) - std::f64::consts::PI;
    let separation = half as f64 / f64::from(SAMPLE_RATE);
    expected + phase_advance / (TAU * separation)
}

fn complex_projection(
    frames: &[[f64; 2]],
    frequency: f64,
    sample_offset: usize,
) -> [(f64, f64); 2] {
    let mut result = [(0.0, 0.0); 2];
    for (index, frame) in frames.iter().enumerate() {
        let weight = 0.5 - 0.5 * (TAU * index as f64 / (frames.len() - 1) as f64).cos();
        let phase = TAU * frequency * (sample_offset + index) as f64 / f64::from(SAMPLE_RATE);
        for channel in 0..2 {
            result[channel].0 += frame[channel] * weight * phase.cos();
            result[channel].1 -= frame[channel] * weight * phase.sin();
        }
    }
    result
}

fn magnitude(value: (f64, f64)) -> f64 {
    value.0.hypot(value.1)
}

fn optional(value: Option<f64>, precision: usize) -> String {
    value.map_or_else(String::new, |value| format!("{value:.precision$}"))
}

fn bus_level(levels: &[f64; 8], bus: usize) -> f64 {
    let indices = match bus {
        0 => [6, 7],
        1 => [4, 5],
        2 => [1, 3],
        3 => [0, 2],
        _ => unreachable!(),
    };
    let power = indices
        .into_iter()
        .map(|index| 10.0_f64.powf(levels[index] / 10.0))
        .sum::<f64>()
        / 2.0;
    10.0 * power.max(1.0e-30).log10()
}

fn spectral_amplitude(frames: &[[f64; 2]], frequency: f64) -> f64 {
    let count = frames.len();
    let mut real = [0.0; 2];
    let mut imaginary = [0.0; 2];
    let mut weight_sum = 0.0;
    for (index, frame) in frames.iter().enumerate() {
        let weight = 0.5 - 0.5 * (TAU * index as f64 / (count - 1) as f64).cos();
        let phase = TAU * frequency * index as f64 / f64::from(SAMPLE_RATE);
        for channel in 0..2 {
            real[channel] += frame[channel] * weight * phase.cos();
            imaginary[channel] -= frame[channel] * weight * phase.sin();
        }
        weight_sum += weight;
    }
    let amplitudes =
        [0, 1].map(|channel| 2.0 * real[channel].hypot(imaginary[channel]) / weight_sum);
    ((amplitudes[0] * amplitudes[0] + amplitudes[1] * amplitudes[1]) * 0.5).sqrt()
}

fn key_off_energy_t999(audio: &Audio) -> Result<f64, String> {
    let onset = onset(audio)?;
    let release = detect_key_off(audio, onset)?;
    let tail_frames = SAMPLE_RATE as usize / 10;
    let tail = audio
        .frames
        .get(release..release + tail_frames)
        .ok_or_else(|| "pedal capture needs 100 ms after key-off".to_owned())?;
    let noise_frames = onset.min(SAMPLE_RATE as usize / 10);
    let noise_power = if noise_frames == 0 {
        0.0
    } else {
        frame_power(&audio.frames[..noise_frames])
    };
    let corrected = tail
        .iter()
        .map(|frame| (0.5 * (frame[0] * frame[0] + frame[1] * frame[1]) - noise_power).max(0.0))
        .collect::<Vec<_>>();
    let total = corrected.iter().sum::<f64>();
    if total <= 1.0e-30 {
        return Err("pedal key-off response is below its corrected noise floor".to_owned());
    }
    let mut accumulated = 0.0;
    corrected
        .iter()
        .enumerate()
        .find_map(|(index, energy)| {
            accumulated += energy;
            (accumulated >= total * 0.999).then_some(index as f64 / f64::from(SAMPLE_RATE))
        })
        .ok_or_else(|| "pedal key-off energy did not converge".to_owned())
}

fn frame_power(frames: &[[f64; 2]]) -> f64 {
    frames
        .iter()
        .map(|frame| 0.5 * (frame[0] * frame[0] + frame[1] * frame[1]))
        .sum::<f64>()
        / frames.len() as f64
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

    #[test]
    fn pedal_bus_candidate_is_relative_to_its_anchor() {
        let model = [0.0; 8];
        let mut reference = model;
        reference[6] = 6.020_599_913;
        reference[7] = 6.020_599_913;
        let mut csv = String::new();
        write_bus_candidates(
            &mut csv,
            "07-pedal-16ft",
            &model,
            &reference,
            3,
            "quality-pass",
        );
        let first = csv.lines().next().unwrap();
        assert!(first.contains(",2.000000,multiplier,6.020600,"));
        assert!(
            csv.lines()
                .skip(1)
                .all(|line| line.contains(",1.000000,multiplier,0.000000,"))
        );
    }

    #[test]
    fn transformer_triplet_isolates_the_difference_product() {
        let c_hz = note_frequency(C_NOTE);
        let f_hz = note_frequency(F_NOTE);
        let difference_hz = f_hz - c_hz;
        let capture = |c_level: f64, f_level: f64, difference_level: f64| {
            let mut frames = vec![[0.0, 0.0]; SAMPLE_RATE as usize * 4];
            for (index, frame) in frames.iter_mut().enumerate() {
                if (SAMPLE_RATE as usize / 4..SAMPLE_RATE as usize * 3).contains(&index) {
                    let time = index as f64 / f64::from(SAMPLE_RATE);
                    let sample = c_level * (TAU * c_hz * time).sin()
                        + f_level * (TAU * f_hz * time).sin()
                        + difference_level * (TAU * difference_hz * time).sin();
                    *frame = [sample, sample];
                }
            }
            Audio {
                sample_rate: SAMPLE_RATE,
                frames,
            }
        };
        let levels = capture_intermodulation(&[
            capture(0.2, 0.0, 0.0),
            capture(0.0, 0.2, 0.0),
            capture(0.2, 0.2, 0.01),
        ])
        .unwrap();
        assert!((levels.difference_dbc + 26.020_599_913).abs() < 0.05);
        assert!(levels.third_order_dbc < -100.0);
    }

    #[test]
    fn an_injection_fit_recovers_a_known_output_transformer_trim() {
        let target = 0.15;
        let reference_trims = trim_for(NO_TRIM, TransformerPath::Injection.unit(), target, 0.0);
        let observations = Level::ALL
            .into_iter()
            .map(|level| Observation {
                path: TransformerPath::Injection,
                level,
                model_dbc: predicted_third_order_dbc(TransformerPath::Injection, level, NO_TRIM),
                reference_dbc: predicted_third_order_dbc(
                    TransformerPath::Injection,
                    level,
                    reference_trims,
                ),
                status: QualityStatus::Pass,
                qualification: "quality-pass",
            })
            .collect::<Vec<_>>();
        let fit = fit_path(TransformerPath::Injection, &observations, NO_TRIM);
        assert!(
            (fit.drive_trim - target).abs() <= 0.011,
            "recovered {}",
            fit.drive_trim
        );
        assert!(fit.rms_error_db < 0.2, "residual {}", fit.rms_error_db);
        assert!(fit.self_check_db < 1.0e-9);
        assert_eq!(fit.levels, "low+nominal+high");
    }

    #[test]
    fn manual_paths_stay_underdetermined_without_an_injection_capture() {
        let observations = [Observation {
            path: TransformerPath::Upper,
            level: Level::Nominal,
            model_dbc: -60.0,
            reference_dbc: -55.0,
            status: QualityStatus::Warning,
            qualification: "quality-warning",
        }];
        let csv = transformer_fit_candidates(&observations);
        let row = csv.lines().nth(1).expect("one candidate row");
        assert_eq!(csv.lines().count(), 2);
        assert!(row.starts_with("t2-drive-trim,t2+t3,t2,,"));
        assert!(row.ends_with("underdetermined-requires-t3-injection"));
    }

    #[test]
    fn a_failed_reference_triplet_does_not_produce_a_candidate() {
        let observations = [Observation {
            path: TransformerPath::Injection,
            level: Level::Nominal,
            model_dbc: -60.0,
            reference_dbc: -55.0,
            status: QualityStatus::Fail,
            qualification: "quality-fail",
        }];
        let csv = transformer_fit_candidates(&observations);
        assert_eq!(csv.lines().count(), 2);
        assert!(
            csv.contains("injection-captures-rejected-by-quality-gate"),
            "{csv}"
        );
    }

    #[test]
    fn capture_quality_accepts_the_reference_protocol() {
        let mut frames = vec![[0.0, 0.0]; SAMPLE_RATE as usize * 4];
        for (index, frame) in frames.iter_mut().enumerate() {
            if (SAMPLE_RATE as usize / 4..SAMPLE_RATE as usize * 3).contains(&index) {
                let sample =
                    0.25 * (TAU * 32.692_306_5 * index as f64 / f64::from(SAMPLE_RATE)).sin();
                *frame = [sample, sample];
            }
        }
        let mut audio = Audio {
            sample_rate: SAMPLE_RATE,
            frames,
        };
        let quality = capture_quality("07-pedal-16ft", &audio).unwrap();
        assert_eq!(quality.status, QualityStatus::Pass);
        assert!(quality.onset_error_ms.abs() < 1.0);
        assert!(quality.key_off_error_ms.unwrap().abs() < 6.0);
        assert!(quality.drift_cents.unwrap().abs() < 0.1);

        audio.frames[SAMPLE_RATE as usize] = [1.0, 1.0];
        let rejected = capture_quality("07-pedal-16ft", &audio).unwrap();
        assert_eq!(rejected.status, QualityStatus::Fail);
        assert!(rejected.issues.contains("clipping"));
    }
}
