// SPDX-License-Identifier: GPL-2.0-or-later

//! Where to draw the cabinet, seen from above.
//!
//! The angles come from the engine's own rotor model, so nothing here decides
//! how fast anything turns. What this works out is the frame: the cabinet sits
//! at a fixed scale, the microphones stand wherever they have been put, and
//! the view widens to hold both rather than letting a distant pair fall off
//! the edge.

use rf_organ_dsp::RotaryGeometry;

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
    pub left_mic: (f64, f64),
    pub right_mic: (f64, f64),
    /// Where the line from the pivot to the pair's centre ends.
    pub axis_end: (f64, f64),
    pub horn_sweep: f64,
    pub drum_sweep: f64,
    pub horn_scale: f64,
    pub drum_scale: f64,
    pub distance_cm: f64,
}

pub fn frame(geometry: RotaryGeometry) -> Frame {
    let (pivot_x, pivot_y) = PIVOT;
    let distance = f64::from(geometry.mic_distance_m);
    let front = pivot_y + distance * SCALE;
    let centre = pivot_x + f64::from(geometry.mic_centre_m) * SCALE;
    let half = f64::from(geometry.mic_half_width_m) * SCALE;

    // Frame whatever is on the table: the cabinet, and the microphones
    // wherever they are. Moving them back zooms out instead of pushing them
    // out of the picture.
    let margin = MARGIN_M * SCALE;
    let top = pivot_y - CABINET_BACK_M * SCALE;
    let bottom = front + margin;
    let near = (centre - half).min(pivot_x - CABINET_HALF_M * SCALE) - margin;
    let far = (centre + half).max(pivot_x + CABINET_HALF_M * SCALE) + margin;
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
        left_mic: (centre - half, front),
        right_mic: (centre + half, front),
        axis_end: (centre, front),
        horn_sweep: f64::from(geometry.horn_radius_m) * SCALE,
        drum_sweep: f64::from(geometry.drum_radius_m) * SCALE,
        horn_scale: f64::from(geometry.horn_radius_m) / HORN_RADIUS_M,
        drum_scale: f64::from(geometry.drum_radius_m) / DRUM_RADIUS_M,
        distance_cm: distance * 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rf_organ_dsp::{
        MIC_DISTANCE_RANGE_M, MIC_OFFSET_MAX_M, MIC_SPACING_MAX_M, MicrophoneArray, Rotary,
    };

    fn geometry(array: MicrophoneArray) -> RotaryGeometry {
        let mut rotary = Rotary::new(600.0);
        assert!(rotary.set_microphones(array));
        rotary.geometry()
    }

    fn holds(frame: &Frame, point: (f64, f64)) -> bool {
        let (x, y, width, height) = frame.view_box;
        (x..=x + width).contains(&point.0) && (y..=y + height).contains(&point.1)
    }

    /// Wherever the pair is put, inside the documented ranges, both
    /// microphones and the cabinet stay in the picture.
    #[test]
    fn the_frame_holds_whatever_is_on_the_table() {
        let (near, far) = MIC_DISTANCE_RANGE_M;
        for distance in [near, 0.35, 0.9, far] {
            for offset in [-MIC_OFFSET_MAX_M, 0.0, MIC_OFFSET_MAX_M] {
                for spacing in [0.0, MIC_SPACING_MAX_M] {
                    let frame = frame(geometry(MicrophoneArray {
                        distance_m: distance,
                        spacing_m: spacing,
                        offset_m: offset,
                        pattern: 0.5,
                    }));
                    assert!(holds(&frame, frame.left_mic), "{frame:?}");
                    assert!(holds(&frame, frame.right_mic), "{frame:?}");
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

    /// The microphones stand in front of the cabinet and the far ones are
    /// further away, which is the whole point of the drawing.
    #[test]
    fn distance_reads_as_distance() {
        let near = frame(geometry(MicrophoneArray {
            distance_m: 0.2,
            ..MicrophoneArray::default()
        }));
        let far = frame(geometry(MicrophoneArray {
            distance_m: 1.5,
            ..MicrophoneArray::default()
        }));
        assert!(near.left_mic.1 > PIVOT.1, "the pair is behind the cabinet");
        assert!(far.left_mic.1 > near.left_mic.1);
        assert!(far.view_box.3 > near.view_box.3, "the view did not widen");
        assert!((near.distance_cm - 20.0).abs() < 1.0e-6);
    }

    /// A pair with no spacing between it is one point, and an offset moves
    /// that point off the axis.
    #[test]
    fn spacing_and_offset_land_where_they_should() {
        let together = frame(geometry(MicrophoneArray {
            spacing_m: 0.0,
            ..MicrophoneArray::default()
        }));
        assert_eq!(together.left_mic, together.right_mic);
        let shifted = frame(geometry(MicrophoneArray {
            offset_m: 0.25,
            ..MicrophoneArray::default()
        }));
        assert!((shifted.axis_end.0 - PIVOT.0 - 0.25 * SCALE).abs() < 1.0e-6);
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
