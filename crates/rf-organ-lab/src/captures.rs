// SPDX-License-Identifier: GPL-2.0-or-later

//! Shared description of the deterministic capture suite.
//!
//! The render and the comparator agree on capture names, registrations and
//! signal levels here, so that a reference session recorded with the protocol
//! in `docs/CALIBRATION.md` lines up with the generated files.

use rf_organ_dsp::{
    ConsoleElectronics, ConsoleStage, DRAWBAR_COUNT, MANUAL_FIRST_NOTE, MANUAL_KEY_COUNT,
    MatchingTransformer, OrganEngine, OrganPart, PEDAL_DRAWBAR_COUNT, PercussionDecay,
    PercussionHarmonic, PercussionVolume, Registration, RotaryMode, ScannerMode, TransformerUnit,
    drawbar_wheel, gear_frequency,
};
use std::f64::consts::{PI, TAU};

pub const SAMPLE_RATE: u32 = 48_000;
pub const PHRASE_SECONDS: usize = 4;
/// Shared transformer character used by every transformer capture. The
/// per-unit trims are the quantity under measurement and stay at zero here.
pub const CHARACTER: (f32, f32) = (0.38, 0.32);
/// Per-unit calibration trims, ordered T1, T2, T3.
pub type Trims = [(f32, f32); 3];
pub const NO_TRIM: Trims = [(0.0, 0.0); 3];
pub const C_NOTE: u8 = 72;
pub const F_NOTE: u8 = 77;
/// Injection gate ramp. Long enough to avoid a click, far shorter than the
/// 20 ms onset-timing warning threshold.
const INJECTION_RAMP_SECONDS: f64 = 0.005;

pub const PHRASE_CAPTURES: [&str; 9] = [
    "01-direct-888",
    "02-third-percussion",
    "03-scanner-c3",
    "04-rotary-chorale",
    "05-rotary-tremolo",
    "06-full-console",
    "07-pedal-16ft",
    "08-pedal-8ft",
    "09-pedal-16ft-8ft",
];
pub const IMPULSE_CAPTURE: &str = "rotary-cabinet-impulse";

/// The bench injection a preamplifier's stage asymmetry is read off.
///
/// One steady tone into the console's input at three documented drives, the
/// tone control flat and the pedal wide open. It is not a phrase and is not in
/// [`capture_names`]: it is a measurement signal, played into the AO-28's own
/// input rather than through the generator, which is the only way to see what
/// the stages do without the wheels in front of them.
pub const CONSOLE_CAPTURES: [ConsoleCapture; 3] = [
    ConsoleCapture {
        id: "console-injection-clean",
        drive: 0.0,
    },
    ConsoleCapture {
        id: "console-injection-baseline",
        drive: 0.32,
    },
    ConsoleCapture {
        id: "console-injection-hard",
        drive: 0.75,
    },
];
/// The tone, its level and how long it is held.
pub const CONSOLE_PROBE_HZ: f64 = 1_000.0;
pub const CONSOLE_PROBE_AMPLITUDE: f64 = 0.25;
pub const CONSOLE_PROBE_SECONDS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConsoleCapture {
    pub id: &'static str,
    pub drive: f32,
}

