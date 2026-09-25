//! Turning one anchor's raw model outputs into a box and five landmarks.

// The model predicts offsets from a `Prior`, not positions, and the offsets are *variance-encoded*: divided by a
// constant during training so the network's outputs sit in a comparable range across the box's centre and its
// extents. Decoding multiplies them back.
//
// **Per anchor rather than over the whole array**, so that `filter::surviving` decodes only the anchors that clear the
// confidence threshold — see its body for what that saves.

use super::anchors::Prior;

use crate::models::face::{Face, Point, Rect};

// Both variances are pinned by a test that repeats rather than references them.
/// The variance applied to the centre offsets and to every landmark offset.
const VARIANCE_CENTRE: f32 = 0.1;

/// The variance applied to the extent offsets, which are exponentiated rather than added.
const VARIANCE_EXTENT: f32 = 0.2;

/// Anchor `index`'s bounding box, from the four `loc` values belonging to it, in the prior's normalised space —
/// [`filter`](super::filter) scales the result to the detector's square.
pub(super) fn decode_box(loc: &[f32], prior: Prior, index: usize) -> Rect {
    let offset = index * 4;

    // The centre moves by an offset scaled by the prior's own extent; the extents are multiplied by the exponential
    // of their offset, which is what lets one anchor cover a range of face sizes rather than only its own.
    let centre_x = prior.cx + loc[offset] * VARIANCE_CENTRE * prior.sx;
    let centre_y = prior.cy + loc[offset + 1] * VARIANCE_CENTRE * prior.sy;

    let width = prior.sx * (loc[offset + 2] * VARIANCE_EXTENT).exp();
    let height = prior.sy * (loc[offset + 3] * VARIANCE_EXTENT).exp();

    let (half_width, half_height) = (width * 0.5, height * 0.5);

    Rect::new(
        Point::new(centre_x - half_width, centre_y - half_height),
        Point::new(centre_x + half_width, centre_y + half_height),
    )
}

