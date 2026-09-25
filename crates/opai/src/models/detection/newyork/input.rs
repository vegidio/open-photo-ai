//! Presenting a photograph of any size and aspect to a detector that accepts one fixed square.

// Three decisions in the letterbox are not mechanics.
//
// **The aspect ratio is preserved.** Scaling each axis independently to fill the square would distort every face,
// and a detector trained on undistorted faces would find fewer of them and place their landmarks wrongly.
//
// **The padding is the negative per-channel means**, which is what a zero pixel becomes after the subtraction below.
// Padding with a literal zero would feed the model a bright border; repeating or mirroring the picture's own edge
// would present it with content that is not in the photograph, which it may find a face in.
//
// **The image goes at the top-left rather than the centre**, so that the relationship between a coordinate the
// detector reports and a coordinate in the source is a single scale factor with no offset to undo.
//
// This is not `imaging::tensor`, though it looks like a candidate for that module. Those conversions mirror edge
// pixels outwards and normalise over `[0,1]` or `[-1,1]`; this zero-pads and subtracts per-channel means in **BGR**
// order. They share the letters CHW and nothing else. `imaging`'s header also already states that its tiling driver
// never resizes, and this resizes.

use image::DynamicImage;
use image::imageops::{FilterType, resize};

use crate::error::InferenceError;

// The model's training means, and deliberately not the ImageNet RGB ones: `(104, 117, 123)` read as blue, green, red.
// Transcribed from the reference's `input.go`.
/// The RetinaFace per-channel means, in the **BGR** order the tensor is laid out in.
pub(super) const MEANS: [f32; 3] = [104.0, 117.0, 123.0];

/// A photograph fitted to the detector's square, and the dimensions it was actually fitted to.
pub(super) struct Letterboxed {
    // The two travel together because the second is what undoes the first: see `scale`.
    /// The tensor, `3 * size * size` floats, BGR planar CHW, each value the pixel minus its channel's mean.
    pub(super) tensor: Vec<f32>,
    /// The source's own width, which the rescale carries coordinates back into.
    source_width: u32,
    /// The source's own height.
    source_height: u32,
    /// The width the source was resized to, in whole pixels.
    resized_width: u32,
    /// The height the source was resized to, in whole pixels.
    resized_height: u32,
}

impl Letterboxed {
    /// What one coordinate in the detector's square is worth in the source's pixels, per axis.
    ///
    /// The **integer** dimensions the image was actually resized to, not the ideal float ratio, so this undoes
    /// exactly what the letterbox did rather than approximately.
    ///
    /// The two axes differ whenever the truncation bit, which it does for most aspect ratios.
    pub(super) fn scale(&self) -> (f32, f32) {
        // A method here rather than arithmetic at the call site because this type is what knows both halves — and two
        // answers to one question is how the rescale and the preprocessing would drift apart.
        (
            self.source_width as f32 / self.resized_width as f32,
            self.source_height as f32 / self.resized_height as f32,
        )
    }
}

