//! The one pass over a photograph's pixels that the light, colour and monochrome signals all read.
//!
//! It reads a regular grid of samples through [`Sampler`], so an 8-bit and a 16-bit source take one path and a 16-bit
//! one is measured at 16 bits. Each sample is added to three sets of accumulators: a brightness histogram for the light
//! signal, the Shades of Gray sums for the colour signal, and the chroma of each brightness level for the monochrome
//! signal. No signal reads a pixel of its own.

// A stride rather than a resize. A resize costs the full-resolution pass the bound exists to avoid, and averaging
// neighbours narrows the tails of the brightness distribution, which are exactly what the light signal reads. A
// regular subsample of a million points reproduces the whole frame's percentiles and colour sums far more closely than
// either signal's thresholds need.
//
// Every accumulator is per sample, with no neighbour read. That is what rules out the colour estimators that need
// gradients, such as Gray Edge.
//
// No cancellation inside the loop. A million samples is a few tens of milliseconds, and the caller checks before and
// after the pass.

use std::sync::LazyLock;

#[cfg(test)]
use image::DynamicImage;
use imaging::tensor::Sampler;

use crate::models::colorization::srgb_to_linear;

/// The most samples one pass reads: about a megapixel, whatever the photograph's size.
pub(super) const MAX_SAMPLES: u64 = 1_048_576;

/// How many bins the brightness histogram has.
///
/// More than 256 so that a 16-bit source is not quantised back to 8 bits before its percentiles are read.
pub(super) const LUMA_BINS: usize = 1024;

// At half rather than at zero, because the soft edge of a cut-out is premultiplied towards black too, and zero would
// let the darkest part of it through. Anti-aliased edges are thin, so the exact cutoff matters little.
/// The least alpha a sample needs to be read. Below it, its premultiplied colour is mostly the black it was composited
/// against, and counting it would make every cut-out look dark.
pub(super) const OPAQUE_ENOUGH: u16 = u16::MAX / 2 + 1;

/// The Minkowski norm of the Shades of Gray estimate.
pub(super) const NORM: i32 = 6;

/// The brightest linear channel a sample needs to be colour evidence. Below it, it is too dark to carry any colour.
const EVIDENCE_MIN: f64 = 0.01;

/// The brightest linear channel a sample may have and still be colour evidence. Above it, it is clipped, and its
/// colour is the clipping's rather than the light's.
const EVIDENCE_MAX: f64 = 0.95;

/// How many brightness levels the monochrome signal's chroma is gathered in.
pub(super) const LEVELS: usize = 32;

// Half an 8-bit step, so that an 8-bit source's 0 and 255 are clipped and its 1 and 254 are not. One margin for both
// passes: a clipped channel has lost its colour here and its noise in the block pass, and at the same place.
/// How close to either end of the range a channel may come before the sample counts as clipped.
const CLIP_MARGIN: u16 = 128;

/// Whether any channel of `rgb` is within [`CLIP_MARGIN`] of either end of its range.
pub(super) fn clipped(rgb: [u16; 3]) -> bool {
    rgb.iter().any(|channel| *channel <= CLIP_MARGIN || *channel >= u16::MAX - CLIP_MARGIN)
}

// Not colorization's table, which is kept in the exact form its bit-for-bit tests pin. This one holds only what the
// colour signal sums: the linear value raised to the norm. Lazy, because it is 65,536 `powf` calls and only a process
// that analyses a photograph should pay for them.
/// Every sample [`Sampler::rgb`] can return, linearised and raised to [`NORM`].
static POWERED: LazyLock<Box<[f64; 65536]>> = LazyLock::new(|| {
    (0..=u16::MAX)
        .map(|w| srgb_to_linear(f64::from(w) / 65535.0).powi(NORM))
        .collect::<Box<[f64]>>()
        .try_into()
        .expect("one entry per u16")
});

/// The distance between two samples on each axis for a photograph of `width` by `height`.
///
/// 1 at or below [`MAX_SAMPLES`] pixels. Above it, `ceil(sqrt(pixels / MAX_SAMPLES))`, widened further when the grid
/// that gives still holds more than [`MAX_SAMPLES`], which the rounding up of each axis can do by a row or a column.
pub(super) fn stride(width: u32, height: u32) -> u32 {
    let pixels = u64::from(width) * u64::from(height);
    let mut stride = ((pixels as f64 / MAX_SAMPLES as f64).sqrt().ceil() as u32).max(1);

    while grid(width, height, stride) > MAX_SAMPLES {
        stride += 1;
    }

    stride
}

