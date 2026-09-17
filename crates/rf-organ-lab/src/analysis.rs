// SPDX-License-Identifier: GPL-2.0-or-later

use crate::captures::{C_NOTE, CHARACTER, F_NOTE, Level, note_frequency};
use crate::signal::{decay_time, decibels, peak, rms, zero_crossing_frequency};
use rf_organ_dsp::{
    ConsoleElectronics, DRAWBAR_COUNT, Leslie, LeslieMode, MANUAL_FIRST_NOTE, MatchingTransformer,
    OrganEngine, OrganPart, PercussionDecay, PercussionHarmonic, PercussionVolume, ScannerMode,
    ScannerVibrato, TransformerUnit, drawbar_wheel, gear_frequency,
};
use std::f64::consts::TAU;
use std::fmt::Write as _;

const SAMPLE_RATE: usize = 48_000;
const WINDOW: usize = SAMPLE_RATE / 100;

pub struct Artifacts {
    pub measurements: String,
    pub percussion_envelope: String,
    pub percussion_recovery: String,
    pub keying_contacts: String,
    pub scanner_sidebands: String,
    pub scanner_line_response: String,
    pub leslie_rotor_response: String,
    pub pedal_spectrum: String,
    pub pedal_release: String,
    pub expression_response: String,
    pub tone_control_response: String,
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
    let (expression, expression_response) = expression_probe();
    measurements.extend(expression);
    let (tone_control, tone_control_response) = tone_control_probe();
    measurements.extend(tone_control);
    let (transformer, transformer_intermodulation) = transformer_probe();
    measurements.extend(transformer);
    let (calibration, transformer_calibration) = transformer_calibration_probe();
    measurements.extend(calibration);
    let (pedal, pedal_spectrum, pedal_release) = pedal_probe()?;
    measurements.extend(pedal);
    let (percussion, percussion_envelope, percussion_recovery) = percussion_probe()?;
    measurements.extend(percussion);
    let (keying, keying_contacts) = keying_probe()?;
    measurements.extend(keying);
    let (scanner, scanner_sidebands, scanner_line_response) = scanner_probe();
    measurements.extend(scanner);
    let (leslie, leslie_rotor_response) = leslie_probe();
    measurements.extend(leslie);
    Ok(Artifacts {
        measurements: measurement_csv(&measurements),
        percussion_envelope,
        percussion_recovery,
        keying_contacts,
        scanner_sidebands,
        scanner_line_response,
        leslie_rotor_response,
        pedal_spectrum,
        pedal_release,
        expression_response,
        tone_control_response,
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
        assert!(engine.set_manual_drawbar(OrganPart::Pedal, index, position));
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
    assert!(engine.set_manual_drawbar(OrganPart::Pedal, 0, 8));
    assert!(engine.note_on_part(OrganPart::Pedal, 24, 1.0));
    for _ in 0..SAMPLE_RATE {
        engine.next_sample();
    }
    assert!(engine.note_off_part(OrganPart::Pedal, 24, 1.0));
    Ok((0..SAMPLE_RATE / 10)
        .map(|_| f64::from(engine.next_sample()[0]))
        .collect())
}

fn tonewheel_probe() -> Result<Vec<Measurement>, String> {
    const NOTE: u8 = 69;
    const BUS: usize = 2;
    let mut engine = clean_engine()?;
    assert!(engine.set_manual_drawbar(OrganPart::Upper, BUS, 8));
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
        assert!(engine.set_manual_drawbar(OrganPart::Upper, drawbar, 8));
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
    assert!(engine.set_manual_drawbar(OrganPart::Upper, bus, 8));
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

fn leslie_probe() -> (Vec<Measurement>, String) {
    const ACCELERATE_SECONDS: usize = 8;
    const BRAKE_SECONDS: usize = 8;
    let mut leslie = Leslie::new(SAMPLE_RATE as f32);
    assert!(leslie.set_acceleration(0.5));
    leslie.set_mode(LeslieMode::Tremolo);
    let mut csv =
        String::from("phase,time_seconds,horn_hz,drum_hz,horn_target_hz,drum_target_hz\n");
    let mut horn_t63 = None;
    let mut drum_t63 = None;
    let mut horn_t90 = None;
    let mut drum_t90 = None;
    for frame in 0..=SAMPLE_RATE * ACCELERATE_SECONDS {
        if frame > 0 {
            leslie.process(0.0);
        }
        let state = leslie.diagnostics();
        let time = frame as f64 / SAMPLE_RATE as f64;
        threshold_time(
            &mut horn_t63,
            state.horn_speed_hz,
            state.horn_target_hz * 0.632_120_55,
            time,
        );
        threshold_time(
            &mut drum_t63,
            state.drum_speed_hz,
            state.drum_target_hz * 0.632_120_55,
            time,
        );
        threshold_time(
            &mut horn_t90,
            state.horn_speed_hz,
            state.horn_target_hz * 0.9,
            time,
        );
        threshold_time(
            &mut drum_t90,
            state.drum_speed_hz,
            state.drum_target_hz * 0.9,
            time,
        );
        if frame.is_multiple_of(WINDOW) {
            write_rotor_row(&mut csv, "accelerate", time, state);
        }
    }
    let brake_start = leslie.diagnostics();
    leslie.set_mode(LeslieMode::Brake);
    let mut horn_brake_t63 = None;
    let mut drum_brake_t63 = None;
    for frame in 0..=SAMPLE_RATE * BRAKE_SECONDS {
        if frame > 0 {
            leslie.process(0.0);
        }
        let state = leslie.diagnostics();
        let time = frame as f64 / SAMPLE_RATE as f64;
        falling_threshold_time(
            &mut horn_brake_t63,
            state.horn_speed_hz,
            brake_start.horn_speed_hz * 0.367_879_45,
            time,
        );
        falling_threshold_time(
            &mut drum_brake_t63,
            state.drum_speed_hz,
            brake_start.drum_speed_hz * 0.367_879_45,
            time,
        );
        if frame.is_multiple_of(WINDOW) {
            write_rotor_row(&mut csv, "brake", time, state);
        }
    }
    let value = |value: Option<f64>| value.unwrap_or(f64::NAN);
    (
        vec![
            Measurement {
                probe: "leslie-horn",
                metric: "tremolo-target",
                value: f64::from(brake_start.horn_target_hz),
                unit: "Hz",
            },
            Measurement {
                probe: "leslie-drum",
                metric: "tremolo-target",
                value: f64::from(brake_start.drum_target_hz),
                unit: "Hz",
            },
            Measurement {
                probe: "leslie-horn",
                metric: "acceleration-t63",
                value: value(horn_t63),
                unit: "s",
            },
            Measurement {
                probe: "leslie-drum",
                metric: "acceleration-t63",
                value: value(drum_t63),
                unit: "s",
            },
            Measurement {
                probe: "leslie-horn",
                metric: "acceleration-t90",
                value: value(horn_t90),
                unit: "s",
            },
            Measurement {
                probe: "leslie-drum",
                metric: "acceleration-t90",
                value: value(drum_t90),
                unit: "s",
            },
            Measurement {
                probe: "leslie-horn",
                metric: "brake-t63",
                value: value(horn_brake_t63),
                unit: "s",
            },
            Measurement {
                probe: "leslie-drum",
                metric: "brake-t63",
                value: value(drum_brake_t63),
                unit: "s",
            },
        ],
        csv,
    )
}

fn clean_engine() -> Result<OrganEngine, String> {
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).map_err(|error| error.0.to_owned())?;
    for part in [OrganPart::Upper, OrganPart::Lower] {
        for drawbar in 0..DRAWBAR_COUNT {
            assert!(engine.set_manual_drawbar(part, drawbar, 0));
        }
    }
    for drawbar in 0..2 {
        assert!(engine.set_manual_drawbar(OrganPart::Pedal, drawbar, 0));
    }
    assert!(engine.set_contact_spread(0.0));
    assert!(engine.set_contact_bounce(0.0));
    assert!(engine.set_leakage(0.0));
    assert!(engine.set_transformer(0.0, 0.0));
    assert!(engine.set_console(0.0, 0.0, 0.0));
    assert!(engine.set_expression_character(0.0));
    assert!(engine.set_output_level(1.0));
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    engine.set_leslie_mode(LeslieMode::Off);
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

fn threshold_time(result: &mut Option<f64>, value: f32, threshold: f32, time: f64) {
    if result.is_none() && value >= threshold {
        *result = Some(time);
    }
}

fn falling_threshold_time(result: &mut Option<f64>, value: f32, threshold: f32, time: f64) {
    if result.is_none() && value <= threshold {
        *result = Some(time);
    }
}

fn write_rotor_row(
    csv: &mut String,
    phase: &str,
    time: f64,
    state: rf_organ_dsp::LeslieDiagnostics,
) {
    writeln!(
        csv,
        "{phase},{time:.6},{:.9},{:.9},{:.9},{:.9}",
        state.horn_speed_hz, state.drum_speed_hz, state.horn_target_hz, state.drum_target_hz
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

    #[test]
    fn leslie_probe_observes_independent_rotor_inertia() {
        let (measurements, _) = with_analysis_stack(leslie_probe);
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
