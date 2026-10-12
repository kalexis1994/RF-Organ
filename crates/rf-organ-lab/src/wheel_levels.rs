// SPDX-License-Identifier: GPL-2.0-or-later
//! The wheels' levels and the manuals' key tapering, from published readings
//! of real consoles, and the check that the engine reproduces them.
//!
//! Four files in `data/`, reduced by `tools/kon-wheel-levels.py` from Kon
//! Zissis's tonewheel generator compilation:
//!
//! - `busbar-levels.csv`: each wheel straight onto a busbar, as the B-3/C-3
//!   service manual's Table V prescribes - that table and six consoles;
//! - `manual-levels.csv`: each wheel through one key and drawbar, as
//!   Hammond's second chart lays out - that chart and six consoles - with
//!   the key and drawbar each was read through;
//! - `key-tapering.csv`: every manual contact's wheel and tapering wire, in
//!   ohms, across five measured manuals;
//! - `source-impedance.csv`: the impedance each wheel drives its wire from,
//!   from three organs read open-circuit, onto a busbar and through the keys.
//!
//! A busbar sits on a matching-transformer tap of a fraction of an ohm, so a
//! wheel delivers a current: V / Z straight onto it, V / (Z + R) through a
//! tapering wire of R ohms. That is the whole model. The busbar readings fix
//! each wheel's V / Z - the shipped `TAPER` - and the wires and impedances fix
//! what each key's contact lets through - the shipped `KEY_TAPER`. The key
//! readings are then a check: the engine, played through the key and drawbar
//! each was read on, has to come out where they do.

use crate::signal::{decibels, spectral_amplitude};
use rf_organ_dsp::{
    DRAWBAR_COUNT, KEY_TAPER, MANUAL_KEY_COUNT, OrganEngine, OrganPart, PEDAL_KEY_COUNT,
    Registration, ScannerMode, TONEWHEEL_COUNT, drawbar_wheel, gear_frequency,
};
use std::fmt::Write as _;

const SAMPLE_RATE: f64 = 48_000.0;
const SETTLE: usize = 4_800;
const WINDOW: usize = 12_000;
const MANUAL_FIRST_NOTE: u8 = 36;
const PEDAL_FIRST_NOTE: u8 = 24;
/// The drawbars an 888 registration pulls: 16', 5 1/3' and 8'.
const ANCHOR_BUSES: [usize; 3] = [0, 1, 2];
/// The wheels the pedals' 16' fundamentals sound.
const PEDAL_FUNDAMENTALS: usize = 25;

/// One published reading per wheel: median, lowest and highest, in dB, and
/// the key and drawbar it was read through where it was read through one.
pub struct Readings {
    pub first_wheel: usize,
    pub levels: Vec<[f64; 3]>,
    pub paths: Vec<Option<(usize, usize)>>,
}

fn data_lines(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .skip(1)
        .map(|line| line.split(',').collect())
}

fn number<T: std::str::FromStr>(field: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    field
        .trim()
        .parse::<T>()
        .map_err(|error| format!("{field}: {error}"))
}

fn readings(text: &str) -> Result<Readings, String> {
    let mut first_wheel = None;
    let mut levels = Vec::new();
    let mut paths = Vec::new();
    for fields in data_lines(text) {
        let (wheel, median, lowest, highest, path) = match fields[..] {
            [wheel, median, lowest, highest] => (wheel, median, lowest, highest, None),
            [wheel, median, lowest, highest, key, drawbar] => (
                wheel,
                median,
                lowest,
                highest,
                Some((number::<usize>(key)? - 1, number::<usize>(drawbar)? - 1)),
            ),
            _ => return Err(format!("malformed reading: {fields:?}")),
        };
        let wheel = number::<usize>(wheel)?;
        let expected = first_wheel.get_or_insert(wheel);
        if wheel != *expected + levels.len() {
            return Err(format!("wheel {wheel} is out of order"));
        }
        levels.push([number(median)?, number(lowest)?, number(highest)?]);
        paths.push(path);
    }
    let first_wheel = first_wheel.ok_or("no readings")?;
    if first_wheel + levels.len() - 1 != TONEWHEEL_COUNT {
        return Err("the readings do not reach the top wheel".to_owned());
    }
    Ok(Readings {
        first_wheel,
        levels,
        paths,
    })
}