/// Fits `source` into a `size` square and writes the detector's input tensor: the whole image scaled, down or up, until
/// its longer side reaches `size`, aspect preserved, placed at the top-left, and the rest of the square filled with the
/// negative per-channel means, which the detector reads as nothing rather than as picture.
///
/// # Errors
///
/// [`InferenceError::Untileable`] where `source` has no area.
pub(super) fn letterbox(source: &DynamicImage, size: u32) -> Result<Letterboxed, InferenceError> {
    let (width, height) = (source.width(), source.height());

    // There is no scaling of a zero-area image into the square, and a run over a square of nothing but padding is not
    // a detection of anything — so it is refused rather than run. The reference falls back to the square target here
    // instead, as a defensive path rather than an intent: `int(NaN)` is implementation-defined in Go, so the bad input
    // would propagate as a plausible-looking tensor. Rust has no such trap, and a refusal is the honest answer.
    if width == 0 || height == 0 {
        return Err(InferenceError::Untileable { width, height });
    }

    let (resized_width, resized_height) = fit(width, height, size);

    // `Lanczos3` rather than the reference's `imaging.Lanczos`. Different kernels, so boxes differ in sub-pixel
    // placement — which is the same axis the reference's own FP16/FP32 comparison found, and is expected. A
    // difference in the *count* or the *order* of the faces is a port bug and not the resampler.
    //
    // Flattened to three channels before the resample rather than after: the detector has no use for alpha, and
    // resampling it would let a transparent region's colour bleed into the picture the detector is shown.
    //
    // Borrowed where the source already is eight-bit RGB, which is the overwhelmingly common case: `to_rgb8`
    // always allocates and copies, and for a 24-megapixel photograph that is a 72 MB copy made only to be resampled
    // and dropped.
    let resized = match source {
        DynamicImage::ImageRgb8(buffer) => resize(buffer, resized_width, resized_height, FilterType::Lanczos3),
        other => resize(&other.to_rgb8(), resized_width, resized_height, FilterType::Lanczos3),
    };

    let plane = (size * size) as usize;
    let mut tensor = vec![0.0_f32; 3 * plane];

    // Pre-filled with the padding value, so the real image region below simply overwrites it and the padding is
    // written in one place regardless of how the image is read.
    for (channel, mean) in MEANS.iter().enumerate() {
        tensor[channel * plane..(channel + 1) * plane].fill(-mean);
    }

    // Walked as rows of the buffer's own storage rather than through `get_pixel`, which bounds-checks and
    // recomputes `y * width + x` for each of up to 409 600 reads.
    for (y, source_row) in resized.as_raw().chunks_exact(resized_width as usize * 3).enumerate() {
        let row = y * size as usize;
        for (x, pixel) in source_row.as_chunks::<3>().0.iter().enumerate() {
            let index = row + x;

            // BGR, not RGB: the source pixel's blue goes into the first plane. Getting this wrong is invisible in a
            // working model's shape and catastrophic in its output.
            tensor[index] = f32::from(pixel[2]) - MEANS[0];
            tensor[plane + index] = f32::from(pixel[1]) - MEANS[1];
            tensor[2 * plane + index] = f32::from(pixel[0]) - MEANS[2];
        }
    }

    Ok(Letterboxed { tensor, source_width: width, source_height: height, resized_width, resized_height })
}