/// Anchor `index`'s five landmarks, from the ten `landmarks` values belonging to it.
pub(super) fn decode_landmarks(landmarks: &[f32], prior: Prior, index: usize) -> [Point; Face::LANDMARKS] {
    // Ten values per anchor rather than eight, which is the stride most easily confused with the box's.
    let offset = index * 10;

    // The **centre** variance for both axes and **no exponential**: a landmark is a point rather than an extent, so a
    // copy of `decode_box`'s width term here would show up as points drifting outward with the offset's magnitude.
    std::array::from_fn(|point| {
        Point::new(
            prior.cx + landmarks[offset + point * 2] * VARIANCE_CENTRE * prior.sx,
            prior.cy + landmarks[offset + point * 2 + 1] * VARIANCE_CENTRE * prior.sy,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The variances, repeated here rather than referenced, so that editing the implementation's values fails this.
    // That is the whole point of pinning them: a wrong variance produces a well-formed, consistently wrong box.
    const TEST_VARIANCE_CENTRE: f32 = 0.1;
    const TEST_VARIANCE_EXTENT: f32 = 0.2;

    /// The reference's own tolerance, from `decode_test.go`.
    #[track_caller]
    fn assert_close(name: &str, got: f32, want: f32) {
        assert!((f64::from(got) - f64::from(want)).abs() <= 1e-6, "{name} = {got}, want {want}");
    }

    #[test]
    fn a_zero_offset_decodes_back_to_the_prior_itself() {
        // The case that catches a variance applied to the wrong term or a sign flip, both of which still produce a
        // well-formed box. Transcribed from `TestDecodeBoxWithZeroOffsets`.
        let prior = Prior { cx: 0.5, cy: 0.25, sx: 0.1, sy: 0.2 };

        let decoded = decode_box(&[0.0, 0.0, 0.0, 0.0], prior, 0);

        assert_close("min.x", decoded.min.x, prior.cx - prior.sx / 2.0);
        assert_close("min.y", decoded.min.y, prior.cy - prior.sy / 2.0);
        assert_close("max.x", decoded.max.x, prior.cx + prior.sx / 2.0);
        assert_close("max.y", decoded.max.y, prior.cy + prior.sy / 2.0);
    }

    #[test]
    fn a_non_zero_offset_decodes_to_the_reference_formula() {
        // Computed independently from the reference formula rather than from the implementation.
        // `TestDecodeBoxAppliesVariances`.
        let prior = Prior { cx: 0.4, cy: 0.6, sx: 0.2, sy: 0.3 };
        let loc = [0.5_f32, -0.25, 0.75, -0.5];

        let decoded = decode_box(&loc, prior, 0);

        let centre_x = prior.cx + loc[0] * TEST_VARIANCE_CENTRE * prior.sx;
        let centre_y = prior.cy + loc[1] * TEST_VARIANCE_CENTRE * prior.sy;
        let width = prior.sx * (loc[2] * TEST_VARIANCE_EXTENT).exp();
        let height = prior.sy * (loc[3] * TEST_VARIANCE_EXTENT).exp();

        assert_close("min.x", decoded.min.x, centre_x - width / 2.0);
        assert_close("min.y", decoded.min.y, centre_y - height / 2.0);
        assert_close("max.x", decoded.max.x, centre_x + width / 2.0);
        assert_close("max.y", decoded.max.y, centre_y + height / 2.0);
    }

    #[test]
    fn a_box_is_read_at_the_stride_of_four_the_index_names() {
        // Decoding is per-anchor precisely so the caller can skip the discarded ones, which means the index
        // arithmetic has to land on the right stride. Anchor 0 is deliberately non-zero, so a decode that ignores the
        // index cannot accidentally pass. `TestDecodeBoxReadsTheIndexedAnchor`.
        let prior = Prior { cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1 };
        let loc = [9.0_f32, 9.0, 9.0, 9.0, 0.0, 0.0, 0.0, 0.0];

        let decoded = decode_box(&loc, prior, 1);

        assert_close("min.x", decoded.min.x, prior.cx - prior.sx / 2.0);
        assert_close("max.y", decoded.max.y, prior.cy + prior.sy / 2.0);
    }

    #[test]
    fn zero_landmark_offsets_put_every_point_at_the_anchors_centre() {
        // A copy-paste of the box's width term would show up here as points drifting outward.
        // `TestDecodeLandmarkWithZeroOffsets`.
        let prior = Prior { cx: 0.3, cy: 0.7, sx: 0.15, sy: 0.15 };

        let decoded = decode_landmarks(&[0.0_f32; 10], prior, 0);

        assert_eq!(decoded.len(), Face::LANDMARKS);
        for point in decoded {
            assert_close("landmark x", point.x, prior.cx);
            assert_close("landmark y", point.y, prior.cy);
        }
    }

    #[test]
    fn landmark_offsets_are_scaled_by_the_centre_variance_on_both_axes() {
        // `TestDecodeLandmarkAppliesVariance`, with a prior whose two extents differ so that a decode using `sx` for
        // both axes fails rather than coincidentally agreeing.
        let prior = Prior { cx: 0.3, cy: 0.7, sx: 0.15, sy: 0.25 };
        let raw = [1.0_f32, -1.0, 2.0, -2.0, 3.0, -3.0, 4.0, -4.0, 5.0, -5.0];

        let decoded = decode_landmarks(&raw, prior, 0);

        for (point, decoded) in decoded.iter().enumerate().map(|(index, point)| (index, *point)) {
            assert_close("landmark x", decoded.x, prior.cx + raw[point * 2] * TEST_VARIANCE_CENTRE * prior.sx);
            assert_close("landmark y", decoded.y, prior.cy + raw[point * 2 + 1] * TEST_VARIANCE_CENTRE * prior.sy);
        }
    }

    #[test]
    fn landmarks_are_read_at_the_stride_of_ten_the_index_names() {
        // `TestDecodeLandmarkReadsTheIndexedAnchor`.
        let prior = Prior { cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1 };

        let mut raw = [0.0_f32; 20];
        raw[..10].fill(9.0);

        let decoded = decode_landmarks(&raw, prior, 1);

        for point in decoded {
            assert_close("landmark x", point.x, prior.cx);
            assert_close("landmark y", point.y, prior.cy);
        }
    }
}