/// The injections that see one stage at a time.
///
/// A probe at a stage's grid and another at its plate, at the drive the
/// console is actually set to. Nothing before or after that stage is in the
/// way, so the second harmonic in the reading belongs to that stage and to
/// nothing else - which is the only way the three can be told apart.
pub const CONSOLE_STAGE_CAPTURES: [ConsoleStageCapture; 6] = [
    ConsoleStageCapture {
        id: "console-stage-v4a-baseline",
        stage: ConsoleStage::V4A,
        drive: 0.32,
    },
    ConsoleStageCapture {
        id: "console-stage-v4a-hard",
        stage: ConsoleStage::V4A,
        drive: 0.75,
    },
    ConsoleStageCapture {
        id: "console-stage-v4b-baseline",
        stage: ConsoleStage::V4B,
        drive: 0.32,
    },
    ConsoleStageCapture {
        id: "console-stage-v4b-hard",
        stage: ConsoleStage::V4B,
        drive: 0.75,
    },
    ConsoleStageCapture {
        id: "console-stage-v3b-baseline",
        stage: ConsoleStage::V3B,
        drive: 0.32,
    },
    ConsoleStageCapture {
        id: "console-stage-v3b-hard",
        stage: ConsoleStage::V3B,
        drive: 0.75,
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConsoleStageCapture {
    pub id: &'static str,
    pub stage: ConsoleStage,
    pub drive: f32,
}

/// Renders one stage's injection: the same tone, into that stage alone.
pub fn render_console_stage(capture: &ConsoleStageCapture) -> Vec<f32> {
    let rate = SAMPLE_RATE as f64;
    let frames = SAMPLE_RATE as usize * CONSOLE_PROBE_SECONDS;
    let mut electronics = ConsoleElectronics::new(SAMPLE_RATE as f32);
    assert!(electronics.set(capture.drive, 0.0, 0.0));
    let mut output = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let angle = TAU * CONSOLE_PROBE_HZ * frame as f64 / rate;
        let sample = electronics.stage_sample(
            capture.stage,
            (CONSOLE_PROBE_AMPLITUDE * angle.sin()) as f32,
        );
        output.push(sample);
        output.push(sample);
    }
    output
}

/// Renders one console injection: a steady tone through the preamplifier
/// alone, with the generator, the cabinet and the pedal all out of the way.
pub fn render_console_injection(capture: &ConsoleCapture) -> Vec<f32> {
    let rate = SAMPLE_RATE as f64;
    let settle = SAMPLE_RATE as usize / 4;
    let frames = SAMPLE_RATE as usize * CONSOLE_PROBE_SECONDS;
    let mut electronics = ConsoleElectronics::new(SAMPLE_RATE as f32);
    assert!(electronics.set(capture.drive, 0.0, 0.0));
    assert!(electronics.set_expression_character(0.0));
    let mut output = Vec::with_capacity(frames * 2);
    for frame in 0..settle + frames {
        let angle = TAU * CONSOLE_PROBE_HZ * frame as f64 / rate;
        let sample = electronics.process((CONSOLE_PROBE_AMPLITUDE * angle.sin()) as f32, 1.0);
        if frame >= settle {
            output.push(sample);
            output.push(sample);
        }
    }
    output
}

/// The chromatic pass that a per-wheel taper is read off. It is not in
/// [`capture_names`]: those are phrases, compared to each other whole and
/// gated as such, and this is a sweep of sixty-one separate notes that is
/// read one stretch at a time.
pub const TAPER_CAPTURE: &str = "generator-taper-sweep";
/// Seconds each key is held, and the silence after it. Long enough for a
/// steady reading, short enough that the whole manual fits in a file somebody
/// will actually record.
pub const TAPER_KEY_SECONDS: f64 = 0.2;
pub const TAPER_GAP_SECONDS: f64 = 0.05;
/// The drawbar the sweep uses. Eight foot is the fundamental, so the wheel
/// under each key is the one that key is named after.
pub const TAPER_BUS: usize = 2;
/// Key struck in the percussion captures, and the registration behind it. The
/// 888 registration puts nothing on the 2 2/3' bus, so the third-harmonic
/// percussion stands alone in the spectrum at three times the 8' frequency.
pub const PERCUSSION_NOTE: u8 = 60;
pub const PERCUSSION_DRAWBARS: [u8; 3] = [8, 8, 8];
/// Expression character the calibration render uses, and the quantity the
/// expression captures exist to fit.
pub const EXPRESSION_CHARACTER: f32 = 0.55;

#[derive(Clone, Copy, Debug)]
pub struct PercussionCapture {
    pub id: &'static str,
    pub volume: PercussionVolume,
    pub decay: PercussionDecay,
}

/// One capture per tablet combination, recorded at one gain like the
/// transformer grid: the level difference between the Normal and Soft pairs is
/// itself a measurement.
pub const PERCUSSION_CAPTURES: [PercussionCapture; 4] = [
    PercussionCapture {
        id: "37-percussion-normal-fast",
        volume: PercussionVolume::Normal,
        decay: PercussionDecay::Fast,
    },
    PercussionCapture {
        id: "38-percussion-normal-slow",
        volume: PercussionVolume::Normal,
        decay: PercussionDecay::Slow,
    },
    PercussionCapture {
        id: "39-percussion-soft-fast",
        volume: PercussionVolume::Soft,
        decay: PercussionDecay::Fast,
    },
    PercussionCapture {
        id: "40-percussion-soft-slow",
        volume: PercussionVolume::Soft,
        decay: PercussionDecay::Slow,
    },
];

/// Keys and registration of the expression captures. Two octaves apart with
/// every drawbar out puts energy on the 16' bus of the low key and on the 8'
/// and 1' buses of the high one, which is how one capture can be read at a
/// low, a middle and a high frequency.
pub const EXPRESSION_NOTES: [u8; 2] = [48, 84];
pub const EXPRESSION_LOW: (u8, usize) = (48, 0);
pub const EXPRESSION_MID: (u8, usize) = (84, 2);
pub const EXPRESSION_HIGH: (u8, usize) = (84, 8);

#[derive(Clone, Copy, Debug)]
pub struct ExpressionCapture {
    pub id: &'static str,
    /// Documented pedal position, heel to toe.
    pub position: f32,
}

/// Five pedal positions at one recording gain. Every reading is relative to
/// the toe capture, so the absolute gain of the session cancels.
pub const EXPRESSION_CAPTURES: [ExpressionCapture; 5] = [
    ExpressionCapture {
        id: "41-expression-heel",
        position: 0.125,
    },
    ExpressionCapture {
        id: "42-expression-quarter",
        position: 0.25,
    },
    ExpressionCapture {
        id: "43-expression-half",
        position: 0.5,
    },
    ExpressionCapture {
        id: "44-expression-three-quarter",
        position: 0.75,
    },
    ExpressionCapture {
        id: "45-expression-toe",
        position: 1.0,
    },
];

/// Which transformers a capture exercises. A microphone capture of a manual
/// always traverses two units; only the bench injection reaches T3 alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformerPath {
    Upper,
    Lower,
    Injection,
}

