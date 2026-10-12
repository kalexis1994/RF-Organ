// SPDX-License-Identifier: GPL-2.0-or-later
//! Where every number in the model comes from.
//!
//! Physical plausibility and empirical calibration are different milestones,
//! and a constant that does not say which one it has reached is a constant
//! nobody can trust or replace. Every scalar the engine is built from is listed
//! here with what it rests on and the span it could plausibly take, and a test
//! refuses a new one that is not. The values are read from the constants
//! themselves, so this list cannot disagree with the engine about what they are.
//!
//! `docs/PROVENANCE.md` is generated from this table by the laboratory and is
//! held to it by a test there.

use crate::{
    electronics, leslie_amp, manual, pedal, percussion, rotary, tonewheel, transformer,
    vibrato_line,
};

/// What a constant rests on, from firmest to softest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Basis {
    /// Stated in a manufacturer's document: a service manual, a schematic, an
    /// owner's manual or a patent.
    Documented,
    /// Measured on real instruments by someone else, and published.
    Measured,
    /// Follows by physics or arithmetic from documented or measured figures.
    Derived,
    /// Taken from another model rather than from an instrument.
    Borrowed,
    /// Chosen here, pending a measurement.
    Provisional,
}

impl Basis {
    pub const ALL: [Self; 5] = [
        Self::Documented,
        Self::Measured,
        Self::Derived,
        Self::Borrowed,
        Self::Provisional,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Documented => "documented",
            Self::Measured => "measured",
            Self::Derived => "derived",
            Self::Borrowed => "borrowed",
            Self::Provisional => "provisional",
        }
    }
}

/// The span a constant could plausibly take, and what says so.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Span {
    /// The value is the documented or derived figure itself.
    Exact,
    /// A span a document or a measurement gives.
    Sourced(f64, f64),
    /// A span chosen here because nothing published bounds it. It says where
    /// a sensitivity survey should look, not where the truth is.
    Assumed(f64, f64),
}

