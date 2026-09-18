// SPDX-License-Identifier: GPL-2.0-or-later

use crate::captures::{C_NOTE, CHARACTER, F_NOTE, Level, bus_frequency, note_frequency};
use crate::signal::{decay_time, decibels, peak, rms, zero_crossing_frequency};
use rf_organ_dsp::{
    ConsoleElectronics, DRAWBAR_COUNT, LEVEL_SILENT_DB, MANUAL_FIRST_NOTE, MANUAL_KEY_COUNT,
    MainsFrequency, MatchingTransformer, MicrophoneArray, MicrophonePair, MicrophoneType,
    OrganEngine, OrganPart, PercussionDecay, PercussionHarmonic, PercussionVolume, Registration,
    Rotary, RotaryGeometry, RotaryMode, RotaryPlacement, ScannerMode, ScannerVibrato, StopAngle,
    TransformerUnit, compartment_companions, drawbar_wheel, gear_frequency,
};
use std::f64::consts::{PI, TAU};
use std::fmt::Write as _;

const SAMPLE_RATE: usize = 48_000;
const WINDOW: usize = SAMPLE_RATE / 100;

pub struct Artifacts {
    pub measurements: String,
    pub percussion_envelope: String,
    pub percussion_recovery: String,
    pub keying_contacts: String,
    pub key_click_response: String,
    pub registration_response: String,
    pub scanner_sidebands: String,
    pub scanner_line_response: String,
    pub scanner_line_cutoff: String,
    pub rotary_rotor_response: String,
    pub rotary_doppler: String,
    pub rotary_stop_angle: String,
    pub drive_wobble: String,
    pub pedal_spectrum: String,
    pub pedal_release: String,
    pub expression_response: String,
    pub tone_control_response: String,
    pub console_distortion: String,
    pub generator_taper: String,
    pub generator_leakage: String,
    pub transformer_intermodulation: String,
    pub transformer_calibration: String,
}

#[derive(Clone, Copy)]
struct Measurement {
    probe: &'static str,
    metric: &'static str,
    value: f64,
    unit: &'static str,
}

pub fn analyze() -> Result<Artifacts, String> {
    std::thread::Builder::new()
        .name("rf-organ-analysis".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(analyze_inner)
        .map_err(|error| format!("starting analysis worker: {error}"))?
        .join()
        .map_err(|_| "analysis worker panicked".to_owned())?
}

fn analyze_inner() -> Result<Artifacts, String> {
    let mut measurements = Vec::new();
    measurements.extend(tonewheel_probe()?);
    let (taper, generator_taper) = generator_taper_probe()?;
    measurements.extend(taper);
    let (leakage, generator_leakage) = generator_leakage_probe()?;
    measurements.extend(leakage);
    let (expression, expression_response) = expression_probe();
    measurements.extend(expression);
    let (tone_control, tone_control_response) = tone_control_probe();
    measurements.extend(tone_control);
    let (distortion, console_distortion) = console_distortion_probe();
    measurements.extend(distortion);
    let (transformer, transformer_intermodulation) = transformer_probe();
    measurements.extend(transformer);
    let (calibration, transformer_calibration) = transformer_calibration_probe();
    measurements.extend(calibration);
    let (pedal, pedal_spectrum, pedal_release) = pedal_probe()?;
    measurements.extend(pedal);
    let (percussion, percussion_envelope, percussion_recovery) = percussion_probe()?;
    measurements.extend(percussion);
    measurements.extend(percussion_touch_probe()?);
    let (keying, keying_contacts) = keying_probe()?;
    measurements.extend(keying);
    let (click, key_click_response) = key_click_probe()?;
    measurements.extend(click);
    let (registration, registration_response) = registration_probe()?;
    measurements.extend(registration);
    let (scanner, scanner_sidebands, scanner_line_response) = scanner_probe();
    measurements.extend(scanner);
    let (cutoff, scanner_line_cutoff) = scanner_cutoff_probe();
    measurements.extend(cutoff);
    let (rotary, rotary_rotor_response) = rotary_probe();
    measurements.extend(rotary);
    let (doppler, rotary_doppler) = rotary_doppler_probe();
    measurements.extend(doppler);
    let (stop, rotary_stop_angle) = rotary_stop_probe();
    measurements.extend(stop);
    measurements.extend(rotary_supply_probe());
    measurements.extend(rotary_capsule_probe());
    let (wobble, drive_wobble) = drive_wobble_probe()?;
    measurements.extend(wobble);
    Ok(Artifacts {
        measurements: measurement_csv(&measurements),
        percussion_envelope,
        percussion_recovery,
        keying_contacts,
        key_click_response,
        registration_response,
        scanner_sidebands,
        scanner_line_response,
        scanner_line_cutoff,
        rotary_rotor_response,
        rotary_doppler,
        rotary_stop_angle,
        drive_wobble,
        pedal_spectrum,
        pedal_release,
        expression_response,
        tone_control_response,
        console_distortion,
        generator_taper,
        generator_leakage,
        transformer_intermodulation,
        transformer_calibration,
    })
}

fn expression_probe() -> (Vec<Measurement>, String) {
    const POSITIONS: [(&str, f32); 5] = [
        ("12.5", 0.125),
        ("25", 0.25),
        ("50", 0.5),
        ("75", 0.75),
        ("100", 1.0),
    ];
    const FREQUENCIES: [(&str, f64); 3] = [("low", 80.0), ("mid", 1_000.0), ("high", 8_000.0)];
    let mut measurements = Vec::new();
    let mut csv =
        String::from("position,section_capacitance_pf,low_corner_hz,frequency_hz,gain_db\n");
    for (position_name, position) in POSITIONS {
        for (band, frequency) in FREQUENCIES {
            let (gain_db, diagnostics) = expression_gain(position, frequency);
            let probe = match position_name {
                "12.5" => "expression-12.5",
                "25" => "expression-25",
                "50" => "expression-50",
                "75" => "expression-75",
                "100" => "expression-100",
                _ => unreachable!(),
            };
            let metric = match band {
                "low" => "low-gain",
                "mid" => "mid-gain",
                "high" => "high-gain",
                _ => unreachable!(),
            };
            measurements.push(Measurement {
                probe,
                metric,
                value: gain_db,
                unit: "dB",
            });
            writeln!(
                &mut csv,
                "{position:.3},{:.6},{:.6},{frequency:.3},{gain_db:.6}",
                diagnostics.section_capacitance_pf, diagnostics.low_corner_hz
            )
            .expect("string write cannot fail");
        }
    }
    (measurements, csv)
}

fn expression_gain(
    position: f32,
    frequency: f64,
) -> (f64, rf_organ_dsp::ConsoleElectronicsDiagnostics) {
    const AMPLITUDE: f64 = 0.25;
    const SETTLE: usize = SAMPLE_RATE / 4;
    let mut electronics = ConsoleElectronics::new(SAMPLE_RATE as f32);
    assert!(electronics.set(0.0, 0.0, 0.0));
    assert!(electronics.set_expression_character(0.55));
    let mut samples = Vec::with_capacity(SAMPLE_RATE);
    for frame in 0..SETTLE + SAMPLE_RATE {
        let input = (TAU * frequency * frame as f64 / SAMPLE_RATE as f64).sin() * AMPLITUDE;
        let output = electronics.process(input as f32, position);
        if frame >= SETTLE {
            samples.push(f64::from(output));
        }
    }
    let gain = spectral_amplitude(&samples, frequency) / AMPLITUDE;
    (decibels(gain), electronics.diagnostics())
}

fn tone_control_probe() -> (Vec<Measurement>, String) {
    const CONTROLS: [(&str, f32); 5] = [
        ("minus-9", -1.0),
        ("minus-4.5", -0.5),
        ("neutral", 0.0),
        ("plus-4.5", 0.5),
        ("plus-9", 1.0),
    ];
    const FREQUENCIES: [(&str, f64); 3] = [("low", 100.0), ("mid", 1_000.0), ("high", 10_000.0)];
    let mut measurements = Vec::new();
    let mut csv = String::from("control_db,tone_corner_hz,frequency_hz,relative_gain_db\n");
    for (control_name, control) in CONTROLS {
        for (band, frequency) in FREQUENCIES {
            let neutral = tone_control_gain(0.0, frequency).0;
            let (gain, diagnostics) = tone_control_gain(control, frequency);
            let relative_gain_db = decibels(gain / neutral);
            let probe = match control_name {
                "minus-9" => "tone-minus-9",
                "minus-4.5" => "tone-minus-4.5",
                "neutral" => "tone-neutral",
                "plus-4.5" => "tone-plus-4.5",
                "plus-9" => "tone-plus-9",
                _ => unreachable!(),
            };
            let metric = match band {
                "low" => "low-gain",
                "mid" => "mid-gain",
                "high" => "high-gain",
                _ => unreachable!(),
            };
            measurements.push(Measurement {
                probe,
                metric,
                value: relative_gain_db,
                unit: "dB",
            });
            writeln!(
                &mut csv,
                "{:.3},{:.3},{frequency:.3},{relative_gain_db:.6}",
                control * 9.0,
                diagnostics.tone_corner_hz
            )
            .expect("string write cannot fail");
        }
    }
    (measurements, csv)
}

fn tone_control_gain(
    control: f32,
    frequency: f64,
) -> (f64, rf_organ_dsp::ConsoleElectronicsDiagnostics) {
    const AMPLITUDE: f64 = 0.25;
    const SETTLE: usize = SAMPLE_RATE / 4;
    let mut electronics = ConsoleElectronics::new(SAMPLE_RATE as f32);
    assert!(electronics.set(0.0, 0.0, control));
    assert!(electronics.set_expression_character(0.0));
    let mut samples = Vec::with_capacity(SAMPLE_RATE);
    for frame in 0..SETTLE + SAMPLE_RATE {
        let input = (TAU * frequency * frame as f64 / SAMPLE_RATE as f64).sin() * AMPLITUDE;
        let output = electronics.process(input as f32, 1.0);
        if frame >= SETTLE {
            samples.push(f64::from(output));
        }
    }
    (
        spectral_amplitude(&samples, frequency) / AMPLITUDE,
        electronics.diagnostics(),
    )
}

/// Harmonic distortion of the console chain against its drive control, at two
/// signal levels. The AO-28's tube stages are the reason the console is not a
/// clean amplifier, and this is the surface a reference console's own
/// distortion would be fitted against: an asymmetric stage makes second
/// harmonic, a symmetric one makes third, and their ratio says which is
/// responsible.
fn console_distortion_probe() -> (Vec<Measurement>, String) {
    const DRIVES: [(&str, f32); 5] = [
        ("clean", 0.0),
        ("quarter", 0.25),
        ("baseline", 0.32),
        ("three-quarter", 0.75),
        ("maximum", 1.0),
    ];
    const LEVELS: [(&str, f64); 2] = [("quiet", 0.025), ("loud", 0.25)];
    const PROBE_HZ: f64 = 1_000.0;
    let mut measurements = Vec::new();
    let mut csv = String::from(
        "drive_name,drive,level_name,input_dbfs,fundamental_dbfs,gain_db,h2_dbc,h3_dbc,h4_dbc,h5_dbc,thd_percent\n",
    );
    for (drive_name, drive) in DRIVES {
        for (level_name, amplitude) in LEVELS {
            let samples = render_console_tone(drive, amplitude, PROBE_HZ);
            let fundamental = spectral_amplitude(&samples, PROBE_HZ);
            let harmonics: [f64; 4] =
                [2.0, 3.0, 4.0, 5.0].map(|order| spectral_amplitude(&samples, order * PROBE_HZ));
            let distortion_power = harmonics
                .iter()
                .map(|amplitude| amplitude * amplitude)
                .sum::<f64>();
            let thd_percent = 100.0 * distortion_power.sqrt() / fundamental.max(1.0e-15);
            writeln!(
                &mut csv,
                "{drive_name},{drive:.3},{level_name},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{thd_percent:.6}",
                decibels(amplitude),
                decibels(fundamental),
                decibels(fundamental / amplitude),
                decibels(harmonics[0] / fundamental),
                decibels(harmonics[1] / fundamental),
                decibels(harmonics[2] / fundamental),
                decibels(harmonics[3] / fundamental),
            )
            .expect("string write cannot fail");
            if level_name == "loud" {
                let probe = match drive_name {
                    "clean" => "console-clean",
                    "quarter" => "console-quarter",
                    "baseline" => "console-baseline",
                    "three-quarter" => "console-three-quarter",
                    _ => "console-maximum",
                };
                measurements.extend([
                    Measurement {
                        probe,
                        metric: "second-harmonic",
                        value: decibels(harmonics[0] / fundamental),
                        unit: "dBc",
                    },
                    Measurement {
                        probe,
                        metric: "third-harmonic",
                        value: decibels(harmonics[1] / fundamental),
                        unit: "dBc",
                    },
                    Measurement {
                        probe,
                        metric: "thd",
                        value: thd_percent,
                        unit: "%",
                    },
                ]);
            }
        }
    }
    (measurements, csv)
}

/// One tone through the console electronics alone, with expression wide open
/// and the tone control neutral, so that only the tube stages act on it.
fn render_console_tone(drive: f32, amplitude: f64, frequency: f64) -> Vec<f64> {
    const SETTLE: usize = SAMPLE_RATE / 4;
    let mut electronics = ConsoleElectronics::new(SAMPLE_RATE as f32);
    assert!(electronics.set(drive, 0.0, 0.0));
    assert!(electronics.set_expression_character(0.0));
    let mut samples = Vec::with_capacity(SAMPLE_RATE);
    for frame in 0..SETTLE + SAMPLE_RATE {
        let input = amplitude * (TAU * frequency * frame as f64 / SAMPLE_RATE as f64).sin();
        let output = electronics.process(input as f32, 1.0);
        if frame >= SETTLE {
            samples.push(f64::from(output));
        }
    }
    samples
}

fn transformer_probe() -> (Vec<Measurement>, String) {
    const CONFIGURATIONS: [(&str, f32, f32); 5] = [
        ("clean", 0.0, 0.0),
        ("baseline", 0.38, 0.32),
        ("drive", 0.75, 0.32),
        ("memory", 0.38, 0.75),
        ("maximum", 1.0, 1.0),
    ];
    const C_HZ: f64 = 523.3;
    const F_HZ: f64 = 698.5;
    const DIFFERENCE_HZ: f64 = F_HZ - C_HZ;
    const THIRD_ORDER_HZ: f64 = 2.0 * C_HZ - F_HZ;
    let mut measurements = Vec::new();
    let mut csv = String::from(
        "configuration,drive,hysteresis,c_hz,f_hz,difference_hz,third_order_hz,c_dbfs,f_dbfs,difference_dbc,third_order_dbc\n",
    );
    for (name, drive, hysteresis) in CONFIGURATIONS {
        let samples = render_transformer_pair(drive, hysteresis, C_HZ, F_HZ, 0.18);
        let c = spectral_amplitude(&samples, C_HZ);
        let f = spectral_amplitude(&samples, F_HZ);
        let difference = spectral_amplitude(&samples, DIFFERENCE_HZ);
        let third_order = spectral_amplitude(&samples, THIRD_ORDER_HZ);
        let carrier = 0.5 * (c + f);
        let difference_dbc = decibels(difference / carrier);
        let third_order_dbc = decibels(third_order / carrier);
        let probe = match name {
            "clean" => "transformer-clean",
            "baseline" => "transformer-baseline",
            "drive" => "transformer-drive",
            "memory" => "transformer-memory",
            "maximum" => "transformer-maximum",
            _ => unreachable!(),
        };
        measurements.push(Measurement {
            probe,
            metric: "difference-product",
            value: difference_dbc,
            unit: "dBc",
        });
        measurements.push(Measurement {
            probe,
            metric: "third-order-product",
            value: third_order_dbc,
            unit: "dBc",
        });
        writeln!(
            &mut csv,
            "{name},{drive:.3},{hysteresis:.3},{C_HZ:.3},{F_HZ:.3},{DIFFERENCE_HZ:.3},{THIRD_ORDER_HZ:.3},{:.6},{:.6},{difference_dbc:.6},{third_order_dbc:.6}",
            decibels(c),
            decibels(f)
        )
        .expect("string write cannot fail");
    }
    (measurements, csv)
}

fn transformer_calibration_probe() -> (Vec<Measurement>, String) {
    const TRIMS: [(&str, f32); 3] = [("minus", -0.2), ("zero", 0.0), ("plus", 0.2)];
    let c_hz = note_frequency(C_NOTE);
    let f_hz = note_frequency(F_NOTE);
    let difference_hz = f_hz - c_hz;
    let third_order_hz = 2.0 * c_hz - f_hz;
    let mut measurements = Vec::new();
    let mut csv = String::from(
        "unit,drive_trim,effective_drive,effective_hysteresis,other_units_drive,level,tone_peak,c_dbfs,f_dbfs,difference_dbc,third_order_dbc\n",
    );
    for unit in TransformerUnit::ALL {
        for (trim, drive_trim) in TRIMS {
            let mut engine = OrganEngine::new(SAMPLE_RATE as f32).expect("valid engine");
            assert!(engine.set_transformer(CHARACTER.0, CHARACTER.1));
            assert!(engine.set_transformer_trim(unit, drive_trim, 0.0));
            let calibration = engine.transformer_diagnostics(unit);
            let others = TransformerUnit::ALL
                .into_iter()
                .filter(|other| *other != unit)
                .map(|other| engine.transformer_diagnostics(other).drive)
                .fold(0.0_f32, f32::max);
            for level in Level::ALL {
                let tone = level.injection_amplitude();
                let samples = render_transformer_pair(
                    calibration.drive,
                    calibration.hysteresis,
                    c_hz,
                    f_hz,
                    tone,
                );
                let c = spectral_amplitude(&samples, c_hz);
                let f = spectral_amplitude(&samples, f_hz);
                let difference = spectral_amplitude(&samples, difference_hz);
                let third_order = spectral_amplitude(&samples, third_order_hz);
                let carrier = 0.5 * (c + f);
                let difference_dbc = decibels(difference / carrier);
                let third_order_dbc = decibels(third_order / carrier);
                if trim == "zero" && unit == TransformerUnit::T3 {
                    measurements.push(Measurement {
                        probe: match level {
                            Level::Low => "transformer-injection-low",
                            Level::Nominal => "transformer-injection-nominal",
                            Level::High => "transformer-injection-high",
                        },
                        metric: "third-order-product",
                        value: third_order_dbc,
                        unit: "dBc",
                    });
                }
                writeln!(
                    &mut csv,
                    "{},{drive_trim:.3},{:.6},{:.6},{others:.6},{},{tone:.6},{:.6},{:.6},{difference_dbc:.6},{third_order_dbc:.6}",
                    unit.label(),
                    calibration.drive,
                    calibration.hysteresis,
                    level.label(),
                    decibels(c),
                    decibels(f)
                )
                .expect("string write cannot fail");
            }
        }
    }
    (measurements, csv)
}

fn render_transformer_pair(
    drive: f32,
    hysteresis: f32,
    c_hz: f64,
    f_hz: f64,
    amplitude: f64,
) -> Vec<f64> {
    const SETTLE: usize = SAMPLE_RATE / 2;
    let mut transformer = MatchingTransformer::new(SAMPLE_RATE as f32);
    assert!(transformer.set(drive, hysteresis));
    let mut samples = Vec::with_capacity(SAMPLE_RATE);
    for frame in 0..SETTLE + SAMPLE_RATE {
        let time = frame as f64 / SAMPLE_RATE as f64;
        let input = amplitude * ((TAU * c_hz * time).sin() + (TAU * f_hz * time).sin());
        let output = transformer.process(input as f32);
        if frame >= SETTLE {
            samples.push(f64::from(output));
        }
    }
    samples
}

fn pedal_probe() -> Result<(Vec<Measurement>, String, String), String> {
    const HARMONICS: [(usize, usize); 8] = [
        (1, 0),
        (2, 12),
        (3, 19),
        (4, 24),
        (6, 31),
        (8, 36),
        (10, 40),
        (12, 43),
    ];
    let registrations = [("pedal-16ft", [8, 0], 0), ("pedal-8ft", [0, 8], 12)];
    let mut measurements = Vec::new();
    let mut spectrum =
        String::from("registration,harmonic,wheel,frequency_hz,level_dbfs,level_dbc\n");
    for (name, drawbars, reference_wheel) in registrations {
        let samples = render_pedal(drawbars)?;
        let reference_frequency = f64::from(
            gear_frequency(reference_wheel)
                .ok_or_else(|| "missing pedal reference wheel".to_owned())?,
        );
        let reference = spectral_amplitude(&samples, reference_frequency);
        measurements.push(Measurement {
            probe: name,
            metric: "reference-level",
            value: decibels(reference),
            unit: "dBFS",
        });
        for (harmonic, wheel) in HARMONICS {
            let frequency = f64::from(
                gear_frequency(wheel).ok_or_else(|| "missing pedal harmonic wheel".to_owned())?,
            );
            let amplitude = spectral_amplitude(&samples, frequency);
            let relative = decibels(amplitude / reference);
            let metric = match harmonic {
                1 => "harmonic-1",
                2 => "harmonic-2",
                3 => "harmonic-3",
                4 => "harmonic-4",
                6 => "harmonic-6",
                8 => "harmonic-8",
                10 => "harmonic-10",
                12 => "harmonic-12",
                _ => unreachable!(),
            };
            measurements.push(Measurement {
                probe: name,
                metric,
                value: relative,
                unit: "dBc",
            });
            writeln!(
                &mut spectrum,
                "{name},{harmonic},{},{frequency:.9},{:.9},{relative:.9}",
                wheel + 1,
                decibels(amplitude)
            )
            .expect("string write cannot fail");
        }
    }

    let release = render_pedal_release()?;
    let total_energy = release.iter().map(|sample| sample * sample).sum::<f64>();
    if total_energy <= 1.0e-30 {
        return Err("pedal release probe produced no filter tail".to_owned());
    }
    let mut accumulated = 0.0;
    let release_t999 = release
        .iter()
        .enumerate()
        .find_map(|(index, sample)| {
            accumulated += sample * sample;
            (accumulated >= total_energy * 0.999).then_some(index as f64 / SAMPLE_RATE as f64)
        })
        .ok_or_else(|| "pedal release energy did not converge".to_owned())?;
    measurements.push(Measurement {
        probe: "pedal-16ft",
        metric: "key-off-energy-t99.9",
        value: release_t999,
        unit: "s",
    });
    let mut release_csv = String::from("time_seconds,amplitude\n");
    for (index, sample) in release.iter().copied().enumerate() {
        writeln!(
            &mut release_csv,
            "{:.9},{sample:.9}",
            index as f64 / SAMPLE_RATE as f64
        )
        .expect("string write cannot fail");
    }
    Ok((measurements, spectrum, release_csv))
}

fn render_pedal(drawbars: [u8; 2]) -> Result<Vec<f64>, String> {
    let mut engine = clean_engine()?;
    for (index, position) in drawbars.into_iter().enumerate() {
        assert!(engine.set_manual_drawbar(
            OrganPart::Pedal,
            Registration::AdjustB,
            index,
            position
        ));
    }
    assert!(engine.note_on_part(OrganPart::Pedal, 24, 1.0));
    let mut samples = Vec::with_capacity(SAMPLE_RATE);
    for frame in 0..SAMPLE_RATE * 3 / 2 {
        let sample = f64::from(engine.next_sample()[0]);
        if frame >= SAMPLE_RATE / 2 {
            samples.push(sample);
        }
    }
    Ok(samples)
}

fn render_pedal_release() -> Result<Vec<f64>, String> {
    let mut engine = clean_engine()?;
    assert!(engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, 0, 8));
    assert!(engine.note_on_part(OrganPart::Pedal, 24, 1.0));
    for _ in 0..SAMPLE_RATE {
        engine.next_sample();
    }
    assert!(engine.note_off_part(OrganPart::Pedal, 24, 1.0));
    Ok((0..SAMPLE_RATE / 10)
        .map(|_| f64::from(engine.next_sample()[0]))
        .collect())
}

