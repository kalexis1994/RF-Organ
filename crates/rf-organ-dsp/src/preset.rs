// SPDX-License-Identifier: GPL-2.0-or-later

//! The preset panel: nine registrations per manual, reached by the nine
//! reverse-colour keys between cancel and the two adjust keys.
//!
//! The panel is not a separate mechanism. The B-3/C-3 service manual describes
//! it as nine bars of screw terminals with one wire per harmonic, and says
//! that fastening a wire to a bar "is equivalent to setting a harmonic drawbar
//! to the corresponding number". So a preset key is a registration and nothing
//! else, and what the panel needs is nine of them per manual.
//!
//! The service manual does not print them. It says the organ ships with "its
//! presets set up as shown in the booklet, 'Creating Beautiful Tone Colors
//! with the Harmonic Drawbars,' which may be obtained free on request", and
//! reproduces only the card that explains how to change them - whose worked
//! example, `006523411` on the upper D#, is explicitly an example rather than
//! a factory setting.
//!
//! The tables below are the standard console registrations, taken from two
//! sources that agree digit for digit, one of which attributes them to a
//! Hammond playing guide. They are documented rather than measured, and the
//! provenance is in `THIRD_PARTY_NOTICES.md`.
//!
//! They are fixed here. On the console they are not: "Preset combinations may
//! be changed at will by removing the console back and following the
//! directions on a card inside." Making them movable from the front is a
//! choice about what belongs in a plugin's parameters, not a claim about the
//! instrument, and is recorded as a limitation.

use crate::manual::DRAWBAR_COUNT;

/// Preset keys per manual: C# through A.
pub const PRESET_COUNT: usize = 9;

/// The swell manual's panel, in keyboard order from C# up to A.
pub const UPPER_PRESETS: [[u8; DRAWBAR_COUNT]; PRESET_COUNT] = [
    [0, 0, 5, 3, 2, 0, 0, 0, 0], // C#  Stopped Flute
    [0, 0, 4, 4, 3, 2, 0, 0, 0], // D   Dulciana
    [0, 0, 8, 7, 4, 0, 0, 0, 0], // D#  French Horn
    [0, 0, 4, 5, 4, 4, 2, 2, 2], // E   Salicional
    [0, 0, 5, 4, 0, 3, 0, 0, 0], // F   Flutes 8' and 4'
    [0, 0, 4, 6, 7, 5, 3, 0, 0], // F#  Oboe Horn
    [0, 0, 5, 6, 4, 4, 3, 2, 0], // G   Swell Diapason
    [0, 0, 6, 8, 7, 6, 5, 4, 0], // G#  Trumpet
    [3, 2, 7, 6, 4, 5, 2, 2, 2], // A   Full Swell
];

/// The great manual's panel, in the same order.
pub const LOWER_PRESETS: [[u8; DRAWBAR_COUNT]; PRESET_COUNT] = [
    [0, 0, 4, 5, 4, 5, 4, 4, 0], // C#  Cello
    [0, 0, 4, 4, 2, 3, 2, 2, 0], // D   Flute and String
    [0, 0, 7, 3, 7, 3, 4, 3, 0], // D#  Clarinet
    [0, 0, 4, 5, 4, 4, 2, 2, 0], // E   Diapason, Gamba and Flute
    [0, 0, 6, 6, 4, 4, 3, 2, 2], // F   Great without reeds
    [0, 0, 5, 6, 4, 2, 2, 0, 0], // F#  Open Diapason
    [0, 0, 6, 8, 4, 5, 4, 3, 3], // G   Full Great
    [0, 0, 8, 0, 3, 0, 0, 0, 0], // G#  Tibia Clausa
    [4, 2, 7, 8, 6, 6, 2, 4, 4], // A   Full Great with 16'
];

/// What the panel's makers called each of the nine, in the same order. The
/// names are the registrations' own, not this model's.
pub const UPPER_PRESET_NAMES: [&str; PRESET_COUNT] = [
    "Stopped Flute",
    "Dulciana",
    "French Horn",
    "Salicional",
    "Flutes 8' & 4'",
    "Oboe Horn",
    "Swell Diapason",
    "Trumpet",
    "Full Swell",
];

pub const LOWER_PRESET_NAMES: [&str; PRESET_COUNT] = [
    "Cello",
    "Flute & String",
    "Clarinet",
    "Diapason, Gamba & Flute",
    "Great no Reeds",
    "Open Diapason",
    "Full Great",
    "Tibia Clausa",
    "Full Great 16'",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A drawbar has nine positions and a registration has nine drawbars.
    /// Both tables were transcribed by hand from printed digits, so the shape
    /// is worth holding.
    #[test]
    fn every_printed_digit_is_a_drawbar_position() {
        for panel in [UPPER_PRESETS, LOWER_PRESETS] {
            for registration in panel {
                assert_eq!(registration.len(), DRAWBAR_COUNT);
                for position in registration {
                    assert!(position <= 8, "{position} is not a drawbar position");
                }
            }
        }
    }

    /// None of the nine is silence: a preset key that sounded like the cancel
    /// key would be a transcription error rather than a tone colour.
    #[test]
    fn no_preset_is_silent() {
        for panel in [UPPER_PRESETS, LOWER_PRESETS] {
            for registration in panel {
                assert!(registration.iter().any(|position| *position > 0));
            }
        }
    }
}
