//! The light signal: whether a photograph's exposure is wrong, read from three percentiles of its brightness.
//!
//! A photograph is **underexposed** when its whole tonal range is compressed into the shadows, so even its brightest
//! part never gets bright. It is **overexposed** when its whole range is lifted off the shadows, so even its darkest
//! part never gets dark. Either way the evidence is a tail of the distribution together with its middle: a night
//! scene that still reaches its highlights, or a high-key one that still has shadows, is exposed by intent.
//!
//! Each direction is one linear score over the darkest tail, the median and the brightest tail, and suggests when it
//! is positive. The weights were fitted on public datasets.

// The reference suggests light adjustment when the mean is extreme and over a third of the pixels are clipped. That
// misses ordinary underexposure, which compresses rather than clips, and fires on low-key and high-key photographs,
// which are clipped by intent. It is not reproduced.
//
// A score rather than a cutoff per percentile. Two cutoffs per direction draw a rectangle, and the boundary between a
// correct photograph and a wrongly exposed one is not one: on real renderings the rectangle found two thirds of the
// underexposed ones where a line through the same three numbers found over four fifths.

use super::pass::{LUMA_BINS, Pass};

/// Where the darkest tail is read: the brightness below which this share of the samples lies.
const LOW_PERCENTILE: f64 = 0.01;

/// Where the brightest tail is read.
const HIGH_PERCENTILE: f64 = 0.995;

// Fitted so that the two directions together flag at most 5% of correct renderings, maximising the weaker direction's
// detection, and so that every scenario the light signal's tests draw stays on its side with a margin. The second
// condition is what keeps the median's weight near zero in the overexposure score: left free, the fit leans on it,
// and a subject on a white background is then overexposed.
/// The underexposure score's weights: a constant, then the darkest tail, the median and the brightest tail.
const UNDER: [f64; 4] = [0.7928, 0.6149, -0.5918, -1.0];

/// The overexposure score's weights, in the same order.
const OVER: [f64; 4] = [-1.0779, 0.646, 0.0192, 1.0];

/// What the light signal measured, each on `[0, 1]` of encoded brightness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Light {
    /// The darkest tail.
    pub(super) low: f64,
    /// The median.
    pub(super) median: f64,
    /// The brightest tail.
    pub(super) high: f64,
}

impl Light {
    /// Reads the three percentiles off a pass, or `None` where it read no samples.
    pub(super) fn measure(pass: &Pass) -> Option<Self> {
        // Every sample counted lands in exactly one bin, so the count is the histogram's total.
        let total = pass.samples;

        (total > 0).then(|| Self {
            low: percentile(&pass.luma, total, LOW_PERCENTILE),
            median: percentile(&pass.luma, total, 0.5),
            high: percentile(&pass.luma, total, HIGH_PERCENTILE),
        })
    }

    /// How strongly the photograph reads as underexposed. It is suggested above zero.
    pub(super) fn under(&self) -> f64 {
        self.score(UNDER)
    }

    /// How strongly the photograph reads as overexposed. It is suggested above zero.
    pub(super) fn over(&self) -> f64 {
        self.score(OVER)
    }

    fn score(&self, [constant, low, median, high]: [f64; 4]) -> f64 {
        constant + low * self.low + median * self.median + high * self.high
    }

    /// Whether the exposure is wrong enough to suggest light adjustment.
    ///
    /// The weights were fitted to hold false alarms down before anything else, so a borderline photograph is left
    /// alone: a missed suggestion costs less than a wrong one on a photograph the user has just opened.
    pub(super) fn calls_for_adjustment(&self) -> bool {
        self.under() > 0.0 || self.over() > 0.0
    }
}

/// The brightness at which `share` of `total` samples lie at or below, as the centre of the bin it falls in.
fn percentile(histogram: &[u64; LUMA_BINS], total: u64, share: f64) -> f64 {
    let wanted = ((total as f64 * share).ceil() as u64).max(1);
    let mut seen = 0;

    let bin = histogram
        .iter()
        .position(|count| {
            seen += count;
            seen >= wanted
        })
        .unwrap_or(LUMA_BINS - 1);

    (bin as f64 + 0.5) / LUMA_BINS as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::DynamicImage;

    use super::super::fixtures::{cut_out, night, on_white, rendered, scene};
    use super::super::pass;

    fn light(source: &DynamicImage) -> Light {
        Light::measure(&pass::read(source)).expect("some samples")
    }

    #[test]
    fn a_correctly_exposed_photograph_is_left_alone() {
        let light = light(&scene(512, 384));

        assert!(!light.calls_for_adjustment(), "{light:?}");
    }

    #[test]
    fn a_photograph_two_stops_under_calls_for_adjustment() {
        let light = light(&rendered(&scene(512, 384), -2.0, [1.0; 3]));

        assert!(light.under() > 0.0, "{light:?}");
        assert!(light.calls_for_adjustment());
    }

    #[test]
    fn a_photograph_two_stops_over_calls_for_adjustment() {
        let light = light(&rendered(&scene(512, 384), 2.0, [1.0; 3]));

        assert!(light.over() > 0.0, "{light:?}");
        assert!(light.calls_for_adjustment());
    }

    #[test]
    fn a_night_scene_with_lights_in_it_is_left_alone() {
        let light = light(&night(512, 384));

        assert!(light.median < 0.15, "the fixture is not dark: {light:?}");
        assert!(!light.calls_for_adjustment(), "{light:?}");
    }

    #[test]
    fn a_subject_on_white_is_left_alone() {
        let light = light(&on_white(512, 384));

        assert!(light.median > 0.99, "the fixture is not mostly white: {light:?}");
        assert!(!light.calls_for_adjustment(), "{light:?}");
    }

    #[test]
    fn a_cut_out_on_transparent_is_measured_by_its_subject_alone() {
        let light = light(&cut_out(512, 384));

        // Counted, the black surround would be four fifths of the samples: the darkest tail and the median both in the
        // first bin.
        assert!(light.low > 1.0 / LUMA_BINS as f64 && light.median > 0.3, "the surround was counted: {light:?}");
        assert!(!light.calls_for_adjustment(), "{light:?}");
    }

    #[test]
    fn a_percentile_is_the_centre_of_the_bin_it_falls_in() {
        let mut histogram = [0; LUMA_BINS];
        histogram[10] = 50;
        histogram[500] = 50;

        assert_eq!(percentile(&histogram, 100, 0.5), 10.5 / LUMA_BINS as f64);
        assert_eq!(percentile(&histogram, 100, 0.51), 500.5 / LUMA_BINS as f64);
    }
}
