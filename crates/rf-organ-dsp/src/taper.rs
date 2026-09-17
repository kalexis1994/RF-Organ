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
