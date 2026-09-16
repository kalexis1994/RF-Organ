// SPDX-License-Identifier: GPL-2.0-or-later
use rf_organ_dsp::{DRAWBAR_COUNT, LeslieMode, OrganEngine};

pub const PARAMETER_COUNT: usize = 19;
pub const DRAWBAR_FIRST: u32 = 2;
pub const DRAWBAR_LAST: u32 = DRAWBAR_FIRST + DRAWBAR_COUNT as u32 - 1;

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
        }
    }
}

impl Settings {
    pub fn valid(self) -> bool {
        finite_range(self.output_level, 0.0, 1.5)
            && unit(self.expression)
            && self.drawbars.iter().all(|position| *position <= 8)
            && unit(self.contact_spread)
            && unit(self.contact_bounce)
            && unit(self.leakage)
            && unit(self.transformer_drive)
            && unit(self.transformer_hysteresis)
            && unit(self.leslie_mix)
            && unit(self.leslie_acceleration)
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
            _ => return None,
        }
        self.valid().then_some(self)
    }

    pub fn apply(self, engine: &mut OrganEngine) {
        debug_assert!(self.valid());
        let _ = engine.set_output_level(self.output_level as f32);
        let _ = engine.set_expression(self.expression as f32);
        for (index, position) in self.drawbars.into_iter().enumerate() {
            let _ = engine.set_drawbar(index, position);
        }
        let _ = engine.set_contact_spread(self.contact_spread as f32);
        let _ = engine.set_contact_bounce(self.contact_bounce as f32);
        let _ = engine.set_leakage(self.leakage as f32);
        let _ = engine.set_transformer(
            self.transformer_drive as f32,
            self.transformer_hysteresis as f32,
        );
        engine.set_leslie_mode(self.leslie_mode);
        let _ = engine.set_leslie_mix(self.leslie_mix as f32);
        let _ = engine.set_leslie_acceleration(self.leslie_acceleration as f32);
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
                leakage: 0.28,
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
