// SPDX-License-Identifier: GPL-2.0-or-later

//! Where to draw the cabinet, seen from above.
//!
//! The angles come from the engine's own rotor model, so nothing here decides
//! how fast anything turns. What this works out is the frame: the cabinet sits
//! at a fixed scale, the microphones stand wherever they have been put, and
//! the view widens to hold both rather than letting a distant pair fall off
//! the edge.

use rf_organ_dsp::{RotaryGeometry, RotaryPlacement};

/// Pixels per metre, and the point every coordinate in the drawing is
/// measured from. These match the static scene in `play.html`.
pub const SCALE: f64 = 95.0;
pub const PIVOT: (f64, f64) = (110.0, 81.0);
/// The shape of the frame the drawing is shown in.
pub const ASPECT: f64 = 148.0 / 200.0;
/// The radii the horn and the drum are drawn at before the engine's own radii
/// scale them.
pub const HORN_RADIUS_M: f64 = 0.18;
pub const DRUM_RADIUS_M: f64 = 0.12;
/// Half the cabinet's own footprint, and the air left around everything.
const CABINET_HALF_M: f64 = 0.36;
const CABINET_BACK_M: f64 = 0.42;
const MARGIN_M: f64 = 0.1;

/// Everything the drawing needs that is not an angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Left, top, width and height, in the drawing's own units.
    pub view_box: (f64, f64, f64, f64),
    /// The horn's pair, then the drum's, each left and right.
    pub microphones: [(f64, f64); 4],
    /// Where the line from the pivot to each pair's centre ends.
    pub horn_axis: (f64, f64),
    pub drum_axis: (f64, f64),
    pub horn_sweep: f64,
    pub drum_sweep: f64,
    pub horn_scale: f64,
    pub drum_scale: f64,
    pub horn_distance_cm: f64,
    pub drum_distance_cm: f64,
}

/// Where one pair stands, in the drawing's units.
struct Stand {
    left: (f64, f64),
    right: (f64, f64),
    centre: (f64, f64),
}

fn stand(place: RotaryPlacement) -> Stand {
    let (pivot_x, pivot_y) = PIVOT;
    let front = pivot_y + f64::from(place.distance_m) * SCALE;
    let centre = pivot_x + f64::from(place.centre_m) * SCALE;
    let half = f64::from(place.half_width_m) * SCALE;
    Stand {
        left: (centre - half, front),
        right: (centre + half, front),
        centre: (centre, front),
    }
}

