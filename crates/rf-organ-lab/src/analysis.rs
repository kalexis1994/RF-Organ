// SPDX-License-Identifier: GPL-2.0-or-later

use rf_organ_dsp::{
    DRAWBAR_COUNT, Leslie, LeslieMode, MANUAL_FIRST_NOTE, OrganEngine, OrganPart, PercussionDecay,
    PercussionHarmonic, PercussionVolume, ScannerMode, ScannerVibrato, drawbar_wheel,
    gear_frequency,
};
use std::f64::consts::TAU;
use std::fmt::Write as _;

const SAMPLE_RATE: usize = 48_000;
const WINDOW: usize = SAMPLE_RATE / 100;

pub struct Artifacts {
    pub measurements: String,
    pub percussion_envelope: String,
    pub scanner_sidebands: String,
    pub leslie_rotor_response: String,
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
    let (percussion, percussion_envelope) = percussion_probe()?;
    measurements.extend(percussion);
    let (scanner, scanner_sidebands) = scanner_probe();
    measurements.extend(scanner);
    let (leslie, leslie_rotor_response) = leslie_probe();
    measurements.extend(leslie);
    Ok(Artifacts {
        measurements: measurement_csv(&measurements),
        percussion_envelope,
        scanner_sidebands,
        leslie_rotor_response,
    })
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

fn percussion_probe() -> Result<(Vec<Measurement>, String), String> {
    let fast = percussion_curve(PercussionDecay::Fast)?;
    let slow = percussion_curve(PercussionDecay::Slow)?;
    let fast_t60 =
        decay_time(&fast, 0.001).ok_or_else(|| "fast percussion did not decay".to_owned())?;
    let slow_t60 =
        decay_time(&slow, 0.001).ok_or_else(|| "slow percussion did not decay".to_owned())?;
    let mut csv = String::from("time_seconds,fast_rms,slow_rms\n");
    for ((time, fast), (_, slow)) in fast.iter().zip(&slow) {
        writeln!(&mut csv, "{time:.6},{fast:.9},{slow:.9}").expect("string write cannot fail");
    }
    Ok((
        vec![
            Measurement {
                probe: "percussion-fast",
                metric: "t60",
                value: fast_t60,
                unit: "s",
            },
            Measurement {
                probe: "percussion-slow",
                metric: "t60",
                value: slow_t60,
                unit: "s",
            },
        ],
        csv,
    ))
}

fn percussion_curve(decay: PercussionDecay) -> Result<Vec<(f64, f64)>, String> {
    let mut engine = clean_engine()?;
    engine.set_percussion_enabled(true);
    engine.set_percussion_harmonic(PercussionHarmonic::Third);
    engine.set_percussion_volume(PercussionVolume::Normal);
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

fn scanner_probe() -> (Vec<Measurement>, String) {
    const CARRIER: f64 = 1_000.0;
    const MODULATION: f64 = 6.9;
    let modes = [
        ("v1", ScannerMode::Vibrato1),
        ("v2", ScannerMode::Vibrato2),
        ("v3", ScannerMode::Vibrato3),
        ("c1", ScannerMode::Chorus1),
        ("c2", ScannerMode::Chorus2),
        ("c3", ScannerMode::Chorus3),
    ];
    let mut measurements = Vec::new();
    let mut csv = String::from("mode,carrier_dbfs,lower_sideband_dbc,upper_sideband_dbc\n");
    for (name, mode) in modes {
        let samples = render_scanner(mode, CARRIER);
        let carrier = spectral_amplitude(&samples, CARRIER);
        let lower = spectral_amplitude(&samples, CARRIER - MODULATION);
        let upper = spectral_amplitude(&samples, CARRIER + MODULATION);
        let carrier_dbfs = decibels(carrier);
        let lower_dbc = decibels(lower / carrier);
        let upper_dbc = decibels(upper / carrier);
        writeln!(
            &mut csv,
            "{name},{carrier_dbfs:.6},{lower_dbc:.6},{upper_dbc:.6}"
        )
        .expect("string write cannot fail");
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
        ]);
    }
    (measurements, csv)
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

fn zero_crossing_frequency(samples: &[f64], sample_rate: f64) -> Option<f64> {
    let mut crossings = Vec::new();
    for (index, pair) in samples.windows(2).enumerate() {
        if pair[0] <= 0.0 && pair[1] > 0.0 {
            let fraction = -pair[0] / (pair[1] - pair[0]);
            crossings.push(index as f64 + fraction);
        }
    }
    let first = *crossings.first()?;
    let last = *crossings.last()?;
    (crossings.len() > 1).then(|| (crossings.len() - 1) as f64 * sample_rate / (last - first))
}

fn decay_time(curve: &[(f64, f64)], ratio: f64) -> Option<f64> {
    let (peak_index, (_, peak)) = curve
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.1.total_cmp(&right.1))?;
    let threshold = peak * ratio;
    curve[peak_index..]
        .iter()
        .find(|(_, level)| *level <= threshold)
        .map(|(time, _)| time - curve[peak_index].0)
}

fn spectral_amplitude(samples: &[f64], frequency: f64) -> f64 {
    let count = samples.len();
    let mut real = 0.0;
    let mut imaginary = 0.0;
    let mut weight_sum = 0.0;
    for (index, sample) in samples.iter().copied().enumerate() {
        let weight = 0.5 - 0.5 * (TAU * index as f64 / (count - 1) as f64).cos();
        let phase = TAU * frequency * index as f64 / SAMPLE_RATE as f64;
        real += sample * weight * phase.cos();
        imaginary -= sample * weight * phase.sin();
        weight_sum += weight;
    }
    2.0 * real.hypot(imaginary) / weight_sum
}

fn decibels(value: f64) -> f64 {
    20.0 * value.max(1.0e-15).log10()
}

fn rms(samples: &[f64]) -> f64 {
    (samples.iter().map(|sample| sample * sample).sum::<f64>() / samples.len() as f64).sqrt()
}

fn peak(samples: &[f64]) -> f64 {
    samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f64::max)
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
        let (measurements, _) = with_analysis_stack(|| percussion_probe().unwrap());
        let fast = measurements[0].value;
        let slow = measurements[1].value;
        assert!((0.2..0.4).contains(&fast), "fast T60 was {fast}");
        assert!((1.0..1.5).contains(&slow), "slow T60 was {slow}");
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
        let (measurements, csv) = with_analysis_stack(scanner_probe);
        assert_eq!(csv.lines().count(), 7);
        assert!(
            measurements
                .iter()
                .all(|measurement| measurement.value.is_finite())
        );
    }
}