/// The whole-pixel dimensions a `width` by `height` image is scaled to so that its longer side is `size`.
///
/// The longer side reaches `size` exactly and the shorter is scaled by the same factor, truncated to whole pixels and
/// floored to at least one pixel.
pub(super) fn fit(width: u32, height: u32, size: u32) -> (u32, u32) {
    // Truncated as the reference truncates. Floored because a 4000x1 photograph scales its height to zero, and a
    // resize to no height is a picture with nothing in it rather than a very wide one.
    let ratio = f64::from(height) / f64::from(width);

    if ratio > 1.0 {
        (((f64::from(size) / ratio) as u32).max(1), size)
    } else {
        (size, ((f64::from(size) * ratio) as u32).max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::super::TARGET_SIZE;
    use super::*;

    use image::{ImageBuffer, Rgb};

    /// A solid image of one colour, which is what makes a channel order readable off the tensor.
    fn solid(width: u32, height: u32, colour: [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_pixel(width, height, Rgb(colour)))
    }

    #[test]
    fn the_longer_side_reaches_the_target_and_the_shorter_keeps_the_aspect_ratio() {
        // Transcribed from the reference's `input_test.go`, both orientations, because the implementation branches on
        // the ratio and a landscape photograph exercises only one arm.
        assert_eq!(fit(1280, 640, TARGET_SIZE), (640, 320), "landscape did not halve the height");
        assert_eq!(fit(640, 1280, TARGET_SIZE), (320, 640), "portrait did not halve the width");
        assert_eq!(fit(500, 500, TARGET_SIZE), (640, 640), "a square did not fill both edges");
        assert_eq!(fit(640, 640, TARGET_SIZE), (640, 640), "an image already at the target was rescaled");
    }

    #[test]
    fn an_extreme_aspect_ratio_still_leaves_a_pixel_on_the_short_side() {
        // The reference has no guard here because Go's `imaging.Resize` treats a zero as "derive it", which is a
        // different behaviour rather than a safer one.
        let (width, height) = fit(4000, 1, TARGET_SIZE);

        assert_eq!(width, TARGET_SIZE);
        assert_eq!(height, 1, "the short side was scaled out of existence");
    }

    #[test]
    fn every_aspect_produces_a_tensor_of_exactly_three_planes_at_the_target() {
        for (width, height) in [(1280_u32, 640_u32), (640, 1280), (500, 500)] {
            let fitted = letterbox(&solid(width, height, [0, 0, 0]), TARGET_SIZE).expect("an image with area");

            assert_eq!(
                fitted.tensor.len(),
                3 * (TARGET_SIZE * TARGET_SIZE) as usize,
                "a {width}x{height} source produced a tensor of the wrong length"
            );
            let (resized_width, resized_height) = fit(width, height, TARGET_SIZE);
            assert_eq!(
                fitted.scale(),
                (width as f32 / resized_width as f32, height as f32 / resized_height as f32),
                "a {width}x{height} source reported a scale that does not undo its own resize"
            );
        }
    }

    #[test]
    fn the_channel_order_is_bgr_rather_than_rgb() {
        // Fed a colour whose three channels are all different, so a tensor written in RGB order is a failure rather
        // than a coincidence. The first plane carries blue.
        let fitted = letterbox(&solid(TARGET_SIZE, TARGET_SIZE, [10, 20, 30]), TARGET_SIZE).expect("an image");
        let plane = (TARGET_SIZE * TARGET_SIZE) as usize;

        assert_eq!(fitted.tensor[0], 30.0 - MEANS[0], "the first plane is not blue");
        assert_eq!(fitted.tensor[plane], 20.0 - MEANS[1], "the second plane is not green");
        assert_eq!(fitted.tensor[2 * plane], 10.0 - MEANS[2], "the third plane is not red");
    }

    #[test]
    fn the_padding_region_is_exactly_the_negative_means() {
        // A landscape source, so the bottom of the square is padding. White pixels, so a padding value that leaked
        // the image would be obvious rather than coincidentally equal.
        let fitted = letterbox(&solid(1280, 640, [255, 255, 255]), TARGET_SIZE).expect("an image");
        let size = TARGET_SIZE as usize;
        let plane = size * size;

        assert_eq!(fitted.resized_height, 320, "the fixture stopped being a letterbox");
        assert_eq!(fitted.scale(), (2.0, 2.0), "a 1280x640 source does not scale by two on both axes");

        for (channel, mean) in MEANS.iter().enumerate() {
            for y in fitted.resized_height as usize..size {
                for x in 0..size {
                    assert_eq!(
                        fitted.tensor[channel * plane + y * size + x],
                        -mean,
                        "padding at ({x}, {y}) of plane {channel} is not the negative mean"
                    );
                }
            }

            // And the image region is not padding, which is what makes the assertion above mean something.
            assert_eq!(fitted.tensor[channel * plane], 255.0 - mean, "plane {channel}'s first pixel is padding");
        }
    }

    #[test]
    fn a_portrait_image_pads_to_the_right_rather_than_below() {
        // The other arm of the branch, and the one that says the image is anchored top-left rather than centred: a
        // centred placement would put padding on *both* sides, and every reported coordinate would then need an
        // offset undone as well as a scale.
        let fitted = letterbox(&solid(640, 1280, [255, 255, 255]), TARGET_SIZE).expect("an image");
        let size = TARGET_SIZE as usize;

        assert_eq!((fitted.resized_width, fitted.resized_height), (320, 640));
        assert_eq!(fitted.scale(), (2.0, 2.0));

        // The first row: picture up to the resized width, padding after it, and nothing before it.
        assert_eq!(
            fitted.tensor[0],
            255.0 - MEANS[0],
            "the top-left pixel is padding, so the image is not anchored there"
        );
        assert_eq!(fitted.tensor[fitted.resized_width as usize], -MEANS[0], "the row past the image is not padding");
        assert_eq!(fitted.tensor[size - 1], -MEANS[0], "the end of the first row is not padding");
    }

    #[test]
    fn an_image_with_no_area_is_refused_rather_than_run_over_a_square_of_padding() {
        // The same refusal, and the same error, the enhancement path makes for an image it cannot partition.
        for (width, height) in [(0_u32, 100_u32), (100, 0), (0, 0)] {
            let empty = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));

            let Err(InferenceError::Untileable { width: refused_width, height: refused_height }) =
                letterbox(&empty, TARGET_SIZE)
            else {
                panic!("a {width}x{height} image was run");
            };

            assert_eq!((refused_width, refused_height), (width, height), "the refusal named the wrong dimensions");
        }
    }
}