pub fn busbar() -> Result<Readings, String> {
    readings(include_str!("../data/busbar-levels.csv"))
}

pub fn manual() -> Result<Readings, String> {
    readings(include_str!("../data/manual-levels.csv"))
}

/// One manual contact: key and drawbar from zero, the wheel from zero, how
/// many of the five charts agree on it, and its tapering wire in ohms.
pub struct Contact {
    pub key: usize,
    pub drawbar: usize,
    pub wheel: usize,
    #[cfg_attr(not(test), allow(dead_code))]
    pub agreeing: usize,
    pub ohms: f64,
}

pub fn wiring() -> Result<Vec<Contact>, String> {
    data_lines(include_str!("../data/key-tapering.csv"))
        .map(|fields| {
            let [key, drawbar, wheel, agreeing, ohms] = fields[..] else {
                return Err(format!("malformed contact: {fields:?}"));
            };
            Ok(Contact {
                key: number::<usize>(key)? - 1,
                drawbar: number::<usize>(drawbar)? - 1,
                wheel: number::<usize>(wheel)? - 1,
                agreeing: number(agreeing)?,
                ohms: number(ohms)?,
            })
        })
        .collect()
}

/// Each wheel's source impedance in ohms: the median across the organs.
pub fn source_impedance() -> Result<[f64; TONEWHEEL_COUNT], String> {
    let mut ohms = [f64::NAN; TONEWHEEL_COUNT];
    for fields in data_lines(include_str!("../data/source-impedance.csv")) {
        let [wheel, median, _, _] = fields[..] else {
            return Err(format!("malformed impedance: {fields:?}"));
        };
        let wheel = number::<usize>(wheel)?;
        if !(1..=TONEWHEEL_COUNT).contains(&wheel) {
            return Err(format!("wheel {wheel} is outside the generator"));
        }
        ohms[wheel - 1] = number(median)?;
    }
    if ohms
        .iter()
        .any(|value| !(value.is_finite() && *value > 0.0))
    {
        return Err("the impedances do not cover every wheel".to_owned());
    }
    Ok(ohms)
}

/// What each contact lets through of its wheel, Z / (Z + R), before it is
/// scaled to the manuals' level.
pub fn key_taper() -> Result<[[f64; DRAWBAR_COUNT]; MANUAL_KEY_COUNT], String> {
    let impedance = source_impedance()?;
    let mut table = [[f64::NAN; DRAWBAR_COUNT]; MANUAL_KEY_COUNT];
    for contact in wiring()? {
        let z = impedance[contact.wheel];
        table[contact.key][contact.drawbar] = z / (z + contact.ohms);
    }
    if table.iter().flatten().any(|gain| !gain.is_finite()) {
        return Err("the tapering does not cover every contact".to_owned());
    }
    Ok(table)
}

/// An engine with nothing sounding and everything that would colour a single
/// wheel turned off. `flat` puts every wheel at unity whatever the shipped
/// taper is.
fn quiet_engine(flat: bool) -> Result<OrganEngine, String> {
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).map_err(|error| error.0.to_owned())?;
    for part in [OrganPart::Upper, OrganPart::Lower] {
        for drawbar in 0..DRAWBAR_COUNT {
            engine.set_manual_drawbar(part, Registration::AdjustB, drawbar, 0);
        }
    }
    for drawbar in 0..2 {
        engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, drawbar, 0);
    }
    if flat {
        for wheel in 0..TONEWHEEL_COUNT {
            engine.set_wheel_level(wheel, 1.0);
        }
    }
    let accepted = [
        engine.set_contact_spread(0.0),
        engine.set_contact_bounce(0.0),
        engine.set_leakage(0.0),
        engine.set_drive_wobble(0.0),
        engine.set_transformer(0.0, 0.0),
        // Both sets of readings are taken with the pedal open, and with the
        // tone control at full brightness where the owners say.
        engine.set_console(0.0, 0.0, 0.0),
        engine.set_expression(1.0),
        engine.set_expression_character(0.0),
        engine.set_output_level(1.0),
    ];
    if accepted.contains(&false) {
        return Err("the engine refused a setting".to_owned());
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

fn through_key(flat: bool, wheel: usize, key: usize, bus: usize) -> Result<f64, String> {
    let mut engine = quiet_engine(flat)?;
    engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, bus, 8);
    engine.note_on_part(OrganPart::Upper, MANUAL_FIRST_NOTE + key as u8, 1.0);
    carrier_db(&mut engine, wheel)
}

