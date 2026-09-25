//! The sharpness signal: whether a photograph is genuinely blurred, so that even its sharpest part is soft.
//!
//! Per block, the signal reads the **blur effect** of Crété-Roffet et al. (2007) on luma. It re-blurs the block with a
//! 9-tap box filter along each axis, and along each axis sums the absolute neighbour differences of the original and
//! how much of each the re-blur removes. A sharp block loses much of its detail when re-blurred, and a blurred block has
//! little left to lose. The block's blur is the larger of the two axes' share kept, and its sharpness is one minus that.
//! Because the measure is a ratio, it depends far less on what the block shows than the variance of the Laplacian does.
//!
//! **Grain is gated out.** A position counts only where its difference on a 3x3-smoothed copy of the block exceeds `k`
//! times the photograph's luma noise, from the noise signal. Differences that are only grain then drop out, and a
//! blurred photograph with grain in it is judged on its blur.
//!
//! **Only blocks with detail count**, and the sharpness is pooled at the **90th percentile**: a portrait with a blurred
//! background has a few sharp blocks on its subject and is left alone, where a shaken or misfocused photograph has no
//! sharp block anywhere.
//!
//! The decision is one linear score over that percentile and the base-2 log of the photograph's pixel count, and
//! suggests when it is positive. The weights were fitted on public datasets.

// The pixel-count term exists because, pixel for pixel, a photograph from a high-resolution camera is softer than a
// small one. The spec forbids suggesting sharpen for that softness alone, and a fixed threshold would suggest it for
// most photographs from a modern phone.
//
// A gate rather than the discount first planned, which reduced every difference by `k` times the noise. No `k` worked:
// the grain differences that survive a discount are removed entirely by the re-blur, which is what sharp detail looks
// like, and a discount large enough to remove them removes a soft edge too. A blurred photograph with grain read as
// sharp at every multiple tried.
//
// Four directions rather than Crété-Roffet's two axes. Along an axis at an angle to a shake, the structure across the
// shake is still crossed at full resolution, so an oblique shake read along the axes alone looks sharp; with the
// diagonals no shake is more than 22.5 degrees off a direction read. On the calibration data it found 5 points more.
//
// Crété-Roffet rather than CPBD (Narvekar and Karam 2011), which was measured beside it. CPBD found more of the mildest
// synthetic blur, but fewer of the real blurred photographs, which are what a user opens.
//
// A percentile rather than a mean or a median, which would suggest sharpen for every photograph with a blurred
// background. Not the variance of the Laplacian, which is unbounded and content-dependent: plain scenes read as blurred
// and grain inflates it.

use super::blocks::Blocks;
use super::noise::MIN_BLOCKS;

/// The length of the re-blur, in pixels.
const TAPS: usize = 9;

/// How many times the photograph's luma noise a smoothed neighbour difference must exceed to count.
const K: f64 = 1.5;

/// The least contrast a block needs to have detail: its neighbour differences summed over the positions that count and
/// averaged over every position, on the 0 to 255 scale.
const CONTRAST_MIN: f64 = 0.5;

/// The directions a block's blur is read along, as steps between neighbours: across, down and both diagonals.
const DIRECTIONS: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

/// Where the blocks' sharpness is pooled.
const PERCENTILE: f64 = 0.9;

// Fitted so that at most 5% of sharp photographs, and of SPAQ's third rated sharpest, are flagged. `K` is 1.5 rather
// than the 1 that detected 0.8 points more, which is within the sample's noise: at 1, grain moves a blurred photograph
// more than a third of the way towards a sharp one. The contrast minimum is 0.5 rather than 0.25, which detected 1.5
// points more but let a steep smooth gradient, as a sky can be, count as detail and read as blurred. The pixel-count
// term came out small and positive: across the calibration data, larger photographs were not softer pixel for pixel
// once their sharpest blocks were read.
/// The score's weights: a constant, then the 90th-percentile sharpness and the base-2 log of the pixel count.
const WEIGHTS: [f64; 3] = [0.4509, -1.0, 0.0077];