impl TransformerPath {
    pub const ALL: [Self; 3] = [Self::Upper, Self::Lower, Self::Injection];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Upper => "t2+t3",
            Self::Lower => "t1+t3",
            Self::Injection => "t3",
        }
    }

    /// The transformer a fit of this path resolves once the others are known.
    pub const fn unit(self) -> TransformerUnit {
        match self {
            Self::Upper => TransformerUnit::T2,
            Self::Lower => TransformerUnit::T1,
            Self::Injection => TransformerUnit::T3,
        }
    }

    pub const fn manual(self) -> Option<OrganPart> {
        match self {
            Self::Upper => Some(OrganPart::Upper),
            Self::Lower => Some(OrganPart::Lower),
            Self::Injection => None,
        }
    }
}

/// Three documented drive levels one 6 dB step apart, so that a fit can use
/// the level dependence of the magnetic model instead of a single point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Low,
    Nominal,
    High,
}

impl Level {
    pub const ALL: [Self; 3] = [Self::Low, Self::Nominal, Self::High];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Nominal => "nominal",
            Self::High => "high",
        }
    }

    /// 8-foot drawbar position used by the manual captures.
    pub const fn drawbar(self) -> u8 {
        match self {
            Self::Low => 4,
            Self::Nominal => 6,
            Self::High => 8,
        }
    }

    /// Peak amplitude of each injected tone, full scale at the T3 primary.
    pub const fn injection_amplitude(self) -> f64 {
        match self {
            Self::Low => 0.045,
            Self::Nominal => 0.09,
            Self::High => 0.18,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notes {
    C,
    F,
    Dyad,
}

impl Notes {
    pub const ALL: [Self; 3] = [Self::C, Self::F, Self::Dyad];

    pub const fn label(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::F => "f",
            Self::Dyad => "c-f",
        }
    }

    pub const fn plays_c(self) -> bool {
        matches!(self, Self::C | Self::Dyad)
    }

    pub const fn plays_f(self) -> bool {
        matches!(self, Self::F | Self::Dyad)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TransformerCapture {
    pub id: &'static str,
    pub path: TransformerPath,
    pub level: Level,
    pub notes: Notes,
}

pub const TRANSFORMER_CAPTURES: [TransformerCapture; 27] = [
    TransformerCapture {
        id: "10-upper-transformer-low-c",
        path: TransformerPath::Upper,
        level: Level::Low,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "11-upper-transformer-low-f",
        path: TransformerPath::Upper,
        level: Level::Low,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "12-upper-transformer-low-c-f",
        path: TransformerPath::Upper,
        level: Level::Low,
        notes: Notes::Dyad,
    },
    TransformerCapture {
        id: "13-upper-transformer-nominal-c",
        path: TransformerPath::Upper,
        level: Level::Nominal,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "14-upper-transformer-nominal-f",
        path: TransformerPath::Upper,
        level: Level::Nominal,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "15-upper-transformer-nominal-c-f",
        path: TransformerPath::Upper,
        level: Level::Nominal,
        notes: Notes::Dyad,
    },
    TransformerCapture {
        id: "16-upper-transformer-high-c",
        path: TransformerPath::Upper,
        level: Level::High,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "17-upper-transformer-high-f",
        path: TransformerPath::Upper,
        level: Level::High,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "18-upper-transformer-high-c-f",
        path: TransformerPath::Upper,
        level: Level::High,
        notes: Notes::Dyad,
    },
    TransformerCapture {
        id: "19-lower-transformer-low-c",
        path: TransformerPath::Lower,
        level: Level::Low,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "20-lower-transformer-low-f",
        path: TransformerPath::Lower,
        level: Level::Low,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "21-lower-transformer-low-c-f",
        path: TransformerPath::Lower,
        level: Level::Low,
        notes: Notes::Dyad,
    },
    TransformerCapture {
        id: "22-lower-transformer-nominal-c",
        path: TransformerPath::Lower,
        level: Level::Nominal,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "23-lower-transformer-nominal-f",
        path: TransformerPath::Lower,
        level: Level::Nominal,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "24-lower-transformer-nominal-c-f",
        path: TransformerPath::Lower,
        level: Level::Nominal,
        notes: Notes::Dyad,
    },
    TransformerCapture {
        id: "25-lower-transformer-high-c",
        path: TransformerPath::Lower,
        level: Level::High,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "26-lower-transformer-high-f",
        path: TransformerPath::Lower,
        level: Level::High,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "27-lower-transformer-high-c-f",
        path: TransformerPath::Lower,
        level: Level::High,
        notes: Notes::Dyad,
    },
    TransformerCapture {
        id: "28-t3-injection-low-c",
        path: TransformerPath::Injection,
        level: Level::Low,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "29-t3-injection-low-f",
        path: TransformerPath::Injection,
        level: Level::Low,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "30-t3-injection-low-c-f",
        path: TransformerPath::Injection,
        level: Level::Low,
        notes: Notes::Dyad,
    },
    TransformerCapture {
        id: "31-t3-injection-nominal-c",
        path: TransformerPath::Injection,
        level: Level::Nominal,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "32-t3-injection-nominal-f",
        path: TransformerPath::Injection,
        level: Level::Nominal,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "33-t3-injection-nominal-c-f",
        path: TransformerPath::Injection,
        level: Level::Nominal,
        notes: Notes::Dyad,
    },
    TransformerCapture {
        id: "34-t3-injection-high-c",
        path: TransformerPath::Injection,
        level: Level::High,
        notes: Notes::C,
    },
    TransformerCapture {
        id: "35-t3-injection-high-f",
        path: TransformerPath::Injection,
        level: Level::High,
        notes: Notes::F,
    },
    TransformerCapture {
        id: "36-t3-injection-high-c-f",
        path: TransformerPath::Injection,
        level: Level::High,
        notes: Notes::Dyad,
    },
];

pub fn transformer_capture(
    path: TransformerPath,
    level: Level,
    notes: Notes,
) -> &'static TransformerCapture {
    TRANSFORMER_CAPTURES
        .iter()
        .find(|capture| capture.path == path && capture.level == level && capture.notes == notes)
        .expect("complete capture grid")
}

/// Every capture name, in render order.
pub fn capture_names() -> Vec<&'static str> {
    PHRASE_CAPTURES
        .into_iter()
        .chain(TRANSFORMER_CAPTURES.iter().map(|capture| capture.id))
        .chain(PERCUSSION_CAPTURES.iter().map(|capture| capture.id))
        .chain(EXPRESSION_CAPTURES.iter().map(|capture| capture.id))
        .chain([IMPULSE_CAPTURE])
        .collect()
}

/// Renders the chromatic pass a taper is read off: every key of the upper
/// manual in turn, on the eight foot alone, each held for the same time with
/// the same silence after it.
///
/// Everything that could colour one key differently from another is turned
/// off - no vibrato, no percussion, no rotary, no leakage, no drive wobble,
/// the console flat and the expression wide open - so that what is left
/// between one key and the next is the generator.
pub fn render_taper_sweep() -> Vec<f32> {
    let rate = SAMPLE_RATE as f64;
    let held = (TAPER_KEY_SECONDS * rate) as usize;
    let gap = (TAPER_GAP_SECONDS * rate) as usize;
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).expect("valid engine");
    for drawbar in 0..DRAWBAR_COUNT {
        let position = if drawbar == TAPER_BUS { 8 } else { 0 };
        let _ =
            engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, drawbar, position);
        let _ = engine.set_manual_drawbar(OrganPart::Lower, Registration::AdjustB, drawbar, 0);
    }
    for drawbar in 0..PEDAL_DRAWBAR_COUNT {
        let _ = engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, drawbar, 0);
    }
    let _ = engine.set_output_level(1.0);
    let _ = engine.set_expression(1.0);
    let _ = engine.set_console(0.0, 0.0, 0.0);
    let _ = engine.set_transformer(CHARACTER.0, CHARACTER.1);
    let _ = engine.set_leakage(0.0);
    let _ = engine.set_drive_wobble(0.0);
    let _ = engine.set_eccentricity(0.0);
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    engine.set_rotary_mode(RotaryMode::Off);
    engine.set_percussion_enabled(false);

    let mut output = Vec::with_capacity(MANUAL_KEY_COUNT * (held + gap) * 2);
    for key in 0..MANUAL_KEY_COUNT {
        let note = MANUAL_FIRST_NOTE + key as u8;
        let _ = engine.note_on_part(OrganPart::Upper, note, 1.0);
        for _ in 0..held {
            output.extend_from_slice(&engine.next_sample());
        }
        engine.all_notes_off();
        for _ in 0..gap {
            output.extend_from_slice(&engine.next_sample());
        }
    }
    output
}