/// Every manual key played alone through its 8-foot contact, which is the
/// one wheel that contact reaches. It gives the generator's taper — how loud
/// each wheel is against its neighbours — and the once-per-revolution level
/// change of an off-centre wheel, both in the same pass and both measurable
/// the same way on a console.
fn generator_taper_probe() -> Result<(Vec<Measurement>, String), String> {
    const BUS: usize = 2;
    const SETTLE: usize = SAMPLE_RATE / 4;
    const WINDOW: usize = SAMPLE_RATE / 2;
    let mut csv = String::from(
        "key,midi_note,wheel,frequency_hz,revolutions_hz,level_dbfs,level_relative_db,eccentricity_sideband_dbc\n",
    );
    let mut rows = Vec::with_capacity(MANUAL_KEY_COUNT);
    for key in 0..MANUAL_KEY_COUNT {
        let note = MANUAL_FIRST_NOTE + key as u8;
        let wheel = drawbar_wheel(key, BUS).ok_or_else(|| format!("no 8' wheel for key {key}"))?;
        let frequency = f64::from(gear_frequency(wheel).ok_or_else(|| "missing wheel".to_owned())?);
        let teeth =
            f64::from(rf_organ_dsp::gear_teeth(wheel).ok_or_else(|| "missing teeth".to_owned())?);
        let revolutions = frequency / teeth;

        let mut engine = clean_engine()?;
        assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, BUS, 8));
        assert!(engine.note_on_part(OrganPart::Upper, note, 1.0));
        let mut samples = Vec::with_capacity(WINDOW);
        for frame in 0..SETTLE + WINDOW {
            let sample = f64::from(engine.next_sample()[0]);
            if frame >= SETTLE {
                samples.push(sample);
            }
        }
        let carrier = spectral_amplitude(&samples, frequency);
        if carrier <= 1.0e-12 {
            return Err(format!("key {key} produced no tone"));
        }
        let lower = spectral_amplitude(&samples, frequency - revolutions);
        let upper = spectral_amplitude(&samples, frequency + revolutions);
        rows.push((
            key,
            note,
            wheel,
            frequency,
            revolutions,
            carrier,
            decibels(0.5 * (lower + upper) / carrier),
        ));
    }

    let median = {
        let mut levels = rows.iter().map(|row| row.5).collect::<Vec<_>>();
        levels.sort_by(f64::total_cmp);
        levels[levels.len() / 2]
    };
    let mut spread: f64 = 0.0;
    for (key, note, wheel, frequency, revolutions, carrier, sideband) in &rows {
        let relative = decibels(carrier / median);
        spread = spread.max(relative.abs());
        writeln!(
            &mut csv,
            "{},{note},{},{frequency:.6},{revolutions:.6},{:.6},{relative:.6},{sideband:.6}",
            key + 1,
            wheel + 1,
            decibels(*carrier)
        )
        .expect("string write cannot fail");
    }
    let sidebands = rows.iter().map(|row| row.6).sum::<f64>() / rows.len() as f64;
    Ok((
        vec![
            Measurement {
                probe: "generator-taper",
                metric: "worst-deviation",
                value: spread,
                unit: "dB",
            },
            Measurement {
                probe: "generator-eccentricity",
                metric: "mean-sideband",
                value: sidebands,
                unit: "dBc",
            },
        ],
        csv,
    ))
}

/// Leakage against how many keys are held. Hammond describes the hash as
/// rising with the number of notes played at once, and the compartment
/// neighbours of the wheels being keyed are where it comes from, so it is
/// measured at those neighbours' own frequencies rather than as broadband
/// noise.
fn generator_leakage_probe() -> Result<(Vec<Measurement>, String), String> {
    const BUS: usize = 2;
    const SETTLE: usize = SAMPLE_RATE / 4;
    const WINDOW: usize = SAMPLE_RATE / 2;
    // Five of the ninety-one wheels have no compartment neighbour in the
    // model's wiring, so these chords stay in the range that does and the
    // report says how many of the keyed wheels were paired.
    const CHORDS: [(&str, &[u8]); 4] = [
        ("one", &[48]),
        ("two", &[48, 52]),
        ("four", &[48, 52, 55, 59]),
        ("eight", &[36, 40, 43, 47, 48, 52, 55, 59]),
    ];
    let mut csv = String::from(
        "notes,keys,companion_wheels,fundamental_dbfs,leakage_dbfs,leakage_dbc
",
    );
    let mut measurements = Vec::new();
    let mut relatives = Vec::new();
    for (name, notes) in CHORDS {
        let mut engine = clean_engine()?;
        assert!(engine.set_leakage(0.2));
        assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, BUS, 8));
        for note in notes {
            assert!(engine.note_on_part(OrganPart::Upper, *note, 1.0));
        }
        let mut samples = Vec::with_capacity(WINDOW);
        for frame in 0..SETTLE + WINDOW {
            let sample = f64::from(engine.next_sample()[0]);
            if frame >= SETTLE {
                samples.push(sample);
            }
        }
        let mut played = 0.0;
        let mut leaked = 0.0;
        let mut paired = 0;
        for note in notes {
            let key = usize::from(note - MANUAL_FIRST_NOTE);
            let wheel = drawbar_wheel(key, BUS).ok_or_else(|| "missing wheel".to_owned())?;
            let frequency =
                f64::from(gear_frequency(wheel).ok_or_else(|| "missing wheel".to_owned())?);
            let amplitude = spectral_amplitude(&samples, frequency);
            played += amplitude * amplitude;
            // Everything else in the same compartment bleeds into this wheel's
            // pickup, and each companion sits at its own frequency.
            for companion in compartment_companions(wheel).into_iter().flatten() {
                let neighbour = f64::from(
                    gear_frequency(companion).ok_or_else(|| "missing companion".to_owned())?,
                );
                let leak = spectral_amplitude(&samples, neighbour);
                leaked += leak * leak;
                paired += 1;
            }
        }
        let fundamental = played.sqrt();
        let leakage = leaked.sqrt();
        let relative = decibels(leakage / fundamental);
        writeln!(
            &mut csv,
            "{name},{},{paired},{:.6},{:.6},{relative:.6}",
            notes.len(),
            decibels(fundamental),
            decibels(leakage)
        )
        .expect("string write cannot fail");
        relatives.push(relative);
        measurements.push(Measurement {
            probe: match name {
                "one" => "leakage-one-note",
                "two" => "leakage-two-notes",
                "four" => "leakage-four-notes",
                _ => "leakage-eight-notes",
            },
            metric: "leakage-level",
            value: relative,
            unit: "dBc",
        });
    }
    // Hammond's own control is over "the rate at which the Leakage Tone
    // increases as more notes are played simultaneously", so the measurement
    // that matters is not the level at any one chord but how much the level
    // rises between one note and eight.
    if let (Some(one), Some(eight)) = (relatives.first(), relatives.last()) {
        measurements.push(Measurement {
            probe: "leakage-rise",
            metric: "one-note-to-eight",
            value: eight - one,
            unit: "dB",
        });
    }

    Ok((measurements, csv))
}