impl Span {
    pub const fn bounds(self) -> Option<(f64, f64)> {
        match self {
            Self::Exact => None,
            Self::Sourced(low, high) | Self::Assumed(low, high) => Some((low, high)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constant {
    /// Module and identifier, as the source spells them.
    pub name: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub basis: Basis,
    pub span: Span,
    /// What the value and its span rest on, in a sentence. The full account,
    /// with quotations, is beside the constant and in THIRD_PARTY_NOTICES.md.
    pub note: &'static str,
}

/// A table of figures, registered as a whole rather than value by value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Table {
    pub name: &'static str,
    pub basis: Basis,
    pub note: &'static str,
}

/// Scalars in the engine that are not part of the model: limits of the host
/// contract, numerical guards and floors of a display. Each says why.
pub const NOT_MODEL: [(&str, &str); 7] = [
    (
        "lib::SAMPLE_RATE_MIN",
        "the host contract, measured as rate invariant",
    ),
    (
        "lib::SAMPLE_RATE_MAX",
        "the host contract, measured as rate invariant",
    ),
    (
        "rotary::RESTING_HZ",
        "below this a rotor is treated as stopped; a numerical floor",
    ),
    (
        "rotary::NEAREST_SQUARED_M2",
        "keeps a path length away from zero; never reached",
    ),
    (
        "rotary::LEVEL_SILENT_DB",
        "the floor a volume reads as silence at; a display convention",
    ),
    (
        "leslie_amp::KNEE_V",
        "rounds the corner where a grid starts to conduct; a numerical smoothing",
    ),
    (
        "transformer::LEAN_CEILING",
        "keeps a leaning core's weak half bending at all; a guard on the reduction",
    ),
];

const fn c(
    name: &'static str,
    value: f64,
    unit: &'static str,
    basis: Basis,
    span: Span,
    note: &'static str,
) -> Constant {
    Constant {
        name,
        value,
        unit,
        basis,
        span,
        note,
    }
}

use Basis::{Borrowed, Derived, Documented, Measured, Provisional};
use Span::{Assumed, Exact, Sourced};

const fn f(value: f32) -> f64 {
    value as f64
}

pub const CONSTANTS: &[Constant] = &[
    // The generator.
    c(
        "tonewheel::SHAFT_RPM_SIXTY",
        f(tonewheel::SHAFT_RPM_SIXTY),
        "rpm",
        Documented,
        Exact,
        "Service manual: the synchronous run motor turns at 1200 rpm on 60 Hz.",
    ),
    c(
        "tonewheel::SHAFT_RPM_FIFTY",
        f(tonewheel::SHAFT_RPM_FIFTY),
        "rpm",
        Documented,
        Exact,
        "Service manual: the 4-pole armature turns at 1500 rpm on 50 Hz.",
    ),
    c(
        "tonewheel::ECCENTRICITY_DEPTH",
        f(tonewheel::ECCENTRICITY_DEPTH),
        "fraction",
        Provisional,
        Assumed(0.0, 0.05),
        "Hammond documents a once-per-revolution level change; its depth varies from organ to organ and is not published.",
    ),
    c(
        "tonewheel::LOW_OCTAVE_THIRD",
        f(tonewheel::LOW_OCTAVE_THIRD),
        "fraction",
        Provisional,
        Assumed(0.0, 0.35),
        "Low wheels are visibly not sinusoidal (Muenster and Pfeifle, ISMA 2019); the profile is not fitted to any wheel.",
    ),
    c(
        "tonewheel::SECOND_OCTAVE_THIRD",
        f(tonewheel::SECOND_OCTAVE_THIRD),
        "fraction",
        Provisional,
        Assumed(0.0, 0.15),
        "As the bottom octave's, smaller.",
    ),
    c(
        "tonewheel::LOW_OCTAVE_FIFTH",
        f(tonewheel::LOW_OCTAVE_FIFTH),
        "fraction",
        Provisional,
        Assumed(0.0, 0.15),
        "As the bottom octave's third.",
    ),
    c(
        "tonewheel::WOW_RATE_HZ",
        f(tonewheel::WOW_RATE_HZ),
        "Hz",
        Provisional,
        Assumed(0.5, 6.0),
        "The service manual describes a resiliently coupled drive; how fast it swings is not published.",
    ),
    c(
        "tonewheel::WOW_DEPTH",
        f(tonewheel::WOW_DEPTH),
        "fraction",
        Provisional,
        Assumed(0.0, 2.0e-3),
        "As the wow rate.",
    ),
    c(
        "tonewheel::FLUTTER_DEPTH",
        f(tonewheel::FLUTTER_DEPTH),
        "fraction",
        Provisional,
        Assumed(0.0, 2.0e-3),
        "As the wow rate.",
    ),
    c(
        "tonewheel::COUPLING_HZ",
        f(tonewheel::COUPLING_HZ),
        "Hz",
        Provisional,
        Assumed(10.0, 80.0),
        "The service manual describes coil-spring couplings; their resonance is not published.",
    ),
    c(
        "tonewheel::COUPLING_DAMPING",
        f(tonewheel::COUPLING_DAMPING),
        "ratio",
        Provisional,
        Assumed(0.2, 1.0),
        "As the coupling's corner.",
    ),
    c(
        "manual::LEAKAGE_CEILING",
        f(manual::LEAKAGE_CEILING),
        "gain",
        Provisional,
        Assumed(0.01, 0.3),
        "Hammond describes leakage and compartments, not its level.",
    ),
    // Keying.
    c(
        "manual::CONTACT_DELAY_CEILING_S",
        f(manual::CONTACT_DELAY_CEILING_S),
        "s",
        Documented,
        Exact,
        "Hammond's contact page: a contact may be delayed by up to 725.6 ms.",
    ),
    c(
        "manual::PRESS_SPREAD_BRISK_S",
        f(manual::PRESS_SPREAD_BRISK_S),
        "s",
        Provisional,
        Assumed(1.0e-4, 2.0e-3),
        "The keyboard-action study that measured contact spread (Moro et al., JASA 2017) is behind a paywall.",
    ),
    c(
        "manual::PRESS_SPREAD_GENTLE_S",
        f(manual::PRESS_SPREAD_GENTLE_S),
        "s",
        Provisional,
        Assumed(2.0e-3, 2.0e-2),
        "As the brisk spread.",
    ),
    c(
        "manual::BOUNCE_GENTLE_S",
        f(manual::BOUNCE_GENTLE_S),
        "s",
        Provisional,
        Assumed(5.0e-4, 5.0e-3),
        "As the brisk spread.",
    ),
    c(
        "manual::BOUNCE_HARD_S",
        f(manual::BOUNCE_HARD_S),
        "s",
        Provisional,
        Assumed(1.0e-3, 1.0e-2),
        "As the brisk spread.",
    ),
    c(
        "manual::BOUNCE_REDRAW_HZ",
        f(manual::BOUNCE_REDRAW_HZ),
        "Hz",
        Borrowed,
        Assumed(1_300.0, 5_200.0),
        "setBfree's 1302 Hz bounce, written as a half period.",
    ),
    c(
        "manual::BOUNCE_RESTITUTION",
        f(manual::BOUNCE_RESTITUTION),
        "ratio",
        Borrowed,
        Assumed(0.2, 0.8),
        "setBfree's dynamic-envelopes branch; that a bounce loses energy is established, by how much is not.",
    ),
    c(
        "manual::RELEASE_SPREAD_S",
        f(manual::RELEASE_SPREAD_S),
        "s",
        Borrowed,
        Sourced(3.6e-4, 1.81e-3),
        "The top of setBfree's click-burst range, which it gives a release and an attack alike.",
    ),
    c(
        "manual::RELEASE_CLICK_SHARE",
        f(manual::RELEASE_CLICK_SHARE),
        "ratio",
        Borrowed,
        Assumed(0.25, 1.0),
        "setBfree's release click at half the attack's, transplanted from a level to a rate.",
    ),
    c(
        "manual::KEY_CLICK_SOFT_S",
        f(manual::KEY_CLICK_SOFT_S),
        "s",
        Provisional,
        Assumed(2.0e-3, 2.0e-2),
        "The XK-7 manual documents both ends of the click control, not how long the gentle one is.",
    ),
    c(
        "manual::LEAKAGE_PER_KEY",
        f(manual::LEAKAGE_PER_KEY),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "Hammond documents that leakage grows with the keys held, not by how much.",
    ),
    c(
        "manual::LEAKAGE_BOOST_DEFAULT",
        f(manual::LEAKAGE_BOOST_DEFAULT),
        "control",
        Provisional,
        Assumed(0.0, 1.0),
        "Where that growth control starts.",
    ),
    c(
        "pedal::RELEASE_VELOCITY_FLOOR",
        f(pedal::RELEASE_VELOCITY_FLOOR),
        "velocity",
        Provisional,
        Assumed(0.0, 1.0),
        "A floor for controllers that send no release velocity; the manuals no longer use one.",
    ),
    c(
        "pedal::L20_CORNER_HZ",
        f(pedal::L20_CORNER_HZ),
        "Hz",
        Provisional,
        Assumed(200.0, 2_000.0),
        "L20 is in the pedal circuit; its value is not legible in the available documents.",
    ),
    c(
        "pedal::PEDAL_BUS_GAIN",
        f(pedal::PEDAL_BUS_GAIN),
        "gain",
        Provisional,
        Assumed(0.01, 0.06),
        "Sets the pedals' level against the manuals'; neither is measured.",
    ),
    c(
        "manual::MANUAL_BUS_GAIN",
        f(manual::MANUAL_BUS_GAIN),
        "gain",
        Provisional,
        Assumed(0.02, 0.1),
        "Sets the manuals' level into the matching transformers; not measured.",
    ),
    // Percussion.
    c(
        "percussion::NORMAL_DRAWBAR_ATTENUATION",
        f(percussion::NORMAL_DRAWBAR_ATTENUATION),
        "gain",
        Documented,
        Exact,
        "Hammond: the vintage console takes about 6 dB out of the upper drawbars at Normal volume.",
    ),
    c(
        "percussion::ATTACK_SECONDS",
        f(percussion::ATTACK_SECONDS),
        "s",
        Provisional,
        Assumed(2.0e-4, 5.0e-3),
        "The percussion capacitor charges through the key contact; no figure is published.",
    ),
    c(
        "percussion::RECOVERY_SECONDS",
        f(percussion::RECOVERY_SECONDS),
        "s",
        Provisional,
        Assumed(0.03, 0.5),
        "Hammond describes the recovery, not its time.",
    ),
    c(
        "percussion::SOFT_LEVEL",
        f(percussion::SOFT_LEVEL),
        "gain",
        Provisional,
        Assumed(0.1, 0.6),
        "Hammond names the settings, not the percussion's own level.",
    ),
    c(
        "percussion::NORMAL_LEVEL",
        f(percussion::NORMAL_LEVEL),
        "gain",
        Provisional,
        Assumed(0.4, 1.0),
        "As the Soft level.",
    ),
    c(
        "percussion::SLOW_T60_S",
        f(percussion::SLOW_T60_S),
        "s",
        Provisional,
        Assumed(0.8, 2.5),
        "Hammond publishes no decay in seconds.",
    ),
    c(
        "percussion::FAST_T60_S",
        f(percussion::FAST_T60_S),
        "s",
        Provisional,
        Assumed(0.15, 0.6),
        "As the slow decay.",
    ),
    // Matching and output transformers.
    c(
        "transformer::ASYMMETRY_DEFAULT",
        f(transformer::ASYMMETRY_DEFAULT),
        "control",
        Provisional,
        Assumed(0.0, 0.5),
        "Neither core material nor history is published; puts the second-order product about 11 dB under the third.",
    ),
    c(
        "transformer::MEMORY_HZ",
        f(transformer::MEMORY_HZ),
        "Hz",
        Provisional,
        Assumed(100.0, 3_000.0),
        "A reduced core; the laboratory fits drive and memory from intermodulation captures.",
    ),
    c(
        "transformer::DRIVE_GAIN",
        f(transformer::DRIVE_GAIN),
        "gain",
        Provisional,
        Assumed(1.0, 10.0),
        "As the memory corner.",
    ),
    c(
        "transformer::MEMORY_BIAS",
        f(transformer::MEMORY_BIAS),
        "gain",
        Provisional,
        Assumed(0.0, 2.0),
        "As the memory corner.",
    ),
    c(
        "transformer::BEND_FLOOR",
        f(transformer::BEND_FLOOR),
        "ratio",
        Provisional,
        Assumed(0.3, 0.9),
        "As the memory corner.",
    ),
    c(
        "transformer::BEND_SPAN",
        f(transformer::BEND_SPAN),
        "ratio",
        Provisional,
        Assumed(0.0, 0.6),
        "As the memory corner.",
    ),
    // The vibrato line.
    c(
        "vibrato_line::INDUCTANCE_H",
        vibrato_line::INDUCTANCE_H,
        "H",
        Documented,
        Exact,
        "Component value of the line's series inductors.",
    ),
    c(
        "vibrato_line::TERMINATION_OHM",
        vibrato_line::TERMINATION_OHM,
        "ohm",
        Documented,
        Exact,
        "Component value of the line's termination.",
    ),
    c(
        "vibrato_line::SOURCE_OHM",
        vibrato_line::SOURCE_OHM,
        "ohm",
        Documented,
        Exact,
        "Rc, in circuit for chorus and shorted for vibrato.",
    ),
    c(
        "vibrato_line::CUTOFF_HZ",
        vibrato_line::CUTOFF_HZ,
        "Hz",
        Derived,
        Exact,
        "One over pi root LC of a uniform section.",
    ),
    c(
        "vibrato_line::IMPEDANCE_OHM",
        vibrato_line::IMPEDANCE_OHM,
        "ohm",
        Derived,
        Exact,
        "Root L over C of a uniform section; a change of variable.",
    ),
    c(
        "vibrato_line::ROTOR_HZ",
        f(vibrato_line::ROTOR_HZ),
        "Hz",
        Provisional,
        Assumed(6.0, 7.2),
        "Driven from the synchronous motor; DAFx-16 quotes about 6 Hz, and 6.9 is not traced to a document here.",
    ),
    // The console preamplifier.
    c(
        "electronics::EXPRESSION_SECTION_CAPACITANCE_PF",
        f(electronics::EXPRESSION_SECTION_CAPACITANCE_PF),
        "pF",
        Documented,
        Exact,
        "AO-28: 60 pF per expression-control section.",
    ),
    c(
        "electronics::EXPRESSION_GRID_RESISTANCE_OHM",
        f(electronics::EXPRESSION_GRID_RESISTANCE_OHM),
        "ohm",
        Documented,
        Exact,
        "AO-28: R34, 15 megohm.",
    ),
    c(
        "electronics::EXPRESSION_LOW_HZ",
        f(electronics::EXPRESSION_LOW_HZ),
        "Hz",
        Derived,
        Exact,
        "The corner R34 makes with one section; its role as the low band's corner is this model's reduction.",
    ),
    c(
        "electronics::EXPRESSION_HIGH_HZ",
        f(electronics::EXPRESSION_HIGH_HZ),
        "Hz",
        Provisional,
        Assumed(2_000.0, 8_000.0),
        "The C17/C22/C24 high shoulder is a calibration target.",
    ),
    c(
        "electronics::EXPRESSION_LOW_SHARE",
        f(electronics::EXPRESSION_LOW_SHARE),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "The transfer between moving and fixed plates is a calibration target.",
    ),
    c(
        "electronics::EXPRESSION_HIGH_SHARE",
        f(electronics::EXPRESSION_HIGH_SHARE),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "As the low band's share.",
    ),
    c(
        "electronics::EXPRESSION_SLEW_S",
        f(electronics::EXPRESSION_SLEW_S),
        "s",
        Provisional,
        Assumed(2.0e-3, 5.0e-2),
        "Stands for the pedal's mechanical travel.",
    ),
    c(
        "electronics::TONE_CONTROL_HZ",
        f(electronics::TONE_CONTROL_HZ),
        "Hz",
        Documented,
        Exact,
        "XK-7 owner's manual: a gently sloped tone control at 200 Hz.",
    ),
    c(
        "electronics::BASS_SHELF_HZ",
        f(electronics::BASS_SHELF_HZ),
        "Hz",
        Provisional,
        Assumed(100.0, 600.0),
        "A modern calibration trim, not a console part.",
    ),
    c(
        "electronics::V4A_ASYMMETRY",
        f(electronics::V4A_ASYMMETRY),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "AO-28 component values are not legible in the available scan; the comparator fits the shared character from a bench injection.",
    ),
    c(
        "electronics::V4B_ASYMMETRY",
        f(electronics::V4B_ASYMMETRY),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "As V4A's.",
    ),
    c(
        "electronics::V3B_ASYMMETRY",
        f(electronics::V3B_ASYMMETRY),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "As V4A's.",
    ),
    c(
        "electronics::V4A_DRIVE_SHARE",
        f(electronics::V4A_DRIVE_SHARE),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "From outside the preamplifier only the chain is visible, not how the stages divide it.",
    ),
    c(
        "electronics::V4B_DRIVE_SHARE",
        f(electronics::V4B_DRIVE_SHARE),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "As V4A's.",
    ),
    c(
        "electronics::V3B_DRIVE_SHARE",
        f(electronics::V3B_DRIVE_SHARE),
        "ratio",
        Provisional,
        Assumed(0.0, 1.0),
        "As V4A's.",
    ),
    c(
        "electronics::STAGE_DRIVE_GAIN",
        f(electronics::STAGE_DRIVE_GAIN),
        "gain",
        Provisional,
        Assumed(2.0, 15.0),
        "How hard the drive control pushes a stage into its curve; the stage values are not legible.",
    ),
    c(
        "electronics::STAGE_MAKEUP",
        f(electronics::STAGE_MAKEUP),
        "gain",
        Provisional,
        Assumed(1.0, 6.0),
        "Keeps a driven stage near its clean level.",
    ),
    c(
        "electronics::DC_BLOCK_HZ",
        f(electronics::DC_BLOCK_HZ),
        "Hz",
        Provisional,
        Assumed(5.0, 40.0),
        "The coupling capacitors after V3B; their values are not legible in the available scan.",
    ),
    c(
        "electronics::STAGE_CHARACTER_DEFAULT",
        f(electronics::STAGE_CHARACTER_DEFAULT),
        "control",
        Provisional,
        Assumed(0.0, 3.0),
        "Where the shared stage character starts; one means the asymmetries as written.",
    ),
    // The rotary cabinet.
    c(
        "rotary::HORN_SLOW_RPM",
        f(rotary::HORN_SLOW_RPM),
        "rpm",
        Measured,
        Sourced(46.0, 50.0),
        "A 147 at the middle pulley measures 48 and a 122XB 46; every source puts it between 46 and 50.",
    ),
    c(
        "rotary::HORN_FAST_RPM",
        f(rotary::HORN_FAST_RPM),
        "rpm",
        Measured,
        Sourced(400.0, 405.0),
        "A 147 measures 400 and a 122XB 405.",
    ),
    c(
        "rotary::DRUM_SLOW_RPM",
        f(rotary::DRUM_SLOW_RPM),
        "rpm",
        Measured,
        Sourced(38.0, 40.0),
        "A 147 measures about 40 and a 122XB 38.",
    ),
    c(
        "rotary::DRUM_FAST_RPM",
        f(rotary::DRUM_FAST_RPM),
        "rpm",
        Measured,
        Sourced(340.0, 395.0),
        "A 147 measures about 342 and a 122XB 395.",
    ),
    c(
        "rotary::HORN_RISE_SECONDS",
        f(rotary::HORN_RISE_SECONDS),
        "s",
        Provisional,
        Sourced(0.8, 12.5),
        "Inside Hammond's documented floor and ceiling for the horn.",
    ),
    c(
        "rotary::HORN_FALL_SECONDS",
        f(rotary::HORN_FALL_SECONDS),
        "s",
        Provisional,
        Sourced(0.8, 12.5),
        "As the horn's rise.",
    ),
    c(
        "rotary::HORN_BRAKE_SECONDS",
        f(rotary::HORN_BRAKE_SECONDS),
        "s",
        Provisional,
        Sourced(0.8, 12.5),
        "As the horn's rise.",
    ),
    c(
        "rotary::DRUM_RISE_SECONDS",
        f(rotary::DRUM_RISE_SECONDS),
        "s",
        Provisional,
        Sourced(1.0, 12.5),
        "Inside Hammond's documented floor and ceiling for the drum.",
    ),
    c(
        "rotary::DRUM_FALL_SECONDS",
        f(rotary::DRUM_FALL_SECONDS),
        "s",
        Provisional,
        Sourced(1.0, 12.5),
        "As the drum's rise.",
    ),
    c(
        "rotary::DRUM_BRAKE_SECONDS",
        f(rotary::DRUM_BRAKE_SECONDS),
        "s",
        Provisional,
        Sourced(1.0, 12.5),
        "As the drum's rise.",
    ),
    c(
        "rotary::HORN_TIME_FLOOR",
        f(rotary::HORN_TIME_FLOOR),
        "s",
        Documented,
        Exact,
        "Hammond's floor for a horn transition.",
    ),
    c(
        "rotary::DRUM_TIME_FLOOR",
        f(rotary::DRUM_TIME_FLOOR),
        "s",
        Documented,
        Exact,
        "Hammond's floor for a drum transition.",
    ),
    c(
        "rotary::TIME_CEILING",
        f(rotary::TIME_CEILING),
        "s",
        Documented,
        Exact,
        "Hammond's ceiling for a transition.",
    ),
    c(
        "rotary::ACCELERATION_BRISKEST",
        f(rotary::ACCELERATION_BRISKEST),
        "ratio",
        Provisional,
        Assumed(0.3, 1.0),
        "How the acceleration control maps onto the times; the middle lands near them.",
    ),
    c(
        "rotary::ACCELERATION_SPAN",
        f(rotary::ACCELERATION_SPAN),
        "ratio",
        Provisional,
        Assumed(0.5, 4.0),
        "As the briskest scale.",
    ),
    c(
        "rotary::MODE_DELAY_SECONDS",
        f(rotary::MODE_DELAY_SECONDS),
        "s",
        Provisional,
        Sourced(0.0, 1.0),
        "Hammond exposes the relay and clutch delay as 0 to 1 s.",
    ),
    c(
        "rotary::RATED_MAINS_HZ",
        f(rotary::RATED_MAINS_HZ),
        "Hz",
        Documented,
        Exact,
        "The supply the quoted speeds belong to.",
    ),
    c(
        "rotary::WOOFER_COIL_H",
        f(rotary::WOOFER_COIL_H),
        "H",
        Documented,
        Exact,
        "L3, in series with the woofer: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "rotary::WOOFER_SHUNT_F",
        f(rotary::WOOFER_SHUNT_F),
        "F",
        Documented,
        Exact,
        "C16B, across the woofer: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "rotary::HORN_SERIES_F",
        f(rotary::HORN_SERIES_F),
        "F",
        Documented,
        Exact,
        "C16A, in series with the horn driver: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "rotary::HORN_COIL_H",
        f(rotary::HORN_COIL_H),
        "H",
        Documented,
        Exact,
        "L2, across the horn driver: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "rotary::DRIVER_OHM",
        f(rotary::DRIVER_OHM),
        "ohm",
        Provisional,
        Assumed(8.0, 32.0),
        "The drivers' nominal rating, taken as a resistance; a driver is not one.",
    ),
    c(
        "leslie_amp::INPUT_SERIES_OHM",
        f(leslie_amp::INPUT_SERIES_OHM),
        "ohm",
        Documented,
        Exact,
        "R1 + R3, in series with the input lines: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::VOLUME_RHEOSTAT_OHM",
        f(leslie_amp::VOLUME_RHEOSTAT_OHM),
        "ohm",
        Documented,
        Exact,
        "R2, the volume rheostat across the lines, at full: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::INPUT_SHUNT_F",
        f(leslie_amp::INPUT_SHUNT_F),
        "F",
        Documented,
        Exact,
        "C7, across the two grids: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::DRIVER_PLATE_LOAD_OHM",
        f(leslie_amp::DRIVER_PLATE_LOAD_OHM),
        "ohm",
        Documented,
        Exact,
        "R8 and R9, the 12AU7 plate loads: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::DRIVER_BIAS_V",
        f(leslie_amp::DRIVER_BIAS_V),
        "V",
        Documented,
        Exact,
        "The 12AU7 pair's cathode voltage: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::COUPLING_F",
        f(leslie_amp::COUPLING_F),
        "F",
        Documented,
        Exact,
        "C9 and C10, into the 6550 grids: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::GRID_LEAK_OHM",
        f(leslie_amp::GRID_LEAK_OHM),
        "ohm",
        Documented,
        Exact,
        "R10 and R11, the 6550 grid leaks: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::FEEDBACK_OHM",
        f(leslie_amp::FEEDBACK_OHM),
        "ohm",
        Documented,
        Exact,
        "R14 and R15, plate to grid: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::FEEDBACK_F",
        f(leslie_amp::FEEDBACK_F),
        "F",
        Documented,
        Exact,
        "C21 and C13, plate to grid: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::OUTPUT_BIAS_V",
        f(leslie_amp::OUTPUT_BIAS_V),
        "V",
        Documented,
        Exact,
        "The 6550 cathode voltage across R12: Electro Music's 122 main amplifier schematic (redrawn 1995).",
    ),
    c(
        "leslie_amp::DRIVER_GAIN",
        f(leslie_amp::DRIVER_GAIN),
        "gain",
        Provisional,
        Assumed(10.0, 17.0),
        "A 12AU7 half's gain at this stage's current, from the valve's curves.",
    ),
    c(
        "leslie_amp::DRIVER_PLATE_OHM",
        f(leslie_amp::DRIVER_PLATE_OHM),
        "ohm",
        Provisional,
        Assumed(6_000.0, 16_000.0),
        "A 12AU7 half's plate resistance at this stage's current, from the valve's curves.",
    ),
    c(
        "leslie_amp::OUTPUT_STAGE_GAIN",
        f(leslie_amp::OUTPUT_STAGE_GAIN),
        "gain",
        Provisional,
        Assumed(10.0, 40.0),
        "A 6550's plate-to-grid gain into a load the schematic does not give.",
    ),
    c(
        "leslie_amp::OUTPUT_CUTOFF_V",
        f(leslie_amp::OUTPUT_CUTOFF_V),
        "V",
        Provisional,
        Assumed(30.0, 50.0),
        "Where a 6550 stops conducting at 310 V on its screens, from the valve's curves.",
    ),
    c(
        "leslie_amp::GRID_CONDUCTION_OHM",
        f(leslie_amp::GRID_CONDUCTION_OHM),
        "ohm",
        Provisional,
        Assumed(500.0, 5_000.0),
        "What a conducting 6550 grid presents; not published.",
    ),
    c(
        "leslie_amp::ORGAN_SOURCE_OHM",
        f(leslie_amp::ORGAN_SOURCE_OHM),
        "ohm",
        Provisional,
        Assumed(50.0, 5_000.0),
        "The console's output through its connector kit; no kit's figure is published.",
    ),
    c(
        "leslie_amp::GRID_VOLTS_PER_UNIT",
        f(leslie_amp::GRID_VOLTS_PER_UNIT),
        "V",
        Provisional,
        Assumed(30.0, 490.0),
        "Anchored so an 888 chord at full expression reaches grid conduction at the middle of the drive control; a setting, not a property. Re-anchored by the laboratory.",
    ),
    c(
        "leslie_amp::DRIVE_SPAN_DB",
        f(leslie_amp::DRIVE_SPAN_DB),
        "dB",
        Provisional,
        Assumed(12.0, 36.0),
        "How far the drive control moves the amplifier's input, end to end.",
    ),
    c(
        "leslie_amp::AMP_DRIVE_DEFAULT",
        f(leslie_amp::AMP_DRIVE_DEFAULT),
        "control",
        Provisional,
        Assumed(0.0, 1.0),
        "Where the drive control starts: the anchor itself.",
    ),
    c(
        "rotary::HORN_SHELF_HZ",
        f(rotary::HORN_SHELF_HZ),
        "Hz",
        Provisional,
        Assumed(1_000.0, 5_000.0),
        "Stands in for the horn's directivity measured at many angles.",
    ),
    c(
        "rotary::HORN_BODY_AWAY",
        f(rotary::HORN_BODY_AWAY),
        "gain",
        Provisional,
        Assumed(0.2, 0.9),
        "As the horn's shelf.",
    ),
    c(
        "rotary::HORN_BODY_FACING",
        f(rotary::HORN_BODY_FACING),
        "gain",
        Provisional,
        Assumed(0.1, 0.8),
        "As the horn's shelf.",
    ),
    c(
        "rotary::HORN_TOP_AWAY",
        f(rotary::HORN_TOP_AWAY),
        "gain",
        Provisional,
        Assumed(0.0, 0.6),
        "As the horn's shelf.",
    ),
    c(
        "rotary::HORN_TOP_FACING",
        f(rotary::HORN_TOP_FACING),
        "gain",
        Provisional,
        Assumed(0.4, 1.0),
        "As the horn's shelf.",
    ),
    c(
        "rotary::DRUM_AWAY",
        f(rotary::DRUM_AWAY),
        "gain",
        Provisional,
        Assumed(0.3, 0.95),
        "Stands in for the drum port's directivity.",
    ),
    c(
        "rotary::DRUM_FACING",
        f(rotary::DRUM_FACING),
        "gain",
        Provisional,
        Assumed(0.05, 0.7),
        "As the drum's level away.",
    ),
    c(
        "rotary::REFLECTION_LEVEL",
        f(rotary::REFLECTION_LEVEL),
        "gain",
        Provisional,
        Assumed(0.0, 1.0),
        "No cabinet's early reflections have been measured for this model.",
    ),
    c(
        "rotary::SUB_LEVEL_DEFAULT_DB",
        f(rotary::SUB_LEVEL_DEFAULT_DB),
        "dB",
        Provisional,
        Sourced(-76.0, 0.0),
        "Hammond documents the woofer's volume range, not where it sits.",
    ),
    c(
        "rotary::SUB_INTO_DRUM",
        f(rotary::SUB_INTO_DRUM),
        "gain",
        Derived,
        Exact,
        "The reference the horn pair's share of the woofer is stated against.",
    ),
    c(
        "rotary::SUB_INTO_HORN",
        f(rotary::SUB_INTO_HORN),
        "gain",
        Provisional,
        Assumed(0.0, 0.6),
        "Hammond says the horn's pair barely hears the woofer, not how little.",
    ),
    c(
        "rotary::DYNAMIC_PRESENCE_HZ",
        f(rotary::DYNAMIC_PRESENCE_HZ),
        "Hz",
        Provisional,
        Assumed(2_000.0, 6_000.0),
        "No capsule is named anywhere citable.",
    ),
    c(
        "rotary::DYNAMIC_PRESENCE_LIFT",
        f(rotary::DYNAMIC_PRESENCE_LIFT),
        "gain",
        Provisional,
        Assumed(0.0, 1.0),
        "As the presence corner.",
    ),
    c(
        "rotary::DYNAMIC_TOP_HZ",
        f(rotary::DYNAMIC_TOP_HZ),
        "Hz",
        Provisional,
        Assumed(6_000.0, 16_000.0),
        "As the presence corner.",
    ),
    c(
        "rotary::PROXIMITY_MAX_LIFT",
        f(rotary::PROXIMITY_MAX_LIFT),
        "gain",
        Provisional,
        Assumed(0.3, 1.5),
        "Stands in for the roll-off a real capsule has under its proximity lift.",
    ),
    c(
        "rotary::HORN_RADIUS_DEFAULT_M",
        f(rotary::HORN_RADIUS_DEFAULT_M),
        "m",
        Provisional,
        Assumed(0.05, 0.40),
        "Not published; a control, and the laboratory compares its Doppler with the geometry's.",
    ),
    c(
        "rotary::DRUM_RADIUS_DEFAULT_M",
        f(rotary::DRUM_RADIUS_DEFAULT_M),
        "m",
        Provisional,
        Assumed(0.05, 0.30),
        "As the horn's radius.",
    ),
    c(
        "rotary::CABINET_HALF_M",
        f(rotary::CABINET_HALF_M),
        "m",
        Provisional,
        Assumed(0.25, 0.45),
        "Not on the page the microphone placement is documented on.",
    ),
    c(
        "rotary::SOUND_SPEED_M_PER_S",
        f(rotary::SOUND_SPEED_M_PER_S),
        "m/s",
        Derived,
        Exact,
        "Speed of sound in air at about 20 degrees Celsius.",
    ),
    c(
        "rotary::MIC_SPACING_MAX_M",
        f(rotary::MIC_SPACING_MAX_M),
        "m",
        Documented,
        Exact,
        "Hammond: up to 40 cm between a pair.",
    ),
    c(
        "rotary::MIC_OFFSET_MAX_M",
        f(rotary::MIC_OFFSET_MAX_M),
        "m",
        Documented,
        Exact,
        "Hammond: up to 50 cm between a pair's centre and the rotor's pivot.",
    ),
    c(
        "rotary::MIC_DISTANCE_DEFAULT_M",
        f(rotary::MIC_DISTANCE_DEFAULT_M),
        "m",
        Provisional,
        Sourced(0.12, 1.7),
        "A default inside Hammond's documented placement range.",
    ),
    c(
        "rotary::MIC_SPACING_DEFAULT_M",
        f(rotary::MIC_SPACING_DEFAULT_M),
        "m",
        Provisional,
        Sourced(0.0, 0.4),
        "As the distance's default.",
    ),
    c(
        "rotary::MIC_PATTERN_DEFAULT",
        f(rotary::MIC_PATTERN_DEFAULT),
        "control",
        Provisional,
        Assumed(0.0, 1.0),
        "A cardioid, from omnidirectional at zero to figure of eight at one.",
    ),
];

pub const TABLES: &[Table] = &[
    Table {
        name: "tonewheel::GEAR_RATIOS_60_HZ",
        basis: Documented,
        note: "The generator's gear train, translated from setBfree's 60 Hz table.",
    },
    Table {
        name: "taper::TAPER",
        basis: Measured,
        note: "Fitted to Hammond's Table V and six consoles read off a busbar, the engine's console response taken out; scaled to keep the pedals' level.",
    },
    Table {
        name: "taper::KEY_TAPER",
        basis: Measured,
        note: "Z / (Z + R) from five measured manuals' tapering wires and three consoles' wheel impedances; checked against key readings not used to set it.",
    },
    Table {
        name: "preset::UPPER_PRESETS",
        basis: Documented,
        note: "The upper preset panel's registrations, quoted digit for digit.",
    },
    Table {
        name: "preset::LOWER_PRESETS",
        basis: Documented,
        note: "The lower preset panel's registrations, quoted digit for digit.",
    },
    Table {
        name: "vibrato_line::CAPACITANCE_F",
        basis: Documented,
        note: "Component values of the line's shunt capacitors.",
    },
    Table {
        name: "vibrato_line::DIVIDERS",
        basis: Documented,
        note: "The six tap dividers' resistances.",
    },
    Table {
        name: "vibrato_line::DEPTH_TERMINAL_NODES",
        basis: Documented,
        note: "Which ladder nodes each depth position wires to the terminals.",
    },
    Table {
        name: "rotary::REFLECTIONS",
        basis: Provisional,
        note: "Three early reflections' levels and delays; none measured.",
    },
];

/// The constant registered under `name`, if there is one.
pub fn find(name: &str) -> Option<&'static Constant> {
    CONSTANTS.iter().find(|constant| constant.name == name)
}

/// Unmeasured constants a model cannot move yet, and why. Everything else
/// that rests on less than a document is a [`Knob`], so the laboratory can
/// find out what it is worth.
pub const NOT_YET_KNOBS: [(&str, &str); 1] = [(
    "rotary::CABINET_HALF_M",
    "only reaches microphones at the cabinet's sides, through geometry the play surface shares",
)];

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::format;
    use std::string::String;
    use std::vec::Vec;

    use crate::model::{Knob, Model};

    /// The modules whose scalars the model is built from, as source text.
    const SOURCES: [(&str, &str); 13] = [
        ("electronics", include_str!("electronics.rs")),
        ("leslie_amp", include_str!("leslie_amp.rs")),
        ("lib", include_str!("lib.rs")),
        ("manual", include_str!("manual.rs")),
        ("pedal", include_str!("pedal.rs")),
        ("percussion", include_str!("percussion.rs")),
        ("preset", include_str!("preset.rs")),
        ("rotary", include_str!("rotary.rs")),
        ("scanner", include_str!("scanner.rs")),
        ("taper", include_str!("taper.rs")),
        ("tonewheel", include_str!("tonewheel.rs")),
        ("transformer", include_str!("transformer.rs")),
        ("vibrato_line", include_str!("vibrato_line.rs")),
    ];

    /// Every `f32` or `f64` constant declared outside the tests, as
    /// `module::NAME`.
    fn declared() -> Vec<String> {
        let mut names = Vec::new();
        for (module, source) in SOURCES {
            let body = source.split("#[cfg(test)]").next().unwrap_or(source);
            for line in body.lines() {
                let line = line.trim_start();
                let line = line
                    .strip_prefix("pub(crate) ")
                    .or_else(|| line.strip_prefix("pub "))
                    .unwrap_or(line);
                let Some(rest) = line.strip_prefix("const ") else {
                    continue;
                };
                let Some((name, kind)) = rest.split_once(':') else {
                    continue;
                };
                let kind = kind.trim_start();
                let name = name.trim();
                let constant_name = !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                if constant_name && (kind.starts_with("f32") || kind.starts_with("f64")) {
                    names.push(format!("{module}::{name}"));
                }
            }
        }
        names
    }

    /// A constant nobody has said the provenance of is the failure this
    /// module exists to prevent, so a new one fails here until it is listed
    /// or explained away.
    #[test]
    fn every_scalar_the_engine_declares_is_accounted_for() {
        let declared = declared();
        assert!(
            declared.len() > 60,
            "the scan found only {}",
            declared.len()
        );
        for name in &declared {
            let registered = find(name).is_some();
            let excused = NOT_MODEL
                .iter()
                .any(|(excused, _)| *excused == name.as_str());
            assert!(
                registered || excused,
                "{name} has no provenance: add it to CONSTANTS, or to NOT_MODEL with a reason"
            );
            assert!(!(registered && excused), "{name} is both");
        }
        for constant in CONSTANTS {
            assert!(
                declared.iter().any(|name| name == constant.name),
                "{} is registered but not declared where its name says",
                constant.name
            );
        }
        for (name, _) in NOT_MODEL {
            assert!(
                declared.iter().any(|declared| declared == name),
                "{name} is excused but not declared"
            );
        }
        for table in TABLES {
            let (module, name) = table.name.split_once("::").expect("module::NAME");
            let (_, source) = SOURCES
                .iter()
                .find(|(declared, _)| *declared == module)
                .unwrap_or_else(|| panic!("{} names no scanned module", table.name));
            assert!(
                source.contains(&format!("const {name}: ")),
                "{} is registered but not declared where its name says",
                table.name
            );
        }
    }

    /// The model a laboratory moves has to be the constants the table
    /// describes, value for value, or a survey would be measuring something
    /// other than the instrument.
    #[test]
    fn every_knob_is_a_registered_unmeasured_constant() {
        let baseline = Model::BASELINE;
        for (index, knob) in Knob::ALL.into_iter().enumerate() {
            assert!(
                Knob::ALL[..index]
                    .iter()
                    .all(|other| other.name() != knob.name()),
                "{} is a knob twice",
                knob.name()
            );
            let constant =
                find(knob.name()).unwrap_or_else(|| panic!("{} is not registered", knob.name()));
            assert!(
                matches!(
                    constant.basis,
                    Basis::Measured | Basis::Borrowed | Basis::Provisional
                ),
                "{} is {} and has nothing to survey",
                knob.name(),
                constant.basis.name()
            );
            assert!(
                f64::from(baseline[knob]) == constant.value,
                "{} starts at {} where the table says {}",
                knob.name(),
                baseline[knob],
                constant.value
            );
        }
        // And the other way: an unmeasured constant is a knob unless it is a
        // control's starting point, which a survey moves through the control,
        // or it says why not.
        for constant in CONSTANTS {
            let unmeasured = matches!(
                constant.basis,
                Basis::Measured | Basis::Borrowed | Basis::Provisional
            );
            let a_default = constant.name.contains("DEFAULT");
            let a_knob = Knob::ALL.iter().any(|knob| knob.name() == constant.name);
            let excused = NOT_YET_KNOBS.iter().any(|(name, _)| *name == constant.name);
            if unmeasured && !a_default {
                assert!(
                    a_knob || excused,
                    "{} is unmeasured but no model can move it",
                    constant.name
                );
            }
            assert!(!(a_knob && excused), "{} is both", constant.name);
        }
    }

    #[test]
    fn every_constant_says_what_it_rests_on() {
        for (index, constant) in CONSTANTS.iter().enumerate() {
            assert!(
                CONSTANTS[..index]
                    .iter()
                    .all(|other| other.name != constant.name),
                "{} is registered twice",
                constant.name
            );
            assert!(!constant.note.is_empty(), "{} has no note", constant.name);
            assert!(!constant.unit.is_empty(), "{} has no unit", constant.name);
            assert!(constant.value.is_finite(), "{}", constant.name);
            // Anything short of a document or a derivation could be other
            // than it is, so it has to say by how much.
            let needs_span = matches!(
                constant.basis,
                Basis::Measured | Basis::Borrowed | Basis::Provisional
            );
            match constant.span.bounds() {
                Some((low, high)) => {
                    assert!(low < high, "{} has an empty span", constant.name);
                    // A tolerance is allowed to stop just short of the value
                    // a model chose, but not to miss it by more than a
                    // rounding.
                    let slack = 1.0e-6 * constant.value.abs().max(1.0);
                    assert!(
                        constant.value >= low - slack && constant.value <= high + slack,
                        "{} = {} lies outside its own span {low}..{high}",
                        constant.name,
                        constant.value
                    );
                }
                None => assert!(
                    !needs_span,
                    "{} is {} and needs a span",
                    constant.name,
                    constant.basis.name()
                ),
            }
        }
    }
}