/// Renders one expression capture: the pedal parked at a documented
/// position, every drawbar out, two keys two octaves apart, with the console
/// drive left where the calibration render puts it so the reading includes the
/// stages the pedal sits between.
pub fn render_expression_phrase(capture: &ExpressionCapture) -> Vec<f32> {
    let frames = SAMPLE_RATE as usize * PHRASE_SECONDS;
    let onset = SAMPLE_RATE as usize / 4;
    let release = SAMPLE_RATE as usize * 3;
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).expect("valid engine");
    for drawbar in 0..DRAWBAR_COUNT {
        let _ = engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, drawbar, 8);
        let _ = engine.set_manual_drawbar(OrganPart::Lower, Registration::AdjustB, drawbar, 0);
    }
    for drawbar in 0..PEDAL_DRAWBAR_COUNT {
        let _ = engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, drawbar, 0);
    }
    let _ = engine.set_output_level(1.0);
    let _ = engine.set_transformer(CHARACTER.0, CHARACTER.1);
    let _ = engine.set_console(0.0, 0.0, 0.0);
    let _ = engine.set_expression_character(EXPRESSION_CHARACTER);
    let _ = engine.set_expression(capture.position);
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    engine.set_rotary_mode(RotaryMode::Off);

    let mut output = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        if frame == onset {
            for note in EXPRESSION_NOTES {
                let _ = engine.note_on_part(OrganPart::Upper, note, 1.0);
            }
        }
        if frame == release {
            engine.all_notes_off();
        }
        output.extend_from_slice(&engine.next_sample());
    }
    output
}

