// SPDX-License-Identifier: GPL-2.0-or-later
use rf_organ_dsp::{
    ConsoleStage, DRAWBAR_COUNT, DRUM_RADIUS_DEFAULT_M, DRUM_RADIUS_RANGE_M, HORN_RADIUS_DEFAULT_M,
    HORN_RADIUS_RANGE_M, LEAKAGE_BOOST_DEFAULT, LEVEL_RANGE_DB, LEVEL_SILENT_DB,
    MIC_DISTANCE_DEFAULT_M, MIC_DISTANCE_RANGE_M, MIC_OFFSET_MAX_M, MIC_PATTERN_DEFAULT,
    MIC_SPACING_DEFAULT_M, MIC_SPACING_MAX_M, MainsFrequency, MicrophoneArray, MicrophonePair,
    MicrophoneType, OrganEngine, OrganPart, PEDAL_DRAWBAR_COUNT, PercussionDecay,
    PercussionHarmonic, PercussionVolume, RotaryMode, STAGE_CHARACTER_DEFAULT,
    STAGE_CHARACTER_RANGE, SUB_LEVEL_DEFAULT_DB, ScannerMode, StopAngle, TransformerUnit,
};

pub const PARAMETER_COUNT: usize = 72;
/// Per-stage offsets from the shared console character, ordered V4A, V4B, V3B.
pub const CONSOLE_STAGE_TRIM_FIRST: u32 = 69;
/// Bipolar per-transformer calibration trims, ordered T1, T2, T3.
pub const TRANSFORMER_TRIM_FIRST: u32 = 45;
pub const DRAWBAR_FIRST: u32 = 2;
pub const DRAWBAR_LAST: u32 = DRAWBAR_FIRST + DRAWBAR_COUNT as u32 - 1;
pub const LOWER_DRAWBAR_FIRST: u32 = 24;
pub const LOWER_DRAWBAR_LAST: u32 = LOWER_DRAWBAR_FIRST + DRAWBAR_COUNT as u32 - 1;
pub const PEDAL_DRAWBAR_FIRST: u32 = 33;
pub const PEDAL_DRAWBAR_LAST: u32 = PEDAL_DRAWBAR_FIRST + PEDAL_DRAWBAR_COUNT as u32 - 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub output_level: f64,
    pub expression: f64,
    pub drawbars: [u8; DRAWBAR_COUNT],
    pub contact_spread: f64,
    pub contact_bounce: f64,
    pub leakage: f64,
    /// How far the resiliently coupled drive is allowed to stray from its
    /// nominal speed.
    pub drive_wobble: f64,
    /// How fast the leakage grows as more keys go down.
    pub leakage_boost: f64,
    /// How lopsided the preamplifier's three stages are, together, and how
    /// each of them sits against that.
    pub console_stage_character: f64,
    pub console_stage_trims: [f64; 3],
    pub transformer_drive: f64,
    pub transformer_hysteresis: f64,
    pub rotary_mode: RotaryMode,
    pub rotary_mix: f64,
    pub rotary_acceleration: f64,
    pub scanner_mode: ScannerMode,
    pub percussion_enabled: bool,
    pub percussion_harmonic: PercussionHarmonic,
    pub percussion_volume: PercussionVolume,
    pub percussion_decay: PercussionDecay,
    pub lower_drawbars: [u8; DRAWBAR_COUNT],
    pub pedal_drawbars: [u8; PEDAL_DRAWBAR_COUNT],
    pub upper_scanner: bool,
    pub lower_scanner: bool,
    pub console_drive: f64,
    pub console_bass: f64,
    pub console_treble: f64,
    pub expression_character: f64,
    /// Microphone placement in metres, as a tape measure would give it.
    pub rotary_horn_mic_distance: f64,
    pub rotary_horn_mic_spacing: f64,
    pub rotary_horn_mic_offset: f64,
    /// Hammond's "Side": the pair goes beyond the cabinet's flanks instead of
    /// standing in front of it.
    pub rotary_horn_mic_sides: bool,
    pub rotary_drum_mic_distance: f64,
    pub rotary_drum_mic_spacing: f64,
    pub rotary_drum_mic_offset: f64,
    pub rotary_drum_mic_sides: bool,
    /// Omnidirectional at zero, cardioid at a half, figure of eight at one.
    pub rotary_mic_pattern: f64,
    /// The radius each rotor's mouth turns at, in metres. Undocumented, so it
    /// is a control; see `THIRD_PARTY_NOTICES.md`.
    pub rotary_horn_radius: f64,
    pub rotary_drum_radius: f64,
    /// Where each rotor comes to rest, in degrees, with one step past the end
    /// of the circle standing for a random angle, as Hammond encodes it.
    pub rotary_horn_stop_angle: f64,
    pub rotary_drum_stop_angle: f64,
    /// The supply the cabinet's motors run from, which is what its rated
    /// speeds are rated against.
    pub rotary_mains: MainsFrequency,
    /// The woofer's own unmodulated bass, in decibels, and which capsule the
    /// pair is.
    pub rotary_sub_level: f64,
    pub rotary_microphone_type: MicrophoneType,
    pub rotary_reflections: f64,
    /// The horn's and the drum's microphone volumes, in decibels.
    pub rotary_horn_level: f64,
    pub rotary_drum_level: f64,
    pub transformer_trims: [[f64; 2]; 3],
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_level: 0.72,
            expression: 1.0,
            drawbars: [8, 8, 8, 0, 0, 0, 0, 0, 0],
            contact_spread: 0.55,
            contact_bounce: 0.45,
            leakage: 0.2,
            drive_wobble: 1.0,
            leakage_boost: LEAKAGE_BOOST_DEFAULT as f64,
            console_stage_character: STAGE_CHARACTER_DEFAULT as f64,
            console_stage_trims: [0.0; 3],
            transformer_drive: 0.38,
            transformer_hysteresis: 0.32,
            rotary_mode: RotaryMode::Off,
            rotary_mix: 0.82,
            rotary_acceleration: 0.5,
            scanner_mode: ScannerMode::Off,
            percussion_enabled: false,
            percussion_harmonic: PercussionHarmonic::Third,
            percussion_volume: PercussionVolume::Normal,
            percussion_decay: PercussionDecay::Fast,
            lower_drawbars: [8, 8, 8, 0, 0, 0, 0, 0, 0],
            pedal_drawbars: [8, 0],
            upper_scanner: true,
            lower_scanner: false,
            console_drive: 0.32,
            console_bass: 0.0,
            console_treble: 0.0,
            expression_character: 0.55,
            rotary_horn_mic_distance: MIC_DISTANCE_DEFAULT_M as f64,
            rotary_horn_mic_spacing: MIC_SPACING_DEFAULT_M as f64,
            rotary_horn_mic_offset: 0.0,
            rotary_horn_mic_sides: false,
            rotary_drum_mic_distance: MIC_DISTANCE_DEFAULT_M as f64,
            rotary_drum_mic_spacing: MIC_SPACING_DEFAULT_M as f64,
            rotary_drum_mic_offset: 0.0,
            rotary_drum_mic_sides: false,
            rotary_mic_pattern: MIC_PATTERN_DEFAULT as f64,
            rotary_horn_radius: HORN_RADIUS_DEFAULT_M as f64,
            rotary_drum_radius: DRUM_RADIUS_DEFAULT_M as f64,
            rotary_horn_stop_angle: 0.0,
            rotary_drum_stop_angle: 0.0,
            rotary_mains: MainsFrequency::Sixty,
            rotary_sub_level: SUB_LEVEL_DEFAULT_DB as f64,
            rotary_microphone_type: MicrophoneType::Condenser,
            rotary_reflections: 0.22,
            rotary_horn_level: 0.0,
            rotary_drum_level: 0.0,
            transformer_trims: [[0.0; 2]; 3],
        }
    }
}