fn tonewheel_probe() -> Result<Vec<Measurement>, String> {
    const NOTE: u8 = 69;
    const BUS: usize = 2;
    let mut engine = clean_engine()?;
    assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, BUS, 8));
    assert!(engine.note_on_part(OrganPart::Upper, NOTE, 1.0));
    let mut samples = Vec::with_capacity(SAMPLE_RATE * 2);
    for _ in 0..SAMPLE_RATE * 2 {
        samples.push(f64::from(engine.next_sample()[0]));
    }
    let start = SAMPLE_RATE / 2;
    let end = SAMPLE_RATE * 3 / 2;
    let measured = zero_crossing_frequency(&samples[start..end], SAMPLE_RATE as f64)
        .ok_or_else(|| "tonewheel frequency probe produced no crossings".to_owned())?;
    let key = usize::from(NOTE - MANUAL_FIRST_NOTE);
    let wheel = drawbar_wheel(key, BUS).ok_or_else(|| "invalid tonewheel probe".to_owned())?;
    let expected = f64::from(gear_frequency(wheel).ok_or_else(|| "missing wheel".to_owned())?);
    let error_cents = 1_200.0 * (measured / expected).log2();
    let analysis = &samples[start..end];
    Ok(vec![
        Measurement {
            probe: "tonewheel-a4",
            metric: "expected-frequency",
            value: expected,
            unit: "Hz",
        },
        Measurement {
            probe: "tonewheel-a4",
            metric: "measured-frequency",
            value: measured,
            unit: "Hz",
        },
        Measurement {
            probe: "tonewheel-a4",
            metric: "frequency-error",
            value: error_cents,
            unit: "cent",
        },
        Measurement {
            probe: "tonewheel-a4",
            metric: "rms-level",
            value: rms(analysis),
            unit: "linear",
        },
        Measurement {
            probe: "tonewheel-a4",
            metric: "peak-level",
            value: peak(analysis),
            unit: "linear",
        },
    ])
}

/// Percussion, measured the way a reference capture would have to: one key,
/// every combination of the two tablets, and the drawbar registration left
/// where it was so that the documented Normal-volume drawbar cut shows up.
fn percussion_probe() -> Result<(Vec<Measurement>, String, String), String> {
    const SETTINGS: [(&str, PercussionVolume, PercussionDecay); 4] = [
        (
            "normal-fast",
            PercussionVolume::Normal,
            PercussionDecay::Fast,
        ),
        (
            "normal-slow",
            PercussionVolume::Normal,
            PercussionDecay::Slow,
        ),
        ("soft-fast", PercussionVolume::Soft, PercussionDecay::Fast),
        ("soft-slow", PercussionVolume::Soft, PercussionDecay::Slow),
    ];
    let mut measurements = Vec::new();
    let mut csv = String::from("setting,time_seconds,rms\n");
    for (name, volume, decay) in SETTINGS {
        let curve = percussion_curve(volume, decay)?;
        let t60 =
            decay_time(&curve, 0.001).ok_or_else(|| format!("{name} percussion did not decay"))?;
        let peak = curve
            .iter()
            .map(|(_, level)| *level)
            .fold(0.0_f64, f64::max);
        let attack_seconds = percussion_attack(volume, decay)?;
        let probe = match name {
            "normal-fast" => "percussion-fast",
            "normal-slow" => "percussion-slow",
            "soft-fast" => "percussion-soft-fast",
            _ => "percussion-soft-slow",
        };
        measurements.extend([
            Measurement {
                probe,
                metric: "t60",
                value: t60,
                unit: "s",
            },
            Measurement {
                probe,
                metric: "peak-level",
                value: decibels(peak),
                unit: "dBFS",
            },
            Measurement {
                probe,
                metric: "attack-time",
                value: attack_seconds * 1_000.0,
                unit: "ms",
            },
        ]);
        for (time, level) in &curve {
            writeln!(&mut csv, "{name},{time:.6},{level:.9}").expect("string write cannot fail");
        }
    }

    // The documented console takes about 6 dB out of the upper drawbars when
    // percussion is on at Normal volume, and leaves them alone at Soft.
    let reference = drawbar_level(None)?;
    for (name, volume) in [
        ("percussion-normal", PercussionVolume::Normal),
        ("percussion-soft", PercussionVolume::Soft),
    ] {
        measurements.push(Measurement {
            probe: name,
            metric: "drawbar-level",
            value: decibels(drawbar_level(Some(volume))? / reference),
            unit: "dB",
        });
    }

    measurements.extend(percussion_harmonic_probe()?);
    Ok((measurements, csv, percussion_recovery_probe()?))
}

/// Which generator each harmonic tablet actually reaches. Second percussion
/// should come from the 4' bus and third from the 2 2/3' bus, so with every
/// drawbar closed the strike is alone and its frequency identifies the bus.
fn percussion_harmonic_probe() -> Result<Vec<Measurement>, String> {
    const SELECTION: [(&str, PercussionHarmonic, usize); 2] = [
        ("percussion-second", PercussionHarmonic::Second, 3),
        ("percussion-third", PercussionHarmonic::Third, 4),
    ];
    const NOTE: u8 = 60;
    let mut measurements = Vec::new();
    for (probe, harmonic, bus) in SELECTION {
        let mut engine = clean_engine()?;
        engine.set_percussion_enabled(true);
        engine.set_percussion_harmonic(harmonic);
        engine.set_percussion_volume(PercussionVolume::Normal);
        engine.set_percussion_decay(PercussionDecay::Slow);
        assert!(engine.note_on_part(OrganPart::Upper, NOTE, 1.0));
        let samples = (0..SAMPLE_RATE / 4)
            .map(|_| f64::from(engine.next_sample()[0]))
            .collect::<Vec<_>>();
        let measured = zero_crossing_frequency(&samples[SAMPLE_RATE / 100..], SAMPLE_RATE as f64)
            .ok_or_else(|| format!("{probe} produced no strike"))?;
        let key = usize::from(NOTE - MANUAL_FIRST_NOTE);
        let wheel = drawbar_wheel(key, bus).ok_or_else(|| format!("{probe} has no bus"))?;
        let expected =
            f64::from(gear_frequency(wheel).ok_or_else(|| format!("{probe} has no wheel"))?);
        measurements.extend([
            Measurement {
                probe,
                metric: "strike-frequency",
                value: measured,
                unit: "Hz",
            },
            Measurement {
                probe,
                metric: "strike-frequency-error",
                value: 1_200.0 * (measured / expected).log2(),
                unit: "cent",
            },
        ]);
    }
    Ok(measurements)
}

/// Steady drawbar level of the upper manual with percussion off, or on at the
/// given volume. Measured after the percussion has decayed away so that only
/// the drawbar path is left.
fn drawbar_level(volume: Option<PercussionVolume>) -> Result<f64, String> {
    let mut engine = clean_engine()?;
    for drawbar in 0..3 {
        assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, drawbar, 8));
    }
    if let Some(volume) = volume {
        engine.set_percussion_enabled(true);
        engine.set_percussion_harmonic(PercussionHarmonic::Third);
        engine.set_percussion_volume(volume);
        engine.set_percussion_decay(PercussionDecay::Fast);
    }
    assert!(engine.note_on_part(OrganPart::Upper, 60, 1.0));
    let mut samples = Vec::with_capacity(SAMPLE_RATE);
    for frame in 0..SAMPLE_RATE * 3 {
        let sample = f64::from(engine.next_sample()[0]);
        if frame >= SAMPLE_RATE * 2 {
            samples.push(sample);
        }
    }
    Ok(rms(&samples))
}

/// How much percussion is left when a key is struck again a given time after
/// the last one was released. The console recharges only with every key up,
/// which is why fast detached playing loses the effect.
fn percussion_recovery_probe() -> Result<String, String> {
    const GAPS_MS: [usize; 8] = [0, 25, 50, 100, 200, 400, 800, 1_600];
    let mut csv = String::from("gap_ms,peak_level,relative_db\n");
    let mut reference = 0.0;
    for (index, gap) in GAPS_MS.into_iter().enumerate() {
        let mut engine = clean_engine()?;
        engine.set_percussion_enabled(true);
        engine.set_percussion_harmonic(PercussionHarmonic::Third);
        engine.set_percussion_volume(PercussionVolume::Normal);
        engine.set_percussion_decay(PercussionDecay::Fast);
        assert!(engine.note_on_part(OrganPart::Upper, 60, 1.0));
        for _ in 0..SAMPLE_RATE / 2 {
            engine.next_sample();
        }
        assert!(engine.note_off_part(OrganPart::Upper, 60, 1.0));
        for _ in 0..gap * SAMPLE_RATE / 1_000 {
            engine.next_sample();
        }
        assert!(engine.note_on_part(OrganPart::Upper, 60, 1.0));
        let peak = (0..SAMPLE_RATE / 4)
            .map(|_| f64::from(engine.next_sample()[0]).abs())
            .fold(0.0, f64::max);
        if index == GAPS_MS.len() - 1 {
            reference = peak;
        }
        writeln!(&mut csv, "{gap},{peak:.9},").expect("string write cannot fail");
    }
    if reference <= 0.0 {
        return Err("percussion recovery probe produced no signal".to_owned());
    }
    // Rewrite the relative column now that the fully recovered strike is known.
    let mut relative = String::from("gap_ms,peak_level,relative_db\n");
    for line in csv.lines().skip(1) {
        let mut fields = line.split(',');
        let gap = fields.next().unwrap_or_default();
        let peak: f64 = fields.next().unwrap_or("0").parse().unwrap_or(0.0);
        writeln!(
            &mut relative,
            "{gap},{peak:.9},{:.6}",
            decibels(peak / reference)
        )
        .expect("string write cannot fail");
    }
    Ok(relative)
}

/// Time from the key contact to the loudest part of the strike, measured in
/// quarter-millisecond windows because the envelope curve's 10 ms grid is far
/// coarser than the attack itself.
fn percussion_attack(volume: PercussionVolume, decay: PercussionDecay) -> Result<f64, String> {
    const WINDOW: usize = SAMPLE_RATE / 4_000;
    let mut engine = clean_engine()?;
    engine.set_percussion_enabled(true);
    engine.set_percussion_harmonic(PercussionHarmonic::Third);
    engine.set_percussion_volume(volume);
    engine.set_percussion_decay(decay);
    assert!(engine.note_on_part(OrganPart::Upper, 60, 1.0));
    let samples = (0..SAMPLE_RATE / 10)
        .map(|_| f64::from(engine.next_sample()[0]))
        .collect::<Vec<_>>();
    let (windows, _) = samples.as_chunks::<WINDOW>();
    let (index, _) = windows.iter().map(|window| rms(window)).enumerate().fold(
        (0, 0.0),
        |(at, peak), (index, level)| {
            if level > peak {
                (index, level)
            } else {
                (at, peak)
            }
        },
    );
    Ok((index * WINDOW) as f64 / SAMPLE_RATE as f64)
}

fn percussion_curve(
    volume: PercussionVolume,
    decay: PercussionDecay,
) -> Result<Vec<(f64, f64)>, String> {
    let mut engine = clean_engine()?;
    engine.set_percussion_enabled(true);
    engine.set_percussion_harmonic(PercussionHarmonic::Third);
    engine.set_percussion_volume(volume);
    engine.set_percussion_decay(decay);
    assert!(engine.note_on_part(OrganPart::Upper, 60, 1.0));
    let mut samples = Vec::with_capacity(SAMPLE_RATE * 2);
    for _ in 0..SAMPLE_RATE * 2 {
        samples.push(f64::from(engine.next_sample()[0]));
    }
    let (windows, _) = samples.as_chunks::<WINDOW>();
    Ok(windows
        .iter()
        .enumerate()
        .map(|(index, samples)| (index as f64 * 0.01, rms(samples)))
        .collect())
}

/// When each of the nine key contacts arrives, measured one drawbar at a time
/// so that the protocol can be repeated on a console: open a single drawbar,
/// strike one key, and time the onset.
fn keying_probe() -> Result<(Vec<Measurement>, String), String> {
    const FOOTAGES: [&str; DRAWBAR_COUNT] =
        ["16", "5-1-3", "8", "4", "2-2-3", "2", "1-3-5", "1-1-3", "1"];
    const VELOCITIES: [(&str, f32); 3] = [("soft", 0.25), ("medium", 0.6), ("hard", 1.0)];
    let mut measurements = Vec::new();
    let mut csv = String::from("velocity,bus,footage,closure_ms,first_ten_ms_db,steady_dbfs\n");
    for (velocity_name, velocity) in VELOCITIES {
        let mut first = f64::MAX;
        let mut last: f64 = 0.0;
        for (bus, footage) in FOOTAGES.into_iter().enumerate() {
            let samples = render_contact(bus, velocity)?;
            let steady = rms(&samples[SAMPLE_RATE / 4..]);
            if steady <= 1.0e-12 {
                return Err(format!("contact {bus} produced no tone"));
            }
            let threshold = steady * 0.1;
            let window = SAMPLE_RATE / 2_000;
            let closure = samples
                .chunks_exact(window)
                .position(|chunk| rms(chunk) > threshold)
                .ok_or_else(|| format!("contact {bus} never closed"))?;
            let closure_ms = (closure * window) as f64 * 1_000.0 / SAMPLE_RATE as f64;
            let click = rms(&samples[..SAMPLE_RATE / 100]);
            first = first.min(closure_ms);
            last = last.max(closure_ms);
            writeln!(
                &mut csv,
                "{velocity_name},{},{footage},{closure_ms:.6},{:.6},{:.6}",
                bus + 1,
                decibels(click / steady),
                decibels(steady)
            )
            .expect("string write cannot fail");
        }
        let probe = match velocity_name {
            "soft" => "keying-soft",
            "medium" => "keying-medium",
            _ => "keying-hard",
        };
        measurements.push(Measurement {
            probe,
            metric: "contact-spread",
            value: last - first,
            unit: "ms",
        });
    }
    Ok((measurements, csv))
}

/// One drawbar open, one key struck at the given velocity, with the contact
/// model at its default spread and bounce.
fn render_contact(bus: usize, velocity: f32) -> Result<Vec<f64>, String> {
    let mut engine = clean_engine()?;
    assert!(engine.set_contact_spread(0.55));
    assert!(engine.set_contact_bounce(0.45));
    assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, bus, 8));
    assert!(engine.note_on_part(OrganPart::Upper, 60, velocity));
    Ok((0..SAMPLE_RATE / 2)
        .map(|_| f64::from(engine.next_sample()[0]))
        .collect())
}