/// Renders one percussion capture with the same timing as the rest of the
/// suite: contact at 250 ms, release at three seconds, four seconds total.
pub fn render_percussion_phrase(capture: &PercussionCapture) -> Vec<f32> {
    let frames = SAMPLE_RATE as usize * PHRASE_SECONDS;
    let onset = SAMPLE_RATE as usize / 4;
    let release = SAMPLE_RATE as usize * 3;
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).expect("valid engine");
    for manual in [OrganPart::Upper, OrganPart::Lower] {
        for drawbar in 0..DRAWBAR_COUNT {
            let _ = engine.set_manual_drawbar(manual, Registration::AdjustB, drawbar, 0);
        }
    }
    for drawbar in 0..PEDAL_DRAWBAR_COUNT {
        let _ = engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, drawbar, 0);
    }
    for (drawbar, position) in PERCUSSION_DRAWBARS.into_iter().enumerate() {
        let _ =
            engine.set_manual_drawbar(OrganPart::Upper, Registration::AdjustB, drawbar, position);
    }
    let _ = engine.set_output_level(1.0);
    let _ = engine.set_transformer(CHARACTER.0, CHARACTER.1);
    let _ = engine.set_console(0.0, 0.0, 0.0);
    let _ = engine.set_expression_character(0.0);
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    engine.set_rotary_mode(RotaryMode::Off);
    engine.set_percussion_enabled(true);
    engine.set_percussion_harmonic(PercussionHarmonic::Third);
    engine.set_percussion_volume(capture.volume);
    engine.set_percussion_decay(capture.decay);

    let mut output = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        if frame == onset {
            let _ = engine.note_on_part(OrganPart::Upper, PERCUSSION_NOTE, 1.0);
        }
        if frame == release {
            engine.all_notes_off();
        }
        output.extend_from_slice(&engine.next_sample());
    }
    output
}