impl Settings {
    pub fn valid(self) -> bool {
        finite_range(self.output_level, 0.0, 1.5)
            && unit(self.expression)
            && self.drawbars.iter().all(|position| *position <= 8)
            && self.lower_drawbars.iter().all(|position| *position <= 8)
            && self.pedal_drawbars.iter().all(|position| *position <= 8)
            && unit(self.contact_spread)
            && unit(self.contact_bounce)
            && unit(self.leakage)
            && unit(self.drive_wobble)
            && unit(self.leakage_boost)
            && finite_range(
                self.console_stage_character,
                f64::from(STAGE_CHARACTER_RANGE.0),
                f64::from(STAGE_CHARACTER_RANGE.1),
            )
            && self.console_stage_trims.iter().all(|trim| bipolar(*trim))
            && unit(self.transformer_drive)
            && unit(self.transformer_hysteresis)
            && unit(self.rotary_mix)
            && unit(self.rotary_acceleration)
            && unit(self.console_drive)
            && bipolar(self.console_bass)
            && bipolar(self.console_treble)
            && unit(self.expression_character)
            && placement(
                self.rotary_horn_mic_distance,
                self.rotary_horn_mic_spacing,
                self.rotary_horn_mic_offset,
            )
            && placement(
                self.rotary_drum_mic_distance,
                self.rotary_drum_mic_spacing,
                self.rotary_drum_mic_offset,
            )
            && unit(self.rotary_mic_pattern)
            && finite_range(
                self.rotary_horn_radius,
                HORN_RADIUS_RANGE_M.0 as f64,
                HORN_RADIUS_RANGE_M.1 as f64,
            )
            && finite_range(
                self.rotary_drum_radius,
                DRUM_RADIUS_RANGE_M.0 as f64,
                DRUM_RADIUS_RANGE_M.1 as f64,
            )
            && finite_range(self.rotary_horn_stop_angle, 0.0, 360.0)
            && finite_range(self.rotary_drum_stop_angle, 0.0, 360.0)
            && level(self.rotary_sub_level)
            && unit(self.rotary_reflections)
            && level(self.rotary_horn_level)
            && level(self.rotary_drum_level)
            && self
                .transformer_trims
                .iter()
                .flatten()
                .all(|trim| bipolar(*trim))
    }

