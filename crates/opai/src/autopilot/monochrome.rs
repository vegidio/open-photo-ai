//! The monochrome signal: whether a photograph carries no colour at all, and so calls for colorization.
//!
//! The samples are gathered in [`LEVELS`] bands of encoded Rec. 709 luma, and the **residual chroma** is the pooled
//! within-level standard deviation of `B - Y` and `R - Y` together: the colour left once each brightness level's own
//! tint is removed. A grayscale file leaves none, and neither does one that was saved as colour and compressed, since an
//! encoder's rounding is far below the constant. The decision is one score, the log of the residual against a constant.
//!
//! **Only grayscale files are found.** A black-and-white print that was toned, aged or scanned in colour carries
//! colour that its brightness does not explain (stains, uneven toning, the scanner's noise), and measured on real
//! scans that colour is more than the haziest and darkest colour photographs carry. No constant separates the two, so
//! the constant sits where no colour photograph is flagged, and such prints are left alone. The measurements are in the
//! design of the OpenSpec change `add-autopilot-colorization`.

// The residual rather than a plain "every pixel is grey" test, because a grayscale photograph saved as a colour JPEG
// comes back with a few pixels a level or two off grey, and the pooled deviation absorbs that where an exact test
// would not.
//
// Alternatives measured and rejected, none finding more than 1.3% of real scans at a 1% false-alarm budget: the share
// of samples beyond a fixed residual, the residual over a centre crop (to drop a print's mount), the residual after
// averaging the photograph down (to cancel grain and scanner noise), the two together, and the spread off the dominant
// hue rather than in every direction.
//
// Encoded `B - Y` and `R - Y` rather than CIELab's a* and b*: a grey has no chroma in either, and these are the
// differences the block pass already works in. Lab belongs to the colorization family.

use super::pass::{LEVELS, Pass};

/// The fewest samples with no channel clipped the signal needs to conclude anything.
const MIN_SAMPLES: u64 = 1000;

// A clean grey photograph measures exactly zero, and still does after a JPEG round trip. The five colour-set images
// below this were grayscale files with an encoder's rounding in them, and the least residual of any other image in the
// colour sets is 0.0008.
/// The residual chroma, on the `[0, 1]` scale of the encoded channels, below which a photograph is suggested
/// colorization.
const MAX_RESIDUAL: f64 = 0.0005;

/// What the monochrome signal measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Monochrome {
    /// How many samples with no channel clipped were gathered.
    pub(super) samples: u64,
    /// The pooled within-level standard deviation of `B - Y` and `R - Y` together, on the `[0, 1]` scale.
    pub(super) residual: f64,
}

impl Monochrome {
    /// Reads the residual chroma off a pass, or `None` where fewer than [`MIN_SAMPLES`] samples were gathered.
    pub(super) fn measure(pass: &Pass) -> Option<Self> {
        let levels = &pass.levels;
        let samples = levels.count();

        if samples < MIN_SAMPLES {
            return None;
        }

        // Each level's sum of squared deviations from its own mean, in both differences, summed over the levels.
        let within: f64 = (0..LEVELS)
            .filter(|level| levels.counts[*level] > 0)
            .map(|level| {
                let count = levels.counts[level] as f64;
                (0..2)
                    .map(|index| levels.squares[level][index] - levels.sums[level][index].powi(2) / count)
                    .sum::<f64>()
            })
            .sum();

        // Clamped, because a level of identical samples can come out a rounding error below zero.
        Some(Self { samples, residual: (within.max(0.0) / samples as f64).sqrt() })
    }

    /// How strongly the photograph reads as carrying no colour. It is suggested above zero.
    ///
    /// Infinite for a photograph with no residual at all, which is every one whose three channels are equal.
    pub(super) fn score(&self) -> f64 {
        MAX_RESIDUAL.ln() - self.residual.ln()
    }

