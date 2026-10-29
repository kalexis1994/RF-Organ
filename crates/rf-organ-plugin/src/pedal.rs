// SPDX-License-Identifier: GPL-2.0-or-later
//! The sustain pedal as the cabinet's speed switch.
//!
//! A tonewheel console has no sustain: a note sounds for as long as its key is
//! down. The pedal a controller keyboard carries is therefore free, and the
//! organ clones that use it use it for the cabinet. Hammond's own offer two
//! ways, and this follows them. Alternating, the default, changes speed on
//! each press and stops the rotors when the pedal is held; Hammond's
//! XK-3c documents the stop after a press longer than one second, and its
//! Leslie 3300 after one and a half. Momentary runs the rotors fast for as
//! long as the pedal is down and returns them to the selected speed when it
//! comes up.
//!
//! Nothing here allocates or reads the clock: the hold is counted in the
//! frames the processor renders.

use rf_organ_dsp::RotaryMode;

/// How long the pedal has to stay down before an alternating press becomes a
/// stop, in seconds: the XK-3c's figure.
pub const HOLD_TO_BRAKE_S: f64 = 1.0;

/// What the sustain pedal does.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum SustainPedal {
    /// Ignored, as the console would.
    Off = 0,
    /// Each press changes speed; holding it stops the rotors.
    #[default]
    Alternate = 1,
    /// Fast while held, the selected speed when released.
    Momentary = 2,
}

impl SustainPedal {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Off),
            1 => Some(Self::Alternate),
            2 => Some(Self::Momentary),
            _ => None,
        }
    }
}

/// The pedal's position and the gesture it is part of.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpeedPedal {
    down: bool,
    /// Frames the pedal has been down, while an alternating press can still
    /// become a stop.
    held: Option<u32>,
    /// The selected speed before this press changed it, which a press that
    /// turns into a stop puts back, so that releasing the rotors afterwards
    /// returns to where they were rather than to what the press began.
    before: RotaryMode,
}

/// What a pedal event asks of the cabinet: the speed the rotors return to
/// from now on, and the mode the cabinet is put in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Switch {
    pub selected: RotaryMode,
    pub engaged: RotaryMode,
}

impl SpeedPedal {
    /// The pedal at `down`, given what it does, the speed currently selected
    /// and the mode the cabinet is in. A pedal on a cabinet that is switched
    /// out does nothing, as a switch on a disconnected cabinet would not.
    pub fn set(
        &mut self,
        down: bool,
        mode: SustainPedal,
        selected: RotaryMode,
        engaged: RotaryMode,
    ) -> Option<Switch> {
        if down == self.down {
            return None;
        }
        self.down = down;
        self.held = None;
        if mode == SustainPedal::Off || engaged == RotaryMode::Off {
            return None;
        }
        match (mode, down) {
            (SustainPedal::Alternate, true) => {
                self.held = Some(0);
                self.before = selected;
                // From a stop, a press lets the rotors go at the speed that
                // was selected; otherwise it throws the switch.
                let next = if engaged == RotaryMode::Brake {
                    match selected {
                        RotaryMode::Tremolo => RotaryMode::Tremolo,
                        _ => RotaryMode::Chorale,
                    }
                } else if selected == RotaryMode::Tremolo {
                    RotaryMode::Chorale
                } else {
                    RotaryMode::Tremolo
                };
                Some(Switch {
                    selected: next,
                    engaged: next,
                })
            }
            (SustainPedal::Momentary, true) => Some(Switch {
                selected,
                engaged: RotaryMode::Tremolo,
            }),
            (SustainPedal::Momentary, false) => Some(Switch {
                selected,
                engaged: selected,
            }),
            _ => None,
        }
    }

    /// One rendered frame. An alternating press held for `hold_frames`
    /// stops the rotors, once.
    pub fn tick(&mut self, hold_frames: u32) -> Option<Switch> {
        let held = self.held.as_mut()?;
        *held += 1;
        if *held < hold_frames {
            return None;
        }
        self.held = None;
        Some(Switch {
            selected: self.before,
            engaged: RotaryMode::Brake,
        })
    }

