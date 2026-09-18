// SPDX-License-Identifier: GPL-2.0-or-later

//! Sample-rate invariance survey.
//!
//! The engine accepts 32 to 192 kHz, but every other probe in the laboratory
//! runs at 48 kHz. This module measures one quantity per subsystem at several
//! rates and reports how far each one moves from the 48 kHz baseline. A
//! quantity that drifts with the sample rate is a coefficient that was written
//! in samples where it should have been written in seconds or hertz.

use crate::captures::CHARACTER;
use crate::signal::{decay_time, decibels, rms, spectral_amplitude, zero_crossing_frequency};
use rf_organ_dsp::{
    ConsoleElectronics, DRAWBAR_COUNT, MANUAL_FIRST_NOTE, MatchingTransformer, OrganEngine,
    OrganPart, PercussionDecay, PercussionHarmonic, PercussionVolume, Registration, Rotary,
    RotaryMode, ScannerMode, ScannerVibrato, drawbar_wheel, gear_frequency,
};
use std::f64::consts::TAU;
use std::fmt::Write as _;

/// The documented minimum and maximum plus the two common studio rates.
pub const RATES: [u32; 5] = [32_000, 44_100, 48_000, 96_000, 192_000];
/// Index of the 48 kHz column every other rate is compared against.
const BASELINE: usize = 2;

/// One quantity measured at one sample rate.
type Probe = fn(u32) -> Result<f64, String>;
/// Probe name, metric name, unit, tolerance and the measurement itself.
type ProbeSpec = (&'static str, &'static str, &'static str, f64, Probe);

#[derive(Clone, Debug)]
pub struct Invariant {
    pub probe: &'static str,
    pub metric: &'static str,
    pub unit: &'static str,
    /// Largest deviation from the 48 kHz value this quantity may show before
    /// it counts as sample-rate dependent.
    pub tolerance: f64,
    pub values: [f64; RATES.len()],
}

impl Invariant {
    pub const fn baseline(&self) -> f64 {
        self.values[BASELINE]
    }

    pub fn worst_deviation(&self) -> f64 {
        self.values
            .iter()
            .map(|value| (value - self.baseline()).abs())
            .fold(0.0, f64::max)
    }

    pub fn holds(&self) -> bool {
        self.values.iter().all(|value| value.is_finite())
            && self.worst_deviation() <= self.tolerance
    }
}

/// Runs the survey on a worker thread, like the main analysis pass: an engine
/// is a large value and the 192 kHz renders are long.
pub fn survey() -> Result<Vec<Invariant>, String> {
    std::thread::Builder::new()
        .name("rf-organ-invariance".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(survey_inner)
        .map_err(|error| format!("starting invariance worker: {error}"))?
        .join()
        .map_err(|_| "invariance worker panicked".to_owned())?
}

fn survey_inner() -> Result<Vec<Invariant>, String> {
    let probes: [ProbeSpec; 11] = [
        ("tonewheel-a4", "frequency", "Hz", 0.05, tonewheel_frequency),
        (
            "tonewheel-a4",
            "frequency-error",
            "cent",
            0.05,
            tonewheel_error_cents,
        ),
        ("chord-888", "steady-level", "dBFS", 0.5, chord_level),
        ("percussion-fast", "t60", "s", 0.02, percussion_t60),
        ("scanner-c3", "sideband", "dBc", 1.0, scanner_sideband),
        ("rotary-tremolo", "horn-rate", "Hz", 0.05, rotary_horn_rate),
        ("rotary-tremolo", "horn-t63", "s", 0.05, rotary_horn_t63),
        ("expression-50", "mid-gain", "dB", 0.3, expression_gain),
        ("tone-plus-9", "high-gain", "dB", 0.5, tone_gain),
        (
            "transformer-baseline",
            "third-order-product",
            "dBc",
            0.5,
            transformer_third_order,
        ),
        (
            "pedal-16ft",
            "key-off-energy-t99.9",
            "s",
            0.01,
            pedal_key_off,
        ),
    ];
    let mut invariants = Vec::with_capacity(probes.len());
    for (probe, metric, unit, tolerance, measure) in probes {
        let mut values = [0.0; RATES.len()];
        for (slot, rate) in values.iter_mut().zip(RATES) {
            *slot = measure(rate)?;
        }
        invariants.push(Invariant {
            probe,
            metric,
            unit,
            tolerance,
            values,
        });
    }
    Ok(invariants)
}

pub fn report(invariants: &[Invariant]) -> String {
    let mut csv = String::from("probe,metric,unit,tolerance");
    for rate in RATES {
        write!(&mut csv, ",value_{rate}_hz").expect("string write cannot fail");
    }
    csv.push_str(",worst_deviation,status\n");
    for invariant in invariants {
        write!(
            &mut csv,
            "{},{},{},{:.6}",
            invariant.probe, invariant.metric, invariant.unit, invariant.tolerance
        )
        .expect("string write cannot fail");
        for value in invariant.values {
            write!(&mut csv, ",{value:.6}").expect("string write cannot fail");
        }
        writeln!(
            &mut csv,
            ",{:.6},{}",
            invariant.worst_deviation(),
            if invariant.holds() {
                "invariant"
            } else {
                "rate-dependent"
            }
        )
        .expect("string write cannot fail");
    }
    csv
}

fn clean_engine(rate: u32) -> Result<OrganEngine, String> {
    let mut engine = OrganEngine::new(rate as f32).map_err(|error| error.0.to_owned())?;
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
    assert!(engine.set_transformer(0.0, 0.0));
    assert!(engine.set_console(0.0, 0.0, 0.0));
    assert!(engine.set_expression_character(0.0));
    assert!(engine.set_output_level(1.0));
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    engine.set_rotary_mode(RotaryMode::Off);
    Ok(engine)
}

fn render(engine: &mut OrganEngine, skip: usize, keep: usize) -> Vec<f64> {
    let mut samples = Vec::with_capacity(keep);
    for frame in 0..skip + keep {
        let sample = f64::from(engine.next_sample()[0]);
        if frame >= skip {
            samples.push(sample);
        }
    }
    samples
}

fn tonewheel_frequency(rate: u32) -> Result<f64, String> {
    const NOTE: u8 = 69;
    const BUS: usize = 2;
    let mut engine = clean_engine(rate)?;
    assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, BUS, 8));
    assert!(engine.note_on_part(OrganPart::Upper, NOTE, 1.0));
    let samples = render(&mut engine, rate as usize / 2, rate as usize);
    zero_crossing_frequency(&samples, f64::from(rate))
        .ok_or_else(|| format!("no zero crossings at {rate} Hz"))
}

