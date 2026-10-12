// SPDX-License-Identifier: GPL-2.0-or-later
//! How the engine's wheels stand against each other, against two published
//! sets of readings from real consoles.
//!
//! `data/busbar-levels.csv` is each wheel connected straight to a busbar, as
//! the B-3/C-3 service manual's Table V prescribes: that table and six
//! consoles. `data/manual-levels.csv` is each wheel through one key and
//! drawbar, as Hammond's second "approximate output voltage readings" chart
//! lays out: that chart and six consoles. Both were read at the
//! preamplifier's output and are reduced by `tools/kon-wheel-levels.py`.
//!
//! Neither is fitted. The busbar readings leave out the key tapering every
//! played note goes through, which on a console compensates the generator's
//! filter step at wheel 44 and its loud bottom octave; the manual readings
//! carry the tapering of the one path each wheel was read on, which is not
//! the tapering of the others. Turning ohms into gain would join them, and
//! the readings available do not pin that conversion down. So the engine is
//! measured against both and the distance is reported, which is what a model
//! of the busbar circuit has to close.
//!
//! What is checked rather than reported is the wiring: `data/key-tapering.csv`
//! names the wheel every key's contact reaches on five measured manuals, and
//! the engine has to reach the same one.

use crate::signal::{decibels, spectral_amplitude};
use rf_organ_dsp::{
    DRAWBAR_COUNT, MANUAL_KEY_COUNT, OrganEngine, OrganPart, PEDAL_KEY_COUNT, Registration,
    ScannerMode, TONEWHEEL_COUNT, drawbar_wheel, gear_frequency,
};
use std::fmt::Write as _;

const SAMPLE_RATE: f64 = 48_000.0;
const SETTLE: usize = 4_800;
const WINDOW: usize = 12_000;
const MANUAL_FIRST_NOTE: u8 = 36;
const PEDAL_FIRST_NOTE: u8 = 24;

/// One published reading per wheel: median, lowest and highest, in dB.
pub struct Readings {
    pub first_wheel: usize,
    pub levels: Vec<[f64; 3]>,
}

fn data_lines(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .skip(1)
        .map(|line| line.split(',').collect())
}

fn readings(text: &str) -> Result<Readings, String> {
    let mut first_wheel = None;
    let mut levels = Vec::new();
    for fields in data_lines(text) {
        let [wheel, median, lowest, highest] = fields[..] else {
            return Err(format!("malformed reading: {fields:?}"));
        };
        let wheel = wheel.parse::<usize>().map_err(|error| error.to_string())?;
        let expected = first_wheel.get_or_insert(wheel);
        if wheel != *expected + levels.len() {
            return Err(format!("wheel {wheel} is out of order"));
        }
        let parse = |field: &str| field.parse::<f64>().map_err(|error| error.to_string());
        levels.push([parse(median)?, parse(lowest)?, parse(highest)?]);
    }
    let first_wheel = first_wheel.ok_or("no readings")?;
    if first_wheel + levels.len() - 1 != TONEWHEEL_COUNT {
        return Err("the readings do not reach the top wheel".to_owned());
    }
    Ok(Readings {
        first_wheel,
        levels,
    })
}

pub fn busbar() -> Result<Readings, String> {
    readings(include_str!("../data/busbar-levels.csv"))
}

pub fn manual() -> Result<Readings, String> {
    readings(include_str!("../data/manual-levels.csv"))
}

/// Every key and drawbar of a manual, the wheel its contact reaches and how
/// many of the five charts agree on it.
#[cfg(test)]
pub fn wiring() -> Result<Vec<(usize, usize, usize, usize)>, String> {
    data_lines(include_str!("../data/key-tapering.csv"))
        .map(|fields| {
            let [key, drawbar, wheel, agreeing, _ohms] = fields[..] else {
                return Err(format!("malformed contact: {fields:?}"));
            };
            let number = |field: &str| field.parse::<usize>().map_err(|error| error.to_string());
            Ok((
                number(key)?,
                number(drawbar)?,
                number(wheel)?,
                number(agreeing)?,
            ))
        })
        .collect()
}

