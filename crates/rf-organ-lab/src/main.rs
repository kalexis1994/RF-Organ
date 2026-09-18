// SPDX-License-Identifier: GPL-2.0-or-later

mod analysis;
mod captures;
mod compare;
mod invariance;
mod signal;
mod wav;

use captures::{PHRASE_SECONDS as SECONDS, SAMPLE_RATE, TRANSFORMER_CAPTURES};
use rf_organ_dsp::{
    OrganEngine, OrganPart, PercussionDecay, PercussionHarmonic, PercussionVolume, Registration,
    Rotary, RotaryMode, ScannerMode, TONEWHEEL_COUNT, gear_frequency,
};
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("ERROR: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let command = arguments.next();
    match command.as_deref().and_then(|command| command.to_str()) {
        Some("render") => {
            let destination = arguments.next().ok_or("render requires OUTPUT_DIRECTORY")?;
            let trims = match arguments.next() {
                None => captures::NO_TRIM,
                Some(flag) if flag == *"--transformer-drive-trims" => {
                    let values = arguments
                        .next()
                        .ok_or("--transformer-drive-trims requires T1,T2,T3")?;
                    parse_drive_trims(values.to_str().ok_or("trims must be UTF-8")?)?
                }
                Some(_) => return Err(usage().into()),
            };
            if arguments.next().is_some() {
                return Err(usage().into());
            }
            render_suite(&PathBuf::from(destination), trims)
        }
        Some("compare") => {
            let model = arguments.next().ok_or("compare requires MODEL_DIRECTORY")?;
            let reference = arguments
                .next()
                .ok_or("compare requires REFERENCE_DIRECTORY")?;
            let destination = arguments
                .next()
                .ok_or("compare requires OUTPUT_DIRECTORY")?;
            if arguments.next().is_some() {
                return Err(usage().into());
            }
            compare_suite(
                &PathBuf::from(model),
                &PathBuf::from(reference),
                &PathBuf::from(destination),
            )
        }
        Some("sweep") => {
            let destination = arguments.next().ok_or("sweep requires OUTPUT_DIRECTORY")?;
            if arguments.next().is_some() {
                return Err(usage().into());
            }
            sweep_suite(&PathBuf::from(destination))
        }
        _ => Err(usage().into()),
    }
}

/// Measures one quantity per subsystem at every supported sample rate and
/// benchmarks the engine at each of them. Exits with an error when a quantity
/// that should be defined in seconds or hertz moves with the sample rate.
fn sweep_suite(destination: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(destination)?;
    let invariants = invariance::survey()?;
    fs::write(
        destination.join("sample-rate-invariance.csv"),
        invariance::report(&invariants),
    )?;
    let performance = invariance::benchmark()?;
    fs::write(
        destination.join("performance.csv"),
        invariance::performance_report(&performance),
    )?;
    let slowest = performance
        .iter()
        .filter(|measurement| measurement.load == invariance::Load::Rotary)
        .map(|measurement| measurement.realtime_factor)
        .fold(f64::INFINITY, f64::min);
    let rate_dependent = invariants
        .iter()
        .filter(|invariant| !invariant.holds())
        .map(|invariant| format!("{}/{}", invariant.probe, invariant.metric))
        .collect::<Vec<_>>();
    println!(
        "RF_ORGAN_LAB_SWEPT path={} invariants={} rate_dependent={} slowest_realtime_factor={slowest:.1}",
        destination.display(),
        invariants.len(),
        rate_dependent.len()
    );
    if rate_dependent.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "sample-rate dependent quantities: {}",
            rate_dependent.join(", ")
        )
        .into())
    }
}

const fn usage() -> &'static str {
    "usage:\n  rf-organ-lab render OUTPUT_DIRECTORY [--transformer-drive-trims T1,T2,T3]\n  rf-organ-lab compare MODEL_DIRECTORY REFERENCE_DIRECTORY OUTPUT_DIRECTORY\n  rf-organ-lab sweep OUTPUT_DIRECTORY"
}

