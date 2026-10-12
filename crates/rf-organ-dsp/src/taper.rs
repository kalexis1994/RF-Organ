// SPDX-License-Identifier: GPL-2.0-or-later

//! The generator's per-wheel taper.
//!
//! A tone wheel's output does not reach the busbars at whatever level its
//! pickup happens to give it. The service manual describes what stands in the
//! way: "there are filters consisting of small transformers and condensers
//! associated with certain frequencies", each with "a single tapped winding"
//! whose tap is grounded, so that the side "connected to the corresponding
//! magnet coil through a condenser, forms a resonant circuit for the
//! fundamental frequency of that coil", which "tends to emphasize the
//! fundamental and suppress harmonics". Which wheels carry one is given only
//! in two figures, and the scans of those figures are past reading.
//!
//! So this table is every wheel at unity, and that is a statement of ignorance
//! rather than a claim about a console: a real generator is not flat here. It
//! is a table rather than a constant because the thing that would replace it
//! is a measurement, and `rf-organ-lab compare` produces exactly this list
//! from a chromatic capture of a real instrument - one line per wheel, ready
//! to be pasted in, with the wheels the capture could not reach left alone.
//!
//! Until that happens, `docs/ARCHITECTURE.md` says the taper is flat, and the
//! laboratory's own taper report shows a flat curve, which is what a reader
//! should see: nothing here is pretending to have been measured.
//!
//! Published readings exist and were tried. Kon Zissis's compilation has each
//! wheel read straight off a busbar - Hammond's Table V and six consoles,
//! which agree with one another - and each wheel read through one key and
//! drawbar, Hammond's second chart and six more. The busbar readings fall
//! 7.5 dB between wheels 43 and 44, where the generator's filters begin, and
//! carry the bottom octave far above the next; the readings through the keys
//! show neither, because the key tapering wires compensate both. A table built
//! from the busbar readings alone would put both artefacts into every note,
//! and one built from the key readings would put the tapering of the one path
//! each wheel was read on into all the others. Joining them needs the gain a
//! tapering wire of so many ohms gives, and fitted to these readings that
//! conversion does not survive leaving one console out. So the table stays
//! flat, and `rf-organ-lab wheel-levels` reports how far it is from both.

use crate::tonewheel::TONEWHEEL_COUNT;

/// Level of each of the 91 wheels, in order of increasing frequency, as the
/// generator's terminal strip numbers them.
pub const TAPER: [f32; TONEWHEEL_COUNT] = [1.0; TONEWHEEL_COUNT];

#[cfg(test)]
mod tests {
    use super::*;

    /// The table covers the generator and says nothing it has not been told.
    #[test]
    fn the_taper_is_flat_and_complete() {
        assert_eq!(TAPER.len(), TONEWHEEL_COUNT);
        assert!(
            TAPER.iter().all(|level| *level == 1.0),
            "the taper claims a measurement that has not been made"
        );
        assert!(TAPER.iter().all(|level| level.is_finite() && *level >= 0.0));
    }
}