pub fn frame(geometry: RotaryGeometry) -> Frame {
    let (pivot_x, pivot_y) = PIVOT;
    let horn = stand(geometry.horn);
    let drum = stand(geometry.drum);
    let microphones = [horn.left, horn.right, drum.left, drum.right];

    // Frame whatever is on the table: the cabinet, and both pairs wherever
    // they are. Moving one back zooms out instead of pushing it out of the
    // picture.
    let margin = MARGIN_M * SCALE;
    let top = pivot_y - CABINET_BACK_M * SCALE;
    let mut bottom = pivot_y;
    let mut near = pivot_x - CABINET_HALF_M * SCALE;
    let mut far = pivot_x + CABINET_HALF_M * SCALE;
    for (x, y) in microphones {
        bottom = bottom.max(y);
        near = near.min(x);
        far = far.max(x);
    }
    bottom += margin;
    near -= margin;
    far += margin;
    let (mut width, mut height) = (far - near, bottom - top);
    if width / height > ASPECT {
        height = width / ASPECT;
    } else {
        width = height * ASPECT;
    }

    Frame {
        view_box: (
            0.5 * (near + far) - 0.5 * width,
            0.5 * (top + bottom) - 0.5 * height,
            width,
            height,
        ),
        microphones,
        horn_axis: horn.centre,
        drum_axis: drum.centre,
        horn_sweep: f64::from(geometry.horn_radius_m) * SCALE,
        drum_sweep: f64::from(geometry.drum_radius_m) * SCALE,
        horn_scale: f64::from(geometry.horn_radius_m) / HORN_RADIUS_M,
        drum_scale: f64::from(geometry.drum_radius_m) / DRUM_RADIUS_M,
        horn_distance_cm: f64::from(geometry.horn.distance_m) * 100.0,
        drum_distance_cm: f64::from(geometry.drum.distance_m) * 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rf_organ_dsp::{
        MIC_DISTANCE_RANGE_M, MIC_OFFSET_MAX_M, MIC_SPACING_MAX_M, MicrophoneArray, MicrophonePair,
        Rotary,
    };

    fn geometry(horn: MicrophonePair, drum: MicrophonePair) -> RotaryGeometry {
        let mut rotary = Rotary::new(600.0);
        assert!(rotary.set_microphones(MicrophoneArray {
            horn,
            drum,
            pattern: 0.5,
        }));
        rotary.geometry()
    }

    fn holds(frame: &Frame, point: (f64, f64)) -> bool {
        let (x, y, width, height) = frame.view_box;
        (x..=x + width).contains(&point.0) && (y..=y + height).contains(&point.1)
    }

    /// Wherever either pair is put, inside the documented ranges, all four
    /// microphones and the cabinet stay in the picture.
    #[test]
    fn the_frame_holds_whatever_is_on_the_table() {
        let (near, far) = MIC_DISTANCE_RANGE_M;
        for distance in [near, 0.35, far] {
            for offset in [-MIC_OFFSET_MAX_M, 0.0, MIC_OFFSET_MAX_M] {
                for spacing in [0.0, MIC_SPACING_MAX_M] {
                    let moved = MicrophonePair {
                        distance_m: distance,
                        spacing_m: spacing,
                        offset_m: offset,
                        at_the_sides: false,
                    };
                    for (horn, drum) in [
                        (moved, MicrophonePair::default()),
                        (MicrophonePair::default(), moved),
                        (moved, moved),
                    ] {
                        let frame = frame(geometry(horn, drum));
                        for microphone in frame.microphones {
                            assert!(holds(&frame, microphone), "{frame:?}");
                        }
                        assert!(holds(&frame, PIVOT), "{frame:?}");
                        let (_, _, width, height) = frame.view_box;
                        assert!(
                            (width / height - ASPECT).abs() < 1.0e-9,
                            "the picture is the wrong shape: {frame:?}"
                        );
                    }
                }
            }
        }
    }

    /// Each pair is drawn at its own distance, which is what having two of
    /// them is for.
    #[test]
    fn the_pairs_are_drawn_apart() {
        let frame = frame(geometry(
            MicrophonePair {
                distance_m: 0.2,
                ..MicrophonePair::default()
            },
            MicrophonePair {
                distance_m: 1.5,
                ..MicrophonePair::default()
            },
        ));
        let [horn_left, _, drum_left, _] = frame.microphones;
        assert!(
            horn_left.1 > PIVOT.1,
            "the horn's pair is behind the cabinet"
        );
        assert!(
            drum_left.1 > horn_left.1,
            "the drum's pair is not further out"
        );
        assert!((frame.horn_distance_cm - 20.0).abs() < 1.0e-6);
        assert!((frame.drum_distance_cm - 150.0).abs() < 1.0e-4);
    }

    /// A pair with no spacing between it is one point, and an offset moves
    /// that point off the axis.
    #[test]
    fn spacing_and_offset_land_where_they_should() {
        let frame = frame(geometry(
            MicrophonePair {
                spacing_m: 0.0,
                ..MicrophonePair::default()
            },
            MicrophonePair {
                offset_m: 0.25,
                ..MicrophonePair::default()
            },
        ));
        let [horn_left, horn_right, _, _] = frame.microphones;
        assert_eq!(horn_left, horn_right);
        assert!((frame.drum_axis.0 - PIVOT.0 - 0.25 * SCALE).abs() < 1.0e-4);
    }

    /// The rotors are drawn at the radii the engine is using.
    #[test]
    fn the_rotors_are_drawn_at_the_radii_in_use() {
        let mut rotary = Rotary::new(600.0);
        assert!(rotary.set_rotor_radii(0.27, 0.06));
        let frame = frame(rotary.geometry());
        assert!((frame.horn_sweep - 0.27 * SCALE).abs() < 1.0e-4);
        assert!((frame.horn_scale - 0.27 / HORN_RADIUS_M).abs() < 1.0e-6);
        assert!((frame.drum_scale - 0.06 / DRUM_RADIUS_M).abs() < 1.0e-6);
    }
}
