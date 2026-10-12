// SPDX-License-Identifier: GPL-2.0-or-later
//! The rotary cabinet's amplifier, measured on its own.
//!
//! Three things are held here. The drive control's anchor: at its middle, an
//! 888 chord at full expression - the calibration's direct phrase - just
//! reaches the 6550 grids' conduction, which is what `GRID_VOLTS_PER_UNIT` is
//! set to and a test keeps it to. The character of the clipping: a balanced
//! driver into a push-pull pair clips symmetrically, so the third harmonic
//! leads the second. And the blocking: after the grids have conducted, the
//! coupling capacitors take the time 0.1 uF and 220 k give them to let go.

use crate::captures::SAMPLE_RATE;
use crate::signal::{decibels, peak, spectral_amplitude};
use crate::{Scenario, render_phrase};
use rf_organ_dsp::LeslieAmp;
use rf_organ_dsp::provenance::find;
use std::f64::consts::TAU;
use std::fmt::Write as _;

/// The output level the calibration phrases are rendered at; the amplifier
/// sits before it.
const PHRASE_OUTPUT_LEVEL: f64 = 0.72;

fn constant(name: &str) -> Result<f64, String> {
    find(name)
        .map(|constant| constant.value)
        .ok_or_else(|| format!("{name} is not registered"))
}

/// The direct phrase's peak where it enters the cabinet, and the grid volts
/// per unit that put that peak at the 6550 grids' conduction.
pub fn anchor() -> Result<(f64, f64), String> {
    let stereo = render_phrase(Scenario::Direct).map_err(|error| error.to_string())?;
    let left = stereo
        .iter()
        .step_by(2)
        .map(|sample| f64::from(*sample))
        .collect::<Vec<_>>();
    let entering = peak(&left) / PHRASE_OUTPUT_LEVEL;
    if entering <= 0.0 {
        return Err("the direct phrase is silent".to_owned());
    }
    Ok((entering, constant("leslie_amp::OUTPUT_BIAS_V")? / entering))
}

fn tone(amp: &mut LeslieAmp, frequency: f64, level: f64, frames: usize) -> Vec<f64> {
    let rate = f64::from(SAMPLE_RATE);
    (0..frames)
        .map(|frame| {
            let input = level * (TAU * frequency * frame as f64 / rate).sin();
            f64::from(amp.process(input as f32))
        })
        .collect()
}

/// The level, in the engine's units, at which a tone's peak reaches the
/// grids' conduction at the given drive.
fn edge(drive: f64) -> Result<f64, String> {
    let anchor = constant("leslie_amp::GRID_VOLTS_PER_UNIT")?;
    let span = constant("leslie_amp::DRIVE_SPAN_DB")?;
    let bias = constant("leslie_amp::OUTPUT_BIAS_V")?;
    Ok(bias / (anchor * 10.0_f64.powf((drive - 0.5) * span / 20.0)))
}

/// Gain at the tone's peak, and the second and third harmonics against the
/// fundamental, for a 1 kHz tone at `times` the edge.
pub fn clipping(drive: f64, times: f64) -> Result<(f64, f64, f64), String> {
    let mut amp = LeslieAmp::new(SAMPLE_RATE as f32);
    if !amp.set_drive(drive as f32) {
        return Err(format!("drive {drive} refused"));
    }
    let level = times * edge(drive)?;
    let frames = SAMPLE_RATE as usize / 2;
    let out = tone(&mut amp, 1_000.0, level, frames);
    let steady = &out[frames / 2..];
    let rate = f64::from(SAMPLE_RATE);
    let fundamental = spectral_amplitude(steady, 1_000.0, rate);
    Ok((
        decibels(peak(steady) / level),
        decibels(spectral_amplitude(steady, 2_000.0, rate) / fundamental),
        decibels(spectral_amplitude(steady, 3_000.0, rate) / fundamental),
    ))
}