/// The same probe against its mechanical target, so that a rate which retunes
/// the generator consistently is still caught.
fn tonewheel_error_cents(rate: u32) -> Result<f64, String> {
    Ok(1_200.0 * (tonewheel_frequency(rate)? / a4_frequency()).log2())
}

fn chord_level(rate: u32) -> Result<f64, String> {
    let mut engine = clean_engine(rate)?;
    for drawbar in 0..3 {
        assert!(engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, drawbar, 8));
    }
    for note in [48, 55, 60, 64] {
        assert!(engine.note_on_part(OrganPart::Upper, note, 0.86));
    }
    let samples = render(&mut engine, rate as usize / 2, rate as usize);
    Ok(decibels(rms(&samples)))
}

fn percussion_t60(rate: u32) -> Result<f64, String> {
    let window = rate as usize / 100;
    let mut engine = clean_engine(rate)?;
    engine.set_percussion_enabled(true);
    engine.set_percussion_harmonic(PercussionHarmonic::Third);
    engine.set_percussion_volume(PercussionVolume::Normal);
    engine.set_percussion_decay(PercussionDecay::Fast);
    assert!(engine.note_on_part(OrganPart::Upper, 60, 1.0));
    let samples = render(&mut engine, 0, rate as usize * 2);
    let curve = samples
        .chunks_exact(window)
        .enumerate()
        .map(|(index, chunk)| (index as f64 * 0.01, rms(chunk)))
        .collect::<Vec<_>>();
    decay_time(&curve, 0.001).ok_or_else(|| format!("percussion did not decay at {rate} Hz"))
}

fn scanner_sideband(rate: u32) -> Result<f64, String> {
    const CARRIER: f64 = 1_000.0;
    const MODULATION: f64 = 6.9;
    let mut scanner = ScannerVibrato::new(rate as f32);
    scanner.set_mode(ScannerMode::Chorus3);
    let skip = rate as usize / 2;
    let keep = rate as usize * 2;
    let mut samples = Vec::with_capacity(keep);
    for frame in 0..skip + keep {
        let input = (TAU * CARRIER * frame as f64 / f64::from(rate)).sin() as f32 * 0.5;
        let sample = scanner.process(input);
        if frame >= skip {
            samples.push(f64::from(sample));
        }
    }
    let carrier = spectral_amplitude(&samples, CARRIER, f64::from(rate));
    let lower = spectral_amplitude(&samples, CARRIER - MODULATION, f64::from(rate));
    let upper = spectral_amplitude(&samples, CARRIER + MODULATION, f64::from(rate));
    Ok(decibels(0.5 * (lower + upper) / carrier))
}