fn scanner_probe() -> (Vec<Measurement>, String, String) {
    const CARRIER: f64 = 1_000.0;
    const MODULATION: f64 = 6.9;
    /// One revolution carries the scanner out to the far terminal and back,
    /// so the modulation is not a sinusoid and the spectrum keeps going.
    const ORDERS: [f64; 3] = [1.0, 2.0, 3.0];
    const BANDS: [f64; 5] = [100.0, 440.0, 1_000.0, 4_000.0, 8_000.0];
    let modes = [
        ("v1", ScannerMode::Vibrato1),
        ("v2", ScannerMode::Vibrato2),
        ("v3", ScannerMode::Vibrato3),
        ("c1", ScannerMode::Chorus1),
        ("c2", ScannerMode::Chorus2),
        ("c3", ScannerMode::Chorus3),
    ];
    let mut measurements = Vec::new();
    let mut csv = String::from(
        "mode,carrier_dbfs,lower_sideband_dbc,upper_sideband_dbc,lower_sideband2_dbc,upper_sideband2_dbc,lower_sideband3_dbc,upper_sideband3_dbc\n",
    );
    let mut response = String::from("mode,frequency_hz,gain_db\n");
    for (name, mode) in modes {
        let samples = render_scanner(mode, CARRIER);
        let carrier = spectral_amplitude(&samples, CARRIER);
        let sidebands = ORDERS.map(|order| {
            (
                decibels(spectral_amplitude(&samples, CARRIER - order * MODULATION) / carrier),
                decibels(spectral_amplitude(&samples, CARRIER + order * MODULATION) / carrier),
            )
        });
        let carrier_dbfs = decibels(carrier);
        let (lower_dbc, upper_dbc) = sidebands[0];
        writeln!(
            &mut csv,
            "{name},{carrier_dbfs:.6},{lower_dbc:.6},{upper_dbc:.6},{:.6},{:.6},{:.6},{:.6}",
            sidebands[1].0, sidebands[1].1, sidebands[2].0, sidebands[2].1
        )
        .expect("string write cannot fail");
        for frequency in BANDS {
            writeln!(
                &mut response,
                "{name},{frequency:.3},{:.6}",
                decibels(scanner_gain(mode, frequency))
            )
            .expect("string write cannot fail");
        }
        let probe = match mode {
            ScannerMode::Vibrato1 => "scanner-v1",
            ScannerMode::Vibrato2 => "scanner-v2",
            ScannerMode::Vibrato3 => "scanner-v3",
            ScannerMode::Chorus1 => "scanner-c1",
            ScannerMode::Chorus2 => "scanner-c2",
            ScannerMode::Chorus3 => "scanner-c3",
            ScannerMode::Off => unreachable!(),
        };
        measurements.extend([
            Measurement {
                probe,
                metric: "carrier-level",
                value: carrier_dbfs,
                unit: "dBFS",
            },
            Measurement {
                probe,
                metric: "lower-sideband",
                value: lower_dbc,
                unit: "dBc",
            },
            Measurement {
                probe,
                metric: "upper-sideband",
                value: upper_dbc,
                unit: "dBc",
            },
            Measurement {
                probe,
                metric: "mid-gain",
                value: decibels(scanner_gain(mode, 1_000.0)),
                unit: "dB",
            },
        ]);
    }
    (measurements, csv, response)
}

/// Level a steady tone comes back at, measured over a whole number of scanner
/// revolutions so that the rotor's own cycle averages out exactly. It captures
/// the ladder's lowpass shape together with the insertion loss of the selected
/// switch position.
fn scanner_gain(mode: ScannerMode, frequency: f64) -> f64 {
    const AMPLITUDE: f64 = 0.5;
    const REVOLUTIONS: f64 = 5.0;
    let settle = SAMPLE_RATE / 4;
    let window =
        (REVOLUTIONS * SAMPLE_RATE as f64 / f64::from(rf_organ_dsp::SCANNER_ROTOR_HZ)) as usize;
    let mut scanner = ScannerVibrato::new(SAMPLE_RATE as f32);
    scanner.set_mode(mode);
    let mut power = 0.0;
    for frame in 0..settle + window {
        let input = (TAU * frequency * frame as f64 / SAMPLE_RATE as f64).sin() as f32 * 0.5;
        let sample = f64::from(scanner.process(input));
        if frame >= settle {
            power += sample * sample;
        }
    }
    (2.0 * power / window as f64).sqrt() / AMPLITUDE
}

/// Whether the clock moves the vibrato line's response.
///
/// The components put the ladder's corner at just over seven kilohertz and
/// nothing about a sample rate should move it. Integrating a circuit by the
/// trapezoidal rule bends frequency, though, and by more the slower the clock.
///
/// The reading is a gain at fixed frequencies rather than a corner frequency.
/// An eighteen-section ladder ripples, so the curve crosses any particular
/// level more than once and asking where it crosses is asking the wrong
/// question; asking what the gain is at a stated frequency is not.
fn scanner_cutoff_probe() -> (Vec<Measurement>, String) {
    const RATES: [u32; 5] = [32_000, 44_100, 48_000, 96_000, 192_000];
    // The last one is the corner the components put there.
    let bands = [
        1_000.0,
        3_000.0,
        5_000.0,
        rf_organ_dsp::SCANNER_LINE_CUTOFF_HZ,
    ];
    let mut csv = String::from(
        "sample_rate_hz,frequency_hz,gain_db
",
    );
    let mut measurements = Vec::new();
    let mut spread = 0.0_f64;
    let mut corner = Vec::new();
    for frequency in bands {
        let (mut lowest, mut highest) = (f64::MAX, f64::MIN);
        for rate in RATES {
            let gain = decibels(line_gain(rate, frequency));
            lowest = lowest.min(gain);
            highest = highest.max(gain);
            writeln!(&mut csv, "{rate},{frequency:.1},{gain:.6}")
                .expect("string write cannot fail");
            if frequency == bands[bands.len() - 1] {
                corner.push(gain);
            }
        }
        spread = spread.max(highest - lowest);
    }
    let corner_mean = corner.iter().sum::<f64>() / corner.len() as f64;
    measurements.push(Measurement {
        probe: "scanner-line",
        metric: "rate-spread",
        value: spread,
        unit: "dB",
    });
    measurements.push(Measurement {
        probe: "scanner-line",
        metric: "corner-gain",
        value: corner_mean,
        unit: "dB",
    });
    (measurements, csv)
}

/// Gain of the line at one frequency, at a given rate.
///
/// The scanner is turning while this is measured, so the reading is taken over
/// a whole number of its revolutions and as power rather than at one bin: over
/// a full scan the answer is the line's, and over part of one it is whatever
/// the rotor happened to be doing.
fn line_gain(rate: u32, frequency: f64) -> f64 {
    const AMPLITUDE: f64 = 0.5;
    const REVOLUTIONS: f64 = 5.0;
    let settle = rate as usize / 4;
    let window =
        (REVOLUTIONS * f64::from(rate) / f64::from(rf_organ_dsp::SCANNER_ROTOR_HZ)) as usize;
    let mut scanner = ScannerVibrato::new(rate as f32);
    scanner.set_mode(ScannerMode::Vibrato1);
    let mut power = 0.0;
    for frame in 0..settle + window {
        let input = (TAU * frequency * frame as f64 / f64::from(rate)).sin() * AMPLITUDE;
        let sample = f64::from(scanner.process(input as f32));
        if frame >= settle {
            power += sample * sample;
        }
    }
    (2.0 * power / window as f64).sqrt() / AMPLITUDE
}

fn render_scanner(mode: ScannerMode, carrier: f64) -> Vec<f64> {
    let mut scanner = ScannerVibrato::new(SAMPLE_RATE as f32);
    scanner.set_mode(mode);
    let total = SAMPLE_RATE * 3;
    let skip = SAMPLE_RATE / 2;
    let mut output = Vec::with_capacity(total - skip);
    for frame in 0..total {
        let input = (TAU * carrier * frame as f64 / SAMPLE_RATE as f64).sin() as f32 * 0.5;
        let sample = scanner.process(input);
        if frame >= skip {
            output.push(f64::from(sample));
        }
    }
    output
}

/// The three transitions a cabinet makes, measured the way Hammond defines
/// them: the time to cross the whole speed range. Every phase starts from a
/// settled rotor, so the rise covers slow to fast, the fall covers fast to
/// slow, and the brake covers fast to a standstill.
fn rotary_probe() -> (Vec<Measurement>, String) {
    const PHASE_SECONDS: usize = 20;
    let mut rotary = Rotary::new(SAMPLE_RATE as f32);
    assert!(rotary.set_acceleration(0.5));
    let mut csv = String::from(
        "phase,time_seconds,horn_hz,drum_hz,horn_rpm,drum_rpm,horn_target_hz,drum_target_hz\n",
    );
    let mut measurements = Vec::new();

    // Settle at the slow speed first; a cabinet switched on from rest is not
    // what any of these times describe.
    rotary.set_mode(RotaryMode::Chorale);
    for _ in 0..SAMPLE_RATE * PHASE_SECONDS {
        rotary.process(0.0);
    }
    let settle = rotary.diagnostics();
    measurements.extend([
        Measurement {
            probe: "rotary-horn",
            metric: "chorale-speed",
            value: f64::from(settle.horn_speed_rpm),
            unit: "rpm",
        },
        Measurement {
            probe: "rotary-drum",
            metric: "chorale-speed",
            value: f64::from(settle.drum_speed_rpm),
            unit: "rpm",
        },
    ]);

    for (phase, mode) in [
        ("rise", RotaryMode::Tremolo),
        ("fall", RotaryMode::Chorale),
        ("brake", RotaryMode::Brake),
    ] {
        let before = rotary.diagnostics();
        if phase == "brake" {
            // Braking is specified from the fast speed.
            rotary.set_mode(RotaryMode::Tremolo);
            for _ in 0..SAMPLE_RATE * PHASE_SECONDS {
                rotary.process(0.0);
            }
        }
        let start = rotary.diagnostics();
        rotary.set_mode(mode);
        let mut horn_arrived = None;
        let mut drum_arrived = None;
        for frame in 0..=SAMPLE_RATE * PHASE_SECONDS {
            if frame > 0 {
                rotary.process(0.0);
            }
            let state = rotary.diagnostics();
            let time = frame as f64 / SAMPLE_RATE as f64;
            if horn_arrived.is_none() && (state.horn_speed_hz - state.horn_target_hz).abs() < 1.0e-6
            {
                horn_arrived = Some(time);
            }
            if drum_arrived.is_none() && (state.drum_speed_hz - state.drum_target_hz).abs() < 1.0e-6
            {
                drum_arrived = Some(time);
            }
            if frame.is_multiple_of(WINDOW) && time <= 12.0 {
                write_rotor_row(&mut csv, phase, time, state);
            }
        }
        let arrived = rotary.diagnostics();
        let span = |from: f32, to: f32| f64::from((to - from).abs() * 60.0);
        for (probe, measured, from, to) in [
            (
                "rotary-horn",
                horn_arrived,
                start.horn_speed_hz,
                arrived.horn_speed_hz,
            ),
            (
                "rotary-drum",
                drum_arrived,
                start.drum_speed_hz,
                arrived.drum_speed_hz,
            ),
        ] {
            measurements.extend([
                Measurement {
                    probe,
                    metric: match phase {
                        "rise" => "rise-time",
                        "fall" => "fall-time",
                        _ => "brake-time",
                    },
                    value: measured.unwrap_or(f64::NAN),
                    unit: "s",
                },
                Measurement {
                    probe,
                    metric: match phase {
                        "rise" => "rise-span",
                        "fall" => "fall-span",
                        _ => "brake-span",
                    },
                    value: span(from, to),
                    unit: "rpm",
                },
            ]);
        }
        let _ = before;
    }

    let final_state = rotary.diagnostics();
    measurements.extend([
        Measurement {
            probe: "rotary-horn",
            metric: "stopped-speed",
            value: f64::from(final_state.horn_speed_rpm),
            unit: "rpm",
        },
        Measurement {
            probe: "rotary-drum",
            metric: "stopped-speed",
            value: f64::from(final_state.drum_speed_rpm),
            unit: "rpm",
        },
    ]);
    (measurements, csv)
}

/// Straight-line distance from a rotor's mouth to a microphone, in metres.
fn path_length(distance: f64, lateral: f64, radius: f64, angle: f64) -> f64 {
    let along = distance - radius * angle.cos();
    let across = lateral - radius * angle.sin();
    (along * along + across * across).sqrt()
}

/// What the geometry says the pitch deviation and the arrival difference
/// should be, worked out from the path lengths alone.
struct GeometryPrediction {
    peak_cents: f64,
    far_field_cents: f64,
    arrival_span_us: f64,
}

fn predict(
    geometry: RotaryGeometry,
    place: RotaryPlacement,
    radius: f32,
    rotor_hz: f64,
) -> GeometryPrediction {
    const STEPS: usize = 4096;
    let distance = f64::from(place.distance_m);
    let half_width = f64::from(place.half_width_m);
    let speed = f64::from(geometry.sound_speed_m_per_s);
    let radius = f64::from(radius);
    let angular = TAU * rotor_hz;
    let step = TAU / STEPS as f64;
    let mut peak_cents: f64 = 0.0;
    let mut shortest = f64::MAX;
    let mut longest = f64::MIN;
    for index in 0..STEPS {
        let angle = index as f64 * step;
        // What a listener hears is the carrier times one minus the rate the
        // path is lengthening, over the speed of sound.
        let ahead = path_length(distance, half_width, radius, angle + 0.5 * step);
        let behind = path_length(distance, half_width, radius, angle - 0.5 * step);
        let lengthening = (ahead - behind) / step * angular;
        let cents = 1200.0 * (1.0 - lengthening / speed).log2();
        peak_cents = peak_cents.max(cents.abs());
        let difference = path_length(distance, half_width, radius, angle)
            - path_length(distance, -half_width, radius, angle);
        shortest = shortest.min(difference);
        longest = longest.max(difference);
    }
    GeometryPrediction {
        peak_cents,
        // The far-field form the literature gives: the tangential speed of
        // the radiating point over the speed of sound.
        far_field_cents: 1200.0 * (1.0 + radius * angular / speed).log2(),
        arrival_span_us: (longest - shortest) / speed * 1.0e6,
    }
}

/// Accumulates a phase that is allowed to leave the interval a four-quadrant
/// arctangent returns it in.
fn unwrap(previous: &mut f64, total: &mut f64, phase: f64) -> f64 {
    let mut delta = phase - *previous;
    while delta > PI {
        delta -= TAU;
    }
    while delta < -PI {
        delta += TAU;
    }
    *previous = phase;
    *total += delta;
    *total
}