    /// Forget a hold in progress: a program or a reset has put the cabinet
    /// somewhere else, and a stop decided before it would land on that.
    pub fn cancel_hold(&mut self) {
        self.held = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOLD: u32 = 48_000;

    fn press(pedal: &mut SpeedPedal, mode: SustainPedal, now: Switch) -> Option<Switch> {
        pedal.set(true, mode, now.selected, now.engaged)
    }

    fn release(pedal: &mut SpeedPedal, mode: SustainPedal, now: Switch) -> Option<Switch> {
        pedal.set(false, mode, now.selected, now.engaged)
    }

    const CHORALE: Switch = Switch {
        selected: RotaryMode::Chorale,
        engaged: RotaryMode::Chorale,
    };

    #[test]
    fn each_press_throws_the_switch() {
        let mut pedal = SpeedPedal::default();
        let fast = press(&mut pedal, SustainPedal::Alternate, CHORALE).expect("switch");
        assert_eq!(fast.engaged, RotaryMode::Tremolo);
        assert_eq!(release(&mut pedal, SustainPedal::Alternate, fast), None);
        let slow = press(&mut pedal, SustainPedal::Alternate, fast).expect("switch");
        assert_eq!(slow, CHORALE);
    }

    #[test]
    fn holding_the_pedal_stops_the_rotors_and_keeps_the_speed() {
        let mut pedal = SpeedPedal::default();
        assert!(press(&mut pedal, SustainPedal::Alternate, CHORALE).is_some());
        for _ in 1..HOLD {
            assert_eq!(pedal.tick(HOLD), None);
        }
        let stopped = pedal.tick(HOLD).expect("stop");
        assert_eq!(stopped.engaged, RotaryMode::Brake);
        // The press meant a stop, so the speed it began is taken back.
        assert_eq!(stopped.selected, RotaryMode::Chorale);
        assert_eq!(pedal.tick(HOLD), None, "a hold stops the rotors once");
        assert_eq!(release(&mut pedal, SustainPedal::Alternate, stopped), None);
        // The next press lets them go at the speed they had.
        let going = press(&mut pedal, SustainPedal::Alternate, stopped).expect("release");
        assert_eq!(going, CHORALE);
    }

    #[test]
    fn a_short_press_never_stops_the_rotors() {
        let mut pedal = SpeedPedal::default();
        let fast = press(&mut pedal, SustainPedal::Alternate, CHORALE).expect("switch");
        for _ in 1..HOLD {
            assert_eq!(pedal.tick(HOLD), None);
        }
        assert_eq!(release(&mut pedal, SustainPedal::Alternate, fast), None);
        for _ in 0..2 * HOLD {
            assert_eq!(pedal.tick(HOLD), None);
        }
    }

    #[test]
    fn momentary_is_fast_only_while_held() {
        let mut pedal = SpeedPedal::default();
        let held = press(&mut pedal, SustainPedal::Momentary, CHORALE).expect("switch");
        assert_eq!(held.engaged, RotaryMode::Tremolo);
        assert_eq!(held.selected, RotaryMode::Chorale);
        for _ in 0..2 * HOLD {
            assert_eq!(pedal.tick(HOLD), None, "momentary never stops the rotors");
        }
        assert_eq!(
            release(&mut pedal, SustainPedal::Momentary, held),
            Some(CHORALE)
        );
    }

    #[test]
    fn a_pedal_that_is_off_or_a_cabinet_switched_out_does_nothing() {
        let mut pedal = SpeedPedal::default();
        assert_eq!(press(&mut pedal, SustainPedal::Off, CHORALE), None);
        assert_eq!(release(&mut pedal, SustainPedal::Off, CHORALE), None);
        let out = Switch {
            selected: RotaryMode::Off,
            engaged: RotaryMode::Off,
        };
        assert_eq!(press(&mut pedal, SustainPedal::Alternate, out), None);
        for _ in 0..2 * HOLD {
            assert_eq!(pedal.tick(HOLD), None);
        }
    }

    #[test]
    fn a_repeated_position_is_not_a_press() {
        let mut pedal = SpeedPedal::default();
        let fast = press(&mut pedal, SustainPedal::Alternate, CHORALE).expect("switch");
        assert_eq!(press(&mut pedal, SustainPedal::Alternate, fast), None);
    }

    #[test]
    fn the_indices_are_the_choices() {
        for pedal in [
            SustainPedal::Off,
            SustainPedal::Alternate,
            SustainPedal::Momentary,
        ] {
            assert_eq!(SustainPedal::from_index(pedal as u8), Some(pedal));
        }
        assert_eq!(SustainPedal::from_index(3), None);
    }
}