/// How many samples a grid of `stride` over `width` by `height` holds.
fn grid(width: u32, height: u32, stride: u32) -> u64 {
    u64::from(width.div_ceil(stride)) * u64::from(height.div_ceil(stride))
}

/// Everything one pass gathered, for the three signals to read.
pub(super) struct Pass {
    /// How many samples were read, after the transparent ones were skipped.
    pub(super) samples: u64,
    /// Every sample's brightness: Rec. 709 luma over the gamma-encoded channels, binned over `[0, 1]`.
    pub(super) luma: Box<[u64; LUMA_BINS]>,
    /// The colour signal's evidence.
    pub(super) gray: Shades,
    /// The monochrome signal's evidence.
    pub(super) levels: Levels,
}

/// The colour signal's accumulators: per channel, the sum of the linear value raised to [`NORM`].
///
/// Only samples inside the evidence band are counted, so [`count`](Self::count) can be below [`Pass::samples`].
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Shades {
    /// How many samples carried evidence.
    pub(super) count: u64,
    /// The sums of R, G and B raised to [`NORM`].
    pub(super) sums: [f64; 3],
}

impl Shades {
    fn add(&mut self, rgb: [u16; 3]) {
        let powered = rgb.map(|sample| POWERED[usize::from(sample)]);
        let brightest = powered[0].max(powered[1]).max(powered[2]);

        // The band compared in powered form, which the power keeps in order on `[0, 1]`.
        if brightest <= EVIDENCE_MIN.powi(NORM) || brightest >= EVIDENCE_MAX.powi(NORM) {
            return;
        }

        self.count += 1;
        for (sum, value) in self.sums.iter_mut().zip(powered) {
            *sum += value;
        }
    }
}

/// The monochrome signal's accumulators: for each of [`LEVELS`] bands of encoded Rec. 709 luma, how many samples fell
/// in it and the sums of their `B - Y` and `R - Y` and of those squared.
///
/// Only samples with no channel clipped are counted, so [`count`](Self::count) can be below [`Pass::samples`].
#[derive(Debug, Clone, Default)]
pub(super) struct Levels {
    /// Per level, how many samples fell in it.
    pub(super) counts: [u64; LEVELS],
    /// Per level, the sums of `B - Y` and `R - Y`, over the encoded channels on `[0, 1]`.
    pub(super) sums: [[f64; 2]; LEVELS],
    /// Per level, the sums of `B - Y` and `R - Y` squared.
    pub(super) squares: [[f64; 2]; LEVELS],
}

impl Levels {
    fn add(&mut self, rgb: [u16; 3]) {
        let Some((level, chroma)) = level_and_chroma(rgb) else { return };

        self.counts[level] += 1;
        for (index, difference) in chroma.into_iter().enumerate() {
            self.sums[level][index] += difference;
            self.squares[level][index] += difference * difference;
        }
    }

    /// How many samples were counted, across every level.
    pub(super) fn count(&self) -> u64 {
        self.counts.iter().sum()
    }
}

// A clipped channel has lost the part of its value that carried the colour, so a clipped sample's chroma is pulled
// towards zero whatever the scene. Counting it would move a high-key or low-key colour photograph towards monochrome,
// which is the direction of a false alarm.
/// The brightness level of one sample and its encoded `B - Y` and `R - Y`, or `None` where a channel is clipped.
fn level_and_chroma(rgb: [u16; 3]) -> Option<(usize, [f64; 2])> {
    if clipped(rgb) {
        return None;
    }

    let [r, g, b] = rgb.map(|channel| f64::from(channel) / f64::from(u16::MAX));
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let level = ((luma * LEVELS as f64) as usize).min(LEVELS - 1);

    // `B - Y` and `R - Y` written as differences between the channels, which are exactly zero where they are equal.
    // Subtracting the luma instead leaves a rounding error, and a grey photograph a residual that is not quite zero.
    Some((level, [0.2126 * (b - r) + 0.7152 * (b - g), 0.7152 * (r - g) + 0.0722 * (r - b)]))
}

/// [`read_from`] over the whole of `source`, for a test that has no sampler to share.
#[cfg(test)]
pub(super) fn read(source: &DynamicImage) -> Pass {
    read_from(&Sampler::new(source), source.width(), source.height())
}