/// Doppler and stereo image, against the geometry that causes them.
///
/// A rotor's mouth is a point travelling on a circle, so the line to a
/// microphone lengthens and shortens as it turns, and the pitch deviation a
/// listener hears is how fast that line changes over the speed of sound. The
/// far-field form of it is in the literature as the tangential speed of the
/// radiating point over the speed of sound. This probe measures what comes out
/// of the cabinet and compares it against both the exact path and that
/// formula: the radii are provisional, but what they produce is not guesswork.
fn rotary_doppler_probe() -> (Vec<Measurement>, String) {
    const SETTLE_SECONDS: usize = 12;
    const CAPTURE_SECONDS: usize = 3;
    // The deviation is read over a tenth of a rotation, which is long enough
    // that what is left of the carrier does not reach the answer and short
    // enough that the peak of a 6 Hz sweep survives it.
    const BLOCK: usize = 480;
    const SKIP_SECONDS: f64 = 0.6;
    let mut csv = String::from(
        "rotor,time_seconds,cents_left,cents_right,level_left_db,level_right_db,arrival_us\n",
    );
    let mut measurements = Vec::new();

    for (probe, carrier, horn) in [
        ("rotary-horn", 2_000.0_f64, true),
        ("rotary-drum", 400.0_f64, false),
    ] {
        let mut rotary = Rotary::new(SAMPLE_RATE as f32);
        assert!(rotary.set_mix(1.0));
        assert!(rotary.set_acceleration(0.5));
        // One rotor at a time, with the room switched off and the woofer
        // silent: this probe is about the path from one mouth to the
        // microphones, and the woofer's own bass arrives unmodulated, which
        // would dilute the very deviation being measured.
        assert!(rotary.set_cabinet(0.0));
        let (horn_db, drum_db) = if horn {
            (0.0, LEVEL_SILENT_DB)
        } else {
            (LEVEL_SILENT_DB, 0.0)
        };
        assert!(rotary.set_levels(horn_db, drum_db, LEVEL_SILENT_DB));
        assert!(rotary.set_microphones(MicrophoneArray::default()));
        rotary.set_mode(RotaryMode::Tremolo);

        let omega = TAU * carrier / SAMPLE_RATE as f64;
        let mut frame = 0_usize;
        for _ in 0..SAMPLE_RATE * SETTLE_SECONDS {
            let input = (omega * frame as f64).sin() as f32;
            rotary.process(input);
            frame += 1;
        }
        let state = rotary.diagnostics();
        let rotor_hz = if horn {
            f64::from(state.horn_speed_hz)
        } else {
            f64::from(state.drum_speed_hz)
        };
        let geometry = rotary.geometry();
        let (radius, place) = if horn {
            (geometry.horn_radius_m, geometry.horn)
        } else {
            (geometry.drum_radius_m, geometry.drum)
        };
        let prediction = predict(geometry, place, radius, rotor_hz);

        // Demodulate both channels against the carrier: the drift of the
        // resulting phase is the Doppler shift, and its magnitude is the level.
        // Three poles, because one leaves enough of the carrier's image in
        // the quadrature pair to swamp a deviation of a few tens of cents.
        let smoothing = {
            let x = TAU * 40.0 / SAMPLE_RATE as f64;
            x / (1.0 + x)
        };
        let mut quadrature = [[[0.0_f64; 2]; 3]; 2];
        let mut previous = [0.0_f64; 2];
        let mut total = [0.0_f64; 2];
        let mut history = [
            Vec::with_capacity(SAMPLE_RATE * CAPTURE_SECONDS),
            Vec::with_capacity(SAMPLE_RATE * CAPTURE_SECONDS),
        ];
        let mut level = [
            Vec::with_capacity(SAMPLE_RATE * CAPTURE_SECONDS),
            Vec::with_capacity(SAMPLE_RATE * CAPTURE_SECONDS),
        ];
        for _ in 0..SAMPLE_RATE * CAPTURE_SECONDS {
            let angle = omega * frame as f64;
            let input = angle.sin() as f32;
            let output = rotary.process(input);
            let (cosine, sine) = (angle.cos(), angle.sin());
            for channel in 0..2 {
                let value = f64::from(output[channel]);
                let mut pair = [value * cosine, -value * sine];
                for stage in &mut quadrature[channel] {
                    stage[0] += smoothing * (pair[0] - stage[0]);
                    stage[1] += smoothing * (pair[1] - stage[1]);
                    pair = *stage;
                }
                let phase = pair[1].atan2(pair[0]);
                history[channel].push(unwrap(&mut previous[channel], &mut total[channel], phase));
                level[channel].push((pair[0] * pair[0] + pair[1] * pair[1]).sqrt());
            }
            frame += 1;
        }

        let skip = (SKIP_SECONDS * SAMPLE_RATE as f64) as usize;
        let mut peak_cents: f64 = 0.0;
        let mut rise_cents = f64::MIN;
        let mut fall_cents = f64::MAX;
        let mut quietest = f64::MAX;
        let mut loudest = f64::MIN;
        let mut nearest = f64::MAX;
        let mut furthest = f64::MIN;
        for index in (skip + BLOCK)..history[0].len() {
            let mut cents = [0.0_f64; 2];
            for channel in 0..2 {
                let drift = history[channel][index] - history[channel][index - BLOCK];
                let deviation = drift * SAMPLE_RATE as f64 / (BLOCK as f64 * TAU);
                cents[channel] = 1200.0 * ((carrier + deviation) / carrier).log2();
            }
            peak_cents = peak_cents.max(cents[0].abs());
            rise_cents = rise_cents.max(cents[0]);
            fall_cents = fall_cents.min(cents[0]);
            let amplitude = level[0][index];
            if amplitude > 0.0 {
                quietest = quietest.min(amplitude);
                loudest = loudest.max(amplitude);
            }
            // The phase the two channels differ by, at a known carrier, is the
            // difference in how long the sound took to reach each microphone.
            let arrival = (history[0][index] - history[1][index]) / (TAU * carrier) * 1.0e6;
            nearest = nearest.min(arrival);
            furthest = furthest.max(arrival);
            if index % (SAMPLE_RATE / 200) == 0 {
                let seconds = index as f64 / SAMPLE_RATE as f64;
                let _ = writeln!(
                    csv,
                    "{probe},{seconds:.4},{:.3},{:.3},{:.3},{:.3},{arrival:.2}",
                    cents[0],
                    cents[1],
                    decibels(level[0][index]),
                    decibels(level[1][index]),
                );
            }
        }

        measurements.extend([
            Measurement {
                probe,
                metric: "doppler-rise",
                value: rise_cents,
                unit: "cents",
            },
            Measurement {
                probe,
                metric: "doppler-fall",
                value: fall_cents,
                unit: "cents",
            },
            Measurement {
                probe,
                metric: "doppler-geometric",
                value: prediction.peak_cents,
                unit: "cents",
            },
            Measurement {
                probe,
                metric: "doppler-far-field",
                value: prediction.far_field_cents,
                unit: "cents",
            },
            Measurement {
                probe,
                metric: "doppler-geometry-error",
                value: peak_cents - prediction.peak_cents,
                unit: "cents",
            },
            Measurement {
                probe,
                metric: "arrival-span",
                value: furthest - nearest,
                unit: "microseconds",
            },
            Measurement {
                probe,
                metric: "arrival-span-geometric",
                value: prediction.arrival_span_us,
                unit: "microseconds",
            },
            Measurement {
                probe,
                metric: "level-swing",
                value: decibels(loudest) - decibels(quietest),
                unit: "dB",
            },
        ]);
    }
    (measurements, csv)
}

/// How far apart two angles are on a circle, in degrees.
fn degrees_apart(left: f64, right: f64) -> f64 {
    let gap = (left - right).abs() % 360.0;
    if gap > 180.0 { 360.0 - gap } else { gap }
}

/// Where the rotors come to rest, and how long getting there takes.
///
/// Hammond documents a stop angle per rotor, from 0 to 359 degrees or a random
/// one, and separately a brake time defined as the time to stop from the fast
/// speed. A rotor slowing at a fixed rate covers whatever angle its speed
/// happens to give it, so the two cannot both hold unless the shape of the
/// deceleration is free. This probe checks that both do hold: the rotor lands
/// where it was aimed, and a stop from the fast speed still takes the
/// documented time.
fn rotary_stop_probe() -> (Vec<Measurement>, String) {
    const AIMS: [f32; 6] = [0.0, 37.0, 90.0, 180.0, 275.0, 359.0];
    let mut csv =
        String::from("start,aim_degrees,horn_degrees,drum_degrees,error_degrees,seconds\n");
    let mut measurements = Vec::new();
    let mut worst = [0.0_f64; 2];
    let mut from_fast_seconds = 0.0_f64;

    for (start, mode) in [("fast", RotaryMode::Tremolo), ("slow", RotaryMode::Chorale)] {
        let mut longest = 0.0_f64;
        for aim in AIMS {
            let angle = StopAngle::from_degrees(aim).expect("documented angle");
            let mut rotary = Rotary::new(SAMPLE_RATE as f32);
            assert!(rotary.set_stop_angles(angle, angle));
            rotary.set_mode(mode);
            for _ in 0..SAMPLE_RATE * 12 {
                rotary.process(0.0);
            }
            rotary.set_mode(RotaryMode::Brake);
            let mut frames = 0_usize;
            let mut turning = true;
            for _ in 0..SAMPLE_RATE * 20 {
                rotary.process(0.0);
                if turning {
                    frames += 1;
                    let state = rotary.diagnostics();
                    if state.horn_speed_hz <= 0.0 && state.drum_speed_hz <= 0.0 {
                        turning = false;
                    }
                }
            }
            let state = rotary.diagnostics();
            let horn = f64::from(state.horn_angle_degrees);
            let drum = f64::from(state.drum_angle_degrees);
            let aim = f64::from(aim);
            let seconds = frames as f64 / SAMPLE_RATE as f64;
            worst[0] = worst[0].max(degrees_apart(horn, aim));
            worst[1] = worst[1].max(degrees_apart(drum, aim));
            longest = longest.max(seconds);
            let error = degrees_apart(horn, aim).max(degrees_apart(drum, aim));
            let _ = writeln!(
                csv,
                "{start},{aim:.0},{horn:.3},{drum:.3},{error:.3},{seconds:.4}"
            );
        }
        if start == "fast" {
            from_fast_seconds = longest;
        }
    }

    measurements.extend([
        Measurement {
            probe: "rotary-horn",
            metric: "stop-angle-error",
            value: worst[0],
            unit: "degrees",
        },
        Measurement {
            probe: "rotary-drum",
            metric: "stop-angle-error",
            value: worst[1],
            unit: "degrees",
        },
        Measurement {
            probe: "rotary-cabinet",
            metric: "stop-from-fast",
            value: from_fast_seconds,
            unit: "seconds",
        },
    ]);
    (measurements, csv)
}

/// What the supply does to the speeds.
///
/// The rotors are belted to alternating-current motors, which turn at the
/// supply's frequency over their pole pairs, so every speed a cabinet reaches
/// is in proportion to the supply it is plugged into. The quoted speeds belong
/// to sixty cycles; this reports what the same cabinet does on fifty, which
/// has to be five sixths of them, and how much sooner it gets there, since the
/// belt ramps at the rate it always did over a shorter range.
fn rotary_supply_probe() -> Vec<Measurement> {
    let settle = |mains: MainsFrequency, mode: RotaryMode| {
        let mut rotary = Rotary::new(SAMPLE_RATE as f32);
        rotary.set_mains(mains);
        rotary.set_mode(mode);
        for _ in 0..SAMPLE_RATE * 14 {
            rotary.process(0.0);
        }
        let state = rotary.diagnostics();
        (
            f64::from(state.horn_speed_rpm),
            f64::from(state.drum_speed_rpm),
        )
    };
    let (rated_horn, rated_drum) = settle(MainsFrequency::Sixty, RotaryMode::Tremolo);
    let (exported_horn, exported_drum) = settle(MainsFrequency::Fifty, RotaryMode::Tremolo);
    let (slow_horn, _) = settle(MainsFrequency::Fifty, RotaryMode::Chorale);
    vec![
        Measurement {
            probe: "rotary-horn",
            metric: "fifty-hertz-tremolo",
            value: exported_horn,
            unit: "rpm",
        },
        Measurement {
            probe: "rotary-horn",
            metric: "fifty-hertz-chorale",
            value: slow_horn,
            unit: "rpm",
        },
        Measurement {
            probe: "rotary-horn",
            metric: "supply-ratio",
            value: exported_horn / rated_horn,
            unit: "ratio",
        },
        Measurement {
            probe: "rotary-drum",
            metric: "fifty-hertz-tremolo",
            value: exported_drum,
            unit: "rpm",
        },
        Measurement {
            probe: "rotary-drum",
            metric: "supply-ratio",
            value: exported_drum / rated_drum,
            unit: "ratio",
        },
    ]
}

/// The woofer and the capsules.
///
/// Two things are being separated here. The woofer's bass leaves the cabinet
/// without passing through a rotor, so raising it has to make the cabinet's
/// sweep shallower rather than deeper - it is adding a steady sound to a
/// moving one. And a directional capsule gains bass as it nears what it is
/// pointed at, by an amount the distance and the pattern decide, which is why
/// that part of the microphone choice belongs to the geometry and not to the
/// capsule's character.
fn rotary_capsule_probe() -> Vec<Measurement> {
    let sweep_depth = |sub_db: f32| {
        let mut rotary = Rotary::new(SAMPLE_RATE as f32);
        assert!(rotary.set_mix(1.0));
        assert!(rotary.set_cabinet(0.0));
        assert!(rotary.set_levels(0.0, 0.0, sub_db));
        rotary.set_mode(RotaryMode::Tremolo);
        let tone = |index: usize| ((index as f64 * TAU * 200.0 / SAMPLE_RATE as f64).sin()) as f32;
        for index in 0..SAMPLE_RATE * 12 {
            rotary.process(tone(index));
        }
        let (mut quietest, mut loudest, mut envelope) = (f64::MAX, 0.0_f64, 0.0_f64);
        for index in 0..SAMPLE_RATE {
            let [left, _] = rotary.process(tone(index));
            envelope += 0.002 * (f64::from(left.abs()) - envelope);
            if index > SAMPLE_RATE / 4 {
                quietest = quietest.min(envelope);
                loudest = loudest.max(envelope);
            }
        }
        decibels(loudest) - decibels(quietest)
    };

    let band = |frequency: f64, distance_m: f32, pattern: f32, capsule| {
        let mut rotary = Rotary::new(SAMPLE_RATE as f32);
        assert!(rotary.set_mix(1.0));
        assert!(rotary.set_cabinet(0.0));
        assert!(rotary.set_levels(0.0, 0.0, LEVEL_SILENT_DB));
        rotary.set_microphone_type(capsule);
        let stand = MicrophonePair {
            distance_m,
            spacing_m: 0.0,
            offset_m: 0.0,
            at_the_sides: false,
        };
        assert!(rotary.set_microphones(MicrophoneArray {
            horn: stand,
            drum: stand,
            pattern,
        }));
        rotary.set_mode(RotaryMode::Brake);
        let mut energy = 0.0_f64;
        for index in 0..SAMPLE_RATE {
            let tone = ((index as f64 * TAU * frequency / SAMPLE_RATE as f64).sin()) as f32;
            let [left, _] = rotary.process(tone);
            if index > SAMPLE_RATE / 2 {
                energy += f64::from(left) * f64::from(left);
            }
        }
        decibels(energy.sqrt())
    };

    // How much closer in gains, against how much of that is just being nearer.
    let lift = |pattern: f32| {
        band(50.0, 0.15, pattern, MicrophoneType::Condenser)
            - band(50.0, 1.5, pattern, MicrophoneType::Condenser)
    };
    let directional = lift(1.0);
    let omnidirectional = lift(0.0);

    vec![
        Measurement {
            probe: "rotary-cabinet",
            metric: "sweep-depth-woofer-silent",
            value: sweep_depth(LEVEL_SILENT_DB),
            unit: "dB",
        },
        Measurement {
            probe: "rotary-cabinet",
            metric: "sweep-depth-woofer-open",
            value: sweep_depth(0.0),
            unit: "dB",
        },
        Measurement {
            probe: "rotary-cabinet",
            metric: "proximity-lift-directional",
            value: directional - omnidirectional,
            unit: "dB",
        },
        Measurement {
            probe: "rotary-cabinet",
            metric: "closing-in-omnidirectional",
            value: omnidirectional,
            unit: "dB",
        },
        Measurement {
            probe: "rotary-cabinet",
            metric: "capsule-top",
            value: band(10_000.0, 0.35, 0.5, MicrophoneType::Dynamic)
                - band(10_000.0, 0.35, 0.5, MicrophoneType::Condenser),
            unit: "dB",
        },
        Measurement {
            probe: "rotary-cabinet",
            metric: "capsule-presence",
            value: band(4_000.0, 0.35, 0.5, MicrophoneType::Dynamic)
                - band(4_000.0, 0.35, 0.5, MicrophoneType::Condenser),
            unit: "dB",
        },
    ]
}

