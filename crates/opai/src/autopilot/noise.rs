//! The noise signal: whether a photograph shows visible grain, judged where it has the least detail of its own.
//!
//! Per block and per channel, the signal reads **Immerkær's fast noise estimate** (1996): the mean absolute response of
//! the kernel `[1 -2 1; -2 4 -2; 1 -2 1]`, scaled by `sqrt(pi/2) / 6`. The kernel is zero on constant regions, linear
//! ramps and smooth curvature, so what it responds to is mostly noise. Texture excites it too, so only the **flattest**
//! blocks are read: the tenth with the least luma gradient, and at least [`MIN_BLOCKS`].
//!
//! From those it takes three medians: the luma noise, the chroma noise (the root-sum-square of the `B - Y` and `R - Y`
//! estimates) and the luma itself, since sensor noise grows with brightness. The decision is one linear score over the
//! three, and suggests when it is positive. The weights were fitted on public datasets.

// A median over the flattest blocks rather than a mean over all of them: a mean reads a photograph full of grass as
// grainy, which the spec forbids.
//
// Every window is read, including those across a JPEG's 8-pixel block edges. Skipping them was measured and made no
// difference to detection, on JPEG and PNG sources alike. Nor did Chen, Zhu and Heng's PCA estimator, or 32-pixel
// blocks. The measurements are in the design of the OpenSpec change `add-autopilot-noise-sharpness`.

use super::blocks::Blocks;

/// The fewest blocks the signal concludes from. With fewer, it concludes nothing.
pub(super) const MIN_BLOCKS: usize = 4;

/// The share of the blocks read as the flattest.
const FLATTEST: f64 = 0.1;

/// The least share of a block's positions that must be measurable for its estimate to count. A block of blown sky has
/// none.
const MEASURABLE: f64 = 0.5;

// Fitted so that at most 5% of clean photographs, and of SPAQ's third rated least noisy, are flagged, and so that every
// scenario the tests draw stays on its side with a margin of 0.5. The second condition is what sets the constant: an
// unconstrained fit reached the same detection with most of its weight on chroma, and then read the colour edges in a
// detailed clean photograph as grain. The median luma weighs against suggesting, because grain shows most in shadows.
/// The score's weights: a constant, then the luma noise and the chroma noise on the 0 to 255 scale, and the median luma
/// on `[0, 1]`.
const WEIGHTS: [f64; 4] = [-0.782, 1.0, 0.6, -4.5];

/// What the noise signal measured over the flattest blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Noise {
    /// The median luma noise, as a standard deviation on the 0 to 255 scale.
    pub(super) luma: f64,
    /// The median chroma noise, on the same scale.
    pub(super) chroma: f64,
    /// The median of the blocks' mean luma, on `[0, 1]`.
    pub(super) median: f64,
}

impl Noise {
    /// Reads the noise off the flattest blocks, or `None` where fewer than [`MIN_BLOCKS`] could be measured.
    pub(super) fn measure(blocks: &Blocks) -> Option<Self> {
        let mut measured: Vec<(f64, [f64; 3], f64)> = blocks
            .blocks
            .iter()
            .filter_map(|block| {
                let estimates =
                    immerkaer([&block.luma, &block.chroma[0], &block.chroma[1]], &block.clipped, blocks.size)?;

                Some((activity(&block.luma, blocks.size), estimates, mean(&block.luma)))
            })
            .collect();

        if measured.len() < MIN_BLOCKS {
            return None;
        }

        measured.sort_by(|a, b| a.0.total_cmp(&b.0));
        let flattest = ((measured.len() as f64 * FLATTEST).ceil() as usize).max(MIN_BLOCKS);
        let flattest = &measured[..flattest];

        let median_of = |value: fn(&(f64, [f64; 3], f64)) -> f64| median(flattest.iter().map(value).collect());

        Some(Self {
            luma: 255.0 * median_of(|(_, [luma, _, _], _)| *luma),
            chroma: 255.0 * median_of(|(_, [_, blue, red], _)| blue.hypot(*red)),
            median: median_of(|(_, _, mean)| *mean),
        })
    }

    /// How strongly the photograph reads as grainy. It is suggested above zero.
    pub(super) fn score(&self) -> f64 {
        let [constant, luma, chroma, median] = WEIGHTS;
        constant + luma * self.luma + chroma * self.chroma + median * self.median
    }

    /// Whether the grain is visible enough to suggest denoise.
    ///
    /// The weights were fitted to hold false alarms down before anything else, so a borderline photograph is left
    /// alone, for the reason light adjustment's is.
    pub(super) fn calls_for_denoise(&self) -> bool {
        self.score() > 0.0
    }
}

/// Immerkær's estimate of the noise in each of `channels` of a block, as standard deviations on the channels' own scale,
/// or `None` where too few positions could be measured.
///
/// A position is skipped where its window touches a clipped sample, where clipping has removed the noise. Which
/// positions those are depends on `clipped` alone, so the channels are measured together and either all or none are.
pub(super) fn immerkaer<const N: usize>(channels: [&[f32]; N], clipped: &[bool], size: u32) -> Option<[f64; N]> {
    let side = size as usize;
    let mut sums = [0.0_f64; N];
    let mut counted = 0_usize;
    let mut eligible = 0_usize;

    for y in 1..side - 1 {
        for x in 1..side - 1 {
            eligible += 1;

            let window = |dx: usize, dy: usize| (y + dy - 1) * side + x + dx - 1;
            if (0..3).any(|dy| (0..3).any(|dx| clipped[window(dx, dy)])) {
                continue;
            }

            for (sum, channel) in sums.iter_mut().zip(channels) {
                let at = |dx: usize, dy: usize| f64::from(channel[window(dx, dy)]);
                let response = at(0, 0) - 2.0 * at(1, 0) + at(2, 0) - 2.0 * at(0, 1) + 4.0 * at(1, 1) - 2.0 * at(2, 1)
                    + at(0, 2)
                    - 2.0 * at(1, 2)
                    + at(2, 2);

                *sum += response.abs();
            }
            counted += 1;
        }
    }

    (counted > 0 && counted as f64 >= eligible as f64 * MEASURABLE)
        .then(|| sums.map(|sum| std::f64::consts::FRAC_PI_2.sqrt() / 6.0 * sum / counted as f64))
}

