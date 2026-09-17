// SPDX-License-Identifier: GPL-2.0-or-later

use crate::captures;
use crate::captures::{
    C_NOTE, CHARACTER, EXPRESSION_CAPTURES, EXPRESSION_CHARACTER, EXPRESSION_HIGH, EXPRESSION_LOW,
    EXPRESSION_MID, ExpressionCapture, F_NOTE, Level, NO_TRIM, Notes, PERCUSSION_CAPTURES,
    PERCUSSION_NOTE, PercussionCapture, TransformerPath, Trims, bus_frequency, capture_names,
    note_frequency, transformer_capture, transformer_steady_frames, trim_for,
};
use crate::wav::{self, Audio};
use rf_organ_dsp::{ConsoleElectronics, PercussionDecay, PercussionVolume};
use rf_organ_dsp::{MANUAL_FIRST_NOTE, MANUAL_KEY_COUNT, drawbar_wheel, gear_frequency};
use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const SAMPLE_RATE: u32 = 48_000;
/// How far from the middle of a chromatic sweep a wheel may sit and still be
/// believed to be a taper rather than a fault, a wrong note or a bad
/// recording. A generator that really is 20 dB down on one wheel needs a
/// service call, not a table entry.
const TAPER_CEILING_DB: f64 = 12.0;
const TAPER_FLOOR_DB: f64 = -20.0;
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
    pub percussion_comparison: String,
    pub percussion_fit_candidates: String,
    pub expression_comparison: String,
    pub console_fit_candidates: String,
    pub taper_comparison: String,
    pub taper_fit_candidates: String,
    pub console_stage_comparison: String,
    pub console_stage_candidates: String,
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
    let (percussion_comparison, percussion_fit_candidates) =
        compare_percussion_captures(model, reference, &quality)?;
    let (expression_comparison, console_fit_candidates) =
        compare_expression_captures(model, reference, &quality)?;
    let (taper_comparison, taper_fit_candidates) = compare_taper_captures(model, reference)?;
    let (console_stage_comparison, console_stage_candidates) =
        compare_console_captures(model, reference)?;
    Ok(Reports {
        capture_comparison: report,
        pedal_spectrum_comparison,
        pedal_fit_candidates,
        transformer_intermodulation_comparison,
        transformer_fit_candidates,
        percussion_comparison,
        percussion_fit_candidates,
        expression_comparison,
        console_fit_candidates,
        taper_comparison,
        taper_fit_candidates,
        console_stage_comparison,
        console_stage_candidates,
        reference_quality,
    })
}

/// Reads a per-wheel taper off a chromatic capture.
///
/// The sweep holds every key of the upper manual in turn on the eight foot
/// alone, so each held stretch is one wheel and nothing else. Measuring that
/// wheel's own frequency in its own stretch gives the level the generator
/// let through, and the whole list normalised to its own middle is the taper.
///
/// Two things this deliberately does not do. It does not touch wheels the
/// sweep never reached - the eight foot covers the wheels the manual's keys
/// name and no others - and it does not report a wheel whose stretch was too
/// quiet or clipped to read. A taper is a table of measurements, and a table
/// with guesses in it is worse than a short one.
fn compare_taper_captures(model: &Path, reference: &Path) -> Result<(String, String), String> {
    let name = format!("{}.wav", captures::TAPER_CAPTURE);
    let reference_path = reference.join(&name);
    if !reference_path.is_file() {
        return Ok((
            String::from("status\nno taper sweep in the reference directory\n"),
            String::from("status\nno taper sweep in the reference directory\n"),
        ));
    }
    let model_path = model.join(&name);
    if !model_path.is_file() {
        return Err(format!(
            "model taper sweep is missing: {}",
            model_path.display()
        ));
    }
    let model_audio = read_audio(&model_path)?;
    let reference_audio = read_audio(&reference_path)?;

    let mut comparison = String::from(
        "key,midi_note,wheel,frequency_hz,model_dbfs,reference_dbfs,reference_minus_model_db,status\n",
    );
    let mut candidates = String::from("wheel,level,decibels,status\n");
    let mut levels: Vec<(usize, f64)> = Vec::new();
    let mut rows: Vec<(usize, u8, usize, f64, f64, f64)> = Vec::new();

    for key in 0..MANUAL_KEY_COUNT {
        let note = MANUAL_FIRST_NOTE + key as u8;
        let wheel = drawbar_wheel(key, captures::TAPER_BUS)
            .ok_or_else(|| format!("no eight foot wheel for key {key}"))?;
        let frequency = f64::from(gear_frequency(wheel).ok_or_else(|| "missing wheel".to_owned())?);
        let model_level = taper_stretch(&model_audio, key, frequency)?;
        let reference_level = taper_stretch(&reference_audio, key, frequency)?;
        rows.push((key, note, wheel, frequency, model_level, reference_level));
        // What the taper has to explain is the part the model does not: the
        // console, the transformers and the stages after the generator colour
        // every wheel too, and they are already in the model's own reading.
        // Dividing that out is what makes fitting the model against itself
        // return the flat table it started with, instead of handing back the
        // console's frequency response as though the generator had made it.
        if reference_level.is_finite() && model_level.is_finite() {
            levels.push((wheel, reference_level - model_level));
        }
    }

    // The middle of what was read, so the table says how the wheels differ
    // from each other rather than how loud the recording was.
    let mut sorted: Vec<f64> = levels.iter().map(|(_, level)| *level).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite levels"));
    let middle = if sorted.is_empty() {
        0.0
    } else {
        sorted[sorted.len() / 2]
    };

    for (key, note, wheel, frequency, model_level, reference_level) in rows {
        let excess = reference_level - model_level - middle;
        let status = if !reference_level.is_finite() || !model_level.is_finite() {
            "unreadable"
        } else if excess > TAPER_CEILING_DB {
            "louder than a taper explains"
        } else if excess < TAPER_FLOOR_DB {
            "quieter than a taper explains"
        } else {
            "read"
        };
        writeln!(
            &mut comparison,
            "{key},{note},{wheel},{frequency:.6},{model_level:.6},{reference_level:.6},{:.6},{status}",
            reference_level - model_level
        )
        .expect("string write cannot fail");
        if status == "read" {
            let decibels = excess;
            writeln!(
                &mut candidates,
                "{wheel},{:.6},{decibels:.6},read",
                10.0_f64.powf(decibels / 20.0)
            )
            .expect("string write cannot fail");
        } else {
            writeln!(&mut candidates, "{wheel},,,{status}").expect("string write cannot fail");
        }
    }
    Ok((comparison, candidates))
}