fn rotary_rotor(rate: u32, seconds: usize) -> (f64, Option<f64>) {
    let mut rotary = Rotary::new(rate as f32);
    assert!(rotary.set_acceleration(0.5));
    rotary.set_mode(RotaryMode::Tremolo);
    let mut t63 = None;
    for frame in 0..=rate as usize * seconds {
        if frame > 0 {
            rotary.process(0.0);
        }
        let state = rotary.diagnostics();
        if t63.is_none() && state.horn_speed_hz >= state.horn_target_hz * 0.632_120_55 {
            t63 = Some(frame as f64 / f64::from(rate));
        }
    }
    (f64::from(rotary.diagnostics().horn_speed_hz), t63)
}

fn rotary_horn_rate(rate: u32) -> Result<f64, String> {
    Ok(rotary_rotor(rate, 8).0)
}

fn rotary_horn_t63(rate: u32) -> Result<f64, String> {
    rotary_rotor(rate, 8)
        .1
        .ok_or_else(|| format!("horn never reached 63% at {rate} Hz"))
}

fn electronics_gain(rate: u32, frequency: f64, expression: f32, tone: f32) -> f64 {
    const AMPLITUDE: f64 = 0.25;
    let mut electronics = ConsoleElectronics::new(rate as f32);
    assert!(electronics.set(0.0, 0.0, tone));
    assert!(electronics.set_expression_character(if tone == 0.0 { 0.55 } else { 0.0 }));
    let settle = rate as usize / 4;
    let keep = rate as usize;
    let mut samples = Vec::with_capacity(keep);
    for frame in 0..settle + keep {
        let input = (TAU * frequency * frame as f64 / f64::from(rate)).sin() * AMPLITUDE;
        let output = electronics.process(input as f32, expression);
        if frame >= settle {
            samples.push(f64::from(output));
        }
    }
    spectral_amplitude(&samples, frequency, f64::from(rate)) / AMPLITUDE
}

fn expression_gain(rate: u32) -> Result<f64, String> {
    Ok(decibels(electronics_gain(rate, 1_000.0, 0.5, 0.0)))
}

fn tone_gain(rate: u32) -> Result<f64, String> {
    let neutral = electronics_gain(rate, 10_000.0, 1.0, 0.0);
    let boosted = electronics_gain(rate, 10_000.0, 1.0, 1.0);
    Ok(decibels(boosted / neutral))
}

fn transformer_third_order(rate: u32) -> Result<f64, String> {
    const C_HZ: f64 = 523.3;
    const F_HZ: f64 = 698.5;
    const AMPLITUDE: f64 = 0.18;
    let mut transformer = MatchingTransformer::new(rate as f32);
    assert!(transformer.set(CHARACTER.0, CHARACTER.1));
    let settle = rate as usize / 2;
    let keep = rate as usize;
    let mut samples = Vec::with_capacity(keep);
    for frame in 0..settle + keep {
        let time = frame as f64 / f64::from(rate);
        let input = AMPLITUDE * ((TAU * C_HZ * time).sin() + (TAU * F_HZ * time).sin());
        let output = transformer.process(input as f32);
        if frame >= settle {
            samples.push(f64::from(output));
        }
    }
    let rate = f64::from(rate);
    let carrier =
        0.5 * (spectral_amplitude(&samples, C_HZ, rate) + spectral_amplitude(&samples, F_HZ, rate));
    let third_order = spectral_amplitude(&samples, 2.0 * C_HZ - F_HZ, rate);
    Ok(decibels(third_order / carrier))
}