/// Parses the optional per-transformer drive trims. They exist so that a
/// synthetic reference set with known coefficients can be produced and fed
/// back through the comparator; the calibration render uses no trim.
fn parse_drive_trims(values: &str) -> Result<captures::Trims, Box<dyn Error>> {
    let mut trims = captures::NO_TRIM;
    let mut parsed = 0;
    for (slot, value) in trims.iter_mut().zip(values.split(',')) {
        let drive = value
            .trim()
            .parse::<f32>()
            .map_err(|error| format!("invalid drive trim {value}: {error}"))?;
        if !(-1.0..=1.0).contains(&drive) {
            return Err(format!("drive trim {drive} is outside -1..1").into());
        }
        slot.0 = drive;
        parsed += 1;
    }
    if parsed != trims.len() || values.split(',').count() != trims.len() {
        return Err("--transformer-drive-trims needs exactly T1,T2,T3".into());
    }
    Ok(trims)
}

fn compare_suite(model: &Path, reference: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(destination)?;
    let reports = compare::compare_directories(model, reference)?;
    fs::write(
        destination.join("capture-comparison.csv"),
        reports.capture_comparison,
    )?;
    fs::write(
        destination.join("pedal-spectrum-comparison.csv"),
        reports.pedal_spectrum_comparison,
    )?;
    fs::write(
        destination.join("pedal-fit-candidates.csv"),
        reports.pedal_fit_candidates,
    )?;
    fs::write(
        destination.join("reference-quality.csv"),
        reports.reference_quality,
    )?;
    fs::write(
        destination.join("transformer-intermodulation-comparison.csv"),
        reports.transformer_intermodulation_comparison,
    )?;
    fs::write(
        destination.join("transformer-fit-candidates.csv"),
        reports.transformer_fit_candidates,
    )?;
    fs::write(
        destination.join("percussion-comparison.csv"),
        reports.percussion_comparison,
    )?;
    fs::write(
        destination.join("percussion-fit-candidates.csv"),
        reports.percussion_fit_candidates,
    )?;
    fs::write(
        destination.join("expression-comparison.csv"),
        reports.expression_comparison,
    )?;
    fs::write(
        destination.join("console-fit-candidates.csv"),
        reports.console_fit_candidates,
    )?;
    fs::write(
        destination.join("taper-comparison.csv"),
        reports.taper_comparison,
    )?;
    fs::write(
        destination.join("taper-fit-candidates.csv"),
        reports.taper_fit_candidates,
    )?;
    fs::write(
        destination.join("console-stage-comparison.csv"),
        reports.console_stage_comparison,
    )?;
    fs::write(
        destination.join("console-stage-candidates.csv"),
        reports.console_stage_candidates,
    )?;
    println!(
        "RF_ORGAN_LAB_COMPARED model={} reference={} path={}",
        model.display(),
        reference.display(),
        destination.display()
    );
    Ok(())
}