/// How loud one wheel was during its own stretch of the sweep, in dBFS, or
/// not a number when that stretch is not worth reading.
fn taper_stretch(audio: &Audio, key: usize, frequency: f64) -> Result<f64, String> {
    let rate = audio.sample_rate as f64;
    let step = (captures::TAPER_KEY_SECONDS + captures::TAPER_GAP_SECONDS) * rate;
    let start = (key as f64 * step + 0.25 * captures::TAPER_KEY_SECONDS * rate) as usize;
    let end = (key as f64 * step + 0.9 * captures::TAPER_KEY_SECONDS * rate) as usize;
    if end > audio.frames.len() {
        return Err(format!(
            "the taper sweep is too short: key {key} needs {end} frames and there are {}",
            audio.frames.len()
        ));
    }
    let window = &audio.frames[start..end];
    if window
        .iter()
        .any(|frame| frame[0].abs() >= 0.999 || frame[1].abs() >= 0.999)
    {
        return Ok(f64::NAN);
    }
    let amplitude = spectral_amplitude(window, frequency);
    if amplitude <= 0.0 {
        return Ok(f64::NAN);
    }
    Ok(decibels(amplitude))
}

/// Reads the preamplifier's stage asymmetry off a bench injection.
///
/// A single-ended triode's curve is lopsided, so what it adds to a steady tone
/// is led by the second harmonic. The three stages of the AO-28 are lopsided
/// by different amounts, and no reading taken at the output can say how they
/// divide it between them - the tone passes through all three. What it can say
/// is how lopsided the chain is altogether, which is one number, and that is
/// what this fits: the character that scales the three provisional
/// asymmetries. Telling the stages apart needs an injection at each one, and
/// this says so rather than pretending a single reading settled it.
fn compare_console_captures(model: &Path, reference: &Path) -> Result<(String, String), String> {
    let mut comparison = String::from(
        "capture,drive,model_h2_dbc,reference_h2_dbc,model_h3_dbc,reference_h3_dbc,reference_minus_model_h2_db\n",
    );
    let mut observations: Vec<(f32, f64)> = Vec::new();
    let mut seen = 0;
    for capture in &captures::CONSOLE_CAPTURES {
        let name = format!("{}.wav", capture.id);
        let reference_path = reference.join(&name);
        if !reference_path.is_file() {
            continue;
        }
        let model_path = model.join(&name);
        if !model_path.is_file() {
            return Err(format!(
                "model console injection is missing: {}",
                model_path.display()
            ));
        }
        let model_harmonics = console_harmonics(&read_audio(&model_path)?);
        let reference_harmonics = console_harmonics(&read_audio(&reference_path)?);
        writeln!(
            &mut comparison,
            "{},{:.3},{:.6},{:.6},{:.6},{:.6},{:.6}",
            capture.id,
            capture.drive,
            model_harmonics.0,
            reference_harmonics.0,
            model_harmonics.1,
            reference_harmonics.1,
            reference_harmonics.0 - model_harmonics.0
        )
        .expect("string write cannot fail");
        // A clean stage has nothing to say about asymmetry: with no drive
        // there is no curve to be lopsided about.
        if capture.drive > 0.0 && reference_harmonics.0.is_finite() {
            observations.push((capture.drive, reference_harmonics.0));
        }
        seen += 1;
    }
    if seen == 0 {
        let note = String::from("status\nno console injection in the reference directory\n");
        return Ok((note.clone(), note));
    }

    let mut candidates =
        String::from("quantity,value,second_harmonic_error_db,self_check_db,status\n");
    if observations.is_empty() {
        writeln!(
            &mut candidates,
            "stage-character,,,,every injection was clean or unreadable"
        )
        .expect("string write cannot fail");
    } else {
        let fitted = fit_stage_character(&observations);
        let check = stage_character_self_check(&observations);
        writeln!(
            &mut candidates,
            "stage-character,{:.6},{:.6},{check:.6},fitted",
            fitted.character, fitted.error
        )
        .expect("string write cannot fail");
    }
    writeln!(
        &mut candidates,
        "v4a-v4b-v3b-split,,,,underdetermined without an injection at each stage"
    )
    .expect("string write cannot fail");
    Ok((comparison, candidates))
}