/// Renders one complete transformer capture: four seconds, contact at 250 ms,
/// release at 3 s, interleaved stereo.
pub fn render_transformer_phrase(capture: &TransformerCapture, trims: Trims) -> Vec<f32> {
    let frames = SAMPLE_RATE as usize * PHRASE_SECONDS;
    let onset = SAMPLE_RATE as usize / 4;
    let release = SAMPLE_RATE as usize * 3;
    let mut output = Vec::with_capacity(frames * 2);
    match capture.path.manual() {
        Some(part) => {
            let mut engine = manual_engine(capture, trims);
            for frame in 0..frames {
                if frame == onset {
                    press(&mut engine, part, capture.notes);
                }
                if frame == release {
                    engine.all_notes_off();
                }
                output.extend_from_slice(&engine.next_sample());
            }
        }
        None => {
            let mut transformer = injection_transformer(trims);
            for frame in 0..frames {
                let sample = transformer.process(injection_input(capture, frame, onset, release));
                output.extend_from_slice(&[sample, sample]);
            }
        }
    }
    output
}

/// Renders only the steady state of a capture, skipping the attack. The
/// fitting sweep uses this to evaluate candidate trims without paying for a
/// full four-second phrase per evaluation.
pub fn transformer_steady_frames(
    capture: &TransformerCapture,
    trims: Trims,
    settle_frames: usize,
    measure_frames: usize,
) -> Vec<[f64; 2]> {
    let mut output = Vec::with_capacity(measure_frames);
    match capture.path.manual() {
        Some(part) => {
            let mut engine = manual_engine(capture, trims);
            press(&mut engine, part, capture.notes);
            for frame in 0..settle_frames + measure_frames {
                let [left, right] = engine.next_sample();
                if frame >= settle_frames {
                    output.push([f64::from(left), f64::from(right)]);
                }
            }
        }
        None => {
            let mut transformer = injection_transformer(trims);
            let release = settle_frames + measure_frames + 1;
            for frame in 0..settle_frames + measure_frames {
                let sample = transformer.process(injection_input(capture, frame, 0, release));
                if frame >= settle_frames {
                    output.push([f64::from(sample), f64::from(sample)]);
                }
            }
        }
    }
    output
}

fn manual_engine(capture: &TransformerCapture, trims: Trims) -> OrganEngine {
    let part = capture.path.manual().expect("manual capture");
    let mut engine = OrganEngine::new(SAMPLE_RATE as f32).expect("valid engine");
    for manual in [OrganPart::Upper, OrganPart::Lower] {
        for drawbar in 0..DRAWBAR_COUNT {
            let _ = engine.set_manual_drawbar(manual, Registration::AdjustB, drawbar, 0);
        }
    }
    for drawbar in 0..PEDAL_DRAWBAR_COUNT {
        let _ = engine.set_manual_drawbar(OrganPart::Pedal, Registration::AdjustB, drawbar, 0);
    }
    let _ = engine.set_manual_drawbar(part, Registration::AdjustB, 2, capture.level.drawbar());
    let _ = engine.set_output_level(1.0);
    let _ = engine.set_transformer(CHARACTER.0, CHARACTER.1);
    for unit in TransformerUnit::ALL {
        let (drive, hysteresis) = trims[unit.index()];
        let _ = engine.set_transformer_trim(unit, drive, hysteresis);
    }
    let _ = engine.set_console(0.0, 0.0, 0.0);
    let _ = engine.set_expression_character(0.0);
    engine.set_scanner_mode(ScannerMode::Off);
    engine.set_scanner_manuals(false, false);
    engine.set_rotary_mode(RotaryMode::Off);
    engine
}