fn render_suite(destination: &Path, trims: captures::Trims) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(destination)?;
    fs::write(
        destination.join("tonewheel-frequencies.csv"),
        frequency_table(),
    )?;

    for scenario in Scenario::ALL {
        let samples = render_phrase(scenario)?;
        fs::write(
            destination.join(format!("{}.wav", scenario.id())),
            wav::encode_f32(&samples, 2, SAMPLE_RATE)?,
        )?;
    }
    for capture in &TRANSFORMER_CAPTURES {
        let samples = captures::render_transformer_phrase(capture, trims);
        fs::write(
            destination.join(format!("{}.wav", capture.id)),
            wav::encode_f32(&samples, 2, SAMPLE_RATE)?,
        )?;
    }
    for capture in &captures::PERCUSSION_CAPTURES {
        let samples = captures::render_percussion_phrase(capture);
        fs::write(
            destination.join(format!("{}.wav", capture.id)),
            wav::encode_f32(&samples, 2, SAMPLE_RATE)?,
        )?;
    }
    for capture in &captures::EXPRESSION_CAPTURES {
        let samples = captures::render_expression_phrase(capture);
        fs::write(
            destination.join(format!("{}.wav", capture.id)),
            wav::encode_f32(&samples, 2, SAMPLE_RATE)?,
        )?;
    }
    for capture in &captures::CONSOLE_CAPTURES {
        let samples = captures::render_console_injection(capture);
        fs::write(
            destination.join(format!("{}.wav", capture.id)),
            wav::encode_f32(&samples, 2, SAMPLE_RATE)?,
        )?;
    }
    for capture in &captures::CONSOLE_STAGE_CAPTURES {
        let samples = captures::render_console_stage(capture);
        fs::write(
            destination.join(format!("{}.wav", capture.id)),
            wav::encode_f32(&samples, 2, SAMPLE_RATE)?,
        )?;
    }
    let sweep = captures::render_taper_sweep();
    fs::write(
        destination.join(format!("{}.wav", captures::TAPER_CAPTURE)),
        wav::encode_f32(&sweep, 2, SAMPLE_RATE)?,
    )?;
    let impulse = render_rotary_impulse();
    fs::write(
        destination.join("rotary-cabinet-impulse.wav"),
        wav::encode_f32(&impulse, 2, SAMPLE_RATE)?,
    )?;
    fs::write(destination.join("manifest.txt"), manifest(trims))?;
    let analysis = analysis::analyze()?;
    fs::write(destination.join("measurements.csv"), analysis.measurements)?;
    fs::write(
        destination.join("percussion-envelope.csv"),
        analysis.percussion_envelope,
    )?;
    fs::write(
        destination.join("percussion-recovery.csv"),
        analysis.percussion_recovery,
    )?;
    fs::write(
        destination.join("keying-contacts.csv"),
        analysis.keying_contacts,
    )?;
    fs::write(
        destination.join("key-click-response.csv"),
        analysis.key_click_response,
    )?;
    fs::write(
        destination.join("registration-response.csv"),
        analysis.registration_response,
    )?;
    fs::write(
        destination.join("preset-panel-response.csv"),
        analysis.preset_panel_response,
    )?;
    fs::write(
        destination.join("scanner-sidebands.csv"),
        analysis.scanner_sidebands,
    )?;
    fs::write(
        destination.join("scanner-line-response.csv"),
        analysis.scanner_line_response,
    )?;
    fs::write(
        destination.join("scanner-line-cutoff.csv"),
        analysis.scanner_line_cutoff,
    )?;
    fs::write(
        destination.join("rotary-rotor-response.csv"),
        analysis.rotary_rotor_response,
    )?;
    fs::write(
        destination.join("rotary-doppler.csv"),
        analysis.rotary_doppler,
    )?;
    fs::write(
        destination.join("rotary-stop-angle.csv"),
        analysis.rotary_stop_angle,
    )?;
    fs::write(destination.join("drive-wobble.csv"), analysis.drive_wobble)?;
    fs::write(
        destination.join("pedal-spectrum.csv"),
        analysis.pedal_spectrum,
    )?;
    fs::write(
        destination.join("pedal-release.csv"),
        analysis.pedal_release,
    )?;
    fs::write(
        destination.join("expression-response.csv"),
        analysis.expression_response,
    )?;
    fs::write(
        destination.join("tone-control-response.csv"),
        analysis.tone_control_response,
    )?;
    fs::write(
        destination.join("console-distortion.csv"),
        analysis.console_distortion,
    )?;
    fs::write(
        destination.join("generator-taper.csv"),
        analysis.generator_taper,
    )?;
    fs::write(
        destination.join("generator-leakage.csv"),
        analysis.generator_leakage,
    )?;
    fs::write(
        destination.join("transformer-intermodulation.csv"),
        analysis.transformer_intermodulation,
    )?;
    fs::write(
        destination.join("transformer-calibration.csv"),
        analysis.transformer_calibration,
    )?;
    println!("RF_ORGAN_LAB_RENDERED path={}", destination.display());
    Ok(())
}

#[derive(Clone, Copy)]
enum Scenario {
    Direct,
    Percussion,
    Scanner,
    Chorale,
    Tremolo,
    FullConsole,
    Pedal16,
    Pedal8,
    PedalBoth,
}

impl Scenario {
    const ALL: [Self; 9] = [
        Self::Direct,
        Self::Percussion,
        Self::Scanner,
        Self::Chorale,
        Self::Tremolo,
        Self::FullConsole,
        Self::Pedal16,
        Self::Pedal8,
        Self::PedalBoth,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::Direct => captures::PHRASE_CAPTURES[0],
            Self::Percussion => captures::PHRASE_CAPTURES[1],
            Self::Scanner => captures::PHRASE_CAPTURES[2],
            Self::Chorale => captures::PHRASE_CAPTURES[3],
            Self::Tremolo => captures::PHRASE_CAPTURES[4],
            Self::FullConsole => captures::PHRASE_CAPTURES[5],
            Self::Pedal16 => captures::PHRASE_CAPTURES[6],
            Self::Pedal8 => captures::PHRASE_CAPTURES[7],
            Self::PedalBoth => captures::PHRASE_CAPTURES[8],
        }
    }

    const fn is_pedal(self) -> bool {
        matches!(self, Self::Pedal16 | Self::Pedal8 | Self::PedalBoth)
    }
}