/// What the resiliently coupled drive does to the pitch, and whether it does
/// it to every wheel at once.
///
/// Hammond's own description is of a drive that is springy at every joint, and
/// of 91 wheels geared to one shaft. Both halves of that have a consequence
/// worth measuring. The first is that the pitch cannot sit still. The second
/// is stronger and is what tells this apart from a generator whose wheels
/// merely drift: if they share a shaft then they must stray together, in the
/// same proportion at the same moment, and two notes measured at once have to
/// agree.
fn drive_wobble_probe() -> Result<(Vec<Measurement>, String), String> {
    // Both notes sit above the wheels that carry a shaped profile, so what
    // the demodulator sees is as close to one tone as this generator gets.
    const LOW: u8 = 60;
    const HIGH: u8 = 84;
    const BUS: usize = 2;
    const SECONDS: usize = 4;
    // A fiftieth of a second: long enough to quiet the estimator, short
    // against the slowest thing the drive does.
    const BLOCK: usize = 960;

    let deviations = |wobble: f32| -> Result<(Vec<f64>, Vec<f64>, f64), String> {
        let mut tracks = Vec::new();
        let (mut lowest, mut highest) = (f64::MAX, f64::MIN);
        let mut seen_shaft = false;
        for note in [LOW, HIGH] {
            let mut engine = clean_engine()?;
            assert!(engine.set_drive_wobble(wobble));
            assert!(engine.set_eccentricity(0.0));
            assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, BUS, 8));
            assert!(engine.note_on_part(OrganPart::Upper, note, 1.0));
            // Drawbar 2 is the eight foot, so the wheel behind it sings at
            // the note's own pitch.
            let carrier = bus_frequency(note, BUS);

            let omega = TAU * carrier / SAMPLE_RATE as f64;
            let smoothing = {
                let x = TAU * 30.0 / SAMPLE_RATE as f64;
                x / (1.0 + x)
            };
            let mut quadrature = [[0.0_f64; 2]; 3];
            let mut shaft_filter = [0.0_f64; 3];
            let (mut shaft_total, mut shaft_window) = (0.0_f64, Vec::new());
            let (mut previous, mut total) = (0.0_f64, 0.0_f64);
            let mut phase = Vec::with_capacity(SAMPLE_RATE * SECONDS);
            for frame in 0..SAMPLE_RATE * SECONDS {
                let angle = omega * frame as f64;
                let value = f64::from(engine.next_sample()[0]);
                // The shaft's own state, put through the same three poles the
                // audio goes through, so that the two can be compared at all.
                // An estimator that reads the fast part away reads it away in
                // both, and a difference that survives that is the model's.
                let mut shaft = f64::from(engine.drive_deviation());
                for stage in &mut shaft_filter {
                    *stage += smoothing * (shaft - *stage);
                    shaft = *stage;
                }
                shaft_total += shaft;
                shaft_window.push(shaft_total);
                let mut pair = [value * angle.cos(), -value * angle.sin()];
                for stage in &mut quadrature {
                    stage[0] += smoothing * (pair[0] - stage[0]);
                    stage[1] += smoothing * (pair[1] - stage[1]);
                    pair = *stage;
                }
                let mut delta = pair[1].atan2(pair[0]) - previous;
                previous = pair[1].atan2(pair[0]);
                while delta > PI {
                    delta -= TAU;
                }
                while delta < -PI {
                    delta += TAU;
                }
                total += delta;
                phase.push(total);
            }

            // The drift of that phase over a block is the pitch deviation.
            let skip = SAMPLE_RATE / 2;
            let mut cents = Vec::new();
            for index in (skip + BLOCK)..phase.len() {
                let drift = phase[index] - phase[index - BLOCK];
                let hertz = drift * SAMPLE_RATE as f64 / (BLOCK as f64 * TAU);
                cents.push(1200.0 * ((carrier + hertz) / carrier).log2());
            }
            tracks.push(cents);

            // The same block average the pitch track gets, over the shaft's
            // own filtered reading. Once is enough: both notes share it.
            if !seen_shaft {
                seen_shaft = true;
                for index in (skip + BLOCK)..shaft_window.len() {
                    let mean = (shaft_window[index] - shaft_window[index - BLOCK]) / BLOCK as f64;
                    lowest = lowest.min(mean);
                    highest = highest.max(mean);
                }
            }
        }
        let high = tracks.pop().expect("two notes");
        let low = tracks.pop().expect("two notes");
        // What the shaft itself says it did, for the audio to be held against.
        let swing = 1200.0 * ((1.0 + highest) / (1.0 + lowest)).log2();
        Ok((low, high, swing))
    };

    let swing = |track: &[f64]| {
        let (mut lowest, mut highest) = (f64::MAX, f64::MIN);
        for value in track {
            lowest = lowest.min(*value);
            highest = highest.max(*value);
        }
        highest - lowest
    };

    let (steady_low, _, _) = deviations(0.0)?;
    let (low, high, shaft_swing) = deviations(1.0)?;

    // Do the two notes stray together? Their deviations are compared where
    // both are measured, as a correlation: one means one shaft, zero means two
    // wheels going their own way.
    let span = low.len().min(high.len());
    let (mut low_sum, mut high_sum) = (0.0, 0.0);
    for index in 0..span {
        low_sum += low[index];
        high_sum += high[index];
    }
    let (low_mean, high_mean) = (low_sum / span as f64, high_sum / span as f64);
    let (mut together, mut low_energy, mut high_energy) = (0.0, 0.0, 0.0);
    for index in 0..span {
        let (a, b) = (low[index] - low_mean, high[index] - high_mean);
        together += a * b;
        low_energy += a * a;
        high_energy += b * b;
    }
    let coherence = together / (low_energy * high_energy).sqrt().max(1.0e-12);

    let mut csv = String::from("time_seconds,low_cents,high_cents\n");
    for index in (0..span).step_by(SAMPLE_RATE / 200) {
        let seconds = index as f64 / SAMPLE_RATE as f64;
        let _ = writeln!(csv, "{seconds:.4},{:.4},{:.4}", low[index], high[index]);
    }

    Ok((
        vec![
            Measurement {
                probe: "generator-drive",
                metric: "wobble-swing-low",
                value: swing(&low),
                unit: "cents",
            },
            Measurement {
                probe: "generator-drive",
                metric: "wobble-swing-high",
                value: swing(&high),
                unit: "cents",
            },
            Measurement {
                probe: "generator-drive",
                metric: "wobble-swing-steady",
                value: swing(&steady_low),
                unit: "cents",
            },
            Measurement {
                probe: "generator-drive",
                metric: "wobble-swing-shaft",
                value: shaft_swing,
                unit: "cents",
            },
            Measurement {
                probe: "generator-drive",
                metric: "wobble-coherence",
                value: coherence,
                unit: "ratio",
            },
        ],
        csv,
    ))
}

/// How much percussion a key delivers, against how briskly it was pressed.
///
/// Hammond's own account: the decay "begins at the #1 contact and is released
/// at a specified contact", and "if you press a key very slowly, you may hear
/// the Percussion tone but only the end of the decay or no sound". Nothing
/// here implements that rule as a rule. The supply is discharged by whichever
/// contact touches first, the tone is heard through the harmonic bus contacts,
/// and a gentler press puts more time between the two, which is the whole of
/// the effect.
fn percussion_touch_probe() -> Result<Vec<Measurement>, String> {
    const VELOCITIES: [(&str, f32); 4] = [
        ("brisk", 1.0),
        ("firm", 0.6),
        ("light", 0.25),
        ("gentle", 0.05),
    ];
    let peak = |velocity: f32, delay: f32| -> Result<f64, String> {
        let mut engine = clean_engine()?;
        engine.set_percussion_enabled(true);
        engine.set_percussion_volume(PercussionVolume::Normal);
        engine.set_percussion_decay(PercussionDecay::Slow);
        engine.set_percussion_harmonic(PercussionHarmonic::Third);
        assert!(engine.set_contact_spread(1.0));
        assert!(engine.set_contact_delay(delay));
        assert!(engine.note_on_part(OrganPart::Upper, C_NOTE, velocity));
        let mut peak = 0.0_f64;
        for _ in 0..SAMPLE_RATE / 2 {
            peak = peak.max(f64::from(engine.next_sample()[0].abs()));
        }
        Ok(peak)
    };
    let brisk = peak(VELOCITIES[0].1, 0.0)?;
    let mut measurements = Vec::new();
    for (name, velocity) in VELOCITIES {
        let level = peak(velocity, 0.0)?;
        measurements.push(Measurement {
            probe: match name {
                "brisk" => "percussion-touch-brisk",
                "firm" => "percussion-touch-firm",
                "light" => "percussion-touch-light",
                _ => "percussion-touch-gentle",
            },
            metric: "peak-against-brisk",
            value: decibels(level / brisk.max(1.0e-15)),
            unit: "dB",
        });
    }

    // And with the documented delay wound in, which is the range the console's
    // own control covers and the range the sentence about a slow press needs.
    for (name, amount) in [("quarter", 0.25_f32), ("half", 0.5), ("full", 1.0)] {
        let level = peak(VELOCITIES[0].1, amount)?;
        measurements.push(Measurement {
            probe: match name {
                "quarter" => "percussion-delay-quarter",
                "half" => "percussion-delay-half",
                _ => "percussion-delay-full",
            },
            metric: "peak-against-brisk",
            value: decibels(level / brisk.max(1.0e-15)),
            unit: "dB",
        });
    }
    Ok(measurements)
}

/// How much click a contact makes, against how abruptly it is asked to
/// arrive.
///
/// Hammond's own control puts the click and the attack together: at zero "the
/// note will sound with no click at the onset of the sound as with a
/// traditional electronic instrument", and higher "will create a faster attack
/// as well as introduce Key Click". A click is broadband, so it is measured
/// where the tone is not: well above the wheel's own harmonics, during the
/// first few milliseconds, against the steady level the note settles to.
fn key_click_probe() -> Result<(Vec<Measurement>, String), String> {
    const BUS: usize = 2;
    const SILENCE: usize = SAMPLE_RATE / 200;
    const ONSET: usize = SAMPLE_RATE / 100;

    let mut csv = String::from(
        "key_click,click_dbc,steady_dbfs
",
    );
    let mut measurements = Vec::new();
    for (name, amount) in [
        ("off", 0.0_f32),
        ("quarter", 0.25),
        ("half", 0.5),
        ("full", 1.0),
    ] {
        let mut engine = clean_engine()?;
        assert!(engine.set_key_click(amount));
        assert!(engine.set_contact_bounce(0.0));
        assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, BUS, 8));
        // Silence first. The arrival is a discontinuity and a discontinuity
        // has to have something before it: keying on the first rendered sample
        // puts the step outside the window and the measurement then finds
        // nothing but the tone.
        let mut samples = Vec::with_capacity(SAMPLE_RATE / 2);
        for _ in 0..SILENCE {
            samples.push(f64::from(engine.next_sample()[0]));
        }
        assert!(engine.note_on_part(OrganPart::Upper, C_NOTE, 1.0));
        for _ in 0..SAMPLE_RATE / 2 {
            samples.push(f64::from(engine.next_sample()[0]));
        }
        let steady = rms(&samples[SILENCE + SAMPLE_RATE / 4..]);
        // A click is a step, and what tells a step from a tone is how far the
        // signal moves between one sample and the next. A tone's own jump is
        // bounded by its frequency; a contact arriving adds the whole of
        // whatever the wheel was at. Measuring the jump rather than a band
        // keeps the wheel's own harmonics out of the answer - the first
        // attempt here read a band well above them and found the gentle
        // arrival louder than the abrupt one, because a four millisecond ramp
        // splatters a tone more than a step's energy up there amounts to.
        let jump = |window: &[f64]| {
            window
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).abs())
                .fold(0.0_f64, f64::max)
        };
        let click = jump(&samples[..SILENCE + ONSET]);
        let quiet = jump(&samples[SILENCE + SAMPLE_RATE / 4..]);
        let relative = decibels(click / quiet.max(1.0e-15));
        writeln!(&mut csv, "{name},{relative:.6},{:.6}", decibels(steady))
            .expect("string write cannot fail");
        measurements.push(Measurement {
            probe: match name {
                "off" => "key-click-off",
                "quarter" => "key-click-quarter",
                "half" => "key-click-half",
                _ => "key-click-full",
            },
            metric: "click-over-tone",
            value: relative,
            unit: "dB",
        });
    }
    Ok((measurements, csv))
}

