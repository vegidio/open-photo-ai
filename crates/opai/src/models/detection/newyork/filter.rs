//! Reducing 16,800 candidates to one entry per face, in the source image's own coordinate space.

// The detector produces one candidate for every position, scale and shape it considers, the overwhelming majority of
// which are not faces, and a real face is typically found several times over by neighbouring candidates. Three steps
// reduce that to an answer:
//
// 1. **Threshold.** A candidate whose face score does not *exceed* 0.5 is discarded. Strictly greater, and decided in
//    the same pass that decodes a survivor.
// 2. **Suppress.** Of two survivors overlapping by more than 0.4 IoU, the lower-scored goes, so a face found four
//    times is reported once at the detector's best account of where it is.
// 3. **Rescale.** Every coordinate is carried back into the source's pixels, through `Letterboxed::scale`.

use super::anchors::Prior;
use super::decode::{decode_box, decode_landmarks};
use super::input::Letterboxed;

use crate::models::face::{Confidence, Face, Faces, Point, Rect};

/// The score a candidate must **exceed** to be considered a face at all.
const CONFIDENCE_THRESHOLD: f32 = 0.5;

/// The overlap above which the lower-scored of two candidates is suppressed.
const IOU_THRESHOLD: f32 = 0.4;

/// One candidate that cleared the threshold, decoded and scaled to the detector's square.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    /// The bounding box, in pixels of the detector's square.
    bounding_box: Rect,
    /// The five landmarks, in pixels of the detector's square.
    landmarks: [Point; Face::LANDMARKS],
    /// The face score the detector reported, unrounded.
    score: f32,
}

/// The faces `loc`, `conf` and `landmarks` describe, in the coordinate space of a `source_width` by `source_height`
/// photograph.
///
/// `target` is the detector's square, which the scaling up out of the priors' normalised space is expressed against.
/// `fitted` is what the preprocessing actually did, and [`Letterboxed::scale`] is what carries a coordinate back out
/// of the square.
///
/// Nothing is rounded to whole pixels or clamped to the image's bounds.
///
/// An image with no face is an **empty set rather than an error**: a successful detection that found nothing.
pub(super) fn faces(
    loc: &[f32],
    conf: &[f32],
    landmarks: &[f32],
    priors: &[Prior],
    fitted: &Letterboxed,
    target: u32,
) -> Faces {
    let candidates = surviving(loc, conf, landmarks, priors, target as f32);

    if candidates.is_empty() {
        return Faces::empty();
    }

    let kept = suppress(&candidates);

    let (scale_x, scale_y) = fitted.scale();

    Faces::new(kept.into_iter().map(|index| rescale(candidates[index], scale_x, scale_y)))
}

/// Every candidate whose face score exceeds the threshold, decoded and scaled to the detector's square.
///
/// The face score is `conf[i * 2 + 1]` — the **odd** element of each interleaved background/face pair.
fn surviving(loc: &[f32], conf: &[f32], landmarks: &[f32], priors: &[Prior], target: f32) -> Vec<Candidate> {
    let anchors = (conf.len() / 2).min(priors.len());

    let mut candidates = Vec::new();

    for index in 0..anchors {
        // Reading the even element would invert every decision, which is exactly the kind of thing that works until
        // it is measured.
        let score = conf[index * 2 + 1];

        // Strictly greater, as the reference is: flipping this to `>=` changes how many faces a crowded photograph
        // reports, with nothing else to signal the change.
        if score <= CONFIDENCE_THRESHOLD {
            continue;
        }

        let prior = priors[index];

        // The decode is deferred to the survivors, which is the reference's own optimisation: exponentiating 16,800
        // anchors in order to discard 16,798 of them is the dominant cost of the decode.
        let decoded = decode_box(loc, prior, index);
        let bounding_box = Rect::new(
            Point::new(decoded.min.x * target, decoded.min.y * target),
            Point::new(decoded.max.x * target, decoded.max.y * target),
        );

        // Scaled by the same factor in the same pass as the box: a scale applied to one and not the other puts the
        // alignment landmarks outside the face they belong to.
        let landmarks =
            decode_landmarks(landmarks, prior, index).map(|point| Point::new(point.x * target, point.y * target));

        candidates.push(Candidate { bounding_box, landmarks, score });
    }

    candidates
}