fn render_phrase(scenario: Scenario) -> Result<Vec<f32>, Box<dyn Error>> {
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).map_err(|error| error.0)?;
    configure(&mut engine, scenario);
    let frames = SAMPLE_RATE as usize * SECONDS;
    let mut output = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        phrase_events(&mut engine, scenario, frame);
        output.extend_from_slice(&engine.next_sample());
    }
    Ok(output)
}

fn configure(engine: &mut OrganEngine, scenario: Scenario) {
    let _ = engine.set_output_level(0.72);
    let _ = engine.set_transformer(captures::CHARACTER.0, captures::CHARACTER.1);
    let _ = engine.set_console(0.32, 0.0, 0.0);
    let _ = engine.set_expression_character(0.55);
    let _ = engine.set_rotary_cabinet(0.22);
    match scenario {
        Scenario::Direct => {}
        Scenario::Percussion => {
            engine.set_percussion_enabled(true);
            engine.set_percussion_harmonic(PercussionHarmonic::Third);
            engine.set_percussion_volume(PercussionVolume::Normal);
            engine.set_percussion_decay(PercussionDecay::Fast);
        }
        Scenario::Scanner => {
            engine.set_scanner_mode(ScannerMode::Chorus3);
            engine.set_scanner_manuals(true, false);
        }
        Scenario::Chorale => engine.set_rotary_mode(RotaryMode::Chorale),
        Scenario::Tremolo => engine.set_rotary_mode(RotaryMode::Tremolo),
        Scenario::FullConsole => {
            engine.set_scanner_mode(ScannerMode::Chorus3);
            engine.set_scanner_manuals(true, true);
            engine.set_rotary_mode(RotaryMode::Chorale);
            let _ = engine.set_transformer(0.62, 0.38);
            let _ = engine.set_console(0.48, 0.18, -0.08);
            for (index, position) in [8, 8, 8, 8, 6, 8, 4, 8, 6].into_iter().enumerate() {
                let _ = engine.set_manual_drawbar(
                    OrganPart::Upper,
                    Registration::AdjustB,
                    index,
                    position,
                );
            }
            for (index, position) in [8, 8, 8, 8, 6, 0, 0, 0, 0].into_iter().enumerate() {
                let _ = engine.set_manual_drawbar(
                    OrganPart::Lower,
                    Registration::AdjustB,
                    index,
                    position,
                );
            }
            let _ = engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, 0, 8);
            let _ = engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, 1, 8);
        }
        Scenario::Pedal16 | Scenario::Pedal8 | Scenario::PedalBoth => {
            let registrations = match scenario {
                Scenario::Pedal16 => [8, 0],
                Scenario::Pedal8 => [0, 8],
                Scenario::PedalBoth => [8, 8],
                _ => unreachable!(),
            };
            for (index, position) in registrations.into_iter().enumerate() {
                let _ = engine.set_manual_drawbar(
                    OrganPart::Pedal,
                    Registration::AdjustB,
                    index,
                    position,
                );
            }
        }
    }
}

fn phrase_events(engine: &mut OrganEngine, scenario: Scenario, frame: usize) {
    let quarter = SAMPLE_RATE as usize / 4;
    let on = quarter;
    let off = SAMPLE_RATE as usize * 3;
    if frame == on {
        if scenario.is_pedal() {
            let _ = engine.note_on_part(OrganPart::Pedal, 24, 1.0);
        } else {
            for note in [48, 55, 60, 64] {
                let _ = engine.note_on_part(OrganPart::Upper, note, 0.86);
            }
        }
        if matches!(scenario, Scenario::FullConsole) {
            for note in [48, 55, 60] {
                let _ = engine.note_on_part(OrganPart::Lower, note, 0.78);
            }
            let _ = engine.note_on_part(OrganPart::Pedal, 24, 1.0);
        }
    }
    if frame == off {
        engine.all_notes_off();
    }
    if matches!(scenario, Scenario::Tremolo) && frame == SAMPLE_RATE as usize * 2 {
        engine.set_rotary_mode(RotaryMode::Chorale);
    }
}