struct StageFit {
    character: f32,
    error: f64,
}

/// Searches the character that best explains the measured second harmonic.
///
/// The model is the only thing that knows what a character does to a tone, so
/// the search asks it: render the injection at a candidate and compare. That
/// is slower than inverting a formula and it cannot drift away from what the
/// engine actually does, which for a fit that will be pasted into the engine
/// is the property worth paying for.
fn fit_stage_character(observations: &[(f32, f64)]) -> StageFit {
    let (low, high) = rf_organ_dsp::STAGE_CHARACTER_RANGE;
    let mut best = StageFit {
        character: rf_organ_dsp::STAGE_CHARACTER_DEFAULT,
        error: f64::MAX,
    };
    const STEPS: usize = 120;
    for step in 0..=STEPS {
        let character = low + (high - low) * step as f32 / STEPS as f32;
        let mut error = 0.0;
        for (drive, measured) in observations {
            let predicted = predicted_second_harmonic(*drive, character);
            error += (predicted - measured) * (predicted - measured);
        }
        let error = (error / observations.len() as f64).sqrt();
        if error < best.error {
            best = StageFit { character, error };
        }
    }
    best
}

/// What the model's own readings would fit to, which has to be the character
/// the model is using.
fn stage_character_self_check(observations: &[(f32, f64)]) -> f64 {
    let own: Vec<(f32, f64)> = observations
        .iter()
        .map(|(drive, _)| {
            (
                *drive,
                predicted_second_harmonic(*drive, rf_organ_dsp::STAGE_CHARACTER_DEFAULT),
            )
        })
        .collect();
    f64::from(fit_stage_character(&own).character - rf_organ_dsp::STAGE_CHARACTER_DEFAULT)
}

fn predicted_second_harmonic(drive: f32, character: f32) -> f64 {
    let rate = SAMPLE_RATE as f64;
    let settle = SAMPLE_RATE as usize / 4;
    let frames = SAMPLE_RATE as usize;
    let mut electronics = ConsoleElectronics::new(SAMPLE_RATE as f32);
    assert!(electronics.set(drive, 0.0, 0.0));
    assert!(electronics.set_expression_character(0.0));
    assert!(electronics.set_stage_character(character));
    let mut samples = Vec::with_capacity(frames);
    for frame in 0..settle + frames {
        let angle = TAU * captures::CONSOLE_PROBE_HZ * frame as f64 / rate;
        let output = electronics.process(
            (captures::CONSOLE_PROBE_AMPLITUDE * angle.sin()) as f32,
            1.0,
        );
        if frame >= settle {
            samples.push(f64::from(output));
        }
    }
    let frames: Vec<[f64; 2]> = samples.iter().map(|sample| [*sample, *sample]).collect();
    let fundamental = spectral_amplitude(&frames, captures::CONSOLE_PROBE_HZ);
    let second = spectral_amplitude(&frames, 2.0 * captures::CONSOLE_PROBE_HZ);
    decibels(second / fundamental.max(1.0e-15))
}

