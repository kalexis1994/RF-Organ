// SPDX-License-Identifier: GPL-2.0-or-later
use rf_organ_dsp::{
    DRAWBAR_COUNT, LeslieMode, OrganEngine, OrganPart, PEDAL_DRAWBAR_COUNT, PercussionDecay,
    PercussionHarmonic, PercussionVolume, ScannerMode, TransformerUnit,
};

pub const PARAMETER_COUNT: usize = 51;
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
    pub transformer_drive: f64,
    pub transformer_hysteresis: f64,
    pub leslie_mode: LeslieMode,
    pub leslie_mix: f64,
    pub leslie_acceleration: f64,
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
    pub leslie_mic_distance: f64,
    pub leslie_stereo_width: f64,
    pub leslie_reflections: f64,
    pub leslie_horn_drum_balance: f64,
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
            transformer_drive: 0.38,
            transformer_hysteresis: 0.32,
            leslie_mode: LeslieMode::Off,
            leslie_mix: 0.82,
            leslie_acceleration: 0.5,
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
            leslie_mic_distance: 0.35,
            leslie_stereo_width: 0.75,
            leslie_reflections: 0.22,
            leslie_horn_drum_balance: 0.0,
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
            && unit(self.transformer_drive)
            && unit(self.transformer_hysteresis)
            && unit(self.leslie_mix)
            && unit(self.leslie_acceleration)
            && unit(self.console_drive)
            && bipolar(self.console_bass)
            && bipolar(self.console_treble)
            && unit(self.expression_character)
            && unit(self.leslie_mic_distance)
            && unit(self.leslie_stereo_width)
            && unit(self.leslie_reflections)
            && bipolar(self.leslie_horn_drum_balance)
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
            16 => f64::from(self.leslie_mode as u8),
            17 => self.leslie_mix,
            18 => self.leslie_acceleration,
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
            41 => self.leslie_mic_distance,
            42 => self.leslie_stereo_width,
            43 => self.leslie_reflections,
            44 => self.leslie_horn_drum_balance,
            45..=50 => {
                let trim = (index - TRANSFORMER_TRIM_FIRST) as usize;
                self.transformer_trims[trim / 2][trim % 2]
            }
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
                self.leslie_mode = LeslieMode::from_index(value as u8)?;
            }
            17 => self.leslie_mix = value,
            18 => self.leslie_acceleration = value,
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
            41 => self.leslie_mic_distance = value,
            42 => self.leslie_stereo_width = value,
            43 => self.leslie_reflections = value,
            44 => self.leslie_horn_drum_balance = value,
            45..=50 => {
                let trim = (index - TRANSFORMER_TRIM_FIRST) as usize;
                self.transformer_trims[trim / 2][trim % 2] = value;
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
        engine.set_leslie_mode(self.leslie_mode);
        let _ = engine.set_leslie_mix(self.leslie_mix as f32);
        let _ = engine.set_leslie_acceleration(self.leslie_acceleration as f32);
        let _ = engine.set_leslie_cabinet(
            self.leslie_mic_distance as f32,
            self.leslie_stereo_width as f32,
            self.leslie_reflections as f32,
            self.leslie_horn_drum_balance as f32,
        );
        engine.set_scanner_mode(self.scanner_mode);
        engine.set_scanner_manuals(self.upper_scanner, self.lower_scanner);
        engine.set_percussion_enabled(self.percussion_enabled);
        engine.set_percussion_harmonic(self.percussion_harmonic);
        engine.set_percussion_volume(self.percussion_volume);
        engine.set_percussion_decay(self.percussion_decay);
    }
}

pub fn presets() -> [(&'static str, &'static str, &'static str, Settings); 4] {
    let straight = Settings::default();
    [
        (
            "straight-888",
            "Straight 888",
            "Upper manual, 888 registration, stationary direct output.",
            straight,
        ),
        (
            "chorale-888",
            "Chorale 888",
            "888 registration through the integrated slow rotary cabinet.",
            Settings {
                leslie_mode: LeslieMode::Chorale,
                scanner_mode: ScannerMode::Chorus3,
                upper_scanner: true,
                leslie_mic_distance: 0.5,
                leslie_stereo_width: 0.68,
                ..straight
            },
        ),
        (
            "tremolo-jazz",
            "Tremolo Jazz",
            "Percussive jazz registration with fast integrated rotary motion.",
            Settings {
                drawbars: [8, 8, 8, 0, 0, 0, 0, 8, 0],
                leslie_mode: LeslieMode::Tremolo,
                transformer_drive: 0.52,
                console_drive: 0.44,
                percussion_enabled: true,
                percussion_harmonic: PercussionHarmonic::Third,
                percussion_volume: PercussionVolume::Normal,
                percussion_decay: PercussionDecay::Fast,
                leslie_mic_distance: 0.2,
                leslie_stereo_width: 0.9,
                leslie_reflections: 0.16,
                leslie_horn_drum_balance: 0.12,
                ..straight
            },
        ),
        (
            "gospel-full",
            "Gospel Full",
            "Fuller drawbar registration, transformer drive and slow rotary motion.",
            Settings {
                drawbars: [8, 8, 8, 8, 6, 8, 4, 8, 6],
                leslie_mode: LeslieMode::Chorale,
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
                leslie_mic_distance: 0.42,
                leslie_stereo_width: 0.78,
                leslie_reflections: 0.3,
                leslie_horn_drum_balance: -0.08,
                ..straight
            },
        ),
    ]
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
