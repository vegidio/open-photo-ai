//! Putting back the colour a single diffusion step drifted away from.
//!
//! A one-step model drifts in overall colour and brightness. Left alone the result carries a cast, and — because each
//! region drifts independently — the drift reads as seams that no amount of combining removes, since the regions
//! genuinely disagree about the colour of what they share. An à trous B3-spline transform separates low frequency
//! from detail; the result's low frequencies are replaced with the reference's and its own detail is kept.
//!
//! Two things about how it is applied matter as much as the transform:
//!
//! - **It runs once, on the assembled image, and never per region.** Correcting a region against its own crop bakes
//!   that region's drift in as a step at its boundary instead of removing it — which is the seam, rather than a
//!   smaller version of it.
//! - **Its reference is the resampled image the model was conditioned on**, not the original input, so the correction
//!   undoes only what the model changed rather than also undoing the resample.
//!
//! It runs on the padded canvas, before the alignment extension is cropped off. The transform replicates edge pixels
//! outside the plane, so the two orderings differ within about one kernel support — some 60 pixels — of the right and
//! bottom edges. Correcting first is the reference's order and the better-conditioned one: the extension is a mirror
//! of real picture content, where a replicated crop boundary would be a constant.
//!
//! Both convolutions split their rows across cores through [`for_each_row`], which lives in `imaging::rows` because
//! colorization's compose splits its rows the same way.

use imaging::rows::for_each_row;
use imaging::tensor::TensorShape;

/// How many à trous levels separate "colour and illumination" from "detail".
///
/// Five reach a support of roughly 60 pixels, which is well above the texture the model synthesizes and well below
/// the scale of the drift being corrected.
const LEVELS: usize = 5;

/// `[1, 4, 6, 4, 1] / 16`, the standard à trous scaling function.
const B3_SPLINE: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];

/// Replaces `result`'s low-frequency content with `reference`'s, in place, keeping `result`'s own detail.
///
/// Both are three planes of `width` x `height` in the planar CHW order every conversion in this crate uses. Per
/// channel: add the reference's low frequency and subtract the result's, which is exactly "swap the low frequencies
/// over and leave everything else" written as one pass rather than as a reconstruction.
///
/// # Errors
///
/// Returns [`TensorShape`] when either buffer is not three planes of `width` x `height`, having written nothing. A
/// refusal rather than a correction over whichever length is shorter: two buffers that disagree are a driver whose
/// canvas and whose base plane were allocated at different extents, and correcting the overlap would silently mix one
/// picture's colour into another's geometry.
pub(crate) fn wavelet_color_fix(
    result: &mut [f32],
    reference: &[f32],
    width: u32,
    height: u32,
) -> Result<(), TensorShape> {
    let plane = (width as usize) * (height as usize);
    let expected = 3 * plane;

    if result.len() != expected {
        return Err(TensorShape { expected, actual: result.len(), width, height });
    }
    if reference.len() != expected {
        return Err(TensorShape { expected, actual: reference.len(), width, height });
    }

    if plane == 0 {
        return Ok(());
    }

    // Three planes allocated once for all three channels rather than two per extraction. At a 4x output over a
    // 12-megapixel photograph one plane is 770 MB, so the twelve short-lived allocations this replaces would be pure
    // churn on a machine already holding a 7 GB model.
    let mut result_low = vec![0.0_f32; plane];
    let mut reference_low = vec![0.0_f32; plane];
    let mut scratch = vec![0.0_f32; plane];

    for channel in 0..3 {
        let (from, to) = (channel * plane, (channel + 1) * plane);

        low_frequency(&mut result_low, &result[from..to], &mut scratch, width, height);
        low_frequency(&mut reference_low, &reference[from..to], &mut scratch, width, height);

        for index in 0..plane {
            result[from + index] += reference_low[index] - result_low[index];
        }
    }

    Ok(())
}