/// An engine that passes one contact's wheel and nothing else, every wheel
/// at unity whatever the shipped taper is.
fn flat_engine() -> Result<OrganEngine, String> {
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).map_err(|error| error.0.to_owned())?;
    for part in [OrganPart::Upper, OrganPart::Lower] {
        for drawbar in 0..DRAWBAR_COUNT {
            engine.set_manual_drawbar(part, Registration::AdjustB, drawbar, 0);
        }
    }
    for drawbar in 0..2 {
        engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, drawbar, 0);
    }
    for wheel in 0..TONEWHEEL_COUNT {
        engine.set_wheel_level(wheel, 1.0);
    }
    let accepted = [
        engine.set_contact_spread(0.0),
        engine.set_contact_bounce(0.0),
        engine.set_leakage(0.0),
        engine.set_drive_wobble(0.0),
        engine.set_transformer(0.0, 0.0),
        // Both charts are read with the pedal open; the tone control at full
        // brightness is what the consoles' owners report.
        engine.set_console(0.0, 0.0, 0.0),
        engine.set_expression(1.0),
        engine.set_expression_character(0.0),
        engine.set_output_level(1.0),
    ];
    if accepted.contains(&false) {
        return Err("the flat engine refused a setting".to_owned());
    }
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    Ok(engine)
}

/// How loud one wheel comes out, in dB, read at its own frequency so that
/// nothing else sounding can be counted.
fn carrier_db(engine: &mut OrganEngine, wheel: usize) -> Result<f64, String> {
    let frequency = f64::from(gear_frequency(wheel).ok_or("no such wheel")?);
    let mut samples = Vec::with_capacity(WINDOW);
    for frame in 0..SETTLE + WINDOW {
        let sample = f64::from(engine.next_sample()[0]);
        if frame >= SETTLE {
            samples.push(sample);
        }
    }
    let carrier = spectral_amplitude(&samples, frequency, SAMPLE_RATE);
    if carrier <= 1.0e-12 {
        return Err(format!("wheel {} produced nothing", wheel + 1));
    }
    Ok(decibels(carrier))
}

/// The engine's wheel-by-wheel output in dB. It has no key tapering, so any
/// contact that reaches a wheel reads the same; the manuals reach wheels 13
/// to 91, and the bottom twelve are read through the pedals' 16' and joined
/// at wheel 13, which both reach.
pub fn engine_levels() -> Result<[f64; TONEWHEEL_COUNT], String> {
    let mut levels = [f64::NAN; TONEWHEEL_COUNT];
    for (wheel, level) in levels.iter_mut().enumerate().skip(12) {
        let (key, bus) = (0..DRAWBAR_COUNT)
            .find_map(|bus| {
                (0..MANUAL_KEY_COUNT)
                    .find(|key| drawbar_wheel(*key, bus) == Some(wheel))
                    .map(|key| (key, bus))
            })
            .ok_or_else(|| format!("no manual contact reaches wheel {}", wheel + 1))?;
        let mut engine = flat_engine()?;
        engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, bus, 8);
        engine.note_on_part(OrganPart::Upper, MANUAL_FIRST_NOTE + key as u8, 1.0);
        *level = carrier_db(&mut engine, wheel)?;
    }
    let mut pedal = [f64::NAN; 13];
    for (key, slot) in pedal.iter_mut().enumerate().take(PEDAL_KEY_COUNT.min(13)) {
        let mut engine = flat_engine()?;
        engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, 0, 8);
        engine.note_on_part(OrganPart::Pedal, PEDAL_FIRST_NOTE + key as u8, 1.0);
        *slot = carrier_db(&mut engine, key)?;
    }
    let joined = levels[12];
    for (level, pedal_level) in levels.iter_mut().zip(pedal).take(12) {
        *level = joined + pedal_level - pedal[12];
    }
    Ok(levels)
}

/// How far the engine's shape is from a set of readings: each taken against
/// its own mean over the wheels the readings cover, wheel by wheel, so that
/// only the shape is compared.
pub struct Distance {
    pub rms_db: f64,
    pub inside: usize,
    pub wheels: usize,
    pub per_wheel: Vec<(usize, f64, [f64; 3])>,
}

