//! The colour signal: whether a photograph's colours are shifted as a whole towards one hue, judged by an estimate of
//! the colour of the light it was taken under.
//!
//! The estimate is **Shades of Gray** (Finlayson and Trezzi): per channel, the 6th root of the mean 6th power of the
//! linear value, over the samples neither too dark nor too clipped to carry colour. A cast is a per-channel gain in
//! linear light, and this estimate is homogeneous, so a gain of `g` on a channel moves its estimate by `g`: a stronger
//! cast of the same hue moves it further the same way, never back. A pixel with R = G = B adds equally to every channel,
//! so it pulls the estimate towards neutral rather than towards any hue.
//!
//! The decision is one linear score over the estimate's angle from neutral and its chromaticity. The chromaticity is
//! what makes it hue-aware: correctly balanced photographs lean warm on average, so one angle for every hue would find
//! warm casts more readily than cool ones. The weights were fitted on public datasets.

// The reference's colour heuristic is not reproduced. It counts a pixel with no hue as red, so it flags greyscale,
// overexposed and white-background photographs, and its near-grey test is a fixed saturation cutoff that a strong cast
// pushes pixels past, so a stronger cast is detected less often than a weaker one.
//
// Nor are the CIELab mean shift and least-chromatic evidence this signal was first written with. Measured on real
// renderings they found three fifths of the casts over 15 degrees; this estimate finds over four fifths. The least
// chromatic samples of a cast photograph are the ones whose own colour cancels the cast, so they say little about the
// light.

use super::pass::{NORM, Pass};

// Fitted as a logistic direction between correct renderings and casts of 10 degrees or more, then offset so that 5% of
// correct renderings are flagged. Blue weighs against red: a cool estimate is flagged at a smaller angle than a warm
// one, because correct photographs lean warm.
/// The score's weights: a constant, then the angle from neutral in degrees, the red chromaticity and the blue.
const WEIGHTS: [f64; 4] = [-8.4379, 1.0, -22.7009, 22.7594];

/// What the colour signal measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Cast {
    /// The Shades of Gray estimate of the light, in linear RGB. Only its direction means anything.
    pub(super) light: [f64; 3],
}

impl Cast {
    /// Reads the estimate off a pass, or `None` where no sample carried evidence.
    pub(super) fn measure(pass: &Pass) -> Option<Self> {
        let gray = &pass.gray;
        let count = gray.count as f64;

        (gray.count > 0 && gray.sums.iter().any(|sum| *sum > 0.0))
            .then(|| Self { light: gray.sums.map(|sum| (sum / count).powf(1.0 / f64::from(NORM))) })
    }

    /// The angle between the estimate and neutral grey, in degrees.
    pub(super) fn angle(&self) -> f64 {
        let [r, g, b] = self.light;
        let norm = (r * r + g * g + b * b).sqrt();
        let cosine = (r + g + b) / (norm * 3.0_f64.sqrt());

        cosine.min(1.0).acos().to_degrees()
    }

    /// The estimate's red and blue shares of its sum, each `1/3` at neutral.
    pub(super) fn chromaticity(&self) -> [f64; 2] {
        let [r, g, b] = self.light;
        let sum = r + g + b;

        [r / sum, b / sum]
    }

    /// How strongly the photograph reads as cast. It is suggested above zero.
    pub(super) fn score(&self) -> f64 {
        let [constant, angle, red, blue] = WEIGHTS;
        let [r, b] = self.chromaticity();

        constant + angle * self.angle() + red * r + blue * b
    }

    /// Whether the photograph has a cast worth suggesting colour balance for.
    ///
    /// The weights were fitted to hold false alarms down before anything else, so a borderline photograph is left
    /// alone, for the reason light adjustment's is.
    pub(super) fn calls_for_balance(&self) -> bool {
        self.score() > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::DynamicImage;

    use super::super::fixtures::{cool, foliage, greyscale, rendered, scene, warm};
    use super::super::pass;

    fn cast(source: &DynamicImage) -> Cast {
        Cast::measure(&pass::read(source)).expect("some evidence")
    }

    #[test]
    fn a_correctly_balanced_photograph_is_left_alone() {
        let cast = cast(&scene(512, 384));

        assert!(!cast.calls_for_balance(), "{cast:?} scored {}", cast.score());
    }

    #[test]
    fn a_warm_cast_calls_for_balance() {
        let cast = cast(&rendered(&scene(512, 384), 0.0, warm(1.0)));

        assert!(cast.chromaticity()[0] > cast.chromaticity()[1], "not warm: {cast:?}");
        assert!(cast.calls_for_balance(), "{cast:?} scored {}", cast.score());
    }

    #[test]
    fn a_cool_cast_calls_for_balance() {
        let cast = cast(&rendered(&scene(512, 384), 0.0, cool(1.0)));

        assert!(cast.chromaticity()[1] > cast.chromaticity()[0], "not cool: {cast:?}");
        assert!(cast.calls_for_balance(), "{cast:?} scored {}", cast.score());
    }

    #[test]
    fn a_greyscale_photograph_has_no_hue_and_is_left_alone() {
        let cast = cast(&greyscale(512, 384));

        assert!(
            cast.light[0] == cast.light[1] && cast.light[1] == cast.light[2],
            "a grey was given a hue: {cast:?}"
        );
        assert!(cast.angle() < 1e-6, "{cast:?} is {} degrees from neutral", cast.angle());
        assert!(!cast.calls_for_balance());
    }

    #[test]
    fn a_colourful_photograph_with_neutral_surfaces_is_left_alone() {
        let cast = cast(&foliage(512, 384));

        assert!(!cast.calls_for_balance(), "{cast:?} scored {}", cast.score());
    }

    #[test]
    fn a_stronger_cast_is_found_wherever_a_weaker_one_of_the_same_hue_is() {
        let source = scene(512, 384);
        assert!(!cast(&source).calls_for_balance(), "the unaltered photograph is itself suggested");

        for (hue, gains) in [("warm", warm as fn(f64) -> [f64; 3]), ("cool", cool)] {
            let scores: Vec<f64> = (1..=12)
                .map(|step| cast(&rendered(&source, 0.0, gains(f64::from(step) / 8.0))).score())
                .collect();
            let fired: Vec<bool> = scores.iter().map(|score| *score > 0.0).collect();

            assert!(fired.last().copied().unwrap_or_default(), "{hue}: the strongest cast was missed: {scores:?}");
            assert!(!fired.windows(2).any(|pair| pair[0] && !pair[1]), "{hue}: found, then missed: {scores:?}");
        }
    }

    #[test]
    fn the_estimate_moves_with_a_gain_on_one_channel() {
        // A stop down, so that the gain pushes no sample past the clipping edge of the evidence band. At the 6th power
        // the brightest samples carry the estimate, so losing them would bend it, which is the band doing its job
        // rather than the estimate failing to move.
        let source = rendered(&scene(512, 384), -1.0, [1.0; 3]);
        let base = cast(&source).light;
        let red = cast(&rendered(&source, 0.0, [1.2, 1.0, 1.0])).light;

        // Homogeneity, up to the 16-bit rounding.
        let moved = (red[0] / red[1]) / (base[0] / base[1]);
        assert!((moved - 1.2).abs() < 0.01, "a red gain of 1.2 moved the estimate by {moved}");
    }
}