/// What the twelve reverse-colour keys at the left of a manual do.
///
/// The service manual makes three claims about them and all three are
/// measurable. "As there are no wires connected to these busbars, a preset or
/// adjust key must be depressed before any circuit can be completed" - so
/// cancel is silence. "The adjust keys, A# and B, are connected by flexible
/// wires ... to the corresponding nine drawbars" - so each key reaches its own
/// group and no part of the other. And "Percussion is available only on the
/// upper manual and only when the 'B' preset key is depressed".
///
/// The two groups are given different drawbars here, so that each one's tone
/// is a frequency the other cannot produce and the answer is how much of the
/// other group is present. For the percussion the two groups are made
/// identical instead, so that the only difference left is which key is down.
fn registration_probe() -> Result<(Vec<Measurement>, String), String> {
    // Two buses whose wheels are far apart: the fundamental and the twelfth.
    const A_BUS: usize = 2;
    const B_BUS: usize = 5;
    const FRAMES: usize = SAMPLE_RATE / 2;

    let key = usize::from(C_NOTE - MANUAL_FIRST_NOTE);
    let tone = |bus: usize| -> Result<f64, String> {
        let wheel = drawbar_wheel(key, bus).ok_or("bus has no wheel")?;
        Ok(f64::from(
            gear_frequency(wheel).ok_or("wheel has no frequency")?,
        ))
    };
    let a_hz = tone(A_BUS)?;
    let b_hz = tone(B_BUS)?;

    let render = |registration: Registration, switch: Option<Registration>| -> Vec<f64> {
        let mut engine = clean_engine().expect("engine");
        assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustA, A_BUS, 8));
        assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, B_BUS, 8));
        assert!(engine.set_manual_registration(OrganPart::Upper, registration));
        assert!(engine.note_on_part(OrganPart::Upper, C_NOTE, 1.0));
        let mut samples = Vec::with_capacity(FRAMES);
        for frame in 0..FRAMES {
            if let Some(next) = switch.filter(|_| frame == FRAMES / 2) {
                // Without lifting the key: the adjust key is in series with
                // the busbars, not in front of them.
                assert!(engine.set_manual_registration(OrganPart::Upper, next));
            }
            samples.push(f64::from(engine.next_sample()[0]));
        }
        samples
    };

    let mut csv = String::from("registration,own_dbfs,other_dbfs,other_over_own_db\n");
    let mut measurements = Vec::new();
    for (name, probe, registration, own, other) in [
        (
            "cancel",
            "registration-cancel",
            Registration::Cancel,
            a_hz,
            b_hz,
        ),
        (
            "adjust-a",
            "registration-adjust-a",
            Registration::AdjustA,
            a_hz,
            b_hz,
        ),
        (
            "adjust-b",
            "registration-adjust-b",
            Registration::AdjustB,
            b_hz,
            a_hz,
        ),
    ] {
        let samples = render(registration, None);
        let settled = &samples[SAMPLE_RATE / 8..];
        let own_level = spectral_amplitude(settled, own);
        let other_level = spectral_amplitude(settled, other);
        let relative = decibels(other_level / own_level.max(1.0e-15));
        writeln!(
            &mut csv,
            "{name},{:.6},{:.6},{relative:.6}",
            decibels(own_level),
            decibels(other_level)
        )
        .expect("string write cannot fail");
        measurements.push(Measurement {
            probe,
            // Under cancel there is no own tone either, so what is worth
            // reporting is the level of the whole manual, not a ratio.
            metric: if registration == Registration::Cancel {
                "manual-level"
            } else {
                "other-group-over-own"
            },
            value: if registration == Registration::Cancel {
                decibels(rms(settled))
            } else {
                relative
            },
            unit: "dB",
        });
    }

    // Switching under a held key. The B group's tone is absent in the first
    // half and is the whole of the second.
    let switched = render(Registration::AdjustA, Some(Registration::AdjustB));
    let before = spectral_amplitude(&switched[SAMPLE_RATE / 8..FRAMES / 2], b_hz);
    let after = spectral_amplitude(&switched[FRAMES / 2 + SAMPLE_RATE / 8..], b_hz);
    writeln!(
        &mut csv,
        "switch-held,{:.6},{:.6},{:.6}",
        decibels(after),
        decibels(before),
        decibels(after / before.max(1.0e-15))
    )
    .expect("string write cannot fail");
    measurements.push(Measurement {
        probe: "registration-switch-held",
        metric: "after-over-before",
        value: decibels(after / before.max(1.0e-15)),
        unit: "dB",
    });

    // The percussion, with the two groups made identical so that the key is
    // the only thing that differs between the two readings.
    //
    // What is read is the percussion's own contribution and not the peak of
    // the note: at 888 the tone is most of the peak, so a first attempt that
    // compared the two keys directly reported the percussion as 2.4 dB rather
    // than as absent. Each key is therefore rendered twice, with the
    // percussion switched on and off, and the answer is what switching it on
    // adds. Under a key the percussion is not wired to, that has to be
    // nothing at all.
    let percussion_peak = |registration: Registration, enabled: bool| -> f64 {
        let mut engine = clean_engine().expect("engine");
        for adjust in [Registration::AdjustA, Registration::AdjustB] {
            for bus in 0..3 {
                assert!(engine.set_manual_drawbar(OrganPart::Upper, adjust, bus, 8));
            }
        }
        assert!(engine.set_manual_registration(OrganPart::Upper, registration));
        engine.set_percussion_enabled(enabled);
        assert!(engine.note_on_part(OrganPart::Upper, C_NOTE, 1.0));
        let mut peak = 0.0_f64;
        for _ in 0..SAMPLE_RATE / 10 {
            peak = peak.max(f64::from(engine.next_sample()[0]).abs());
        }
        peak
    };
    for (name, probe, registration) in [
        (
            "percussion-adjust-b",
            "registration-percussion-adjust-b",
            Registration::AdjustB,
        ),
        (
            "percussion-adjust-a",
            "registration-percussion-adjust-a",
            Registration::AdjustA,
        ),
    ] {
        let with = percussion_peak(registration, true);
        let without = percussion_peak(registration, false);
        let added = decibels(with / without.max(1.0e-15));
        writeln!(
            &mut csv,
            "{name},{:.6},{:.6},{added:.6}",
            decibels(with),
            decibels(without)
        )
        .expect("string write cannot fail");
        measurements.push(Measurement {
            probe,
            metric: "percussion-over-none",
            value: added,
            unit: "dB",
        });
    }

    Ok((measurements, csv))
}

fn clean_engine() -> Result<OrganEngine, String> {
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).map_err(|error| error.0.to_owned())?;
    for part in [OrganPart::Upper, OrganPart::Lower] {
        for drawbar in 0..DRAWBAR_COUNT {
            assert!(engine.set_manual_drawbar(part, Registration::AdjustB, drawbar, 0));
        }
    }
    for drawbar in 0..2 {
        assert!(engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, drawbar, 0));
    }
    assert!(engine.set_contact_spread(0.0));
    assert!(engine.set_contact_bounce(0.0));
    assert!(engine.set_leakage(0.0));
    // A probe that is measuring the generator's gearing wants the shaft the
    // gearing assumes: perfectly steady. The drive's own unsteadiness has a
    // probe of its own.
    assert!(engine.set_drive_wobble(0.0));
    assert!(engine.set_transformer(0.0, 0.0));
    assert!(engine.set_console(0.0, 0.0, 0.0));
    assert!(engine.set_expression_character(0.0));
    assert!(engine.set_output_level(1.0));
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    engine.set_rotary_mode(RotaryMode::Off);
    Ok(engine)
}

fn measurement_csv(measurements: &[Measurement]) -> String {
    let mut csv = String::from("probe,metric,value,unit\n");
    for measurement in measurements {
        writeln!(
            &mut csv,
            "{},{},{:.9},{}",
            measurement.probe, measurement.metric, measurement.value, measurement.unit
        )
        .expect("string write cannot fail");
    }
    csv
}

fn spectral_amplitude(samples: &[f64], frequency: f64) -> f64 {
    crate::signal::spectral_amplitude(samples, frequency, SAMPLE_RATE as f64)
}

