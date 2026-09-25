//! The Ab contract, Delhi's and Mumbai's: a gray rendering of the photograph in, the Lab a/b planes out.
//!
//! ```text
//!   input   [1, 3, 512, 512]   the photograph stretched to the square, as gray on all three planes, [0, 1]
//!   output  [1, 2, 512, 512]   the predicted a and b, at the square
//! ```

// Family tier: both DDColor models share it, and nothing outside colorization reads it.

use image::DynamicImage;
use image::imageops::FilterType;

use super::lab;
use super::stretch::gray_tensor;

// The graph predicts only the chroma planes at this size. The photograph keeps its own full-resolution lightness, so
// the square is not a cap on the result's detail.
/// The square the Ab graphs are exported at, and the only one they accept.
pub(super) const SIDE: u32 = 512;

/// How many planes the graph's output carries: a, then b.
pub(super) const CHANNELS: usize = 2;

// The reference's `imaging.Lanczos`: the same `sinc(x)·sinc(x/3)` kernel on support 3, scaled by the ratio when
// downsampling, in both libraries. The difference is that `imaging` stores its horizontal pass as 8-bit straight alpha
// before the vertical one, where `image` keeps floats.
//
// Measured on 2026-09-23 against `imaging@v1.6.2` at this square, in 8-bit levels of the premultiplied samples the
// input builder reads. On `fixtures/test.dat`, which is opaque, the maximum was 5.0 and the mean 0.19, with 37 of
// 786,432 samples beyond 1.5. On a translucent source the maximum was 49 and the mean 0.54. With `imaging`'s own
// weights and a float intermediate the gap closes to 0.50 and 1.13, so the kernel, alignment and edges agree. What is
// left is `imaging`'s intermediate. It clips Lanczos overshoot at sharp edges before the second pass, and beside
// transparent regions it divides by an alpha sum that the negative lobes drive towards zero. This stretch keeps
// neither, deliberately: both are artefacts, and a second resampler kept only to reproduce them is not worth having.
/// The resampler the photograph is stretched to [`SIDE`] through.
pub(super) const FILTER: FilterType = FilterType::Lanczos3;

/// The graph's input: `source` stretched to [`SIDE`], each pixel reduced to its gray and written to all three
/// planes, in CHW order.
///
/// The gray is the photograph's Lab L at zero chroma rendered back to sRGB, which is what DDColor was trained on.
/// [`lab::gray`] reaches it without the Lab round trip: at zero chroma the round trip is the identity on the
/// luminance, so going through L would cost a cube root and three `powf` per pixel to arrive where it started.
pub(super) fn input(source: &DynamicImage) -> Vec<f32> {
    gray_tensor(source, SIDE, FILTER, lab::gray)
}

/// The a and b planes of the graph's output, read straight out of it: the first [`SIDE`] square, then the second.
///
/// # Panics
///
/// Panics where `output` holds fewer than [`CHANNELS`] planes of the square.
pub(super) fn planes(output: &[f32]) -> (&[f32], &[f32]) {
    let plane = (SIDE as usize) * (SIDE as usize);

    (&output[..plane], &output[plane..CHANNELS * plane])
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};
    use imaging::tensor::Sampler;

    use super::*;

    #[test]
    fn the_input_is_a_neutral_gray_in_the_unit_range() {
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

        // Pixel 0 against the Lab round trip the reference computes it through. `gray` agrees with that round trip
        // to 1e-4 in general (see `lab`); here, as in the reference, the pixel is black and the two are equal.
        let pixel = Sampler::new(&source).rgb(0, 0).map(|value| f32::from(value) / 65535.0);
        let [want, ..] = lab::lab_to_rgb(lab::rgb_to_lab(pixel)[0], 0.0, 0.0);

        assert_eq!(tensor[0], want, "pixel 0 is not the gray rendering of its luminance");
    }

    #[test]
    fn the_planes_are_the_first_and_second_square_of_the_output() {
        let plane = (SIDE as usize) * (SIDE as usize);
        let output: Vec<f32> = (0..CHANNELS * plane).map(|index| index as f32).collect();

        let (a, b) = planes(&output);

        assert_eq!(a, &output[..plane], "a is not the first plane");
        assert_eq!(b, &output[plane..], "b is not the second plane");
    }
}