/// Reads one pass over a `width` by `height` photograph through `sampler`, which the block pass shares: at most
/// [`MAX_SAMPLES`] of its pixels on a regular grid, skipping the ones that are mostly transparent.
pub(super) fn read_from(sampler: &Sampler<'_>, width: u32, height: u32) -> Pass {
    let stride = stride(width, height) as usize;

    let mut pass =
        Pass { samples: 0, luma: Box::new([0; LUMA_BINS]), gray: Shades::default(), levels: Levels::default() };

    for y in (0..height).step_by(stride) {
        for x in (0..width).step_by(stride) {
            let (rgb, alpha) = sampler.rgb_and_alpha(x, y);

            if alpha < OPAQUE_ENOUGH {
                continue;
            }

            pass.samples += 1;
            pass.luma[luma_bin(rgb)] += 1;
            pass.gray.add(rgb);
            pass.levels.add(rgb);
        }
    }

    pass
}

// Encoded rather than linear, because the light signal's thresholds are about how the photograph looks and the
// encoded scale is the perceptual one.
/// The brightness bin of one sample: Rec. 709 luma over its gamma-encoded channels.
fn luma_bin([r, g, b]: [u16; 3]) -> usize {
    let luma = (0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)) / f32::from(u16::MAX);

    ((luma * LUMA_BINS as f32) as usize).min(LUMA_BINS - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{ImageBuffer, Luma, Rgb, Rgba};

    #[test]
    fn a_photograph_of_up_to_a_megapixel_is_read_in_full() {
        for (width, height) in [(1, 1), (640, 480), (1024, 1024), (1_048_576, 1), (1, 1_048_576)] {
            assert_eq!(stride(width, height), 1, "{width}x{height} was subsampled");
        }
    }

    #[test]
    fn a_larger_photograph_is_read_at_a_bounded_number_of_samples() {
        // One pixel over, a 24 MP frame, a very large panorama, a thin strip and the widest extents a `u32` holds.
        for (width, height) in [
            (1025, 1024),
            (6000, 4000),
            (30_000, 20_000),
            (1_048_577, 1),
            (1_000_000, 3),
            (u32::MAX, u32::MAX),
        ] {
            let stride = stride(width, height);
            let samples = grid(width, height, stride);

            assert!(stride > 1, "{width}x{height} was read in full");
            assert!(samples <= MAX_SAMPLES, "{width}x{height} at stride {stride} is {samples} samples");
        }

        // And the bound is used rather than overshot: 24 MP is read at about half a million samples or more, not a
        // few thousand.
        assert!(grid(6000, 4000, stride(6000, 4000)) > MAX_SAMPLES / 2);
    }

    #[test]
    fn the_pass_reads_the_stride_it_computed() {
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2100, 1100, Rgb([90, 90, 90])));

        let pass = read(&source);

        assert_eq!(pass.samples, grid(2100, 1100, stride(2100, 1100)));
        assert!(pass.samples <= MAX_SAMPLES);
    }

    #[test]
    fn a_sixteen_bit_gradient_is_measured_at_more_than_eight_bits() {
        let gradient = ImageBuffer::from_fn(4096, 1, |x, _| Luma([(x * 16) as u16]));
        let source = DynamicImage::ImageLuma16(gradient);

        let pass = read(&source);
        let occupied = pass.luma.iter().filter(|count| **count > 0).count();

        assert!(occupied > 256, "a 16-bit gradient landed in only {occupied} brightness bins");
    }

    #[test]
    fn a_fully_transparent_image_yields_no_samples() {
        let source = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(64, 64, Rgba([200, 150, 100, 0])));

        let pass = read(&source);

        assert_eq!(pass.samples, 0);
        assert_eq!(pass.gray.count, 0);
        assert_eq!(pass.levels.count(), 0);
        assert!(pass.luma.iter().all(|count| *count == 0));
    }

    #[test]
    fn a_mostly_transparent_pixel_is_skipped_and_a_mostly_opaque_one_is_not() {
        let mut buffer = ImageBuffer::from_pixel(2, 1, Rgba([200, 150, 100, 127]));
        buffer.put_pixel(1, 0, Rgba([200, 150, 100, 128]));

        let pass = read(&DynamicImage::ImageRgba8(buffer));

        assert_eq!(pass.samples, 1);
    }

    #[test]
    fn a_grey_pixel_adds_equally_to_all_three_sums() {
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_fn(256, 1, |x, _| Rgb([x as u8; 3])));

        let pass = read(&source);
        let [r, g, b] = pass.gray.sums;

        assert!(pass.gray.count > 0, "no grey level was inside the evidence band");
        assert!(r > 0.0 && r == g && g == b, "a grey was given a hue: {:?}", pass.gray.sums);
    }

    #[test]
    fn a_pixel_too_dark_or_too_clipped_is_not_colour_evidence() {
        // Pure black and pure white, a saturated red too dark to carry anything, and a colour with one channel clipped:
        // counted for brightness, not for colour.
        let mut buffer = ImageBuffer::from_pixel(4, 1, Rgb([0, 0, 0]));
        buffer.put_pixel(1, 0, Rgb([255, 255, 255]));
        buffer.put_pixel(2, 0, Rgb([20, 0, 0]));
        buffer.put_pixel(3, 0, Rgb([255, 180, 90]));

        let pass = read(&DynamicImage::ImageRgb8(buffer));

        assert_eq!(pass.samples, 4);
        assert_eq!(pass.gray.count, 0);
        assert_eq!(pass.gray.sums, [0.0; 3]);
    }

    #[test]
    fn a_sample_is_summed_as_its_linear_value_raised_to_the_norm() {
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1, 1, Rgb([200, 140, 80])));

        let pass = read(&source);
        let want = [200_u8, 140, 80].map(|v| srgb_to_linear(f64::from(v) / 255.0).powi(NORM));

        assert_eq!(pass.gray.count, 1);
        for (got, want) in pass.gray.sums.iter().zip(want) {
            assert!((got / want - 1.0).abs() < 1e-12, "{got} != {want}");
        }
    }

    #[test]
    fn a_grey_sample_adds_nothing_to_either_chroma_sum() {
        let source = DynamicImage::ImageRgb16(ImageBuffer::from_fn(4096, 1, |x, _| Rgb([(x * 16) as u16; 3])));

        let levels = read(&source).levels;

        assert!(levels.count() > 4000, "the grey ramp was not counted: {}", levels.count());
        assert!(
            levels.counts.iter().all(|count| *count > 0),
            "a level of the ramp is empty: {:?}",
            levels.counts
        );
        for level in 0..LEVELS {
            for index in 0..2 {
                assert_eq!(levels.sums[level][index], 0.0, "level {level}");
                assert_eq!(levels.squares[level][index], 0.0, "level {level}");
            }
        }
    }

    #[test]
    fn a_clipped_or_transparent_sample_adds_nothing_to_the_levels() {
        // Black, white, a colour with one channel clipped at either end, and a colour hidden by transparency.
        let mut buffer = ImageBuffer::from_pixel(6, 1, Rgba([0, 0, 0, 255]));
        buffer.put_pixel(1, 0, Rgba([255, 255, 255, 255]));
        buffer.put_pixel(2, 0, Rgba([255, 180, 90, 255]));
        buffer.put_pixel(3, 0, Rgba([0, 120, 200, 255]));
        buffer.put_pixel(4, 0, Rgba([200, 150, 100, 0]));
        // And one that is read, so the others' absence is not the whole pass being skipped.
        buffer.put_pixel(5, 0, Rgba([200, 150, 100, 255]));

        let pass = read(&DynamicImage::ImageRgba8(buffer));

        assert_eq!(pass.samples, 5);
        assert_eq!(pass.levels.count(), 1);
    }

    #[test]
    fn an_eight_bit_sample_one_step_from_either_end_is_not_clipped() {
        let mut buffer = ImageBuffer::from_pixel(2, 1, Rgb([1, 128, 60]));
        buffer.put_pixel(1, 0, Rgb([254, 128, 60]));

        assert_eq!(read(&DynamicImage::ImageRgb8(buffer)).levels.count(), 2);
    }

    #[test]
    fn a_sample_is_gathered_at_its_brightness_with_its_chroma() {
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1, 1, Rgb([200, 140, 80])));

        let levels = read(&source).levels;
        let [r, g, b] = [200.0, 140.0, 80.0].map(|v: f64| v / 255.0);
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let level = (luma * LEVELS as f64) as usize;

        assert_eq!(levels.counts[level], 1);
        assert!((levels.sums[level][0] - (b - luma)).abs() < 1e-12, "{:?}", levels.sums[level]);
        assert!((levels.sums[level][1] - (r - luma)).abs() < 1e-12, "{:?}", levels.sums[level]);
        assert!((levels.squares[level][1] - (r - luma).powi(2)).abs() < 1e-12, "{:?}", levels.squares[level]);
    }

    #[test]
    fn a_sixteen_bit_tint_is_gathered_at_more_than_eight_bits() {
        // Red a quarter of an 8-bit step above green and blue: at 8 bits it would be grey.
        let source = DynamicImage::ImageRgb16(ImageBuffer::from_pixel(8, 8, Rgb([32_832, 32_768, 32_768])));

        let levels = read(&source).levels;
        let level = levels.counts.iter().position(|count| *count > 0).expect("a level");

        assert!(levels.sums[level][1] > 0.0, "a 16-bit tint was read as grey: {:?}", levels.sums[level]);
    }
}