fn injection_transformer(trims: Trims) -> MatchingTransformer {
    let mut transformer = MatchingTransformer::new(SAMPLE_RATE as f32);
    let (drive_trim, hysteresis_trim) = trims[TransformerUnit::T3.index()];
    let drive = (CHARACTER.0 + drive_trim).clamp(0.0, 1.0);
    let hysteresis = (CHARACTER.1 + hysteresis_trim).clamp(0.0, 1.0);
    let applied = transformer.set(drive, hysteresis);
    debug_assert!(applied);
    transformer
}

fn injection_input(
    capture: &TransformerCapture,
    frame: usize,
    onset: usize,
    release: usize,
) -> f32 {
    let time = frame as f64 / f64::from(SAMPLE_RATE);
    let amplitude = capture.level.injection_amplitude() * injection_gate(frame, onset, release);
    let mut input = 0.0;
    if capture.notes.plays_c() {
        input += amplitude * (TAU * note_frequency(C_NOTE) * time).sin();
    }
    if capture.notes.plays_f() {
        input += amplitude * (TAU * note_frequency(F_NOTE) * time).sin();
    }
    input as f32
}

fn injection_gate(frame: usize, onset: usize, release: usize) -> f64 {
    let ramp = (INJECTION_RAMP_SECONDS * f64::from(SAMPLE_RATE)) as usize;
    if frame < onset || frame >= release + ramp {
        return 0.0;
    }
    let rising = ((frame - onset) as f64 / ramp as f64).min(1.0);
    let falling = if frame >= release {
        1.0 - (frame - release) as f64 / ramp as f64
    } else {
        1.0
    };
    let gate = rising.min(falling).clamp(0.0, 1.0);
    0.5 - 0.5 * (PI * gate).cos()
}

fn press(engine: &mut OrganEngine, part: OrganPart, notes: Notes) {
    if notes.plays_c() {
        let _ = engine.note_on_part(part, C_NOTE, 1.0);
    }
    if notes.plays_f() {
        let _ = engine.note_on_part(part, F_NOTE, 1.0);
    }
}

/// Builds a trim set that offsets a single unit.
pub fn trim_for(base: Trims, unit: TransformerUnit, drive: f32, hysteresis: f32) -> Trims {
    let mut trims = base;
    trims[unit.index()] = (drive, hysteresis);
    trims
}

/// The generator frequency reached by the 8-foot contact of a manual key.
pub fn note_frequency(note: u8) -> f64 {
    bus_frequency(note, 2)
}

/// The generator frequency reached by one drawbar contact of a manual key.
/// The gear ratios are not exact harmonic multiples, so the third-harmonic
/// percussion has to be looked up rather than computed as three times the
/// fundamental.
pub fn bus_frequency(note: u8, bus: usize) -> f64 {
    let key = usize::from(note - MANUAL_FIRST_NOTE);
    let wheel = drawbar_wheel(key, bus).expect("manual contact");
    f64::from(gear_frequency(wheel).expect("bounded wheel"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_capture_grid_is_complete_and_uniquely_named() {
        let names = capture_names();
        assert_eq!(names.len(), 46);
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
        for path in TransformerPath::ALL {
            for level in Level::ALL {
                for notes in Notes::ALL {
                    let capture = transformer_capture(path, level, notes);
                    assert!(capture.id.contains(level.label()));
                    assert!(capture.id.ends_with(notes.label()));
                }
            }
        }
    }

    #[test]
    fn injection_captures_are_gated_like_a_keyed_note() {
        let capture = transformer_capture(TransformerPath::Injection, Level::High, Notes::Dyad);
        let samples = render_transformer_phrase(capture, NO_TRIM);
        let onset = SAMPLE_RATE as usize / 4;
        assert!(samples[..onset * 2].iter().all(|sample| *sample == 0.0));
        let steady = (onset + SAMPLE_RATE as usize / 2) * 2;
        assert!(
            samples[steady..steady + 2_000]
                .iter()
                .any(|sample| sample.abs() > 0.05)
        );
        // The gate closes at three seconds; what remains is the decaying
        // magnetic state of the transformer itself.
        let after_release = (SAMPLE_RATE as usize * 3 + SAMPLE_RATE as usize / 10) * 2;
        assert!(
            samples[after_release..]
                .iter()
                .all(|sample| sample.abs() < 1.0e-6)
        );
    }
}