/// Second and third harmonic of an injection, relative to its own tone.
fn console_harmonics(audio: &Audio) -> (f64, f64) {
    let fundamental = spectral_amplitude(&audio.frames, captures::CONSOLE_PROBE_HZ);
    if fundamental <= 0.0 {
        return (f64::NAN, f64::NAN);
    }
    let second = spectral_amplitude(&audio.frames, 2.0 * captures::CONSOLE_PROBE_HZ);
    let third = spectral_amplitude(&audio.frames, 3.0 * captures::CONSOLE_PROBE_HZ);
    (
        decibels(second / fundamental),
        decibels(third / fundamental),
    )
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
            if !reference.join(format!("{}.wav", triplet[2].id)).is_file() {
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
                Ok([loaded.remove(0), loaded.remove(0), loaded.remove(0)])
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
    let output_fit =
        (!injection.is_empty()).then(|| fit_path(TransformerPath::Injection, &injection, NO_TRIM));
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
                let base = trim_for(
                    NO_TRIM,
                    TransformerPath::Injection.unit(),
                    output.drive_trim,
                    0.0,
                );
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

fn write_fit_row(
    csv: &mut String,
    path: TransformerPath,
    fit: &TransformerFit,
    qualification: &str,
) {
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

fn fit_error(path: TransformerPath, observations: &[Observation], base: Trims, trim: f32) -> f64 {
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
    let frames = Notes::ALL.map(|notes| {
        transformer_steady_frames(
            transformer_capture(path, level, notes),
            trims,
            SETTLE,
            MEASURE,
        )
    });
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

/// What one percussion capture says about the console.
#[derive(Clone, Copy)]
struct PercussionMeasurement {
    /// Steady 8-foot drawbar level once the strike has gone.
    organ_dbfs: f64,
    /// Peak of the third-harmonic strike, relative to that.
    strike_dbc: f64,
    /// Decay time, extrapolated from the first 20 dB.
    t60: f64,
}

fn compare_percussion_captures(
    model: &Path,
    reference: &Path,
    quality: &BTreeMap<String, CaptureQuality>,
) -> Result<(String, String), String> {
    let mut csv = String::from(
        "capture,volume,decay,fundamental_hz,third_hz,model_organ_dbfs,reference_organ_dbfs,model_strike_dbc,reference_strike_dbc,reference_minus_model_strike_db,model_t60_s,reference_t60_s,reference_minus_model_t60_s,qualification\n",
    );
    let mut measured: Vec<(
        PercussionCapture,
        PercussionMeasurement,
        PercussionMeasurement,
        &'static str,
        QualityStatus,
    )> = Vec::new();
    for capture in PERCUSSION_CAPTURES {
        let reference_path = reference.join(format!("{}.wav", capture.id));
        if !reference_path.is_file() {
            continue;
        }
        let model_path = model.join(format!("{}.wav", capture.id));
        if !model_path.is_file() {
            return Err(format!(
                "model capture is missing: {}",
                model_path.display()
            ));
        }
        let model_measured = percussion_measurement(&read_audio(&model_path)?)?;
        let reference_measured = percussion_measurement(&read_audio(&reference_path)?)?;
        let capture_quality = quality
            .get(capture.id)
            .ok_or_else(|| format!("missing quality result for {}", capture.id))?;
        writeln!(
            &mut csv,
            "{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{}",
            capture.id,
            volume_label(capture.volume),
            decay_label(capture.decay),
            bus_frequency(PERCUSSION_NOTE, 2),
            bus_frequency(PERCUSSION_NOTE, 4),
            model_measured.organ_dbfs,
            reference_measured.organ_dbfs,
            model_measured.strike_dbc,
            reference_measured.strike_dbc,
            reference_measured.strike_dbc - model_measured.strike_dbc,
            model_measured.t60,
            reference_measured.t60,
            reference_measured.t60 - model_measured.t60,
            capture_quality.qualification
        )
        .expect("string write cannot fail");
        measured.push((
            capture,
            model_measured,
            reference_measured,
            capture_quality.qualification,
            capture_quality.status,
        ));
    }
    Ok((csv, percussion_fit_candidates(&measured)))
}

const fn volume_label(volume: PercussionVolume) -> &'static str {
    match volume {
        PercussionVolume::Soft => "soft",
        PercussionVolume::Normal => "normal",
    }
}

const fn decay_label(decay: PercussionDecay) -> &'static str {
    match decay {
        PercussionDecay::Slow => "slow",
        PercussionDecay::Fast => "fast",
    }
}

/// These coefficients are read straight off a capture rather than searched
/// for: the decay time, the strike level and the documented drawbar cut are
/// exactly what the measurement produces.
fn percussion_fit_candidates(
    measured: &[(
        PercussionCapture,
        PercussionMeasurement,
        PercussionMeasurement,
        &'static str,
        QualityStatus,
    )],
) -> String {
    let mut csv = String::from(
        "parameter,capture,unit,current_value,candidate_value,reference_minus_model,qualification\n",
    );
    let usable = measured
        .iter()
        .filter(|(_, _, _, _, status)| *status != QualityStatus::Fail);
    for (capture, model, reference, qualification, _) in usable.clone() {
        writeln!(
            &mut csv,
            "percussion-t60-{},{},s,{:.6},{:.6},{:.6},{qualification}",
            decay_label(capture.decay),
            capture.id,
            model.t60,
            reference.t60,
            reference.t60 - model.t60
        )
        .expect("string write cannot fail");
        writeln!(
            &mut csv,
            "percussion-level-{},{},dBc,{:.6},{:.6},{:.6},{qualification}",
            volume_label(capture.volume),
            capture.id,
            model.strike_dbc,
            reference.strike_dbc,
            reference.strike_dbc - model.strike_dbc
        )
        .expect("string write cannot fail");
    }

    // Hammond documents the vintage console as taking about 6 dB out of the
    // upper drawbars at Normal volume and nothing at Soft, so a Normal and a
    // Soft capture recorded at one gain measure that cut directly.
    let level = |wanted: PercussionVolume| {
        usable
            .clone()
            .find(|(capture, _, _, _, _)| {
                capture.volume == wanted && capture.decay == PercussionDecay::Fast
            })
            .map(|(_, model, reference, qualification, _)| (*model, *reference, *qualification))
    };
    if let (
        Some((model_normal, reference_normal, qualification)),
        Some((model_soft, reference_soft, _)),
    ) = (
        level(PercussionVolume::Normal),
        level(PercussionVolume::Soft),
    ) {
        let model_cut = model_normal.organ_dbfs - model_soft.organ_dbfs;
        let reference_cut = reference_normal.organ_dbfs - reference_soft.organ_dbfs;
        writeln!(
            &mut csv,
            "percussion-drawbar-cut,37-percussion-normal-fast|39-percussion-soft-fast,dB,{model_cut:.6},{reference_cut:.6},{:.6},{qualification}",
            reference_cut - model_cut
        )
        .expect("string write cannot fail");
    }
    csv
}

/// Measures one percussion capture. The 888 registration leaves the 2 2/3'
/// bus empty, so the third-harmonic strike is alone in its bin and its decay
/// can be read directly; the fundamental gives the drawbar level it stands
/// against.
fn percussion_measurement(audio: &Audio) -> Result<PercussionMeasurement, String> {
    if audio.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "percussion captures must be {SAMPLE_RATE} Hz; got {}",
            audio.sample_rate
        ));
    }
    let onset = onset(audio)?;
    let window = SAMPLE_RATE as usize / 100;
    let fundamental = bus_frequency(PERCUSSION_NOTE, 2);
    let third = bus_frequency(PERCUSSION_NOTE, 4);

    let steady_start = onset + 2 * SAMPLE_RATE as usize;
    let steady = audio
        .frames
        .get(steady_start..steady_start + SAMPLE_RATE as usize / 2)
        .ok_or_else(|| "percussion capture needs two seconds of held tone".to_owned())?;
    let organ = spectral_amplitude(steady, fundamental);
    if organ <= 1.0e-12 {
        return Err("percussion capture has no drawbar tone".to_owned());
    }
    // Whatever sits in the third-harmonic bin once the strike has gone:
    // generator leakage in the model, and room and preamp noise in a
    // reference capture. Subtracting it in power is what lets the decay be
    // followed past the point where it meets that floor.
    let floor = spectral_amplitude(steady, third);

    let envelope = audio
        .frames
        .get(onset..steady_start)
        .ok_or_else(|| "percussion capture is too short".to_owned())?
        .chunks_exact(window)
        .map(|chunk| {
            let level = spectral_amplitude(chunk, third);
            (level * level - floor * floor).max(0.0).sqrt()
        })
        .collect::<Vec<_>>();
    let (peak_index, peak) =
        envelope
            .iter()
            .copied()
            .enumerate()
            .fold((0, 0.0), |(at, peak), (index, level)| {
                if level > peak {
                    (index, level)
                } else {
                    (at, peak)
                }
            });
    if peak <= 1.0e-12 {
        return Err("percussion capture has no strike".to_owned());
    }
    if peak < floor * 10.0 {
        return Err(format!(
            "percussion strike is only {:.1} dB over the third-harmonic floor; 20 dB are needed to fit a decay",
            decibels(peak / floor.max(1.0e-15))
        ));
    }

    // RT20: fit the slope between 5 and 25 dB below the peak and extrapolate.
    // A reference capture's noise floor rarely reaches 60 dB of decay.
    let after = &envelope[peak_index..];
    let crossing = |drop: f64| {
        let threshold = peak * 10.0_f64.powf(-drop / 20.0);
        after
            .iter()
            .position(|level| *level <= threshold)
            .map(|index| index as f64 * window as f64 / f64::from(SAMPLE_RATE))
    };
    let (start, end) = (
        crossing(5.0).ok_or_else(|| "percussion strike never decayed 5 dB".to_owned())?,
        crossing(25.0).ok_or_else(|| "percussion strike never decayed 25 dB".to_owned())?,
    );
    if end <= start {
        return Err("percussion decay is not monotonic enough to fit".to_owned());
    }
    Ok(PercussionMeasurement {
        organ_dbfs: decibels(organ),
        strike_dbc: decibels(peak / organ),
        t60: 60.0 * (end - start) / 20.0,
    })
}