    pub fn parameter(self, index: u32) -> Option<f64> {
        Some(match index {
            0 => self.output_level,
            1 => self.expression,
            DRAWBAR_FIRST..=DRAWBAR_LAST => {
                f64::from(self.drawbars[(index - DRAWBAR_FIRST) as usize])
            }
            11 => self.contact_spread,
            12 => self.contact_bounce,
            13 => self.leakage,
            14 => self.transformer_drive,
            15 => self.transformer_hysteresis,
            16 => f64::from(self.rotary_mode as u8),
            17 => self.rotary_mix,
            18 => self.rotary_acceleration,
            19 => f64::from(self.scanner_mode as u8),
            20 => {
                if self.percussion_enabled {
                    1.0
                } else {
                    0.0
                }
            }
            21 => f64::from(self.percussion_harmonic as u8),
            22 => f64::from(self.percussion_volume as u8),
            23 => f64::from(self.percussion_decay as u8),
            LOWER_DRAWBAR_FIRST..=LOWER_DRAWBAR_LAST => {
                f64::from(self.lower_drawbars[(index - LOWER_DRAWBAR_FIRST) as usize])
            }
            PEDAL_DRAWBAR_FIRST..=PEDAL_DRAWBAR_LAST => {
                f64::from(self.pedal_drawbars[(index - PEDAL_DRAWBAR_FIRST) as usize])
            }
            35 => bool_value(self.upper_scanner),
            36 => bool_value(self.lower_scanner),
            37 => self.console_drive,
            38 => self.console_bass,
            39 => self.console_treble,
            40 => self.expression_character,
            41 => self.rotary_horn_mic_distance,
            42 => self.rotary_horn_mic_spacing,
            43 => self.rotary_reflections,
            44 => self.rotary_horn_level,
            45..=50 => {
                let trim = (index - TRANSFORMER_TRIM_FIRST) as usize;
                self.transformer_trims[trim / 2][trim % 2]
            }
            51 => self.rotary_horn_mic_offset,
            61 => self.rotary_drum_mic_distance,
            62 => self.rotary_drum_mic_spacing,
            63 => self.rotary_drum_mic_offset,
            64 => bool_value(self.rotary_horn_mic_sides),
            65 => bool_value(self.rotary_drum_mic_sides),
            66 => self.drive_wobble,
            67 => self.leakage_boost,
            68 => self.console_stage_character,
            69..=71 => self.console_stage_trims[(index - CONSOLE_STAGE_TRIM_FIRST) as usize],
            52 => self.rotary_mic_pattern,
            53 => self.rotary_horn_radius,
            54 => self.rotary_drum_radius,
            55 => self.rotary_horn_stop_angle,
            56 => self.rotary_drum_stop_angle,
            57 => f64::from(self.rotary_mains as u8),
            58 => self.rotary_sub_level,
            60 => self.rotary_drum_level,
            59 => f64::from(self.rotary_microphone_type as u8),
            _ => return None,
        })
    }