fn render_rotary_impulse() -> Vec<f32> {
    let mut rotary = Rotary::new(SAMPLE_RATE as f32);
    rotary.set_mode(RotaryMode::Brake);
    let _ = rotary.set_mix(1.0);
    let _ = rotary.set_cabinet(1.0);
    let frames = SAMPLE_RATE as usize / 2;
    let mut output = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let input = if frame == 0 { 1.0 } else { 0.0 };
        output.extend_from_slice(&rotary.process(input));
    }
    output
}

fn frequency_table() -> String {
    let mut csv = String::from("wheel,frequency_hz\n");
    for wheel in 0..TONEWHEEL_COUNT {
        let frequency = gear_frequency(wheel).expect("bounded wheel");
        writeln!(&mut csv, "{},{}", wheel + 1, frequency).expect("string write cannot fail");
    }
    csv
}

fn manifest(trims: captures::Trims) -> String {
    let drive_trims = trims
        .iter()
        .map(|(drive, _)| format!("{drive:.3}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "RF-Organ deterministic calibration suite\nversion={}\nsample_rate={}\nphrase_seconds={}\nnormalization=none\nformat=IEEE-float WAV stereo\nanalysis=frequency,level,generator-taper,generator-leakage,pedal-spectrum,pedal-release,expression-response,tone-control-response,console-distortion,transformer-intermodulation,transformer-calibration,percussion-envelope,percussion-recovery,keying-contacts,scanner-sidebands,scanner-line-response,rotary-rotor-response,rotary-doppler,rotary-stop-angle,drive-wobble\n",
        env!("CARGO_PKG_VERSION"),
        SAMPLE_RATE,
        SECONDS
    ) + &format!("transformer_drive_trims={drive_trims}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_trims_are_parsed_for_synthetic_reference_sets() {
        let trims = parse_drive_trims("0.1,-0.25,0").expect("three trims");
        assert_eq!(trims[0].0, 0.1);
        assert_eq!(trims[1].0, -0.25);
        assert_eq!(trims[2], (0.0, 0.0));
        assert!(parse_drive_trims("0.1,0.2").is_err());
        assert!(parse_drive_trims("0.1,0.2,0.3,0.4").is_err());
        assert!(parse_drive_trims("0.1,0.2,2.0").is_err());
        assert!(manifest(trims).contains("transformer_drive_trims=0.100,-0.250,0.000"));
    }

    #[test]
    fn frequency_csv_contains_all_physical_wheels() {
        let csv = frequency_table();
        assert_eq!(csv.lines().count(), TONEWHEEL_COUNT + 1);
        assert!(csv.starts_with("wheel,frequency_hz\n1,"));
    }

    #[test]
    fn every_drawbar_can_be_closed_for_reference_renders() {
        let mut engine = OrganEngine::new(48_000.0).unwrap();
        for drawbar in 0..rf_organ_dsp::DRAWBAR_COUNT {
            assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, drawbar, 0));
        }
    }

    #[test]
    fn pedal_reference_scenarios_have_stable_capture_names() {
        assert_eq!(Scenario::Pedal16.id(), "07-pedal-16ft");
        assert_eq!(Scenario::Pedal8.id(), "08-pedal-8ft");
        assert_eq!(Scenario::PedalBoth.id(), "09-pedal-16ft-8ft");
        assert!(Scenario::PedalBoth.is_pedal());
        assert!(!Scenario::FullConsole.is_pedal());
    }

    #[test]
    fn transformer_reference_captures_span_paths_and_levels() {
        assert_eq!(TRANSFORMER_CAPTURES.len(), 27);
        assert_eq!(TRANSFORMER_CAPTURES[0].id, "10-upper-transformer-low-c");
        assert_eq!(
            TRANSFORMER_CAPTURES[13].id,
            "23-lower-transformer-nominal-f"
        );
        assert_eq!(TRANSFORMER_CAPTURES[26].id, "36-t3-injection-high-c-f");
        assert_eq!(captures::capture_names().len(), Scenario::ALL.len() + 37);
    }
}