/// What the sharpness signal measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Sharpness {
    /// The sharpness of the blocks with detail, at the 90th percentile, on `[0, 1]`.
    pub(super) sharpest: f64,
    /// How many blocks had detail.
    pub(super) detailed: usize,
    /// The base-2 log of the photograph's pixel count.
    pub(super) resolution: f64,
}

impl Sharpness {
    /// Reads the sharpness off the blocks of a photograph of `pixels` pixels whose luma noise is `noise`, on the 0 to 255
    /// scale, or `None` where fewer than [`MIN_BLOCKS`] have detail.
    pub(super) fn measure(blocks: &Blocks, noise: f64, pixels: u64) -> Option<Self> {
        Self::pool(&per_block(blocks, noise, K), CONTRAST_MIN, pixels)
    }

    /// The 90th-percentile sharpness over the blocks in `per_block` whose contrast reaches `contrast_min`, or `None`
    /// where fewer than [`MIN_BLOCKS`] do.
    pub(super) fn pool(per_block: &[(f64, f64)], contrast_min: f64, pixels: u64) -> Option<Self> {
        let mut sharpness: Vec<f64> = per_block
            .iter()
            .filter(|(_, contrast)| *contrast >= contrast_min)
            .map(|(blur, _)| 1.0 - blur)
            .collect();

        if sharpness.len() < MIN_BLOCKS {
            return None;
        }

        let rank = ((sharpness.len() as f64 * PERCENTILE).ceil() as usize).clamp(1, sharpness.len()) - 1;
        let sharpest = *sharpness.select_nth_unstable_by(rank, f64::total_cmp).1;

        Some(Self { sharpest, detailed: sharpness.len(), resolution: (pixels.max(1) as f64).log2() })
    }

    /// How strongly the photograph reads as blurred. It is suggested above zero.
    pub(super) fn score(&self) -> f64 {
        let [constant, sharpest, resolution] = WEIGHTS;
        constant + sharpest * self.sharpest + resolution * self.resolution
    }

    /// Whether the photograph is blurred enough to suggest sharpen.
    ///
    /// The weights were fitted to hold false alarms down before anything else, so a borderline photograph is left
    /// alone, for the reason light adjustment's is.
    pub(super) fn calls_for_sharpen(&self) -> bool {
        self.score() > 0.0
    }
}

/// The blur effect and contrast of every block, as [`blur_effect`] reads them with the gate at `k` times the luma
/// `noise`, on the 0 to 255 scale.
pub(super) fn per_block(blocks: &Blocks, noise: f64, k: f64) -> Vec<(f64, f64)> {
    let gate = (k * noise / 255.0) as f32;

    blocks.blocks.iter().filter_map(|block| blur_effect(&block.luma, blocks.size, gate)).collect()
}