    pub fn with_parameter(mut self, index: u32, value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        match index {
            0 => self.output_level = value,
            1 => self.expression = value,
            DRAWBAR_FIRST..=DRAWBAR_LAST
                if value.fract() == 0.0 && (0.0..=8.0).contains(&value) =>
            {
                self.drawbars[(index - DRAWBAR_FIRST) as usize] = value as u8;
            }
            11 => self.contact_spread = value,
            12 => self.contact_bounce = value,
            13 => self.leakage = value,
            14 => self.transformer_drive = value,
            15 => self.transformer_hysteresis = value,
            16 if value.fract() == 0.0 => {
                self.rotary_mode = RotaryMode::from_index(value as u8)?;
            }
            17 => self.rotary_mix = value,
            18 => self.rotary_acceleration = value,
            19 if value.fract() == 0.0 => {
                self.scanner_mode = ScannerMode::from_index(value as u8)?;
            }
            20 if value == 0.0 || value == 1.0 => self.percussion_enabled = value == 1.0,
            21 if value.fract() == 0.0 => {
                self.percussion_harmonic = PercussionHarmonic::from_index(value as u8)?;
            }
            22 if value.fract() == 0.0 => {
                self.percussion_volume = PercussionVolume::from_index(value as u8)?;
            }
            23 if value.fract() == 0.0 => {
                self.percussion_decay = PercussionDecay::from_index(value as u8)?;
            }
            LOWER_DRAWBAR_FIRST..=LOWER_DRAWBAR_LAST
                if value.fract() == 0.0 && (0.0..=8.0).contains(&value) =>
            {
                self.lower_drawbars[(index - LOWER_DRAWBAR_FIRST) as usize] = value as u8;
            }
            PEDAL_DRAWBAR_FIRST..=PEDAL_DRAWBAR_LAST
                if value.fract() == 0.0 && (0.0..=8.0).contains(&value) =>
            {
                self.pedal_drawbars[(index - PEDAL_DRAWBAR_FIRST) as usize] = value as u8;
            }
            35 if value == 0.0 || value == 1.0 => self.upper_scanner = value == 1.0,
            36 if value == 0.0 || value == 1.0 => self.lower_scanner = value == 1.0,
            37 => self.console_drive = value,
            38 => self.console_bass = value,
            39 => self.console_treble = value,
            40 => self.expression_character = value,
            41 => self.rotary_horn_mic_distance = value,
            42 => self.rotary_horn_mic_spacing = value,
            43 => self.rotary_reflections = value,
            44 => self.rotary_horn_level = value,
            45..=50 => {
                let trim = (index - TRANSFORMER_TRIM_FIRST) as usize;
                self.transformer_trims[trim / 2][trim % 2] = value;
            }
            51 => self.rotary_horn_mic_offset = value,
            61 => self.rotary_drum_mic_distance = value,
            62 => self.rotary_drum_mic_spacing = value,
            63 => self.rotary_drum_mic_offset = value,
            64 if value == 0.0 || value == 1.0 => self.rotary_horn_mic_sides = value == 1.0,
            65 if value == 0.0 || value == 1.0 => self.rotary_drum_mic_sides = value == 1.0,
            66 => self.drive_wobble = value,
            67 => self.leakage_boost = value,
            68 => self.console_stage_character = value,
            69..=71 => {
                self.console_stage_trims[(index - CONSOLE_STAGE_TRIM_FIRST) as usize] = value;
            }
            52 => self.rotary_mic_pattern = value,
            53 => self.rotary_horn_radius = value,
            54 => self.rotary_drum_radius = value,
            55 => self.rotary_horn_stop_angle = value,
            56 => self.rotary_drum_stop_angle = value,
            57 if value.fract() == 0.0 => {
                self.rotary_mains = MainsFrequency::from_index(value as u8)?;
            }
            58 => self.rotary_sub_level = value,
            60 => self.rotary_drum_level = value,
            59 if value.fract() == 0.0 => {
                self.rotary_microphone_type = MicrophoneType::from_index(value as u8)?;
            }
            _ => return None,
        }
        self.valid().then_some(self)
    }