/// How long the amplifier takes to give back the gain a burst of clipping
/// took: a loud tone, then a quiet one, and the time for the quiet one's
/// shortfall to fall to a third of what it was when the burst ended.
pub fn recovery_seconds() -> Result<f64, String> {
    let rate = f64::from(SAMPLE_RATE);
    let quiet = 0.05 * edge(0.5)?;
    let fresh = {
        let mut amp = LeslieAmp::new(SAMPLE_RATE as f32);
        let out = tone(&mut amp, 1_000.0, quiet, SAMPLE_RATE as usize / 2);
        peak(&out[out.len() / 2..])
    };
    let mut amp = LeslieAmp::new(SAMPLE_RATE as f32);
    tone(
        &mut amp,
        1_000.0,
        4.0 * edge(0.5)?,
        SAMPLE_RATE as usize / 5,
    );
    let after = tone(&mut amp, 1_000.0, quiet, SAMPLE_RATE as usize / 2);
    let window = SAMPLE_RATE as usize / 500;
    let shortfall = after
        .chunks(window)
        .map(|chunk| decibels(fresh / peak(chunk).max(1.0e-12)))
        .collect::<Vec<_>>();
    let first = shortfall[0];
    if first < 1.0 {
        return Err(format!("a burst took only {first:.2} dB"));
    }
    let index = shortfall
        .iter()
        .position(|value| *value <= first / 3.0)
        .ok_or("the gain never came back")?;
    Ok(index as f64 * window as f64 / rate)
}

pub fn report() -> Result<String, String> {
    let (entering, anchor) = anchor()?;
    let mut csv = String::new();
    let _ = writeln!(
        csv,
        "# direct phrase peak entering the cabinet {entering:.5}; grid volts per unit for the anchor {anchor:.4}; recovery {:.1} ms",
        recovery_seconds()? * 1_000.0
    );
    csv.push_str("drive,chord_peak_gain_db,tone_at_twice_edge_gain_db,second_harmonic_dbc,third_harmonic_dbc\n");
    for step in 0..=10 {
        let drive = f64::from(step) / 10.0;
        let (chord_gain, _, _) = clipping(drive, edge(0.5)? / edge(drive)?)?;
        let (gain, second, third) = clipping(drive, 2.0)?;
        let _ = writeln!(
            csv,
            "{drive:.1},{chord_gain:.2},{gain:.2},{second:.1},{third:.1}"
        );
    }
    Ok(csv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_engine_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(work)
            .expect("thread")
            .join()
            .expect("the work finished")
    }

    /// The shipped anchor is where the direct phrase reaches the grids'
    /// conduction, so that the middle of the drive control means what the
    /// documentation says it means.
    #[test]
    fn the_drive_anchor_is_the_direct_phrase_at_the_edge() {
        let (_, anchor) = on_engine_stack(anchor).expect("anchor");
        let shipped = constant("leslie_amp::GRID_VOLTS_PER_UNIT").expect("shipped");
        let difference = decibels(anchor / shipped);
        assert!(
            difference.abs() < 0.1,
            "the shipped anchor is {difference:.2} dB off: set GRID_VOLTS_PER_UNIT to {anchor:.4}"
        );
    }

    /// A balanced driver into a push-pull pair clips symmetrically.
    #[test]
    fn hard_clipping_is_odd() {
        let (gain, second, third) = clipping(0.5, 3.0).expect("clipping");
        assert!(gain < -3.0, "three times the edge lost only {gain:.2} dB");
        assert!(
            third > second + 20.0,
            "second {second:.1} dBc, third {third:.1} dBc"
        );
        assert!(third > -40.0, "third {third:.1} dBc");
    }

    /// Below the edge the amplifier is nearly linear.
    #[test]
    fn below_the_edge_it_is_nearly_clean() {
        let (gain, _, third) = clipping(0.5, 0.3).expect("clipping");
        assert!(gain.abs() < 0.3, "{gain:.2} dB");
        assert!(third < -40.0, "third {third:.1} dBc");
    }

    /// After clipping, the gain comes back over the time the coupling
    /// capacitor and the grid leak set: 0.1 uF into a little over 220 k is
    /// about 23 ms, and a third of the way there takes a little longer.
    #[test]
    fn the_blocking_lets_go_over_the_coupling_time() {
        let seconds = recovery_seconds().expect("recovery");
        assert!((0.01..0.08).contains(&seconds), "{seconds:.3} s");
    }
}