pub fn distance(engine: &[f64; TONEWHEEL_COUNT], readings: &Readings) -> Distance {
    let covered = readings.first_wheel - 1..TONEWHEEL_COUNT;
    let mean = covered.clone().map(|wheel| engine[wheel]).sum::<f64>() / covered.len() as f64;
    let reading_mean =
        readings.levels.iter().map(|level| level[0]).sum::<f64>() / readings.levels.len() as f64;
    let per_wheel = covered
        .zip(&readings.levels)
        .map(|(wheel, reading)| {
            (
                wheel + 1,
                engine[wheel] - mean,
                reading.map(|level| level - reading_mean),
            )
        })
        .collect::<Vec<_>>();
    let rms_db = (per_wheel
        .iter()
        .map(|(_, engine, [median, ..])| (engine - median).powi(2))
        .sum::<f64>()
        / per_wheel.len() as f64)
        .sqrt();
    let inside = per_wheel
        .iter()
        .filter(|(_, engine, [_, lowest, highest])| (lowest..=highest).contains(&engine))
        .count();
    Distance {
        rms_db,
        inside,
        wheels: per_wheel.len(),
        per_wheel,
    }
}

pub fn report(busbar: &Distance, manual: &Distance) -> String {
    let mut csv = String::from(
        "wheel,engine_busbar_db,busbar_median_db,busbar_lowest_db,busbar_highest_db,engine_manual_db,manual_median_db,manual_lowest_db,manual_highest_db\n",
    );
    for (wheel, engine, [median, lowest, highest]) in &busbar.per_wheel {
        let _ = write!(
            csv,
            "{wheel},{engine:.3},{median:.3},{lowest:.3},{highest:.3}"
        );
        match manual.per_wheel.iter().find(|row| row.0 == *wheel) {
            Some((_, engine, [median, lowest, highest])) => {
                let _ = writeln!(csv, ",{engine:.3},{median:.3},{lowest:.3},{highest:.3}");
            }
            None => csv.push_str(",,,,\n"),
        }
    }
    csv
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five measured manuals name the wheel every key's contact reaches for
    /// every drawbar, foldback included, and the engine reaches the same one.
    /// Where the charts do not agree among themselves - one contact, in one
    /// chart - the majority is held.
    #[test]
    fn every_contact_reaches_the_wheel_measured_consoles_wire_it_to() {
        let wiring = wiring().expect("wiring");
        assert_eq!(wiring.len(), MANUAL_KEY_COUNT * DRAWBAR_COUNT);
        let mut unanimous = 0;
        for (key, drawbar, wheel, agreeing) in wiring {
            assert!(
                agreeing >= 4,
                "key {key} drawbar {drawbar}: the charts disagree"
            );
            if agreeing == 5 {
                unanimous += 1;
            }
            assert_eq!(
                drawbar_wheel(key - 1, drawbar - 1).map(|wheel| wheel + 1),
                Some(wheel),
                "key {key} drawbar {drawbar}"
            );
        }
        assert!(unanimous >= MANUAL_KEY_COUNT * DRAWBAR_COUNT - 1);
    }

    #[test]
    fn the_readings_cover_the_generator() {
        let busbar = busbar().expect("busbar");
        let manual = manual().expect("manual");
        assert_eq!(
            (busbar.first_wheel, busbar.levels.len()),
            (1, TONEWHEEL_COUNT)
        );
        assert_eq!(
            (manual.first_wheel, manual.levels.len()),
            (13, TONEWHEEL_COUNT - 12)
        );
        for [median, lowest, highest] in busbar.levels.iter().chain(&manual.levels) {
            assert!(lowest <= median && median <= highest);
        }
    }

    /// The distance is measured from the shape, not the level: an engine
    /// that is the readings' median moved up by any amount is at distance
    /// zero.
    #[test]
    fn distance_ignores_level() {
        let readings = Readings {
            first_wheel: 1,
            levels: (0..TONEWHEEL_COUNT)
                .map(|wheel| {
                    let level = (wheel as f64 * 0.37).sin() * 6.0;
                    [level, level - 1.0, level + 1.0]
                })
                .collect(),
        };
        let mean =
            readings.levels.iter().map(|level| level[0]).sum::<f64>() / TONEWHEEL_COUNT as f64;
        let mut engine = [0.0; TONEWHEEL_COUNT];
        for (slot, level) in engine.iter_mut().zip(&readings.levels) {
            *slot = level[0] - mean + 40.0;
        }
        let distance = distance(&engine, &readings);
        assert!(distance.rms_db < 1.0e-9);
        assert_eq!(distance.inside, TONEWHEEL_COUNT);
    }
}
