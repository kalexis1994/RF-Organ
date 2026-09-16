// SPDX-License-Identifier: GPL-2.0-or-later

mod analysis;
mod compare;
mod wav;

use rf_organ_dsp::{
    Leslie, LeslieMode, OrganEngine, OrganPart, PercussionDecay, PercussionHarmonic,
    PercussionVolume, ScannerMode, TONEWHEEL_COUNT, gear_frequency,
};
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const SAMPLE_RATE: u32 = 48_000;
const SECONDS: usize = 4;

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
            if arguments.next().is_some() {
                return Err(usage().into());
            }
            render_suite(&PathBuf::from(destination))
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
        _ => Err(usage().into()),
    }
}

const fn usage() -> &'static str {
    "usage:\n  rf-organ-lab render OUTPUT_DIRECTORY\n  rf-organ-lab compare MODEL_DIRECTORY REFERENCE_DIRECTORY OUTPUT_DIRECTORY"
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
    println!(
        "RF_ORGAN_LAB_COMPARED model={} reference={} path={}",
        model.display(),
        reference.display(),
        destination.display()
    );
    Ok(())
}

fn render_suite(destination: &Path) -> Result<(), Box<dyn Error>> {
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
    let impulse = render_leslie_impulse();
    fs::write(
        destination.join("leslie-cabinet-impulse.wav"),
        wav::encode_f32(&impulse, 2, SAMPLE_RATE)?,
    )?;
    fs::write(destination.join("manifest.txt"), manifest())?;
    let analysis = analysis::analyze()?;
    fs::write(destination.join("measurements.csv"), analysis.measurements)?;
    fs::write(
        destination.join("percussion-envelope.csv"),
        analysis.percussion_envelope,
    )?;
    fs::write(
        destination.join("scanner-sidebands.csv"),
        analysis.scanner_sidebands,
    )?;
    fs::write(
        destination.join("leslie-rotor-response.csv"),
        analysis.leslie_rotor_response,
    )?;
    fs::write(
        destination.join("pedal-spectrum.csv"),
        analysis.pedal_spectrum,
    )?;
    fs::write(
        destination.join("pedal-release.csv"),
        analysis.pedal_release,
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
            Self::Direct => "01-direct-888",
            Self::Percussion => "02-third-percussion",
            Self::Scanner => "03-scanner-c3",
            Self::Chorale => "04-leslie-chorale",
            Self::Tremolo => "05-leslie-tremolo",
            Self::FullConsole => "06-full-console",
            Self::Pedal16 => "07-pedal-16ft",
            Self::Pedal8 => "08-pedal-8ft",
            Self::PedalBoth => "09-pedal-16ft-8ft",
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
    let _ = engine.set_transformer(0.38, 0.32);
    let _ = engine.set_console(0.32, 0.0, 0.0);
    let _ = engine.set_expression_character(0.55);
    let _ = engine.set_leslie_cabinet(0.35, 0.75, 0.22, 0.0);
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
        Scenario::Chorale => engine.set_leslie_mode(LeslieMode::Chorale),
        Scenario::Tremolo => engine.set_leslie_mode(LeslieMode::Tremolo),
        Scenario::FullConsole => {
            engine.set_scanner_mode(ScannerMode::Chorus3);
            engine.set_scanner_manuals(true, true);
            engine.set_leslie_mode(LeslieMode::Chorale);
            let _ = engine.set_transformer(0.62, 0.38);
            let _ = engine.set_console(0.48, 0.18, -0.08);
            for (index, position) in [8, 8, 8, 8, 6, 8, 4, 8, 6].into_iter().enumerate() {
                let _ = engine.set_manual_drawbar(OrganPart::Upper, index, position);
            }
            for (index, position) in [8, 8, 8, 8, 6, 0, 0, 0, 0].into_iter().enumerate() {
                let _ = engine.set_manual_drawbar(OrganPart::Lower, index, position);
            }
            let _ = engine.set_manual_drawbar(OrganPart::Pedal, 0, 8);
            let _ = engine.set_manual_drawbar(OrganPart::Pedal, 1, 8);
        }
        Scenario::Pedal16 | Scenario::Pedal8 | Scenario::PedalBoth => {
            let registrations = match scenario {
                Scenario::Pedal16 => [8, 0],
                Scenario::Pedal8 => [0, 8],
                Scenario::PedalBoth => [8, 8],
                _ => unreachable!(),
            };
            for (index, position) in registrations.into_iter().enumerate() {
                let _ = engine.set_manual_drawbar(OrganPart::Pedal, index, position);
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
        engine.set_leslie_mode(LeslieMode::Chorale);
    }
}

fn render_leslie_impulse() -> Vec<f32> {
    let mut leslie = Leslie::new(SAMPLE_RATE as f32);
    leslie.set_mode(LeslieMode::Brake);
    let _ = leslie.set_mix(1.0);
    let _ = leslie.set_cabinet(0.35, 0.75, 1.0, 0.0);
    let frames = SAMPLE_RATE as usize / 2;
    let mut output = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let input = if frame == 0 { 1.0 } else { 0.0 };
        output.extend_from_slice(&leslie.process(input));
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

fn manifest() -> String {
    format!(
        "RF-Organ deterministic calibration suite\nversion={}\nsample_rate={}\nphrase_seconds={}\nnormalization=none\nformat=IEEE-float WAV stereo\nanalysis=frequency,level,pedal-spectrum,pedal-release,percussion-envelope,scanner-sidebands,leslie-rotor-response\n",
        env!("CARGO_PKG_VERSION"),
        SAMPLE_RATE,
        SECONDS
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(engine.set_manual_drawbar(OrganPart::Upper, drawbar, 0));
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
}