fn pedal_key_off(rate: u32) -> Result<f64, String> {
    let mut engine = clean_engine(rate)?;
    assert!(engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, 0, 8));
    assert!(engine.note_on_part(OrganPart::Pedal, 24, 1.0));
    for _ in 0..rate {
        engine.next_sample();
    }
    assert!(engine.note_off_part(OrganPart::Pedal, 24, 1.0));
    let tail = render(&mut engine, 0, rate as usize / 10);
    let total = tail.iter().map(|sample| sample * sample).sum::<f64>();
    if total <= 1.0e-30 {
        return Err(format!("pedal release produced no tail at {rate} Hz"));
    }
    let mut accumulated = 0.0;
    tail.iter()
        .enumerate()
        .find_map(|(index, sample)| {
            accumulated += sample * sample;
            (accumulated >= total * 0.999).then_some(index as f64 / f64::from(rate))
        })
        .ok_or_else(|| format!("pedal release did not converge at {rate} Hz"))
}

/// Registrations the benchmark walks through, each adding one subsystem to
/// the one before it, so that the difference between two rows is what that
/// subsystem costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    /// The generator turning with nothing keyed.
    Idle,
    /// Thirteen keys held across both manuals and the pedals.
    Keys,
    /// The percussion channel on top of them.
    Percussion,
    /// Both manuals switched into the vibrato line.
    Vibrato,
    /// And the rotary cabinet: everything at once.
    Rotary,
}

impl Load {
    pub const ALL: [Self; 5] = [
        Self::Idle,
        Self::Keys,
        Self::Percussion,
        Self::Vibrato,
        Self::Rotary,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Keys => "keys",
            Self::Percussion => "percussion",
            Self::Vibrato => "vibrato",
            Self::Rotary => "rotary",
        }
    }
}

/// Cost of one second of audio at one sample rate and one load.
#[derive(Clone, Copy, Debug)]
pub struct Performance {
    pub sample_rate: u32,
    pub load: Load,
    pub seconds_rendered: f64,
    pub elapsed_seconds: f64,
    pub realtime_factor: f64,
    /// Nanoseconds per sample this load adds over the one before it.
    pub added_ns_per_sample: f64,
}

/// Renders each load at every supported rate and reports how many times
/// faster than real time the engine runs, and what each subsystem adds. The
/// numbers describe this machine and this build; they are a regression signal
/// and a guide for where optimisation is worth spending, not a specification.
pub fn benchmark() -> Result<Vec<Performance>, String> {
    std::thread::Builder::new()
        .name("rf-organ-benchmark".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(benchmark_inner)
        .map_err(|error| format!("starting benchmark worker: {error}"))?
        .join()
        .map_err(|_| "benchmark worker panicked".to_owned())?
}

fn benchmark_inner() -> Result<Vec<Performance>, String> {
    const SECONDS: usize = 3;
    let mut measurements = Vec::with_capacity(RATES.len() * Load::ALL.len());
    for rate in RATES {
        let mut previous_ns = 0.0;
        for load in Load::ALL {
            let mut engine = console(rate, load)?;
            // One warm-up block keeps the first timed sample out of the
            // engine's cold caches.
            for _ in 0..1_024 {
                engine.next_sample();
            }
            let frames = rate as usize * SECONDS;
            let start = std::time::Instant::now();
            let mut sink = 0.0_f32;
            for _ in 0..frames {
                let [left, right] = engine.next_sample();
                sink += left + right;
            }
            let elapsed = start.elapsed().as_secs_f64();
            assert!(sink.is_finite());
            if elapsed <= 0.0 {
                return Err(format!("benchmark clock did not advance at {rate} Hz"));
            }
            let ns_per_sample = elapsed * 1.0e9 / frames as f64;
            measurements.push(Performance {
                sample_rate: rate,
                load,
                seconds_rendered: SECONDS as f64,
                elapsed_seconds: elapsed,
                realtime_factor: SECONDS as f64 / elapsed,
                added_ns_per_sample: ns_per_sample - previous_ns,
            });
            previous_ns = ns_per_sample;
        }
    }
    Ok(measurements)
}

/// The console at one of the benchmark's loads. Every load above `Idle`
/// includes the ones before it, so the rows subtract cleanly.
fn console(rate: u32, load: Load) -> Result<OrganEngine, String> {
    let mut engine = OrganEngine::new(rate as f32).map_err(|error| error.0.to_owned())?;
    for (index, position) in [8, 8, 8, 8, 6, 8, 4, 8, 6].into_iter().enumerate() {
        assert!(engine.set_manual_drawbar(
            OrganPart::Upper,
            Registration::AdjustB,
            index,
            position
        ));
        assert!(engine.set_manual_drawbar(
            OrganPart::Lower,
            Registration::AdjustB,
            index,
            position
        ));
    }
    for drawbar in 0..2 {
        assert!(engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, drawbar, 8));
    }
    assert!(engine.set_transformer(0.62, 0.38));
    assert!(engine.set_console(0.48, 0.18, -0.08));
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    engine.set_rotary_mode(RotaryMode::Off);
    if load == Load::Idle {
        return Ok(engine);
    }
    for note in [48, 52, 55, 60, 64, 67] {
        assert!(engine.note_on_part(OrganPart::Upper, note, 0.9));
        assert!(engine.note_on_part(OrganPart::Lower, note, 0.9));
    }
    assert!(engine.note_on_part(OrganPart::Pedal, 24, 1.0));
    if load == Load::Keys {
        return Ok(engine);
    }
    engine.set_percussion_enabled(true);
    if load == Load::Percussion {
        return Ok(engine);
    }
    engine.set_scanner_mode(ScannerMode::Chorus3);
    engine.set_scanner_manuals(true, true);
    if load == Load::Vibrato {
        return Ok(engine);
    }
    engine.set_rotary_mode(RotaryMode::Tremolo);
    Ok(engine)
}