fn write_rotor_row(
    csv: &mut String,
    phase: &str,
    time: f64,
    state: rf_organ_dsp::RotaryDiagnostics,
) {
    writeln!(
        csv,
        "{phase},{time:.6},{:.9},{:.9},{:.6},{:.6},{:.9},{:.9}",
        state.horn_speed_hz,
        state.drum_speed_hz,
        state.horn_speed_rpm,
        state.drum_speed_rpm,
        state.horn_target_hz,
        state.drum_target_hz
    )
    .expect("string write cannot fail");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_analysis_stack<T: Send + 'static>(probe: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(probe)
            .unwrap()
            .join()
            .unwrap()
    }

    /// The taper is flat until a console is measured, and the eccentricity
    /// shows up as a sideband a revolution away from each tone.
    #[test]
    fn generator_taper_is_flat_and_eccentricity_is_audible_in_the_sidebands() {
        let (measurements, csv) = with_analysis_stack(|| generator_taper_probe().unwrap());
        assert_eq!(csv.lines().count(), MANUAL_KEY_COUNT + 1);
        let value = |probe| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe)
                .expect("probe")
                .value
        };
        // Foldback makes several keys share a wheel, but no wheel may sit far
        // from its neighbours while the taper is flat.
        assert!(
            value("generator-taper") < 1.0,
            "{}",
            value("generator-taper")
        );
        let sideband = value("generator-eccentricity");
        assert!(
            (-60.0..-20.0).contains(&sideband),
            "eccentricity sideband was {sideband} dBc"
        );
    }

    /// Hammond describes leakage as rising with the number of notes held.
    #[test]
    fn leakage_rises_with_the_number_of_notes() {
        let (measurements, csv) = with_analysis_stack(|| generator_leakage_probe().unwrap());
        assert_eq!(csv.lines().count(), 5);
        let value = |probe| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe)
                .expect("probe")
                .value
        };
        // Every keyed wheel has companions in its compartment.
        for line in csv.lines().skip(1) {
            let fields = line.split(',').collect::<Vec<_>>();
            let keys: usize = fields[1].parse().expect("keys");
            let paired: usize = fields[2].parse().expect("paired");
            assert!(paired >= keys * 2, "{line}");
        }
        // It has to rise at every step, not only between the ends: a leakage
        // that merely tracked the notes being played would keep the same
        // level against them and this would be flat.
        let steps = [
            "leakage-one-note",
            "leakage-two-notes",
            "leakage-four-notes",
            "leakage-eight-notes",
        ];
        for pair in steps.windows(2) {
            assert!(
                value(pair[1]) > value(pair[0]) + 0.5,
                "{} did not rise over {}: {} against {}",
                pair[1],
                pair[0],
                value(pair[1]),
                value(pair[0])
            );
        }
        for probe in steps {
            assert!(value(probe) < 0.0, "{probe} leaked at {}", value(probe));
        }
        let rise = measurements
            .iter()
            .find(|measurement| measurement.probe == "leakage-rise")
            .expect("rise")
            .value;
        assert!(
            (2.0..20.0).contains(&rise),
            "eight notes leaked {rise} dB over one"
        );
    }

    /// With the rate turned off the leakage keeps step with what is played,
    /// which is what this did before the rate existed. That case has to
    /// remain reachable, because it is the one a measurement would tell us to
    /// go back to.
    #[test]
    fn the_leakage_rate_can_be_turned_off() {
        // On the analysis stack like every other probe here: an engine holds a
        // contact per key per spring and does not fit on a test thread's own.
        with_analysis_stack(the_leakage_rate_can_be_turned_off_body);
    }

    fn the_leakage_rate_can_be_turned_off_body() {
        let level = |boost: f32, notes: &[u8]| {
            let mut engine = clean_engine().unwrap();
            assert!(engine.set_leakage(0.2));
            assert!(engine.set_leakage_boost(boost));
            assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, 2, 8));
            for note in notes {
                assert!(engine.note_on_part(OrganPart::Upper, *note, 1.0));
            }
            let mut samples = Vec::new();
            for frame in 0..SAMPLE_RATE * 3 / 4 {
                let sample = f64::from(engine.next_sample()[0]);
                if frame >= SAMPLE_RATE / 4 {
                    samples.push(sample);
                }
            }
            let mut played = 0.0;
            let mut leaked = 0.0;
            for note in notes {
                let key = usize::from(note - MANUAL_FIRST_NOTE);
                let wheel = drawbar_wheel(key, 2).expect("wheel");
                let amplitude =
                    spectral_amplitude(&samples, f64::from(gear_frequency(wheel).expect("wheel")));
                played += amplitude * amplitude;
                for companion in compartment_companions(wheel).into_iter().flatten() {
                    let leak = spectral_amplitude(
                        &samples,
                        f64::from(gear_frequency(companion).expect("companion")),
                    );
                    leaked += leak * leak;
                }
            }
            decibels(leaked.sqrt() / played.sqrt())
        };
        let one = level(0.0, &[48]);
        let eight = level(0.0, &[36, 40, 43, 47, 48, 52, 55, 59]);
        assert!(
            (eight - one).abs() < 1.5,
            "with the rate off the leakage still moved: {one} to {eight} dBc"
        );
    }

    #[test]
    fn tonewheel_probe_tracks_the_mechanical_frequency() {
        let measurements = with_analysis_stack(|| tonewheel_probe().unwrap());
        let cents = measurements
            .iter()
            .find(|measurement| measurement.metric == "frequency-error")
            .unwrap()
            .value;
        assert!(cents.abs() < 0.05, "frequency error was {cents} cents");
    }

    #[test]
    fn percussion_probe_separates_fast_and_slow_t60() {
        let (measurements, envelope, recovery) =
            with_analysis_stack(|| percussion_probe().unwrap());
        let value = |probe, metric| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe && measurement.metric == metric)
                .unwrap_or_else(|| panic!("{probe}/{metric}"))
                .value
        };
        let fast = value("percussion-fast", "t60");
        let slow = value("percussion-slow", "t60");
        assert!((0.2..0.4).contains(&fast), "fast T60 was {fast}");
        assert!((1.0..1.5).contains(&slow), "slow T60 was {slow}");
        assert!(
            value("percussion-soft-fast", "peak-level") < value("percussion-fast", "peak-level")
        );
        let attack = value("percussion-fast", "attack-time");
        assert!((0.5..12.0).contains(&attack), "attack was {attack} ms");

        // Second percussion is the 4' bus and third is the 2 2/3' bus, so the
        // third strike sits a fifth above the second.
        let second = value("percussion-second", "strike-frequency");
        let third = value("percussion-third", "strike-frequency");
        assert!(value("percussion-second", "strike-frequency-error").abs() < 1.0);
        assert!(value("percussion-third", "strike-frequency-error").abs() < 1.0);
        let interval = 1_200.0 * (third / second).log2();
        assert!(
            (interval - 702.0).abs() < 5.0,
            "interval was {interval} cents"
        );

        // Hammond documents about 6 dB out of the drawbars at Normal volume,
        // and none at Soft.
        let normal = value("percussion-normal", "drawbar-level");
        assert!(
            (normal + 6.0).abs() < 0.5,
            "drawbar level moved {normal} dB"
        );
        assert!(
            value("percussion-soft", "drawbar-level").abs() < 0.05,
            "soft percussion moved the drawbars"
        );

        assert_eq!(envelope.lines().count(), 4 * 200 + 1);
        let last = recovery.lines().last().expect("recovery rows");
        assert!(last.ends_with("0.000000"), "{last}");
        let immediate: f64 = recovery
            .lines()
            .nth(1)
            .expect("first gap")
            .rsplit(',')
            .next()
            .expect("relative")
            .parse()
            .expect("relative");
        assert!(
            immediate < -6.0,
            "an immediate re-strike returned {immediate} dB"
        );
    }

    #[test]
    fn keying_probe_orders_the_contacts_and_sees_the_click() {
        let (measurements, csv) = with_analysis_stack(|| keying_probe().unwrap());
        assert_eq!(csv.lines().count(), 3 * DRAWBAR_COUNT + 1);
        let spread = |probe| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe)
                .expect("probe")
                .value
        };
        // A slower press spreads the nine contacts further apart.
        assert!(spread("keying-soft") > spread("keying-hard"));
        for probe in ["keying-soft", "keying-medium", "keying-hard"] {
            assert!(
                (0.0..40.0).contains(&spread(probe)),
                "{probe} {}",
                spread(probe)
            );
        }
    }

    #[test]
    fn pedal_probe_separates_the_two_resistor_mixtures() {
        let (measurements, spectrum, release) = with_analysis_stack(|| pedal_probe().unwrap());
        let metric = |probe, name| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe && measurement.metric == name)
                .unwrap()
                .value
        };
        assert!(metric("pedal-16ft", "harmonic-1").abs() < 0.01);
        assert!(metric("pedal-8ft", "harmonic-2").abs() < 0.01);
        assert!(metric("pedal-16ft", "key-off-energy-t99.9") < 0.1);
        assert_eq!(spectrum.lines().count(), 17);
        assert_eq!(release.lines().count(), SAMPLE_RATE / 10 + 1);
    }

    /// The drum radiates without a rotating tone shelf, so what reaches the
    /// microphones is the geometry and nothing else: its deviation has to
    /// land on what the path lengths predict. The horn carries a shelf that
    /// turns with it, which adds phase modulation of its own, so its measured
    /// deviation is larger and no longer symmetric.
    /// The drive's unsteadiness has to reach every wheel at once, in the
    /// same proportion, because they are geared to one shaft. Two notes an
    /// octave and a half apart are measured together and have to agree; a
    /// generator whose wheels drifted on their own would not.
    /// The ladder's corner drifts with the clock, and this pins how much.
    ///
    /// That drift is the price of an unwarped step, and the price was chosen
    /// deliberately: warping the discretisation at the corner removes it
    /// entirely - 4.78 dB of spread becomes 0.001 - but scales the ladder's
    /// delay with the clock, and the vibrato depth that delay produces then
    /// moves 8.7 dB across the same rates. A delay line is kept as a delay.
    /// `vibrato_line.rs` carries the argument; this holds the number, so that
    /// a change to the discretisation has to come past a measurement.
    #[test]
    fn the_ladder_corner_drifts_by_the_amount_the_step_costs() {
        let (measurements, csv) = with_analysis_stack(scanner_cutoff_probe);
        let value = |metric: &str| {
            measurements
                .iter()
                .find(|measurement| measurement.metric == metric)
                .expect("measurement")
                .value
        };
        let mut corner: Vec<f64> = Vec::new();
        for line in csv.lines().skip(1) {
            let fields = line.split(',').collect::<Vec<_>>();
            let frequency: f64 = fields[1].parse().expect("frequency");
            if (frequency - rf_organ_dsp::SCANNER_LINE_CUTOFF_HZ).abs() < 1.0 {
                corner.push(fields[2].parse().expect("gain"));
            }
        }
        assert_eq!(corner.len(), 5);
        let lowest = corner.iter().copied().fold(f64::MAX, f64::min);
        let highest = corner.iter().copied().fold(f64::MIN, f64::max);
        let drift = highest - lowest;
        assert!(
            (3.0..6.0).contains(&drift),
            "the corner drifted {drift} dB across the rates: {corner:?}"
        );
        // The faster the clock the closer the corner gets to the circuit, so
        // the drift has a direction and not only a size.
        assert!(
            corner[corner.len() - 1] > corner[0],
            "the drift lost its direction: {corner:?}"
        );
        assert!(value("rate-spread") < 8.0);
        assert!(value("corner-gain") < 0.0);
    }

    /// The service manual's three claims about the reverse-colour keys, each
    /// as a number. Cancel is a circuit that is not completed, the two adjust
    /// keys reach their own group of drawbars and nothing of the other, and
    /// the percussion is wired to the B key alone.
    #[test]
    fn the_preset_keys_are_the_circuit_the_service_manual_describes() {
        let (measurements, csv) = with_analysis_stack(|| registration_probe().unwrap());
        let value = |probe: &str| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe)
                .expect("probe")
                .value
        };
        assert_eq!(csv.lines().count(), 7);
        // "A preset or adjust key must be depressed before any circuit can be
        // completed", and the cancel key has no contacts at all.
        assert!(
            value("registration-cancel") < -200.0,
            "the cancel key still sounded: {} dB",
            value("registration-cancel")
        );
        // Each adjust key is wired to its own nine drawbars, so the other
        // group's tone is not merely quiet but absent.
        for probe in ["registration-adjust-a", "registration-adjust-b"] {
            assert!(
                value(probe) < -100.0,
                "{probe} leaked the other group at {} dB",
                value(probe)
            );
        }
        // The key is in series with the busbars, so a held note changes with
        // it rather than waiting to be played again.
        assert!(
            value("registration-switch-held") > 60.0,
            "a held note did not follow the key: {} dB",
            value("registration-switch-held")
        );
        // "Percussion is available only on the upper manual and only when the
        // 'B' preset key is depressed." Under the other key the tablet may do
        // nothing whatever, including take the documented level out of the
        // drawbars.
        assert_eq!(value("registration-percussion-adjust-a"), 0.0);
        assert!(
            value("registration-percussion-adjust-b") < -1.0,
            "the percussion did nothing under its own key: {} dB",
            value("registration-percussion-adjust-b")
        );
    }

    /// Hammond's own key click control runs from a note that "will sound with
    /// no 'click' at the onset of the sound as with a traditional electronic
    /// instrument" up to one where a higher value "will create a faster attack
    /// as well as introduce Key Click". Those are the two ends of one thing -
    /// the click and the attack are the same event, a contact either arriving
    /// abruptly or not - so the measurement has to find them in that order.
    #[test]
    fn the_key_click_control_runs_from_no_click_to_a_click() {
        let (measurements, _) = with_analysis_stack(|| key_click_probe().unwrap());
        let value = |probe: &str| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe)
                .expect("probe")
                .value
        };
        let off = value("key-click-off");
        let quarter = value("key-click-quarter");
        let half = value("key-click-half");
        let full = value("key-click-full");
        // At zero the arrival is no louder than the tone's own motion, which
        // is what "no click at the onset" has to mean when the click is
        // measured against the tone that follows it.
        assert!(off < 1.0, "the softest arrival still clicked: {off} dB");
        // And it grows from there, in order, to something plainly audible.
        assert!(
            off < quarter && quarter < half && half < full,
            "the click did not grow in order: {off} {quarter} {half} {full}"
        );
        assert!(full > 15.0, "the abrupt arrival barely clicked: {full} dB");
    }
    /// Hammond's contact page carries a delay with a published ceiling of
    /// 725.6 ms, and its percussion page describes a key "pressed very slowly"
    /// leaving "only the end of the decay or no sound". Those two are the same
    /// sentence from opposite ends, and the model has to be able to get from
    /// one to the other: at the published ceiling the percussion has to be
    /// most of the way gone, and at no delay nothing may have moved at all.
    #[test]
    fn the_documented_delay_reaches_the_documented_percussion() {
        let measurements = with_analysis_stack(|| percussion_touch_probe().unwrap());
        let value = |probe: &str| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe)
                .expect("probe")
                .value
        };
        assert_eq!(value("percussion-touch-brisk"), 0.0);
        // A press alone barely moves it, because the spread a press gives is
        // eight milliseconds against a decay of about a second.
        for probe in [
            "percussion-touch-firm",
            "percussion-touch-light",
            "percussion-touch-gentle",
        ] {
            assert!(
                value(probe).abs() < 0.5,
                "{probe} moved {} dB on its own",
                value(probe)
            );
        }
        // The delay does, and in order.
        let quarter = value("percussion-delay-quarter");
        let half = value("percussion-delay-half");
        let full = value("percussion-delay-full");
        assert!(
            quarter < -1.0 && half < quarter && full < half,
            "the delay did not take the percussion away in order: {quarter} {half} {full}"
        );
        assert!(
            full < -10.0,
            "at the published ceiling the percussion was still {full} dB down"
        );
    }

    /// An odd curve has no even harmonics to give, so a transformer modelled
    /// as one produces no genuine second-order difference product however hard
    /// it is driven. The cores lean now, and the two products have to stand in
    /// the order a transformer stands them in: the symmetric saturation still
    /// leads, and the lean is a perturbation on it.
    #[test]
    fn the_transformer_gives_both_orders_in_the_right_order() {
        let (measurements, _) = with_analysis_stack(transformer_probe);
        let value = |probe: &str, metric: &str| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe && measurement.metric == metric)
                .expect("measurement")
                .value
        };
        for probe in [
            "transformer-baseline",
            "transformer-drive",
            "transformer-maximum",
        ] {
            let second = value(probe, "difference-product");
            let third = value(probe, "third-order-product");
            assert!(
                second > -70.0,
                "{probe} produced no second order at all: {second} dBc"
            );
            assert!(
                second < third,
                "{probe} let the lean overtake the saturation: {second} against {third}"
            );
            assert!(
                third - second < 25.0,
                "{probe} barely leans at all: {second} against {third}"
            );
        }
    }

    #[test]
    fn the_whole_generator_strays_together() {
        let (measurements, csv) = with_analysis_stack(|| drive_wobble_probe().unwrap());
        let value = |metric: &str| {
            measurements
                .iter()
                .find(|measurement| measurement.metric == metric)
                .expect("measurement")
                .value
        };
        assert!(csv.lines().count() > 100);

        let floor = value("wobble-swing-steady");
        assert!(floor < 0.2, "a steady shaft still moved {floor} cents");
        let (low, high) = (value("wobble-swing-low"), value("wobble-swing-high"));
        assert!(
            (0.5..8.0).contains(&low),
            "the drive swings {low} cents, which is not a generator"
        );
        assert!(low > 10.0 * floor, "the swing is in the noise");
        assert!(
            (low - high).abs() < 0.1 * low,
            "the two notes swung differently: {low} against {high}"
        );
        // And what reaches the audio has to be what the mechanism says it
        // did, read through the same estimator so the two are comparable.
        let shaft = value("wobble-swing-shaft");
        assert!(
            (low - shaft).abs() < 0.1 * shaft,
            "the audio strayed {low} cents against the shaft's {shaft}"
        );

        let coherence = value("wobble-coherence");
        assert!(
            coherence > 0.99,
            "the wheels are not sharing a shaft: {coherence}"
        );
    }

    #[test]
    fn doppler_follows_the_path_that_causes_it() {
        let (measurements, csv) = with_analysis_stack(rotary_doppler_probe);
        let value = |probe: &str, metric: &str| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe && measurement.metric == metric)
                .expect("measurement")
                .value
        };
        assert!(csv.lines().count() > 100);
        assert!(
            measurements
                .iter()
                .all(|measurement| measurement.value.is_finite())
        );

        let drum_error = value("rotary-drum", "doppler-geometry-error");
        assert!(drum_error.abs() < 2.0, "drum is off by {drum_error} cents");
        let drum_rise = value("rotary-drum", "doppler-rise");
        let drum_fall = value("rotary-drum", "doppler-fall");
        assert!(
            (drum_rise + drum_fall).abs() < 1.0,
            "drum sweep is lopsided: {drum_rise} and {drum_fall}"
        );
        let span = value("rotary-drum", "arrival-span");
        let predicted = value("rotary-drum", "arrival-span-geometric");
        assert!(
            (span - predicted).abs() < 0.1 * predicted,
            "arrival span {span} against {predicted} microseconds"
        );

        // The horn's own deviation still has to be of the size the geometry
        // sets, even with the shelf on top of it.
        let horn_error = value("rotary-horn", "doppler-geometry-error");
        assert!(horn_error > 0.0 && horn_error < 25.0, "horn {horn_error}");
        assert!(
            value("rotary-horn", "doppler-geometric") > value("rotary-drum", "doppler-geometric")
        );
        for rotor in ["rotary-horn", "rotary-drum"] {
            assert!(value(rotor, "level-swing") > 1.0);
        }
    }

    #[test]
    fn rotary_probe_observes_independent_rotor_inertia() {
        let (measurements, _) = with_analysis_stack(rotary_probe);
        let horn = measurements[2].value;
        let drum = measurements[3].value;
        assert!(horn > 0.0 && drum > horn, "horn={horn} drum={drum}");
        assert!(
            measurements
                .iter()
                .all(|measurement| measurement.value.is_finite())
        );
    }

    #[test]
    fn scanner_probe_produces_finite_sideband_levels() {
        let (measurements, csv, response) = with_analysis_stack(scanner_probe);
        assert_eq!(csv.lines().count(), 7);
        assert_eq!(response.lines().count(), 31);
        assert!(
            measurements
                .iter()
                .all(|measurement| measurement.value.is_finite())
        );
        let gain = |probe| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe && measurement.metric == "mid-gain")
                .expect("probe")
                .value
        };
        // The ladder passes its own passband; no position may run away.
        for probe in [
            "scanner-v1",
            "scanner-v2",
            "scanner-v3",
            "scanner-c1",
            "scanner-c2",
            "scanner-c3",
        ] {
            assert!(
                (-12.0..3.0).contains(&gain(probe)),
                "{probe} {}",
                gain(probe)
            );
        }
        // The line is a lowpass: 8 kHz comes back well below 1 kHz.
        let high = response
            .lines()
            .find(|line| line.starts_with("v3,8000"))
            .expect("v3 at 8 kHz")
            .rsplit(',')
            .next()
            .expect("gain")
            .parse::<f64>()
            .expect("gain");
        assert!(high < gain("scanner-v3") - 6.0, "8 kHz came back at {high}");
    }

    #[test]
    fn expression_probe_preserves_spectral_shoulders() {
        let (measurements, csv) = with_analysis_stack(expression_probe);
        let gain = |probe, metric| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe && measurement.metric == metric)
                .unwrap()
                .value
        };
        assert!(gain("expression-25", "low-gain") > gain("expression-25", "mid-gain"));
        assert!(gain("expression-25", "high-gain") > gain("expression-25", "mid-gain"));
        assert!(gain("expression-100", "mid-gain").abs() < 0.05);
        assert_eq!(csv.lines().count(), 16);
    }

    /// The console's stages are asymmetric, so second harmonic leads, and
    /// distortion has to grow with both drive and level.
    #[test]
    fn console_distortion_grows_with_drive_and_level() {
        let (measurements, csv) = with_analysis_stack(console_distortion_probe);
        assert_eq!(csv.lines().count(), 11);
        let value = |probe, metric| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe && measurement.metric == metric)
                .unwrap_or_else(|| panic!("{probe}/{metric}"))
                .value
        };
        assert!(value("console-clean", "thd") < 0.01);
        assert!(value("console-baseline", "thd") > value("console-quarter", "thd"));
        assert!(value("console-maximum", "thd") > value("console-baseline", "thd"));
        assert!(
            value("console-maximum", "second-harmonic")
                > value("console-maximum", "third-harmonic")
        );
        let quiet = csv
            .lines()
            .find(|line| line.starts_with("maximum,1.000,quiet"))
            .expect("quiet row");
        let loud = csv
            .lines()
            .find(|line| line.starts_with("maximum,1.000,loud"))
            .expect("loud row");
        let thd = |line: &str| {
            line.rsplit(',')
                .next()
                .expect("thd")
                .parse::<f64>()
                .expect("thd")
        };
        assert!(thd(loud) > thd(quiet), "{loud} against {quiet}");
    }

    #[test]
    fn tone_control_probe_is_a_broad_two_hundred_hertz_shelf() {
        let (measurements, csv) = with_analysis_stack(tone_control_probe);
        let gain = |probe, metric| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe && measurement.metric == metric)
                .unwrap()
                .value
        };
        assert!(gain("tone-minus-9", "high-gain") < -8.5);
        assert!(gain("tone-minus-9", "low-gain") > -4.0);
        assert!(gain("tone-plus-9", "high-gain") > 8.5);
        assert!(gain("tone-neutral", "mid-gain").abs() < 0.001);
        assert_eq!(csv.lines().count(), 16);
    }

    #[test]
    fn calibration_probe_moves_one_transformer_and_grows_with_level() {
        let (measurements, csv) = with_analysis_stack(transformer_calibration_probe);
        let rows = csv.lines().skip(1).collect::<Vec<_>>();
        assert_eq!(rows.len(), 27);
        assert!(csv.starts_with(
            "unit,drive_trim,effective_drive,effective_hysteresis,other_units_drive,level,tone_peak,c_dbfs,f_dbfs,difference_dbc,third_order_dbc\n"
        ));
        for row in &rows {
            let fields = row.split(',').collect::<Vec<_>>();
            let effective: f64 = fields[2].parse().expect("effective drive");
            let others: f64 = fields[4].parse().expect("other units");
            let trim: f64 = fields[1].parse().expect("trim");
            assert!((others - f64::from(CHARACTER.0)).abs() < 1.0e-6, "{row}");
            assert!(
                (effective - f64::from(CHARACTER.0) - trim).abs() < 1.0e-6,
                "{row}"
            );
        }
        let level = |probe: &str| {
            measurements
                .iter()
                .find(|measurement| {
                    measurement.probe == probe && measurement.metric == "third-order-product"
                })
                .expect("probe")
                .value
        };
        assert!(level("transformer-injection-low") < level("transformer-injection-nominal"));
        assert!(level("transformer-injection-nominal") < level("transformer-injection-high"));
    }

    #[test]
    fn transformer_probe_exposes_the_intermodulation_products() {
        let (measurements, csv) = with_analysis_stack(transformer_probe);
        let level = |probe| {
            measurements
                .iter()
                .find(|measurement| measurement.probe == probe)
                .unwrap()
                .value
        };
        assert!(level("transformer-clean") < -100.0);
        assert!(level("transformer-baseline") > level("transformer-clean"));
        assert!(level("transformer-maximum") > level("transformer-baseline"));
        assert_eq!(csv.lines().count(), 6);
    }
}