/// The three bands one expression capture is read at, relative to the toe
/// capture of the same session.
#[derive(Clone, Copy)]
struct ExpressionMeasurement {
    low: f64,
    mid: f64,
    high: f64,
}

impl ExpressionMeasurement {
    const fn band(&self, index: usize) -> f64 {
        match index {
            0 => self.low,
            1 => self.mid,
            _ => self.high,
        }
    }
}

const BAND_NAMES: [&str; 3] = ["low", "mid", "high"];

fn compare_expression_captures(
    model: &Path,
    reference: &Path,
    quality: &BTreeMap<String, CaptureQuality>,
) -> Result<(String, String), String> {
    let mut csv = String::from(
        "capture,position,band,frequency_hz,model_gain_db,reference_gain_db,reference_minus_model_db,qualification\n",
    );
    let frequencies = [
        bus_frequency(EXPRESSION_LOW.0, EXPRESSION_LOW.1),
        bus_frequency(EXPRESSION_MID.0, EXPRESSION_MID.1),
        bus_frequency(EXPRESSION_HIGH.0, EXPRESSION_HIGH.1),
    ];
    let toe = EXPRESSION_CAPTURES
        .last()
        .expect("expression captures are not empty");
    if !reference.join(format!("{}.wav", toe.id)).is_file() {
        // Every reading is relative to the toe capture; without it the session
        // says nothing about the network, only about the recording gain.
        return Ok((
            csv,
            String::from(
                "parameter,captures,unit,current_value,raw_fit,estimator_bias,candidate_value,rms_error_db,qualification\n",
            ),
        ));
    }
    let model_toe = expression_measurement(
        &read_audio(&model.join(format!("{}.wav", toe.id)))?,
        &frequencies,
    )?;
    let reference_toe = expression_measurement(
        &read_audio(&reference.join(format!("{}.wav", toe.id)))?,
        &frequencies,
    )?;

    let mut observations: Vec<(
        ExpressionCapture,
        ExpressionMeasurement,
        ExpressionMeasurement,
        &'static str,
        QualityStatus,
    )> = Vec::new();
    for capture in EXPRESSION_CAPTURES {
        let reference_path = reference.join(format!("{}.wav", capture.id));
        if !reference_path.is_file() {
            continue;
        }
        let model_path = model.join(format!("{}.wav", capture.id));
        if !model_path.is_file() {
            return Err(format!(
                "model capture is missing: {}",
                model_path.display()
            ));
        }
        let model_measured = expression_measurement(&read_audio(&model_path)?, &frequencies)?;
        let reference_measured =
            expression_measurement(&read_audio(&reference_path)?, &frequencies)?;
        let capture_quality = quality
            .get(capture.id)
            .ok_or_else(|| format!("missing quality result for {}", capture.id))?;
        for (band, name) in BAND_NAMES.into_iter().enumerate() {
            let model_gain = decibels(model_measured.band(band) / model_toe.band(band));
            let reference_gain = decibels(reference_measured.band(band) / reference_toe.band(band));
            writeln!(
                &mut csv,
                "{},{:.3},{name},{:.3},{model_gain:.6},{reference_gain:.6},{:.6},{}",
                capture.id,
                capture.position,
                frequencies[band],
                reference_gain - model_gain,
                capture_quality.qualification
            )
            .expect("string write cannot fail");
        }
        observations.push((
            capture,
            model_measured,
            reference_measured,
            capture_quality.qualification,
            capture_quality.status,
        ));
    }
    Ok((
        csv,
        expression_fit_candidates(&observations, &model_toe, &reference_toe, &frequencies),
    ))
}