    /// Whether the photograph carries so little colour that colorization is worth suggesting.
    ///
    /// Colorization replaces a photograph's colour rather than blending with it, so a false alarm destroys colour that
    /// was really there. The constant was set where no colour photograph measured was flagged.
    pub(super) fn calls_for_colorization(&self) -> bool {
        self.score() > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{DynamicImage, ImageBuffer, Luma};
    use rust_sak::image::{EncodeOptions, ImageFormat, decode_bytes, encode_writer};

    use super::super::fixtures::{cut_out, desaturated, detailed, muted, printed, scene};
    use super::super::pass;

    fn monochrome(source: &DynamicImage) -> Option<Monochrome> {
        Monochrome::measure(&pass::read(source))
    }

    /// Whether `source` is suggested colorization, with the measurement in the message where it matters.
    fn suggested(source: &DynamicImage) -> (bool, String) {
        let measured = monochrome(source).expect("enough samples");
        (measured.calls_for_colorization(), format!("{measured:?} scored {}", measured.score()))
    }

    #[test]
    fn a_neutral_black_and_white_photograph_is_suggested() {
        let measured = monochrome(&printed(&detailed(512, 384))).expect("enough samples");

        assert_eq!(measured.residual, 0.0, "{measured:?}");
        assert!(measured.calls_for_colorization());
    }

    #[test]
    fn a_photograph_stored_with_a_single_channel_is_suggested() {
        let gradient = ImageBuffer::from_fn(512, 384, |x, y| Luma([(x * 100 + y * 30) as u16]));

        let (fired, why) = suggested(&DynamicImage::ImageLuma16(gradient));

        assert!(fired, "{why}");
    }

    #[test]
    fn a_grayscale_photograph_saved_as_a_colour_jpeg_is_suggested() {
        let grey = DynamicImage::ImageRgb8(printed(&detailed(512, 384)).to_rgb8());
        let mut bytes = Vec::new();
        encode_writer(&grey, &mut bytes, ImageFormat::Jpeg, Some(EncodeOptions::Jpeg { quality: 85 }))
            .expect("a JPEG encodes");

        let (fired, why) = suggested(&decode_bytes(&bytes).expect("a JPEG decodes"));

        assert!(fired, "{why}");
    }

    #[test]
    fn a_colour_photograph_is_left_alone() {
        for (name, source) in [("scene", scene(512, 384)), ("detailed", detailed(512, 384))] {
            let (fired, why) = suggested(&source);

            assert!(!fired, "{name}: {why}");
        }
    }

    #[test]
    fn a_muted_colour_photograph_is_left_alone() {
        // A sixth of the colour with a small red subject, and a sixteenth of the colour with none.
        for (name, source) in [
            ("small subject", muted(512, 384)),
            ("a sixteenth", desaturated(&detailed(512, 384), 1.0 / 16.0)),
        ] {
            let (fired, why) = suggested(&source);

            assert!(!fired, "{name}: {why}");
        }
    }

    #[test]
    fn a_colour_cut_out_on_a_transparent_background_is_left_alone() {
        let (fired, why) = suggested(&cut_out(512, 384));

        assert!(!fired, "{why}");
    }

    #[test]
    fn a_photograph_with_too_few_unclipped_samples_concludes_nothing() {
        let tiny = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(31, 32, image::Rgb([120, 120, 120])));
        let white = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(512, 384, image::Rgb([255, 255, 255])));

        assert!(monochrome(&tiny).is_none(), "992 samples concluded something");
        assert!(monochrome(&white).is_none(), "a clipped frame concluded something");
    }

    #[test]
    fn a_less_colourful_version_is_suggested_wherever_a_more_colourful_one_is() {
        for (name, source) in [("scene", scene(512, 384)), ("detailed", detailed(512, 384))] {
            // From the photograph itself down to a sixty-fourth of its colour, then none.
            let keeps = [1.0, 0.5, 0.25, 0.125, 0.0625, 1.0 / 32.0, 1.0 / 64.0, 0.0];
            let scores: Vec<f64> = keeps
                .iter()
                .map(|keep| monochrome(&desaturated(&source, *keep)).expect("enough samples").score())
                .collect();
            let fired: Vec<bool> = scores.iter().map(|score| *score > 0.0).collect();

            assert!(fired.last().copied().unwrap_or_default(), "{name}: grey was missed: {scores:?}");
            assert!(!fired[0], "{name}: the photograph itself was suggested: {scores:?}");
            assert!(!fired.windows(2).any(|pair| pair[0] && !pair[1]), "{name}: found, then missed: {scores:?}");
        }
    }
}