/// Writes the low-frequency component of `plane` into `dst` by convolving repeatedly with the B3-spline kernel, its
/// taps spread further apart at each level.
///
/// `scratch` is working space of the same length; both it and `dst` are overwritten.
///
/// The transform "with holes": the kernel is never subsampled, so every level stays at full resolution and the result
/// aligns with the input pixel for pixel. That alignment is the point — a decimated wavelet would need interpolating
/// back up, which reintroduces exactly the low-frequency error being measured.
fn low_frequency(dst: &mut [f32], plane: &[f32], scratch: &mut [f32], width: u32, height: u32) {
    dst.copy_from_slice(plane);

    for level in 0..LEVELS {
        let dilation = 1_u32 << level;

        convolve_horizontal(dst, scratch, width, height, dilation);
        convolve_vertical(scratch, dst, width, height, dilation);
    }
}

// The two convolutions below are deliberately not folded into one axis-generic function taking a stride. Each hoists
// its edge clamping out of the inner loop in the way its own axis allows, and those ways differ: along a row the
// clamp depends on the pixel, so the row splits into a clamped margin and a branch-free interior; down a column the
// five row offsets are identical for every pixel in the row, so they are computed once per row. A shared
// implementation would either keep the clamp per tap — the cost being removed — or iterate columns outermost and
// stride across memory instead of along it. This runs over the whole assembled image and is the one CPU-bound pass in
// the pipeline that is not ONNX, which is where the duplication earns its keep.
//
// Both keep the left-to-right summation order, so the result is bit-identical to the naive form.

/// Filters along each row. Only pixels within `2 * dilation` of an edge can clamp — at most 32 columns at the deepest
/// level — so the interior runs branch-free.
fn convolve_horizontal(src: &[f32], dst: &mut [f32], width: u32, height: u32, dilation: u32) {
    let (width, height, dilation) = (width as usize, height as usize, dilation as usize);
    let margin = (2 * dilation).min(width);
    let interior_end = width.saturating_sub(2 * dilation).max(margin);

    for_each_row(dst, width, height, |y, out| {
        let row = &src[y * width..(y + 1) * width];

        for (x, value) in out.iter_mut().enumerate().take(margin) {
            *value = clamped_row_tap(row, x, dilation, width);
        }

        for x in margin..interior_end {
            out[x] = B3_SPLINE[0] * row[x - 2 * dilation]
                + B3_SPLINE[1] * row[x - dilation]
                + B3_SPLINE[2] * row[x]
                + B3_SPLINE[3] * row[x + dilation]
                + B3_SPLINE[4] * row[x + 2 * dilation];
        }

        for (x, value) in out.iter_mut().enumerate().skip(interior_end) {
            *value = clamped_row_tap(row, x, dilation, width);
        }
    });
}

/// The edge case of [`convolve_horizontal`]: replicating the edge pixel outside the image, which keeps the transform
/// from darkening the border the way zero-padding would.
fn clamped_row_tap(row: &[f32], x: usize, dilation: usize, width: usize) -> f32 {
    let mut sum = 0.0;

    for (tap, weight) in B3_SPLINE.iter().enumerate() {
        sum += weight * row[clamped(x, tap, dilation, width)];
    }

    sum
}

/// Filters down each column. The five source rows a given output row reads are the same for every pixel in it, so
/// they are clamped once per row rather than once per pixel.
fn convolve_vertical(src: &[f32], dst: &mut [f32], width: u32, height: u32, dilation: u32) {
    let (width, height, dilation) = (width as usize, height as usize, dilation as usize);

    for_each_row(dst, width, height, |y, out| {
        let mut rows = [0_usize; 5];
        for (tap, offset) in rows.iter_mut().enumerate() {
            *offset = clamped(y, tap, dilation, height) * width;
        }

        for (x, value) in out.iter_mut().enumerate() {
            *value = B3_SPLINE[0] * src[rows[0] + x]
                + B3_SPLINE[1] * src[rows[1] + x]
                + B3_SPLINE[2] * src[rows[2] + x]
                + B3_SPLINE[3] * src[rows[3] + x]
                + B3_SPLINE[4] * src[rows[4] + x];
        }
    });
}