    pub fn apply(self, engine: &mut OrganEngine) {
        debug_assert!(self.valid());
        let _ = engine.set_output_level(self.output_level as f32);
        let _ = engine.set_expression(self.expression as f32);
        for (index, position) in self.drawbars.into_iter().enumerate() {
            let _ = engine.set_manual_drawbar(OrganPart::Upper, index, position);
        }
        for (index, position) in self.lower_drawbars.into_iter().enumerate() {
            let _ = engine.set_manual_drawbar(OrganPart::Lower, index, position);
        }
        for (index, position) in self.pedal_drawbars.into_iter().enumerate() {
            let _ = engine.set_manual_drawbar(OrganPart::Pedal, index, position);
        }
        let _ = engine.set_contact_spread(self.contact_spread as f32);
        let _ = engine.set_contact_bounce(self.contact_bounce as f32);
        let _ = engine.set_leakage(self.leakage as f32);
        let _ = engine.set_drive_wobble(self.drive_wobble as f32);
        let _ = engine.set_leakage_boost(self.leakage_boost as f32);
        let _ = engine.set_console_stage_character(self.console_stage_character as f32);
        for (stage, trim) in ConsoleStage::ALL.into_iter().zip(self.console_stage_trims) {
            let _ = engine.set_console_stage_trim(stage, trim as f32);
        }
        let _ = engine.set_transformer(
            self.transformer_drive as f32,
            self.transformer_hysteresis as f32,
        );
        for (unit, [drive, hysteresis]) in
            TransformerUnit::ALL.into_iter().zip(self.transformer_trims)
        {
            let _ = engine.set_transformer_trim(unit, drive as f32, hysteresis as f32);
        }
        let _ = engine.set_console(
            self.console_drive as f32,
            self.console_bass as f32,
            self.console_treble as f32,
        );
        let _ = engine.set_expression_character(self.expression_character as f32);
        engine.set_rotary_mode(self.rotary_mode);
        let _ = engine.set_rotary_mix(self.rotary_mix as f32);
        let _ = engine.set_rotary_acceleration(self.rotary_acceleration as f32);
        let _ = engine.set_rotary_cabinet(self.rotary_reflections as f32);
        let _ = engine.set_rotary_levels(
            self.rotary_horn_level as f32,
            self.rotary_drum_level as f32,
            self.rotary_sub_level as f32,
        );
        let _ = engine.set_rotary_microphones(MicrophoneArray {
            horn: MicrophonePair {
                distance_m: self.rotary_horn_mic_distance as f32,
                spacing_m: self.rotary_horn_mic_spacing as f32,
                offset_m: self.rotary_horn_mic_offset as f32,
                at_the_sides: self.rotary_horn_mic_sides,
            },
            drum: MicrophonePair {
                distance_m: self.rotary_drum_mic_distance as f32,
                spacing_m: self.rotary_drum_mic_spacing as f32,
                offset_m: self.rotary_drum_mic_offset as f32,
                at_the_sides: self.rotary_drum_mic_sides,
            },
            pattern: self.rotary_mic_pattern as f32,
        });
        let _ = engine.set_rotary_rotor_radii(
            self.rotary_horn_radius as f32,
            self.rotary_drum_radius as f32,
        );
        if let (Some(horn), Some(drum)) = (
            StopAngle::from_degrees(self.rotary_horn_stop_angle as f32),
            StopAngle::from_degrees(self.rotary_drum_stop_angle as f32),
        ) {
            let _ = engine.set_rotary_stop_angles(horn, drum);
        }
        engine.set_mains(self.rotary_mains);
        engine.set_rotary_microphone_type(self.rotary_microphone_type);
        engine.set_scanner_mode(self.scanner_mode);
        engine.set_scanner_manuals(self.upper_scanner, self.lower_scanner);
        engine.set_percussion_enabled(self.percussion_enabled);
        engine.set_percussion_harmonic(self.percussion_harmonic);
        engine.set_percussion_volume(self.percussion_volume);
        engine.set_percussion_decay(self.percussion_decay);
    }
}