/// The blur effect of one block's luma on `[0, 1]`, and its contrast: the neighbour differences summed over the
/// positions that count and averaged over every position, gated or not, on the 0 to 255 scale. `None` where no
/// position counts.
///
/// The blur is read along each of [`DIRECTIONS`] and the most blurred one is kept, so that a blur along one direction
/// only, as camera shake leaves, is found whichever way it runs.
///
/// A position counts where the difference on a 3x3-smoothed copy of the block exceeds `gate`. The smoothing divides
/// grain by three and barely touches an edge, so a gate set a few times above the grain keeps edges, however soft,
/// and drops the differences that are only grain. At the positions that count, the ratio is Crété-Roffet's, over the
/// block's own differences.
pub(super) fn blur_effect(luma: &[f32], size: u32, gate: f32) -> Option<(f64, f64)> {
    let side = size as isize;
    let reach = (TAPS / 2) as isize;

    let mut smooth = vec![0.0_f32; luma.len()];
    for y in 1..side - 1 {
        for x in 1..side - 1 {
            let window = (y - 1..=y + 1).flat_map(|y| (x - 1..=x + 1).map(move |x| luma[(y * side + x) as usize]));
            smooth[(y * side + x) as usize] = window.sum::<f32>() / 9.0;
        }
    }

    let at = |image: &[f32], x: isize, y: isize| image[(y * side + x) as usize];

    let mut blur: Option<f64> = None;
    let mut total = 0.0_f64;
    let mut positions = 0_usize;
    let mut reblur = vec![0.0_f32; luma.len()];

    for (dx, dy) in DIRECTIONS {
        let (mut differences, mut removed) = (0.0_f64, 0.0_f64);

        // The re-blur at every position whose taps are all inside the block: the mean of the taps either side of it
        // along the direction. Filled once per direction, since each position is read both as `(x, y)` and as the
        // step behind its neighbour. A position left out holds the last direction's value, and none is read.
        let span = |v: isize, d: isize| v - reach * d.abs() >= 0 && v + reach * d.abs() < side;
        for y in (0..side).filter(|y| span(*y, dy)) {
            for x in (0..side).filter(|x| span(*x, dx)) {
                reblur[(y * side + x) as usize] =
                    (-reach..=reach).map(|i| at(luma, x + i * dx, y + i * dy)).sum::<f32>() / TAPS as f32;
            }
        }

        // Every position whose difference, re-blur and smoothed value are all inside the block: from `reach + 1`
        // steps back to `reach` steps on, and off the outermost pixels where there is no smoothed value.
        let inside = |v: isize, d: isize| {
            let (low, high) = (v - (reach + 1) * d, v + reach * d);
            low.min(high) >= 1 && low.max(high) < side - 1
        };

        for y in 0..side {
            for x in 0..side {
                if !inside(x, dx) || !inside(y, dy) {
                    continue;
                }
                positions += 1;

                let smoothed = at(&smooth, x, y) - at(&smooth, x - dx, y - dy);
                if smoothed.abs() <= gate {
                    continue;
                }

                let original = (at(luma, x, y) - at(luma, x - dx, y - dy)).abs();
                let reblurred = (at(&reblur, x, y) - at(&reblur, x - dx, y - dy)).abs();

                differences += f64::from(original);
                removed += f64::from((original - reblurred).max(0.0));
            }
        }

        total += differences;
        // A direction with nothing to measure says nothing about the blur along it.
        if differences > 0.0 {
            let kept = (differences - removed) / differences;
            blur = Some(blur.map_or(kept, |other| other.max(kept)));
        }
    }

    blur.map(|blur| (blur, 255.0 * total / positions as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::DynamicImage;

    use super::super::blocks::{self, BLOCK};
    use super::super::fixtures::{defocused, detailed, grainy, in_focus_against, shaken, smooth};
    use super::super::noise::Noise;

    /// The sharpness of `source`, discounting its own noise as the analysis does.
    fn sharpness(source: &DynamicImage) -> Option<Sharpness> {
        let blocks = blocks::read(source);
        let noise = Noise::measure(&blocks).map_or(0.0, |noise| noise.luma);
        let pixels = u64::from(source.width()) * u64::from(source.height());

        Sharpness::measure(&blocks, noise, pixels)
    }

    fn calls_for_sharpen(source: &DynamicImage) -> bool {
        sharpness(source).is_some_and(|sharpness| sharpness.calls_for_sharpen())
    }

    #[test]
    fn a_sharp_photograph_is_left_alone() {
        let sharpness = sharpness(&detailed(512, 384)).expect("detail");

        assert!(!sharpness.calls_for_sharpen(), "{sharpness:?} scored {}", sharpness.score());
    }

    #[test]
    fn an_out_of_focus_photograph_calls_for_sharpen() {
        let sharpness = sharpness(&defocused(&detailed(512, 384), 4.0)).expect("detail");

        assert!(sharpness.calls_for_sharpen(), "{sharpness:?} scored {}", sharpness.score());
    }

    #[test]
    fn a_shaken_photograph_calls_for_sharpen() {
        for angle in [0.0, 30.0, 90.0] {
            let sharpness = sharpness(&shaken(&detailed(512, 384), 12.0, angle)).expect("detail");

            assert!(sharpness.calls_for_sharpen(), "{angle}: {sharpness:?} scored {}", sharpness.score());
        }
    }

    #[test]
    fn a_sharp_subject_against_a_blurred_background_is_left_alone() {
        let sharp = detailed(512, 384);
        let source = in_focus_against(&sharp, &defocused(&sharp, 8.0));
        let sharpness = sharpness(&source).expect("detail");

        assert!(!sharpness.calls_for_sharpen(), "{sharpness:?} scored {}", sharpness.score());
    }

    #[test]
    fn a_photograph_with_no_detail_concludes_nothing() {
        assert!(sharpness(&smooth(512, 384)).is_none(), "{:?}", sharpness(&smooth(512, 384)));
        assert!(!calls_for_sharpen(&smooth(512, 384)));
    }

    #[test]
    fn a_blurred_photograph_with_grain_in_it_calls_for_sharpen_and_denoise() {
        let source = grainy(&defocused(&detailed(512, 384), 4.0), 8.0, 5);
        let noise = Noise::measure(&blocks::read(&source)).expect("enough blocks");

        assert!(noise.calls_for_denoise(), "{noise:?} scored {}", noise.score());
        assert!(calls_for_sharpen(&source), "{:?}", sharpness(&source));
    }

    #[test]
    fn stronger_blur_is_found_wherever_weaker_blur_is() {
        let source = detailed(512, 384);
        assert!(!calls_for_sharpen(&source), "the sharp photograph is itself suggested");

        type Blur = fn(&DynamicImage, f64) -> DynamicImage;
        let blurs: [(&str, Blur); 2] = [
            ("defocus", |source, step| defocused(source, step / 2.0)),
            ("shake", |source, step| shaken(source, 2.0 * step, 20.0)),
        ];

        for (name, blur) in blurs {
            let scores: Vec<f64> = (1..=10)
                .map(|step| sharpness(&blur(&source, f64::from(step))).map_or(f64::NEG_INFINITY, |s| s.score()))
                .collect();
            let fired: Vec<bool> = scores.iter().map(|score| *score > 0.0).collect();

            assert!(fired.last().copied().unwrap_or_default(), "{name}: the strongest blur was missed: {scores:?}");
            assert!(!fired.windows(2).any(|pair| pair[0] && !pair[1]), "{name}: found, then missed: {scores:?}");
        }
    }

    #[test]
    fn grain_does_not_read_as_sharpness() {
        let sharp = sharpness(&detailed(512, 384)).expect("detail");
        let blurred = defocused(&detailed(512, 384), 4.0);
        let clean = sharpness(&blurred).expect("detail");
        let noisy = sharpness(&grainy(&blurred, 8.0, 6)).expect("detail");

        // Strong grain moves a blurred photograph less than a third of the way towards a sharp one.
        let moved = (noisy.sharpest - clean.sharpest) / (sharp.sharpest - clean.sharpest);
        assert!(moved < 1.0 / 3.0, "{noisy:?} against {clean:?} and {sharp:?}");
    }

    #[test]
    fn a_block_with_a_hard_edge_is_sharper_than_the_same_edge_blurred() {
        let size = BLOCK as usize;
        let edge: Vec<f32> = (0..size * size).map(|i| if i % size < size / 2 { 0.2 } else { 0.8 }).collect();
        let soft: Vec<f32> = (0..size * size)
            .map(|i| 0.2 + 0.6 * ((i % size) as f32 - 24.0).clamp(0.0, 16.0) / 16.0)
            .collect();

        let (hard, _) = blur_effect(&edge, BLOCK, 0.0).expect("an edge");
        let (blurred, _) = blur_effect(&soft, BLOCK, 0.0).expect("an edge");

        assert!(hard < 0.2 && blurred > 0.6, "hard {hard}, blurred {blurred}");
    }
}
