// SPDX-License-Identifier: GPL-2.0-or-later

//! Sample-rate-agnostic measurement helpers shared by the laboratory probes.

use std::f64::consts::TAU;

pub fn decibels(value: f64) -> f64 {
    20.0 * value.max(1.0e-15).log10()
}

pub fn rms(samples: &[f64]) -> f64 {
    (samples.iter().map(|sample| sample * sample).sum::<f64>() / samples.len() as f64).sqrt()
}

pub fn peak(samples: &[f64]) -> f64 {
    samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f64::max)
}

/// Hann-windowed single-bin projection, returned as a peak amplitude.
pub fn spectral_amplitude(samples: &[f64], frequency: f64, sample_rate: f64) -> f64 {
    let count = samples.len();
    let mut real = 0.0;
    let mut imaginary = 0.0;
    let mut weight_sum = 0.0;
    for (index, sample) in samples.iter().copied().enumerate() {
        let weight = 0.5 - 0.5 * (TAU * index as f64 / (count - 1) as f64).cos();
        let phase = TAU * frequency * index as f64 / sample_rate;
        real += sample * weight * phase.cos();
        imaginary -= sample * weight * phase.sin();
        weight_sum += weight;
    }
    2.0 * real.hypot(imaginary) / weight_sum
}

pub fn zero_crossing_frequency(samples: &[f64], sample_rate: f64) -> Option<f64> {
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

/// Time from the peak of an RMS curve down to `ratio` of that peak.
pub fn decay_time(curve: &[(f64, f64)], ratio: f64) -> Option<f64> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_projection_recovers_the_amplitude_of_a_tone_at_any_rate() {
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            let samples = (0..rate as usize)
                .map(|index| 0.37 * (TAU * 997.0 * index as f64 / rate).sin())
                .collect::<Vec<_>>();
            let measured = spectral_amplitude(&samples, 997.0, rate);
            assert!((measured - 0.37).abs() < 1.0e-6, "{rate} Hz gave {measured}");
            let frequency = zero_crossing_frequency(&samples, rate).expect("crossings");
            assert!((frequency - 997.0).abs() < 0.01, "{rate} Hz gave {frequency}");
            assert!((peak(&samples) - 0.37).abs() < 1.0e-4);
            assert!((rms(&samples) - 0.37 / 2.0_f64.sqrt()).abs() < 1.0e-4);
        }
    }

    #[test]
    fn decay_time_measures_from_the_peak() {
        let curve = (0..100)
            .map(|index| (index as f64 * 0.01, (-(index as f64) * 0.1).exp()))
            .collect::<Vec<_>>();
        let measured = decay_time(&curve, 0.5).expect("decay");
        assert!((measured - 0.07).abs() < 0.011, "{measured}");
        assert_eq!(decibels(1.0), 0.0);
    }
}