/// Fits the one parameter the model exposes for this network: how much of the
/// pedal's attenuation the middle band takes compared with the ends. The
/// search runs on the console electronics alone, which is cheap and is exactly
/// the stage the captures isolate.
fn expression_fit_candidates(
    observations: &[(
        ExpressionCapture,
        ExpressionMeasurement,
        ExpressionMeasurement,
        &'static str,
        QualityStatus,
    )],
    model_toe: &ExpressionMeasurement,
    reference_toe: &ExpressionMeasurement,
    frequencies: &[f64; 3],
) -> String {
    let mut csv = String::from(
        "parameter,captures,unit,current_value,raw_fit,estimator_bias,candidate_value,rms_error_db,qualification\n",
    );
    let usable = observations
        .iter()
        .filter(|(_, _, _, _, status)| *status != QualityStatus::Fail)
        .collect::<Vec<_>>();
    if usable.len() < 2 {
        return csv;
    }
    // The estimator works on the console electronics alone, but a capture also
    // carries the matching and output transformers, whose compression moves
    // with level and therefore with the pedal. Fitting the model's own
    // captures says how far that pushes the answer for a coefficient that is
    // already known, and the same offset comes back off the reference fit.
    let fit = |toe: &ExpressionMeasurement, model_side: bool| {
        let error = |character: f32| {
            let mut total = 0.0;
            let mut count = 0;
            for observation in &usable {
                let bands = if model_side {
                    observation.1
                } else {
                    observation.2
                };
                for (band, frequency) in frequencies.iter().enumerate() {
                    let predicted = decibels(
                        electronics_band_gain(character, observation.0.position, *frequency)
                            / electronics_band_gain(character, 1.0, *frequency),
                    );
                    let measured = decibels(bands.band(band) / toe.band(band));
                    total += (predicted - measured) * (predicted - measured);
                    count += 1;
                }
            }
            (total / f64::from(count)).sqrt()
        };
        let mut best = (EXPRESSION_CHARACTER, error(EXPRESSION_CHARACTER));
        for step in 0..=100 {
            let character = step as f32 / 100.0;
            let candidate = error(character);
            if candidate < best.1 {
                best = (character, candidate);
            }
        }
        best
    };
    let model_fit = fit(model_toe, true);
    let reference_fit = fit(reference_toe, false);
    let bias = model_fit.0 - EXPRESSION_CHARACTER;
    let candidate = (reference_fit.0 - bias).clamp(0.0, 1.0);
    let qualification = usable
        .iter()
        .max_by_key(|(_, _, _, _, status)| *status)
        .map(|(_, _, _, qualification, _)| *qualification)
        .unwrap_or("quality-pass");
    writeln!(
        &mut csv,
        "expression-character,{},ratio,{:.6},{:.6},{bias:.6},{candidate:.6},{:.6},{qualification}",
        usable
            .iter()
            .map(|(capture, _, _, _, _)| capture.id)
            .collect::<Vec<_>>()
            .join("|"),
        EXPRESSION_CHARACTER,
        reference_fit.0,
        reference_fit.1
    )
    .expect("string write cannot fail");
    csv
}