/// Which pixel tap `tap` of the five reads at `position`, with the edge pixel replicated outside the plane.
///
/// Signed arithmetic so that a tap reaching before the start is a number to clamp rather than a wrap to the top of a
/// `usize`, which would index far outside the buffer.
fn clamped(position: usize, tap: usize, dilation: usize, length: usize) -> usize {
    let offset = position as isize + (tap as isize - 2) * dilation as isize;

    offset.clamp(0, length as isize - 1) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The low-frequency component of one plane, which is what every extraction test below reads.
    fn low(plane: &[f32], width: u32, height: u32) -> Vec<f32> {
        let mut dst = vec![0.0; plane.len()];
        let mut scratch = vec![0.0; plane.len()];

        low_frequency(&mut dst, plane, &mut scratch, width, height);
        dst
    }

    /// A plane whose every pixel is distinct and smooth, so a correction that moved detail is visible.
    fn ramp(width: u32, height: u32) -> Vec<f32> {
        (0..width * height).map(|index| f32::from((index % 97) as u16) / 97.0).collect()
    }

    #[test]
    fn a_constant_plane_is_its_own_low_frequency_at_every_level() {
        // Which is the whole of what "the kernel sums to one and the border replicates" means. Zero-padding would
        // fail this along every edge, and a kernel that did not sum to one would fail it everywhere.
        for (width, height) in [(1, 1), (5, 3), (64, 64), (200, 7)] {
            let plane = vec![0.375_f32; (width * height) as usize];
            let extracted = low(&plane, width, height);

            for (index, value) in extracted.iter().enumerate() {
                assert!(
                    (value - 0.375).abs() < 1e-6,
                    "a constant {width}x{height} plane extracted to {value} at {index}"
                );
            }
        }
    }

    #[test]
    fn a_plane_of_pure_high_frequency_detail_extracts_to_near_its_own_mean() {
        // A checkerboard at one-pixel period: it has no low-frequency content at all, so five levels of a low pass
        // should leave the mean and nothing else. This is the property the correction rests on — what it swaps over
        // is illumination, and a texture like this carries none.
        let (width, height) = (64, 64);
        let plane: Vec<f32> = (0..width * height)
            .map(|index| if (index % width + index / width) % 2 == 0 { 1.0 } else { 0.0 })
            .collect();

        let extracted = low(&plane, width, height);
        let mean = plane.iter().sum::<f32>() / plane.len() as f32;

        for (index, value) in extracted.iter().enumerate() {
            assert!((value - mean).abs() < 0.01, "pure detail extracted to {value} at {index}, not to {mean}");
        }
    }

    #[test]
    fn the_border_is_not_darkened_the_way_zero_padding_would_darken_it() {
        // The defect the clamp exists to prevent, stated against what it would otherwise be: with a plane of ones,
        // zero-padding drives every border pixel below one and the corner furthest below. Replicating leaves them at
        // one exactly.
        let (width, height) = (48, 48);
        let plane = vec![1.0_f32; (width * height) as usize];
        let extracted = low(&plane, width, height);

        let at = |x: u32, y: u32| extracted[(y * width + x) as usize];

        for (x, y) in [(0, 0), (47, 0), (0, 47), (47, 47), (0, 24), (24, 0)] {
            assert!((at(x, y) - 1.0).abs() < 1e-5, "the border at ({x}, {y}) darkened to {}", at(x, y));
        }
        assert!(at(0, 0) >= at(24, 24) - 1e-5, "the corner is darker than the interior");
    }

    #[test]
    fn the_separable_result_matches_a_direct_two_dimensional_convolution() {
        // Separability is an optimisation, and the check is that it is only that: one level of the separable pair
        // against the outer product of the kernel with itself, applied directly over a small plane.
        let (width, height) = (9, 7);
        let plane = ramp(width, height);

        for dilation in [1_u32, 2, 4] {
            let mut scratch = vec![0.0; plane.len()];
            let mut separable = vec![0.0; plane.len()];
            convolve_horizontal(&plane, &mut scratch, width, height, dilation);
            convolve_vertical(&scratch, &mut separable, width, height, dilation);

            let mut direct = vec![0.0_f32; plane.len()];
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let mut sum = 0.0_f32;

                    for (row_tap, row_weight) in B3_SPLINE.iter().enumerate() {
                        for (column_tap, column_weight) in B3_SPLINE.iter().enumerate() {
                            let sy = clamped(y, row_tap, dilation as usize, height as usize);
                            let sx = clamped(x, column_tap, dilation as usize, width as usize);

                            sum += row_weight * column_weight * plane[sy * width as usize + sx];
                        }
                    }

                    direct[y * width as usize + x] = sum;
                }
            }

            for (index, (a, b)) in separable.iter().zip(&direct).enumerate() {
                assert!((a - b).abs() < 1e-6, "at dilation {dilation} the two disagree at {index}: {a} and {b}");
            }
        }
    }

    /// Three planes of `width` x `height`, each a ramp offset so the channels are not identical.
    fn picture(width: u32, height: u32) -> Vec<f32> {
        let plane = ramp(width, height);
        let mut out = Vec::with_capacity(3 * plane.len());

        for channel in 0..3 {
            out.extend(plane.iter().map(|value| value + channel as f32 * 0.05));
        }

        out
    }

    #[test]
    fn correcting_an_image_against_itself_is_the_identity() {
        let (width, height) = (40, 30);
        let reference = picture(width, height);
        let mut result = reference.clone();

        wavelet_color_fix(&mut result, &reference, width, height).unwrap();

        for (index, (a, b)) in result.iter().zip(&reference).enumerate() {
            assert!((a - b).abs() < 1e-5, "correcting against itself moved {index} from {b} to {a}");
        }
    }

    #[test]
    fn correcting_against_a_uniformly_shifted_copy_recovers_the_shift() {
        // The drift the correction exists for, in its purest form: a constant offset over the whole picture is
        // entirely low frequency, so all of it comes back.
        let (width, height) = (40, 30);
        let reference = picture(width, height);
        let shift = 0.2_f32;
        let mut result: Vec<f32> = reference.iter().map(|value| value - shift).collect();

        wavelet_color_fix(&mut result, &reference, width, height).unwrap();

        for (index, (a, b)) in result.iter().zip(&reference).enumerate() {
            assert!((a - b).abs() < 1e-4, "a uniform shift left {index} at {a} rather than {b}");
        }
    }

    #[test]
    fn the_detail_the_model_synthesized_survives_the_correction() {
        // It changes colour and illumination, not texture. A checkerboard laid over a reference with none of it must
        // still be a checkerboard afterwards, at its own amplitude.
        let (width, height) = (64, 64);
        let plane = (width * height) as usize;
        let reference = vec![0.5_f32; 3 * plane];

        let mut result = vec![0.0_f32; 3 * plane];
        for channel in 0..3 {
            for index in 0..plane {
                let detail =
                    if (index % width as usize + index / width as usize).is_multiple_of(2) { 0.1 } else { -0.1 };
                // A cast of its own, on top of the detail, so the correction has something to remove.
                result[channel * plane + index] = 0.8 + detail;
            }
        }

        wavelet_color_fix(&mut result, &reference, width, height).unwrap();

        // The cast is gone — the mean is the reference's.
        let mean = result.iter().sum::<f32>() / result.len() as f32;
        assert!((mean - 0.5).abs() < 0.01, "the cast survived: the mean is {mean}");

        // And the detail is not: adjacent pixels still differ by the amplitude it was given.
        let at = |x: usize, y: usize| result[y * width as usize + x];
        for y in 10..20 {
            for x in 10..20 {
                assert!(
                    (at(x, y) - at(x + 1, y)).abs() > 0.15,
                    "the detail at ({x}, {y}) was flattened to {} and {}",
                    at(x, y),
                    at(x + 1, y)
                );
            }
        }
    }

    #[test]
    fn mismatched_buffer_lengths_are_refused_rather_than_truncated() {
        let (width, height) = (8, 6);
        let reference = picture(width, height);

        // A result the extent does not describe.
        let mut short = vec![0.5_f32; reference.len() - 3];
        let error = wavelet_color_fix(&mut short, &reference, width, height).unwrap_err();
        assert_eq!(error.expected, reference.len());
        assert_eq!(error.actual, reference.len() - 3);
        assert!(
            short.iter().all(|value| (value - 0.5).abs() < f32::EPSILON),
            "a refused correction wrote anyway"
        );

        // A reference the extent does not describe, with a result that it does — the direction that would otherwise
        // correct part of the picture and leave the rest.
        let mut result = picture(width, height);
        let untouched = result.clone();
        let long = vec![0.25_f32; reference.len() + 3];
        let error = wavelet_color_fix(&mut result, &long, width, height).unwrap_err();

        assert_eq!(error.actual, reference.len() + 3);
        assert_eq!(result, untouched, "a refused correction altered the result buffer");
    }
}