pub fn presets() -> [(&'static str, &'static str, &'static str, Settings); 8] {
    let straight = Settings::default();
    [
        (
            "straight-888",
            "Straight 888",
            "Upper manual, 888 registration and stationary direct output.",
            straight,
        ),
        (
            "chorale-888",
            "Chorale 888",
            "888 registration with C3 scanner chorus through the integrated slow rotary cabinet.",
            Settings {
                rotary_mode: RotaryMode::Chorale,
                scanner_mode: ScannerMode::Chorus3,
                upper_scanner: true,
                rotary_horn_mic_distance: 0.9,
                rotary_horn_mic_spacing: 0.24,
                ..straight
            },
        ),
        (
            "tremolo-jazz",
            "Tremolo Jazz",
            "Third-harmonic single-trigger percussion with fast integrated rotary motion.",
            Settings {
                drawbars: [8, 8, 8, 0, 0, 0, 0, 8, 0],
                rotary_mode: RotaryMode::Tremolo,
                transformer_drive: 0.52,
                console_drive: 0.44,
                percussion_enabled: true,
                percussion_harmonic: PercussionHarmonic::Third,
                percussion_volume: PercussionVolume::Normal,
                percussion_decay: PercussionDecay::Fast,
                rotary_horn_mic_distance: 0.25,
                rotary_horn_mic_spacing: 0.38,
                rotary_reflections: 0.16,
                rotary_drum_level: -1.1,
                ..straight
            },
        ),
        (
            "jazz-comp",
            "Jazz Comp",
            "Hollow comping registration with third-harmonic percussion and slow rotary motion.",
            Settings {
                drawbars: [8, 0, 8, 0, 0, 0, 0, 0, 0],
                rotary_mode: RotaryMode::Chorale,
                percussion_enabled: true,
                percussion_harmonic: PercussionHarmonic::Third,
                percussion_volume: PercussionVolume::Normal,
                percussion_decay: PercussionDecay::Fast,
                rotary_horn_mic_distance: 0.5,
                rotary_horn_mic_spacing: 0.28,
                ..straight
            },
        ),
        (
            "ballad-chorus",
            "Ballad Chorus",
            "Fundamental registration through the C3 vibrato line, cabinet stationary.",
            Settings {
                drawbars: [8, 8, 8, 0, 0, 0, 0, 0, 0],
                scanner_mode: ScannerMode::Chorus3,
                upper_scanner: true,
                rotary_mode: RotaryMode::Off,
                output_level: 0.66,
                ..straight
            },
        ),
        (
            "pedal-bass",
            "Pedal Bass",
            "Pedal clavier forward with a quiet lower manual, for left-hand and feet.",
            Settings {
                drawbars: [0, 0, 0, 0, 0, 0, 0, 0, 0],
                lower_drawbars: [8, 4, 6, 0, 0, 0, 0, 0, 0],
                pedal_drawbars: [8, 6],
                rotary_mode: RotaryMode::Off,
                leakage: 0.14,
                ..straight
            },
        ),
        (
            "full-shout",
            "Full Shout",
            "Every drawbar out on both manuals with fast rotary motion and console drive.",
            Settings {
                drawbars: [8, 8, 8, 8, 8, 8, 8, 8, 8],
                lower_drawbars: [8, 8, 8, 0, 0, 0, 0, 0, 0],
                pedal_drawbars: [8, 8],
                rotary_mode: RotaryMode::Tremolo,
                transformer_drive: 0.66,
                console_drive: 0.52,
                rotary_horn_mic_distance: 0.22,
                rotary_horn_mic_spacing: 0.36,
                rotary_reflections: 0.26,
                ..straight
            },
        ),
        (
            "gospel-full",
            "Gospel Full",
            "Full drawbars, C3 scanner chorus, transformer drive and slow rotary motion.",
            Settings {
                drawbars: [8, 8, 8, 8, 6, 8, 4, 8, 6],
                rotary_mode: RotaryMode::Chorale,
                transformer_drive: 0.62,
                console_drive: 0.48,
                console_bass: 0.18,
                console_treble: -0.08,
                leakage: 0.28,
                scanner_mode: ScannerMode::Chorus3,
                lower_drawbars: [8, 8, 8, 8, 6, 0, 0, 0, 0],
                pedal_drawbars: [8, 8],
                upper_scanner: true,
                lower_scanner: true,
                rotary_horn_mic_distance: 0.45,
                rotary_horn_mic_spacing: 0.32,
                rotary_reflections: 0.3,
                rotary_horn_level: -0.7,
                ..straight
            },
        ),
    ]
}

/// One of the three microphone volumes: decibels from unity down to silence.
fn level(value: f64) -> bool {
    finite_range(
        value,
        f64::from(LEVEL_SILENT_DB),
        f64::from(LEVEL_RANGE_DB.1),
    )
}

/// One pair of microphones, inside the ranges Hammond publishes for it.
fn placement(distance: f64, spacing: f64, offset: f64) -> bool {
    finite_range(
        distance,
        f64::from(MIC_DISTANCE_RANGE_M.0),
        f64::from(MIC_DISTANCE_RANGE_M.1),
    ) && finite_range(spacing, 0.0, f64::from(MIC_SPACING_MAX_M))
        && finite_range(
            offset,
            -f64::from(MIC_OFFSET_MAX_M),
            f64::from(MIC_OFFSET_MAX_M),
        )
}

fn unit(value: f64) -> bool {
    finite_range(value, 0.0, 1.0)
}

fn finite_range(value: f64, minimum: f64, maximum: f64) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

fn bipolar(value: f64) -> bool {
    finite_range(value, -1.0, 1.0)
}

fn bool_value(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine's presets and the catalogue the host reads are two lists of
    /// the same thing, so they have to say the same thing.
    #[test]
    fn the_catalogue_lists_every_preset_the_engine_has() {
        let catalogue = include_str!("../../../package/metadata/presets.json");
        for (id, name, description, _) in presets() {
            assert!(
                catalogue.contains(&format!("\"id\": \"{id}\"")),
                "{id} is missing from the catalogue"
            );
            assert!(
                catalogue.contains(name),
                "{name} is missing from the catalogue"
            );
            assert!(
                catalogue.contains(description),
                "{id} has a different description in the catalogue"
            );
        }
        assert_eq!(
            catalogue.matches("\"bank\": \"registrations\"").count(),
            presets().len()
        );
    }

    #[test]
    fn presets_are_valid_and_round_trip_parameters() {
        for (id, _, _, settings) in presets() {
            assert!(settings.valid(), "{id}");
            for index in 0..PARAMETER_COUNT as u32 {
                let value = settings.parameter(index).expect("parameter exists");
                assert_eq!(settings.with_parameter(index, value), Some(settings));
            }
        }
    }
}