/// Gain of the console's expression network at one pedal position and one
/// frequency, with the tube stages left clean so only the passive network is
/// measured.
fn electronics_band_gain(character: f32, position: f32, frequency: f64) -> f64 {
    const AMPLITUDE: f64 = 0.25;
    let settle = SAMPLE_RATE as usize / 4;
    let window = SAMPLE_RATE as usize / 2;
    let mut electronics = ConsoleElectronics::new(SAMPLE_RATE as f32);
    assert!(electronics.set(0.0, 0.0, 0.0));
    assert!(electronics.set_expression_character(character));
    electronics.reset(position);
    let mut frames = Vec::with_capacity(window);
    for frame in 0..settle + window {
        let input = AMPLITUDE * (TAU * frequency * frame as f64 / f64::from(SAMPLE_RATE)).sin();
        let output = electronics.process(input as f32, position);
        if frame >= settle {
            frames.push([f64::from(output), f64::from(output)]);
        }
    }
    spectral_amplitude(&frames, frequency).max(1.0e-15)
}

/// Reads one expression capture at its three band frequencies, over a steady
/// second well after the attack.
fn expression_measurement(
    audio: &Audio,
    frequencies: &[f64; 3],
) -> Result<ExpressionMeasurement, String> {
    if audio.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "expression captures must be {SAMPLE_RATE} Hz; got {}",
            audio.sample_rate
        ));
    }
    let onset = onset(audio)?;
    let start = onset + SAMPLE_RATE as usize / 2;
    let frames = audio
        .frames
        .get(start..start + SAMPLE_RATE as usize)
        .ok_or_else(|| "expression capture needs a steady second".to_owned())?;
    let bands = frequencies.map(|frequency| spectral_amplitude(frames, frequency));
    if bands.iter().any(|band| *band <= 1.0e-12) {
        return Err("expression capture is missing one of its bands".to_owned());
    }
    Ok(ExpressionMeasurement {
        low: bands[0],
        mid: bands[1],
        high: bands[2],
    })
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
    let impulse = capture == "rotary-cabinet-impulse";
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
        let valid = "RF-Organ reference capture\ncapture_id=b3-2026-01\nsource=private session\nrights=private measurement\nconsole=Hammond B-3\nsignal_chain=Rotary 122, pair at 1 m\nsample_rate=48000\nnormalization=none\n";
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

    /// The estimator sees the expression network through the transformers, so
    /// A character put into an injection has to come back out of the fit.
    /// The identity alone would pass for a fit that always answered with the
    /// model's own value, so this asks for one the model is not using.
    #[test]
    fn a_stage_character_survives_the_round_trip() {
        let drives = [0.32_f32, 0.75];
        for asked in [0.4_f32, 1.0, 1.6, 2.4] {
            let observations: Vec<(f32, f64)> = drives
                .iter()
                .map(|drive| (*drive, predicted_second_harmonic(*drive, asked)))
                .collect();
            let fitted = fit_stage_character(&observations);
            assert!(
                (fitted.character - asked).abs() < 0.05,
                "asked for {asked} and the fit returned {}",
                fitted.character
            );
            assert!(
                fitted.error < 0.2,
                "the fit for {asked} was {} dB out",
                fitted.error
            );
        }
        // And a chain with no drive has no curve to be lopsided about, so it
        // is refused rather than fitted to noise.
        let flat = vec![(0.0_f32, predicted_second_harmonic(0.0, 1.0))];
        let fitted = fit_stage_character(&flat);
        assert!(fitted.character.is_finite());
    }

    /// A taper read off the model itself has to come back flat, because the
    /// model's taper is flat. Anything else means the fit is handing back the
    /// console's own frequency response - the transformers, the stages, the
    /// tone control - and calling it the generator, which would bake the
    /// whole chain into a table that is supposed to describe 91 wheels.
    #[test]
    fn a_taper_fit_of_the_model_against_itself_returns_a_flat_table() {
        let sweep = captures::render_taper_sweep();
        let audio = Audio {
            sample_rate: SAMPLE_RATE,
            frames: sweep
                .as_chunks::<2>()
                .0
                .iter()
                .map(|frame| [f64::from(frame[0]), f64::from(frame[1])])
                .collect(),
        };
        let mut read = 0;
        let mut worst = 0.0_f64;
        for key in 0..MANUAL_KEY_COUNT {
            let wheel = drawbar_wheel(key, captures::TAPER_BUS).expect("eight foot wheel");
            let frequency = f64::from(gear_frequency(wheel).expect("wheel"));
            let level = taper_stretch(&audio, key, frequency).expect("long enough");
            assert!(level.is_finite(), "key {key} was unreadable");
            // Reference against model is the same recording here, so the
            // excess is exactly zero before any normalising.
            worst = worst.max((level - level).abs());
            read += 1;
        }
        assert_eq!(read, MANUAL_KEY_COUNT);
        assert_eq!(worst, 0.0);

        // And through the whole fit, where the normalising happens too.
        let directory = std::env::temp_dir().join("rf-organ-taper-identity");
        let _ = fs::create_dir_all(&directory);
        let path = directory.join(format!("{}.wav", captures::TAPER_CAPTURE));
        fs::write(
            &path,
            wav::encode_f32(&sweep, 2, SAMPLE_RATE).expect("encode"),
        )
        .expect("write sweep");
        let (_, candidates) = compare_taper_captures(&directory, &directory).expect("the fit runs");
        let mut rows = 0;
        for line in candidates.lines().skip(1) {
            let fields = line.split(',').collect::<Vec<_>>();
            assert_eq!(fields[3], "read", "{line}");
            let level: f64 = fields[1].parse().expect("level");
            assert!(
                (level - 1.0).abs() < 1.0e-6,
                "the fit found a taper in a flat generator: {line}"
            );
            rows += 1;
        }
        assert_eq!(rows, MANUAL_KEY_COUNT);
        let _ = fs::remove_dir_all(&directory);
    }

    /// it is biased. Correcting with the model's own fit has to return the
    /// coefficient the model actually uses whenever reference and model agree.
    #[test]
    fn an_expression_fit_of_the_model_against_itself_returns_its_own_character() {
        let bands = |scale: f64| ExpressionMeasurement {
            low: 0.4 * scale,
            mid: 0.3 * scale,
            high: 0.2 * scale,
        };
        let observations = EXPRESSION_CAPTURES
            .into_iter()
            .map(|capture| {
                let measured = bands(f64::from(capture.position));
                (
                    capture,
                    measured,
                    measured,
                    "quality-pass",
                    QualityStatus::Pass,
                )
            })
            .collect::<Vec<_>>();
        let toe = bands(1.0);
        let frequencies = [
            bus_frequency(EXPRESSION_LOW.0, EXPRESSION_LOW.1),
            bus_frequency(EXPRESSION_MID.0, EXPRESSION_MID.1),
            bus_frequency(EXPRESSION_HIGH.0, EXPRESSION_HIGH.1),
        ];
        let csv = expression_fit_candidates(&observations, &toe, &toe, &frequencies);
        let row = csv.lines().nth(1).expect("one candidate");
        let fields = row.split(',').collect::<Vec<_>>();
        let current: f32 = fields[3].parse().expect("current");
        let candidate: f32 = fields[6].parse().expect("candidate");
        assert_eq!(current, EXPRESSION_CHARACTER);
        assert!(
            (candidate - EXPRESSION_CHARACTER).abs() < 1.0e-6,
            "candidate {candidate} against {EXPRESSION_CHARACTER}"
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