pub fn performance_report(measurements: &[Performance]) -> String {
    let mut csv = String::from(
        "sample_rate_hz,load,seconds_rendered,elapsed_seconds,realtime_factor,ns_per_sample,added_ns_per_sample
",
    );
    for measurement in measurements {
        let ns_per_sample = measurement.elapsed_seconds * 1.0e9
            / (f64::from(measurement.sample_rate) * measurement.seconds_rendered);
        writeln!(
            &mut csv,
            "{},{},{:.3},{:.6},{:.3},{ns_per_sample:.3},{:.3}",
            measurement.sample_rate,
            measurement.load.label(),
            measurement.seconds_rendered,
            measurement.elapsed_seconds,
            measurement.realtime_factor,
            measurement.added_ns_per_sample
        )
        .expect("string write cannot fail");
    }
    csv
}

/// The expected 8-foot frequency of the A4 probe, for an absolute check that
/// no rate silently retunes the generator.
pub fn a4_frequency() -> f64 {
    let key = usize::from(69 - MANUAL_FIRST_NOTE);
    let wheel = drawbar_wheel(key, 2).expect("8-foot contact");
    f64::from(gear_frequency(wheel).expect("bounded wheel"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cheap half of the survey: the stages that do not run the
    /// generator. It is fast enough for every test run, and it fails for the
    /// same reason the full sweep would.
    #[test]
    fn console_stages_are_rate_independent() {
        for (name, tolerance, measure) in [
            ("expression", 0.3, expression_gain as Probe),
            ("tone", 0.5, tone_gain as Probe),
            ("transformer", 0.5, transformer_third_order as Probe),
            ("scanner", 1.0, scanner_sideband as Probe),
        ] {
            let values = RATES.map(|rate| measure(rate).expect("probe"));
            let baseline = values[BASELINE];
            let deviation = values
                .iter()
                .map(|value| (value - baseline).abs())
                .fold(0.0, f64::max);
            assert!(
                deviation <= tolerance,
                "{name} moved {deviation} across {RATES:?}: {values:?}"
            );
        }
    }

    /// The complete survey renders the generator at five rates, which is too
    /// slow for an ordinary test run. `rf-organ-lab sweep` is the gate; this
    /// keeps the same assertions available through
    /// `cargo test --release -p rf-organ-lab -- --ignored`.
    #[test]
    #[ignore = "renders the full generator at five sample rates"]
    fn every_measured_quantity_survives_the_supported_sample_rates() {
        let invariants = survey().expect("survey");
        assert_eq!(invariants.len(), 11);
        let failures = invariants
            .iter()
            .filter(|invariant| !invariant.holds())
            .map(|invariant| {
                format!(
                    "{}/{} moved {:.6} {} (tolerance {:.6}): {:?}",
                    invariant.probe,
                    invariant.metric,
                    invariant.worst_deviation(),
                    invariant.unit,
                    invariant.tolerance,
                    invariant.values
                )
            })
            .collect::<Vec<_>>();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        let tonewheel = &invariants[0];
        assert!(
            (tonewheel.baseline() - a4_frequency()).abs() < 0.05,
            "A4 measured {} against {}",
            tonewheel.baseline(),
            a4_frequency()
        );
        assert!(report(&invariants).lines().count() == invariants.len() + 1);
    }
}
