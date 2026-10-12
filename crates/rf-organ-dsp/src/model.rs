// SPDX-License-Identifier: GPL-2.0-or-later
//! The model's unmeasured numbers, as values an engine is built with.
//!
//! The plugin builds every engine from `Model::BASELINE`, which is the named
//! constants exactly, so nothing it plays depends on this module existing. The
//! laboratory builds engines from a model with one of them moved, which is how
//! it finds out which of the unknowns the sound actually depends on.

use core::ops::{Index, IndexMut};

macro_rules! knobs {
    ($($knob:ident => $module:ident :: $constant:ident,)*) => {
        /// One of the model's constants that rests on something softer than a
        /// document: a measurement's spread, another model, or a choice.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Knob {
            $($knob,)*
        }

        impl Knob {
            pub const ALL: [Self; KNOB_COUNT] = [$(Self::$knob,)*];

            /// The constant's name in the provenance table.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$knob => concat!(stringify!($module), "::", stringify!($constant)),)*
                }
            }
        }

        const KNOB_COUNT: usize = [$(stringify!($knob),)*].len();

        impl Model {
            /// The named constants, as written.
            pub const BASELINE: Self = Self([$(crate::$module::$constant,)*]);
        }
    };
}

/// A value for every knob.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model([f32; KNOB_COUNT]);

impl Default for Model {
    fn default() -> Self {
        Self::BASELINE
    }
}

impl Index<Knob> for Model {
    type Output = f32;

    fn index(&self, knob: Knob) -> &f32 {
        &self.0[knob as usize]
    }
}

impl IndexMut<Knob> for Model {
    fn index_mut(&mut self, knob: Knob) -> &mut f32 {
        &mut self.0[knob as usize]
    }
}

impl Model {
    /// This model with one knob moved.
    #[must_use]
    pub fn with(mut self, knob: Knob, value: f32) -> Self {
        self[knob] = value;
        self
    }
}

knobs! {
    EccentricityDepth => tonewheel::ECCENTRICITY_DEPTH,
    LowOctaveThird => tonewheel::LOW_OCTAVE_THIRD,
    SecondOctaveThird => tonewheel::SECOND_OCTAVE_THIRD,
    LowOctaveFifth => tonewheel::LOW_OCTAVE_FIFTH,
    WowRateHz => tonewheel::WOW_RATE_HZ,
    WowDepth => tonewheel::WOW_DEPTH,
    FlutterDepth => tonewheel::FLUTTER_DEPTH,
    CouplingHz => tonewheel::COUPLING_HZ,
    CouplingDamping => tonewheel::COUPLING_DAMPING,
    LeakageCeiling => manual::LEAKAGE_CEILING,
    PressSpreadBriskS => manual::PRESS_SPREAD_BRISK_S,
    PressSpreadGentleS => manual::PRESS_SPREAD_GENTLE_S,
    BounceGentleS => manual::BOUNCE_GENTLE_S,
    BounceHardS => manual::BOUNCE_HARD_S,
    BounceRedrawHz => manual::BOUNCE_REDRAW_HZ,
    BounceRestitution => manual::BOUNCE_RESTITUTION,
    ReleaseSpreadS => manual::RELEASE_SPREAD_S,
    ReleaseClickShare => manual::RELEASE_CLICK_SHARE,
    KeyClickSoftS => manual::KEY_CLICK_SOFT_S,
    LeakagePerKey => manual::LEAKAGE_PER_KEY,
    ReleaseVelocityFloor => pedal::RELEASE_VELOCITY_FLOOR,
    L20CornerHz => pedal::L20_CORNER_HZ,
    PedalBusGain => pedal::PEDAL_BUS_GAIN,
    ManualBusGain => manual::MANUAL_BUS_GAIN,
    PercussionAttackS => percussion::ATTACK_SECONDS,
    PercussionRecoveryS => percussion::RECOVERY_SECONDS,
    PercussionSoftLevel => percussion::SOFT_LEVEL,
    PercussionNormalLevel => percussion::NORMAL_LEVEL,
    PercussionSlowT60S => percussion::SLOW_T60_S,
    PercussionFastT60S => percussion::FAST_T60_S,
    CoreMemoryHz => transformer::MEMORY_HZ,
    CoreDriveGain => transformer::DRIVE_GAIN,
    CoreMemoryBias => transformer::MEMORY_BIAS,
    CoreBendFloor => transformer::BEND_FLOOR,
    CoreBendSpan => transformer::BEND_SPAN,
    ScannerRotorHz => vibrato_line::ROTOR_HZ,
    ExpressionHighHz => electronics::EXPRESSION_HIGH_HZ,
    ExpressionLowShare => electronics::EXPRESSION_LOW_SHARE,
    ExpressionHighShare => electronics::EXPRESSION_HIGH_SHARE,
    ExpressionSlewS => electronics::EXPRESSION_SLEW_S,
    BassShelfHz => electronics::BASS_SHELF_HZ,
    V4aAsymmetry => electronics::V4A_ASYMMETRY,
    V4bAsymmetry => electronics::V4B_ASYMMETRY,
    V3bAsymmetry => electronics::V3B_ASYMMETRY,
    V4aDriveShare => electronics::V4A_DRIVE_SHARE,
    V4bDriveShare => electronics::V4B_DRIVE_SHARE,
    V3bDriveShare => electronics::V3B_DRIVE_SHARE,
    StageDriveGain => electronics::STAGE_DRIVE_GAIN,
    StageMakeup => electronics::STAGE_MAKEUP,
    DcBlockHz => electronics::DC_BLOCK_HZ,
    HornSlowRpm => rotary::HORN_SLOW_RPM,
    HornFastRpm => rotary::HORN_FAST_RPM,
    DrumSlowRpm => rotary::DRUM_SLOW_RPM,
    DrumFastRpm => rotary::DRUM_FAST_RPM,
    HornRiseS => rotary::HORN_RISE_SECONDS,
    HornFallS => rotary::HORN_FALL_SECONDS,
    HornBrakeS => rotary::HORN_BRAKE_SECONDS,
    DrumRiseS => rotary::DRUM_RISE_SECONDS,
    DrumFallS => rotary::DRUM_FALL_SECONDS,
    DrumBrakeS => rotary::DRUM_BRAKE_SECONDS,
    AccelerationBriskest => rotary::ACCELERATION_BRISKEST,
    AccelerationSpan => rotary::ACCELERATION_SPAN,
    ModeDelayS => rotary::MODE_DELAY_SECONDS,
    CrossoverHz => rotary::CROSSOVER_HZ,
    HornShelfHz => rotary::HORN_SHELF_HZ,
    HornBodyAway => rotary::HORN_BODY_AWAY,
    HornBodyFacing => rotary::HORN_BODY_FACING,
    HornTopAway => rotary::HORN_TOP_AWAY,
    HornTopFacing => rotary::HORN_TOP_FACING,
    DrumAway => rotary::DRUM_AWAY,
    DrumFacing => rotary::DRUM_FACING,
    ReflectionLevel => rotary::REFLECTION_LEVEL,
    SubIntoHorn => rotary::SUB_INTO_HORN,
    DynamicPresenceHz => rotary::DYNAMIC_PRESENCE_HZ,
    DynamicPresenceLift => rotary::DYNAMIC_PRESENCE_LIFT,
    DynamicTopHz => rotary::DYNAMIC_TOP_HZ,
    ProximityMaxLift => rotary::PROXIMITY_MAX_LIFT,
}