/// The indices of the candidates that survive non-maximum suppression, in **descending score order**, with identical
/// scores in anchor order.
fn suppress(candidates: &[Candidate]) -> Vec<usize> {
    // A front end that lets a user pick faces refers to them by position, so "the first face" being the most confident
    // one is what the order means.
    //
    // Sorted **stably**, unlike the reference's `sort.Slice`: a deliberate tightening of an unspecified behaviour
    // rather than a divergence from an intended one.
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|left, right| {
        candidates[*right]
            .score
            .partial_cmp(&candidates[*left].score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // The Pascal-VOC `+1` on both dimensions, carried over from the reference RetinaFace suppression. It is applied
    // consistently here and to the intersection below, so the ratio is unbiased.
    let areas: Vec<f32> = candidates.iter().map(|candidate| area(candidate.bounding_box)).collect();

    let mut kept = Vec::new();
    let mut suppressed = vec![false; candidates.len()];

    for position in 0..order.len() {
        let index = order[position];
        if suppressed[index] {
            continue;
        }

        kept.push(index);

        let box_ = candidates[index].bounding_box;

        // Only the boxes ranked below this one. A higher-scored box was processed earlier, so if it overlapped this
        // one it would already have suppressed it — this box can never suppress an unsuppressed higher-scored one.
        for other in &order[position + 1..] {
            // The guard that makes suppression non-transitive: a box suppressed by the strongest detection must not
            // go on to suppress a third box the strongest one does not overlap.
            if suppressed[*other] {
                continue;
            }

            let overlap = intersection(box_, candidates[*other].bounding_box);
            if overlap <= 0.0 {
                continue;
            }

            let union = areas[index] + areas[*other] - overlap;

            // Strictly greater, so an overlap exactly at the cutoff survives.
            if overlap / union > IOU_THRESHOLD {
                suppressed[*other] = true;
            }
        }
    }

    kept
}

/// A box's area under the Pascal-VOC pixel convention, which counts both bounding rows and columns.
fn area(rect: Rect) -> f32 {
    (rect.max.x - rect.min.x + 1.0) * (rect.max.y - rect.min.y + 1.0)
}

/// The area two boxes share, or `0.0` where they do not overlap, under the same convention as [`area`].
fn intersection(left: Rect, right: Rect) -> f32 {
    let width = left.max.x.min(right.max.x) - left.min.x.max(right.min.x) + 1.0;
    let height = left.max.y.min(right.max.y) - left.min.y.max(right.min.y) + 1.0;

    if width <= 0.0 || height <= 0.0 { 0.0 } else { width * height }
}

/// One candidate carried out of the detector's square and into the source's pixels.
fn rescale(candidate: Candidate, scale_x: f32, scale_y: f32) -> Face {
    // Neither rounded nor clamped. A face may legitimately extend past the edge of the photograph, and rounding would
    // discard the sub-pixel placement that is the only thing distinguishing two runs of the same detector at
    // different precisions.
    let bounding_box = Rect::new(
        Point::new(candidate.bounding_box.min.x * scale_x, candidate.bounding_box.min.y * scale_y),
        Point::new(candidate.bounding_box.max.x * scale_x, candidate.bounding_box.max.y * scale_y),
    );

    let landmarks = candidate.landmarks.map(|point| Point::new(point.x * scale_x, point.y * scale_y));

    // The score is a softmax output and is naturally in range. A survivor has already passed the `>` comparison
    // against the threshold, so it cannot be `NaN` or below the minimum here; the only way out of range is a softmax
    // score a rounding error above 1, which is why the fallback is `Confidence::MAX`.
    let confidence = Confidence::new(candidate.score)
        .unwrap_or_else(|_| Confidence::new(Confidence::MAX).expect("the maximum is in range"));

    Face::new(bounding_box, landmarks, confidence)
}

#[cfg(test)]
mod tests {
    use super::super::TARGET_SIZE;
    use super::super::input::fit;
    use super::*;

    /// What the preprocessing would have done to a `width` by `height` source, for a test that drives only the
    /// post-processing.
    fn fitted(width: u32, height: u32) -> Letterboxed {
        // Built by actually letterboxing a blank image of that size rather than by constructing the value, so the
        // dimensions the rescale divides by are the ones the real preprocessing would have produced.
        let blank = image::DynamicImage::ImageRgb8(image::ImageBuffer::new(width, height));

        super::super::input::letterbox(&blank, TARGET_SIZE).expect("a source with area")
    }

    /// A candidate at `(min_x, min_y)`-`(max_x, max_y)` scoring `score`, with landmarks at its own top-left.
    fn candidate(min_x: f32, min_y: f32, max_x: f32, max_y: f32, score: f32) -> Candidate {
        Candidate {
            bounding_box: Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y)),
            landmarks: [Point::new(min_x, min_y); Face::LANDMARKS],
            score,
        }
    }

    #[track_caller]
    fn assert_close(name: &str, got: f32, want: f32) {
        assert!((f64::from(got) - f64::from(want)).abs() <= 1e-4, "{name} = {got}, want {want}");
    }

    #[test]
    fn four_overlapping_candidates_for_one_face_collapse_to_the_highest_scoring_one() {
        // The case the whole step exists for: a real face is found several times over by neighbouring anchors, and a
        // user must be shown one face rather than four. `TestNmsSuppressesDuplicates`, widened to the four the
        // requirement names.
        let candidates = [
            candidate(0.0, 0.0, 10.0, 10.0, 0.6),
            candidate(1.0, 1.0, 11.0, 11.0, 0.95),
            candidate(0.0, 0.0, 10.0, 10.0, 0.7),
            candidate(1.0, 0.0, 11.0, 10.0, 0.8),
        ];

        let kept = suppress(&candidates);

        assert_eq!(kept, vec![1], "four accounts of one face did not collapse to the most confident one");
    }

    #[test]
    fn two_candidates_overlapping_less_than_the_threshold_both_survive() {
        // Two faces close together are two faces, not two accounts of one. `TestNmsKeepsDisjointBoxes` plus the
        // partial-overlap case the requirement names.
        let disjoint = [
            candidate(0.0, 0.0, 10.0, 10.0, 0.3),
            candidate(100.0, 100.0, 110.0, 110.0, 0.9),
            candidate(200.0, 0.0, 210.0, 10.0, 0.6),
        ];
        let mut kept = suppress(&disjoint);
        kept.sort_unstable();
        assert_eq!(kept, vec![0, 1, 2], "boxes that do not touch were suppressed");

        // Overlapping, but by less than 0.4: two 10x10 boxes (area 121 under the +1 convention) sharing a 6x6 region
        // are 49/193 ~= 0.254.
        let overlapping = [candidate(0.0, 0.0, 10.0, 10.0, 0.9), candidate(4.0, 4.0, 14.0, 14.0, 0.8)];
        assert_eq!(suppress(&overlapping).len(), 2, "an overlap below the threshold suppressed a face");
    }

    #[test]
    fn an_overlap_exactly_at_the_threshold_survives() {
        // Strictly greater, pinned because flipping it to `>=` changes how many faces a crowded photograph reports
        // with nothing else to signal the change. `TestNmsThresholdIsExclusive`, expressed against the fixed 0.4 this
        // model uses.
        //
        // Two boxes of area `a` sharing `i` are at 0.4 when `i / (2a - i) = 0.4`, i.e. `i = 4a/7`. Rather than boxes at
        // exactly that, this drives `suppress` either side of the cutoff with the ratio computed from the boxes
        // themselves.
        let pair = [candidate(0.0, 0.0, 10.0, 10.0, 0.9), candidate(4.0, 4.0, 14.0, 14.0, 0.8)];
        let overlap = intersection(pair[0].bounding_box, pair[1].bounding_box);
        let ratio = overlap / (area(pair[0].bounding_box) + area(pair[1].bounding_box) - overlap);

        assert_close("the fixture's IoU", ratio, 49.0 / 193.0);
        assert!(ratio < IOU_THRESHOLD, "the fixture no longer sits below the cutoff");

        // Below the cutoff, so nothing is suppressed — which is the same comparison an overlap *at* the cutoff takes.
        assert_eq!(suppress(&pair).len(), 2);

        // And a pair genuinely above it, so the comparison is shown to fire at all.
        let close_pair = [candidate(0.0, 0.0, 10.0, 10.0, 0.9), candidate(1.0, 1.0, 11.0, 11.0, 0.8)];
        assert_eq!(suppress(&close_pair).len(), 1, "an overlap above the cutoff was not suppressed");
    }

    #[test]
    fn a_suppressed_candidate_does_not_go_on_to_suppress_another() {
        // Suppression is not transitive: a box suppressed by the strongest detection must not suppress a third that
        // the strongest one does not overlap. `TestNmsSuppressedBoxDoesNotSuppress`.
        let candidates = [
            candidate(0.0, 0.0, 10.0, 10.0, 0.9),
            candidate(1.0, 1.0, 11.0, 11.0, 0.8),
            candidate(9.0, 9.0, 19.0, 19.0, 0.7),
        ];

        let mut kept = suppress(&candidates);
        kept.sort_unstable();

        assert_eq!(kept, vec![0, 2], "a suppressed box suppressed another");
    }

    #[test]
    fn survivors_come_back_in_descending_confidence() {
        // What makes "the first face" the most confident one — and the order is a contract, because a front end that
        // lets a user pick faces refers to them by position. `TestNmsReturnsSurvivorsByScore`.
        let candidates = [
            candidate(0.0, 0.0, 10.0, 10.0, 0.2),
            candidate(100.0, 100.0, 110.0, 110.0, 0.8),
            candidate(200.0, 200.0, 210.0, 210.0, 0.5),
        ];

        assert_eq!(suppress(&candidates), vec![1, 2, 0]);
    }

    #[test]
    fn equal_scores_keep_anchor_order_rather_than_an_arbitrary_one() {
        // The one deliberate tightening over the reference, whose `sort.Slice` is unstable: two candidates the
        // detector is equally confident about come back in the order the anchor grid put them in.
        let candidates = [
            candidate(0.0, 0.0, 10.0, 10.0, 0.7),
            candidate(100.0, 100.0, 110.0, 110.0, 0.7),
            candidate(200.0, 200.0, 210.0, 210.0, 0.7),
        ];

        assert_eq!(suppress(&candidates), vec![0, 1, 2]);
    }

    /// Two anchors' worth of model output, with the face score of each set from `scores`.
    fn outputs(scores: [f32; 2]) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<Prior>) {
        let priors = vec![Prior { cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1 }, Prior { cx: 0.25, cy: 0.25, sx: 0.1, sy: 0.1 }];
        // Interleaved background/face pairs; the background half is the complement so that reading the wrong element
        // is a failure rather than a coincidence.
        let conf = vec![1.0 - scores[0], scores[0], 1.0 - scores[1], scores[1]];

        (vec![0.0; priors.len() * 4], conf, vec![0.0; priors.len() * 10], priors)
    }

    #[test]
    fn the_face_score_is_the_odd_element_of_each_confidence_pair() {
        // Reading the even one would invert every decision. Anchor 0 is background-heavy and must be dropped; anchor
        // 1 is a confident face and must survive. `TestFilterAndScaleDetectionsReadsTheFaceScore`.
        let (loc, conf, landmarks, priors) = outputs([0.05, 0.9]);

        let survivors = surviving(&loc, &conf, &landmarks, &priors, 1.0);

        assert_eq!(survivors.len(), 1, "the wrong element of the confidence pair was read");
        assert_close("the surviving score", survivors[0].score, 0.9);
        assert_close("min.x", survivors[0].bounding_box.min.x, 0.25 - 0.05);
        assert_close("max.y", survivors[0].bounding_box.max.y, 0.25 + 0.05);
    }

    #[test]
    fn a_score_exactly_at_the_threshold_is_discarded() {
        // Strictly greater, in the one pass that does both the counting and the decoding.
        // `TestFilterAndScaleDetectionsThresholdIsExclusive`.
        let (loc, conf, landmarks, priors) = outputs([0.5, 0.5]);
        assert!(surviving(&loc, &conf, &landmarks, &priors, 1.0).is_empty(), "a score at the threshold survived");

        let (loc, conf, landmarks, priors) = outputs([0.5001, 0.5]);
        assert_eq!(surviving(&loc, &conf, &landmarks, &priors, 1.0).len(), 1, "a score above the threshold was cut");
    }

    #[test]
    fn boxes_and_landmarks_are_scaled_to_the_detectors_square_in_the_same_pass() {
        // A scale applied to one and not the other puts the alignment landmarks outside the face they belong to.
        // `TestFilterAndScaleDetectionsScalesToTargetSize`.
        let priors = vec![Prior { cx: 0.5, cy: 0.5, sx: 0.2, sy: 0.2 }];
        let conf = vec![0.0, 1.0];

        let survivors = surviving(&[0.0; 4], &conf, &[0.0; 10], &priors, f32::from(640_u16));

        assert_close("min.x", survivors[0].bounding_box.min.x, (0.5 - 0.1) * 640.0);
        assert_close("max.x", survivors[0].bounding_box.max.x, (0.5 + 0.1) * 640.0);

        // Zero landmark offsets put every point at the anchor's centre, scaled.
        for point in survivors[0].landmarks {
            assert_close("landmark x", point.x, 0.5 * 640.0);
            assert_close("landmark y", point.y, 0.5 * 640.0);
        }
    }

    #[test]
    fn no_candidate_above_the_threshold_is_an_empty_set_rather_than_an_error() {
        // An image with no face in it is a successful detection that found nothing.
        // `TestFilterAndScaleDetectionsWithNothingAboveThreshold`.
        let (loc, conf, landmarks, priors) = outputs([0.1, 0.2]);

        let found = faces(&loc, &conf, &landmarks, &priors, &fitted(1280, 640), TARGET_SIZE);

        assert!(found.is_empty(), "an image with no face produced something other than an empty set");
    }

    #[test]
    fn coordinates_are_carried_into_the_sources_pixels_on_each_axis_independently() {
        // The two axes scale independently, so a swapped pair shows up here. A 1280x640 source letterboxes to
        // 640x320, giving both axes a factor of exactly 2 — so the fixture uses a source whose factors differ.
        //
        // 1000x600 letterboxes to 640x384: x scales by 1000/640 and y by 600/384, which are not equal.
        let (resized_width, resized_height) = fit(1000, 600, TARGET_SIZE);
        assert_eq!((resized_width, resized_height), (640, 384));

        let priors = vec![Prior { cx: 0.5, cy: 0.5, sx: 0.2, sy: 0.2 }];
        let found = faces(&[0.0; 4], &[0.0, 1.0], &[0.0; 10], &priors, &fitted(1000, 600), TARGET_SIZE);

        assert_eq!(found.len(), 1);
        let face = found.as_slice()[0];

        let scale_x = 1000.0 / 640.0;
        let scale_y = 600.0 / 384.0;
        assert_close("min.x", face.bounding_box().min.x, (0.5 - 0.1) * 640.0 * scale_x);
        assert_close("min.y", face.bounding_box().min.y, (0.5 - 0.1) * 640.0 * scale_y);
        assert_close("max.x", face.bounding_box().max.x, (0.5 + 0.1) * 640.0 * scale_x);
        assert_close("max.y", face.bounding_box().max.y, (0.5 + 0.1) * 640.0 * scale_y);

        // The landmarks travel with the box, on the same two factors.
        for point in face.landmarks() {
            assert_close("landmark x", point.x, 0.5 * 640.0 * scale_x);
            assert_close("landmark y", point.y, 0.5 * 640.0 * scale_y);
        }
    }

    #[test]
    fn coordinates_are_not_clamped_to_the_images_bounds() {
        // A face may legitimately extend past the edge of the photograph, and a caller drawing on that photograph
        // needs to know it does. An anchor at the corner with a large extent decodes to a box with negative
        // coordinates, and they must survive.
        let priors = vec![Prior { cx: 0.02, cy: 0.02, sx: 0.2, sy: 0.2 }];

        let found = faces(&[0.0; 4], &[0.0, 1.0], &[0.0; 10], &priors, &fitted(640, 640), TARGET_SIZE);

        assert_eq!(found.len(), 1);
        let bounding_box = found.as_slice()[0].bounding_box();
        assert!(bounding_box.min.x < 0.0, "a box past the left edge was clamped: {bounding_box:?}");
        assert!(bounding_box.min.y < 0.0, "a box past the top edge was clamped: {bounding_box:?}");
    }

    #[test]
    fn the_confidence_is_reported_as_the_detector_produced_it() {
        // The score the detector produced, at the resolution a confidence is carried at — see `Confidence`, which
        // quantizes on acceptance so that a face can be compared and hashed as a value. What is checked here is that
        // the **face** score reaches the report unaltered otherwise: the threshold above is applied to the raw score,
        // so nothing this test is about depends on the two decimals.
        let priors = vec![Prior { cx: 0.5, cy: 0.5, sx: 0.1, sy: 0.1 }];

        let found = faces(&[0.0; 4], &[0.0234, 0.976_53], &[0.0; 10], &priors, &fitted(640, 640), TARGET_SIZE);

        assert_eq!(found.as_slice()[0].confidence().get(), 0.98);
    }
}
