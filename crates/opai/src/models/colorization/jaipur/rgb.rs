//! The Rgb contract, Jaipur's: a gray rendering of the photograph in, a colorized photograph out, and only its chroma
//! kept.
//!
//! ```text
//!   input   [1, 3, 560, 560]   the photograph stretched to the square, as ITU-601 luma on all three planes, [0, 1]
//!   output  [1, 3, 560, 560]   a colorized RGB rendering at the square; its Lab a/b is the prediction
//! ```
//!
//! ImageNet normalization and DeOldify's `SigmoidRange` denormalization are baked into the exported graph at its two
//! ends, so neither appears here.

// The output keeps the photograph's own full-resolution lightness and takes only the model's chroma. DeOldify makes
// that transfer in YUV; here it is made in Lab, which is perceptually equivalent and reuses the conversion the Ab
// contract's compose is already built on and tested against.
//
// This contract is also why Jaipur's re-export from DeOldify's artistic generator moved nothing here: the backbone went
// from ResNet101 to ResNet34 and the graph lost two thirds of its weights, but the normalization stayed baked in at
// the same two ends.

use image::DynamicImage;
use image::imageops::FilterType;

use super::super::lab;
use super::super::stretch::gray_tensor;

// DeOldify's default `render_factor` of 35 times its render base of 16. Its stable and artistic generators share that
// default, so this did not move when Jaipur was re-exported from the artistic one — worth stating, because it is the
// one number a backbone swap would otherwise be expected to change. Only chroma comes from the graph, so this does not
// cap the result's detail.
/// The square the Rgb graph is exported at, and the only one it accepts.
pub(in crate::models::colorization) const SIDE: u32 = 560;

/// How many planes the graph's output carries: R, G and B.
pub(in crate::models::colorization) const CHANNELS: usize = 3;

// DeOldify stretches to its square bilinearly, and the reference's `imaging.Linear` is that: `1 - |x|` on support 1,
// scaled by the ratio when downsampling, as `image`'s `Triangle` is. The difference is that `imaging` stores its
// horizontal pass as 8-bit straight alpha before the vertical one, where `image` keeps floats.
//
// Measured on 2026-09-23 against `imaging@v1.6.2` at this square, in 8-bit levels of the premultiplied samples the
// input builder reads. On `fixtures/test.dat`, which is opaque, the maximum was 1.0 and the mean 0.20. On a
// translucent source the maximum was 2.28 and the mean 0.31, from `imaging` rounding a low-alpha pixel's colour and
// alpha separately between its passes. With `imaging`'s own weights and a float intermediate the gap closes to 0.50
// and 0.97, so the kernel, alignment and edges agree. See `ab::FILTER` for the Lanczos pair.
/// The resampler the photograph is stretched to [`SIDE`] through.
pub(in crate::models::colorization) const FILTER: FilterType = FilterType::Triangle;

/// The graph's input: `source` stretched to [`SIDE`], each pixel reduced to its ITU-601 luma and written to all three
/// planes, in CHW order.
///
/// The luma is DeOldify's grayscale, PIL's `LA` conversion, computed as the reference computes it: in `f32` over the
/// 16-bit samples [`Sampler::rgb`] answers, which at 8 bits are the reference's `Sample16` numbers value for value.
pub(in crate::models::colorization) fn input(source: &DynamicImage) -> Vec<f32> {
    gray_tensor(source, SIDE, FILTER, luma)
}

/// One pixel's ITU-601 luma on `[0, 1]`, with the reference's grouping and in its precision.
fn luma([r, g, b]: [u16; 3]) -> f32 {
    (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)) / 65535.0
}

/// The Lab a and b planes of the graph's RGB output, each channel clamped to `[0, 1]` first.
///
/// Owned, unlike [`ab::planes`](super::super::ab::planes): the planes are computed rather than already in the output.
///
/// # Panics
///
/// Panics where `output` holds fewer than [`CHANNELS`] planes of the square.
pub(in crate::models::colorization) fn chroma(output: &[f32]) -> (Vec<f32>, Vec<f32>) {
    chroma_at(output, (SIDE as usize) * (SIDE as usize))
}

/// [`chroma`] over planes of `plane` samples, so the tests can check it on a square smaller than the graph's.
fn chroma_at(output: &[f32], plane: usize) -> (Vec<f32>, Vec<f32>) {
    let (red, rest) = output.split_at(plane);
    let (green, blue) = rest.split_at(plane);

    // Clamped because the graph's output can overshoot its range, and a channel outside `[0, 1]` is outside the sRGB
    // transfer's domain.
    red.iter()
        .zip(green)
        .zip(&blue[..plane])
        .map(|((r, g), b)| {
            let [_, a, b] = lab::rgb_to_lab([r, g, b].map(|channel| channel.clamp(0.0, 1.0)));
            (a, b)
        })
        .unzip()
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};
    use imaging::tensor::Sampler;

    use super::*;

    #[test]
    fn the_input_is_a_neutral_luma_in_the_unit_range() {
        // The reference's pattern at the square itself, so what is checked is the rendering rather than the stretch.
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_fn(SIDE, SIDE, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        }));

        let tensor = input(&source);
        let plane = (SIDE as usize) * (SIDE as usize);
        assert_eq!(tensor.len(), 3 * plane, "the input is not three planes of the square");

        for index in (0..plane).step_by(997) {
            let (r, g, b) = (tensor[index], tensor[plane + index], tensor[2 * plane + index]);

            assert!((0.0..=1.0).contains(&r), "{index}: {r} is outside [0, 1]");
            assert!(r == g && r == b, "{index}: the input is not a neutral gray ({r}, {g}, {b})");
        }

        // Pixel 0 against the ITU-601 weights, written out as the reference writes them.
        let [r, g, b] = Sampler::new(&source).rgb(0, 0).map(f32::from);
        assert_eq!(tensor[0], (0.299 * r + 0.587 * g + 0.114 * b) / 65535.0, "pixel 0 is not its ITU-601 luma");
    }

    #[test]
    fn a_gray_output_has_no_chroma() {
        // What keeps a region the graph left gray gray in the result.
        let plane = 64;
        let mut output = vec![0.0_f32; 3 * plane];

        for index in 0..plane {
            let value = index as f32 / plane as f32;
            output[index] = value;
            output[plane + index] = value;
            output[2 * plane + index] = value;
        }

        let (a, b) = chroma_at(&output, plane);

        for index in 0..plane {
            assert!(
                a[index].abs() <= 0.01 && b[index].abs() <= 0.01,
                "{index}: gray gave ({}, {})",
                a[index],
                b[index]
            );
        }
    }

    #[test]
    fn an_output_outside_the_range_is_clamped_rather_than_propagated() {
        let (a, b) = chroma_at(&[1.5, -0.5, 0.5], 1);

        assert!(!a[0].is_nan() && !b[0].is_nan(), "an out-of-range output gave NaN chroma");
    }

    #[test]
    fn chroma_reads_the_graph_square() {
        let plane = (SIDE as usize) * (SIDE as usize);
        let (a, b) = chroma(&vec![0.5_f32; CHANNELS * plane]);

        assert_eq!((a.len(), b.len()), (plane, plane), "the chroma is not one plane of the square each");
    }
}