/// What reaches the engine's output from each wheel straight off a busbar,
/// in dB, with every wheel at unity: the console's own response, which the
/// busbar readings carry too. The manuals reach wheels 13 to 91, and the
/// shipped key taper of the contact used is taken back out; the bottom twelve
/// feed only the pedals, which have no key taper, and are joined at wheel 13.
pub fn busbar_response() -> Result<[f64; TONEWHEEL_COUNT], String> {
    let mut levels = [f64::NAN; TONEWHEEL_COUNT];
    for (wheel, level) in levels.iter_mut().enumerate().skip(12) {
        let (key, bus) = (0..DRAWBAR_COUNT)
            .find_map(|bus| {
                (0..MANUAL_KEY_COUNT)
                    .find(|key| drawbar_wheel(*key, bus) == Some(wheel))
                    .map(|key| (key, bus))
            })
            .ok_or_else(|| format!("no manual contact reaches wheel {}", wheel + 1))?;
        *level = through_key(true, wheel, key, bus)? - decibels(f64::from(KEY_TAPER[key][bus]));
    }
    let mut pedal = [f64::NAN; 13];
    for (key, slot) in pedal.iter_mut().enumerate().take(PEDAL_KEY_COUNT.min(13)) {
        let mut engine = quiet_engine(true)?;
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

/// The two tables the engine ships: the wheel levels that make its busbar
/// response the readings' medians, and the key taper.
///
/// Neither reading says how loud anything is, only how the wheels and the
/// contacts stand against each other, so each table is scaled to keep a level
/// the stages after it were set by. The pedals have no key taper and read the
/// wheel levels directly, so those are scaled to give the pedals' 16'
/// fundamentals the power they had with every wheel at unity; the key taper is
/// then scaled to give an 888 registration, averaged across the keyboard, the
/// power it had with every contact at unity.
pub fn fit_tapers(
    busbar: &Readings,
    response: &[f64; TONEWHEEL_COUNT],
    key_taper: &[[f64; DRAWBAR_COUNT]; MANUAL_KEY_COUNT],
) -> (
    [f64; TONEWHEEL_COUNT],
    [[f64; DRAWBAR_COUNT]; MANUAL_KEY_COUNT],
) {
    let mut decibels = [0.0; TONEWHEEL_COUNT];
    for (wheel, level) in decibels.iter_mut().enumerate() {
        *level = busbar.levels[wheel + 1 - busbar.first_wheel][0] - response[wheel];
    }
    let pedal_power = decibels[..PEDAL_FUNDAMENTALS]
        .iter()
        .map(|level| 10.0_f64.powf(level / 10.0))
        .sum::<f64>()
        / PEDAL_FUNDAMENTALS as f64;
    let offset = 10.0 * pedal_power.log10();
    let taper = decibels.map(|level| 10.0_f64.powf((level - offset) / 20.0));

    let mut power = 0.0;
    let mut contacts = 0;
    for (key, gains) in key_taper.iter().enumerate() {
        for bus in ANCHOR_BUSES {
            if let Some(wheel) = drawbar_wheel(key, bus) {
                power += (taper[wheel] * gains[bus]).powi(2);
                contacts += 1;
            }
        }
    }
    let scale = 1.0 / (power / f64::from(contacts)).sqrt();
    (taper, key_taper.map(|gains| gains.map(|gain| gain * scale)))
}

/// Both tables as `taper.rs` carries them.
pub fn rust_tables(
    taper: &[f64; TONEWHEEL_COUNT],
    key_taper: &[[f64; DRAWBAR_COUNT]; MANUAL_KEY_COUNT],
) -> String {
    let row = |values: &[f64]| {
        values
            .iter()
            .map(|value| format!("{value:.4}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut text = String::from("pub const TAPER: [f32; TONEWHEEL_COUNT] = [\n");
    for chunk in taper.chunks(7) {
        let _ = writeln!(text, "    {},", row(chunk));
    }
    text.push_str("];\n\npub const KEY_TAPER: [[f32; DRAWBAR_COUNT]; MANUAL_KEY_COUNT] = [\n");
    for gains in key_taper {
        let _ = writeln!(text, "    [{}],", row(gains));
    }
    text.push_str("];\n");
    text
}

/// How far a set of engine levels is from a set of readings, in shape only:
/// each taken against its own mean over the wheels compared.
pub struct Distance {
    pub rms_db: f64,
    pub inside: usize,
    pub wheels: usize,
    pub per_wheel: Vec<(usize, f64, [f64; 3])>,
}

pub fn distance(engine: &[(usize, f64)], readings: &Readings) -> Distance {
    let pairs = engine
        .iter()
        .map(|(wheel, level)| {
            (
                *wheel,
                *level,
                readings.levels[wheel + 1 - readings.first_wheel],
            )
        })
        .collect::<Vec<_>>();
    let count = pairs.len() as f64;
    let engine_mean = pairs.iter().map(|pair| pair.1).sum::<f64>() / count;
    let reading_mean = pairs.iter().map(|pair| pair.2[0]).sum::<f64>() / count;
    let per_wheel = pairs
        .into_iter()
        .map(|(wheel, level, reading)| {
            (
                wheel + 1,
                level - engine_mean,
                reading.map(|value| value - reading_mean),
            )
        })
        .collect::<Vec<_>>();
    let rms_db = (per_wheel
        .iter()
        .map(|(_, engine, [median, ..])| (engine - median).powi(2))
        .sum::<f64>()
        / count)
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

/// The engine as shipped, played through the key and drawbar each manual
/// reading was taken on.
pub fn engine_through_keys(manual: &Readings) -> Result<Vec<(usize, f64)>, String> {
    manual
        .paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let wheel = manual.first_wheel - 1 + index;
            let (key, bus) = path.ok_or_else(|| format!("wheel {} has no path", wheel + 1))?;
            Ok((wheel, through_key(false, wheel, key, bus)?))
        })
        .collect()
}

/// The engine as shipped, each wheel straight off a busbar: through the
/// first contact that reaches it, its key taper taken back out.
pub fn engine_at_busbar() -> Result<Vec<(usize, f64)>, String> {
    let response = busbar_response()?;
    Ok((0..TONEWHEEL_COUNT)
        .map(|wheel| {
            (
                wheel,
                response[wheel] + decibels(f64::from(rf_organ_dsp::TAPER[wheel])),
            )
        })
        .collect())
}

pub fn report(busbar: &Distance, manual: &Distance) -> String {
    let mut csv = String::from(
        "wheel,engine_busbar_db,busbar_median_db,busbar_lowest_db,busbar_highest_db,engine_keys_db,keys_median_db,keys_lowest_db,keys_highest_db\n",
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

    /// How close the engine has to come to the key readings, in dB rms. Set from
    /// the first measurement with the model in place, with a little room for the
    /// rounding of the shipped tables; see `docs/ARCHITECTURE.md`.
    const KEY_READINGS_RMS_DB: f64 = 2.5;

    /// Five measured manuals name the wheel every key's contact reaches for
    /// every drawbar, foldback included, and the engine reaches the same one.
    /// Where the charts do not agree among themselves - one contact, in one
    /// chart - the majority is held.
    #[test]
    fn every_contact_reaches_the_wheel_measured_consoles_wire_it_to() {
        let wiring = wiring().expect("wiring");
        assert_eq!(wiring.len(), MANUAL_KEY_COUNT * DRAWBAR_COUNT);
        let unanimous = wiring
            .iter()
            .filter(|contact| contact.agreeing == 5)
            .count();
        for contact in &wiring {
            assert!(contact.agreeing >= 4, "the charts disagree");
            assert_eq!(
                drawbar_wheel(contact.key, contact.drawbar),
                Some(contact.wheel),
                "key {} drawbar {}",
                contact.key + 1,
                contact.drawbar + 1
            );
            assert!(contact.ohms > 0.0);
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
        assert!(manual.paths.iter().all(Option::is_some));
        for [median, lowest, highest] in busbar.levels.iter().chain(&manual.levels) {
            assert!(lowest <= median && median <= highest);
        }
        source_impedance().expect("impedance");
    }

    /// An engine holds a contact per key per spring and does not fit on a
    /// test thread's own stack.
    fn on_engine_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(work)
            .expect("thread")
            .join()
            .expect("the work finished")
    }

    fn fitted() -> (
        [f64; TONEWHEEL_COUNT],
        [[f64; DRAWBAR_COUNT]; MANUAL_KEY_COUNT],
    ) {
        on_engine_stack(|| {
            fit_tapers(
                &busbar().expect("busbar"),
                &busbar_response().expect("response"),
                &key_taper().expect("key taper"),
            )
        })
    }

    /// The key taper the engine ships is Z / (Z + R) from the data, scaled
    /// to the manuals' level.
    #[test]
    fn the_shipped_key_taper_is_the_wires_against_the_wheels() {
        let (_, derived) = fitted();
        for (key, (derived, shipped)) in derived.iter().zip(KEY_TAPER).enumerate() {
            for (bus, (derived, shipped)) in derived.iter().zip(shipped).enumerate() {
                assert!(
                    (derived - f64::from(shipped)).abs() < 1.0e-4,
                    "key {} drawbar {}: ships {shipped}, the data give {derived:.4}: run `rf-organ-lab wheel-levels`",
                    key + 1,
                    bus + 1
                );
            }
        }
    }

    /// The wheel taper the engine ships is the fit to the busbar readings, to
    /// the rounding it is printed at.
    #[test]
    fn the_shipped_taper_is_the_busbar_fit() {
        let (fitted, _) = fitted();
        for (wheel, (fitted, shipped)) in fitted.iter().zip(rf_organ_dsp::TAPER).enumerate() {
            let difference = decibels(*fitted) - decibels(f64::from(shipped));
            assert!(
                difference.abs() < 0.01,
                "wheel {} ships {shipped} where the fit gives {fitted:.4}: run `rf-organ-lab wheel-levels`",
                wheel + 1
            );
        }
    }

    /// The check that is not a fit: played through the key and drawbar each
    /// manual reading was taken on, the engine has to land where those
    /// readings do. Nothing above was fitted to them except the impedance
    /// scale, which is shared by three organs and survives leaving any one
    /// of them out.
    #[test]
    fn the_engine_through_its_keys_matches_the_key_readings() {
        let manual = manual().expect("manual");
        let engine = on_engine_stack(|| engine_through_keys(&self::manual().expect("manual")))
            .expect("engine");
        let distance = distance(&engine, &manual);
        assert!(
            distance.rms_db < KEY_READINGS_RMS_DB,
            "{:.2} dB rms from the key readings",
            distance.rms_db
        );
    }

    /// The distance is measured from the shape, not the level.
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
            paths: vec![None; TONEWHEEL_COUNT],
        };
        let engine = readings
            .levels
            .iter()
            .enumerate()
            .map(|(wheel, level)| (wheel, level[0] + 40.0))
            .collect::<Vec<_>>();
        let distance = distance(&engine, &readings);
        assert!(distance.rms_db < 1.0e-9);
        assert_eq!(distance.inside, TONEWHEEL_COUNT);
    }
}