/// How much detail a block's luma has of its own: its mean gradient magnitude, by forward differences.
pub(super) fn activity(luma: &[f32], size: u32) -> f64 {
    let side = size as usize;
    let mut sum = 0.0_f64;

    for y in 0..side - 1 {
        for x in 0..side - 1 {
            let here = luma[y * side + x];
            let dx = luma[y * side + x + 1] - here;
            let dy = luma[(y + 1) * side + x] - here;
            sum += f64::from(dx.hypot(dy));
        }
    }

    sum / ((side - 1) * (side - 1)) as f64
}

fn mean(values: &[f32]) -> f64 {
    values.iter().map(|value| f64::from(*value)).sum::<f64>() / values.len() as f64
}

/// The median of `values`, which must not be empty.
pub(super) fn median(mut values: Vec<f64>) -> f64 {
    let middle = values.len() / 2;
    *values.select_nth_unstable_by(middle, f64::total_cmp).1
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::DynamicImage;

    use super::super::blocks::{self, BLOCK};
    use super::super::fixtures::{colour_grainy, detailed, grainy, scene, textured};

    fn noise(source: &DynamicImage) -> Noise {
        Noise::measure(&blocks::read(source)).expect("enough blocks")
    }

    #[test]
    fn a_clean_photograph_is_left_alone() {
        for (name, source) in [("scene", scene(512, 384)), ("detailed", detailed(512, 384))] {
            let noise = noise(&source);

            assert!(!noise.calls_for_denoise(), "{name}: {noise:?} scored {}", noise.score());
        }
    }

    #[test]
    fn a_grainy_photograph_calls_for_denoise() {
        let noise = noise(&grainy(&scene(512, 384), 8.0, 1));

        assert!(noise.luma > 4.0, "{noise:?}");
        assert!(noise.calls_for_denoise(), "{noise:?} scored {}", noise.score());
    }

    #[test]
    fn grain_in_the_colour_alone_calls_for_denoise() {
        let noise = noise(&colour_grainy(&scene(512, 384), 10.0, 2));

        assert!(noise.luma < 1.0, "the brightness is not clean: {noise:?}");
        assert!(noise.calls_for_denoise(), "{noise:?} scored {}", noise.score());
    }

    #[test]
    fn a_clean_photograph_full_of_fine_texture_is_left_alone() {
        let source = textured(512, 384);
        let noise = noise(&source);

        // The texture alone, read over every block as the design's rejected mean would read it, is grain: it is the
        // flattest blocks that tell them apart.
        let blocks = blocks::read(&source);
        let all: Vec<f64> = blocks
            .blocks
            .iter()
            .filter_map(|block| immerkaer([&block.luma], &block.clipped, blocks.size).map(|[luma]| luma))
            .collect();
        let mean = 255.0 * all.iter().sum::<f64>() / all.len() as f64;
        assert!(mean > 4.0, "the fixture's texture is not fine enough to look like grain: {mean}");

        assert!(!noise.calls_for_denoise(), "{noise:?} scored {}", noise.score());
    }

    #[test]
    fn stronger_grain_is_found_wherever_weaker_grain_is() {
        for source in [scene(512, 384), detailed(512, 384), textured(512, 384)] {
            assert!(!noise(&source).calls_for_denoise(), "the clean photograph is itself suggested");

            for grain in [grainy as fn(&DynamicImage, f64, u64) -> DynamicImage, colour_grainy] {
                let scores: Vec<f64> =
                    (1..=12).map(|step| noise(&grain(&source, f64::from(step), 3)).score()).collect();
                let fired: Vec<bool> = scores.iter().map(|score| *score > 0.0).collect();

                assert!(fired.last().copied().unwrap_or_default(), "the strongest grain was missed: {scores:?}");
                assert!(!fired.windows(2).any(|pair| pair[0] && !pair[1]), "found, then missed: {scores:?}");
            }
        }
    }

    #[test]
    fn the_estimate_reads_the_noise_it_was_given() {
        // Flat grey with known noise: the estimate is within a tenth of it.
        let flat = DynamicImage::ImageRgb16(image::ImageBuffer::from_pixel(512, 384, image::Rgb([30000_u16; 3])));
        let noise = noise(&grainy(&flat, 6.0, 4));
        let luma = 6.0 * (0.2126_f64.powi(2) + 0.7152_f64.powi(2) + 0.0722_f64.powi(2)).sqrt();

        assert!((noise.luma / luma - 1.0).abs() < 0.1, "{noise:?}, expected {luma}");
    }

    #[test]
    fn a_block_that_is_mostly_clipped_is_not_measured() {
        let size = BLOCK as usize;
        let estimate = immerkaer([&vec![1.0; size * size]], &vec![true; size * size], BLOCK);

        assert!(estimate.is_none());
    }

    #[test]
    fn too_few_blocks_conclude_nothing() {
        let small = DynamicImage::ImageRgb16(image::ImageBuffer::from_pixel(192, 64, image::Rgb([30000_u16; 3])));

        assert!(Noise::measure(&blocks::read(&small)).is_none());
    }
}
