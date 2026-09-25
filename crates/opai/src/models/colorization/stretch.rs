//! The photograph stretched to a colorization graph's square: both contracts' first step.
//!
//! Stretched, not fitted. The graph sees the whole photograph distorted to its square, and the compose undoes the
//! distortion when it reads the predicted chroma back at the photograph's own aspect ratio.

// Family tier: the Ab contract (`ab`) and the Rgb contract (`jaipur::rgb`) both stretch through here, each with its own
// square and filter.
//
// **A source with alpha is flattened against black before it is resampled**, not resampled with its alpha. The
// reference's `imaging.Resize` weights every tap by its alpha — `C' = Σ w·a·c / Σ w·a` and `A' = Σ w·a`
// (`imaging@v1.6.2/resize.go`) — and its input builders then read the result through `Sample16`, which premultiplies
// by `A'`. So what reaches its graph is `C'·A' = Σ w·(a·c)`: exactly an unweighted resample of the photograph already
// composited against black. Flattening through `Sampler::rgb` first reproduces those sums, apart from what `imaging`'s
// 8-bit intermediate does to them. That intermediate rounds and clips each pass, and beside transparent regions it
// divides by an alpha sum a Lanczos kernel's negative lobes drive towards zero. The measured cost of not reproducing it
// is recorded beside each contract's `FILTER`. It also means the graph is fed the same premultiplied pixels the
// compose reads L from, so the two see one photograph.
//
// `imaging::present`'s `resize_exact` on the raw source is not that: it resamples straight alpha unweighted, so the
// colour stored under a fully transparent pixel bleeds into its neighbours at every transparency edge. Whether the
// other whole-image families should do this too is not this file's question.
//
// **At the source's own depth class.** An 8-bit source is stretched at 8 bits, which is what `imaging.Resize`
// produces, and an opaque 8-bit pixel flattens back to exactly itself. A 16-bit source is stretched at 16 bits, where
// the reference narrows it to 8: the run's depth is kept, here as everywhere in this family. Flattening everything to
// 16 bits would instead stretch an opaque `Rgba8` PNG and an `Rgb8` JPEG of one picture at different precisions, for
// no reason a user could see.

use image::DynamicImage;
use image::imageops::FilterType;

use imaging::tensor::{Sampler, flattened};

/// `source` stretched to the `side` square through `filter`, with any alpha composited against black first.
///
/// `Rgb8` and `Rgb16` are resampled as they are. Every other 8-bit layout is flattened to `Rgb8` and everything else to
/// `Rgb16`, so the result carries no alpha and is 8-bit exactly when the source is.
pub(super) fn stretched(source: &DynamicImage, side: u32, filter: FilterType) -> DynamicImage {
    match source {
        // Borrowed: nothing needs flattening, and at 24 megapixels a copy made only to be resampled and dropped is
        // 72 MB.
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgb16(_) => source.resize_exact(side, side, filter),
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_) => {
            DynamicImage::ImageRgb8(flattened::<u8>(&Sampler::new(source), source.width(), source.height()))
                .resize_exact(side, side, filter)
        }
        _ => { DynamicImage::ImageRgb16(flattened::<u16>(&Sampler::new(source), source.width(), source.height())) }
            .resize_exact(side, side, filter),
    }
}

/// A gray graph input: `source` [`stretched`] to the `side` square through `filter`, each pixel reduced to one value by
/// `gray` and written to all three planes, in CHW order. Both contracts feed their graph this shape and differ only in
/// what gray is.
pub(super) fn gray_tensor(
    source: &DynamicImage,
    side: u32,
    filter: FilterType,
    gray: impl Fn([u16; 3]) -> f32,
) -> Vec<f32> {
    let stretched = stretched(source, side, filter);
    let sampler = Sampler::new(&stretched);

    let plane = (side as usize) * (side as usize);
    let mut tensor = vec![0.0_f32; 3 * plane];

    for y in 0..side {
        for x in 0..side {
            let value = gray(sampler.rgb(x, y));
            let index = (y as usize) * (side as usize) + x as usize;

            tensor[index] = value;
            tensor[plane + index] = value;
            tensor[2 * plane + index] = value;
        }
    }

    tensor
}

#[cfg(test)]
mod tests {
    use image::{GenericImageView as _, ImageBuffer, Luma, Rgb, Rgba};

    use super::*;

    /// A picture whose every pixel differs from its neighbours, so a resample from the wrong pixels is a different
    /// number rather than a plausible one.
    fn pattern(width: u32, height: u32) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([((x * 7 + y * 3) % 256) as u8, ((x * 13 + y * 11) % 256) as u8, ((x * 29 + y * 17) % 256) as u8])
        })
    }

    /// `pattern` with alpha varying across the whole range, fully transparent pixels included.
    fn translucent(width: u32, height: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let opaque = pattern(width, height);

        ImageBuffer::from_fn(width, height, |x, y| {
            let Rgb([r, g, b]) = *opaque.get_pixel(x, y);
            Rgba([r, g, b, ((x * 11 + y * 5) % 256) as u8])
        })
    }

    #[test]
    fn the_result_is_the_square_whatever_the_aspect_ratio() {
        for (width, height) in [(64, 48), (7, 300), (1, 1), (600, 17)] {
            for side in [16, 512] {
                let result = stretched(&DynamicImage::ImageRgb8(pattern(width, height)), side, FilterType::Triangle);

                assert_eq!(result.dimensions(), (side, side), "{width}x{height} did not stretch to the {side} square");
            }
        }
    }

    #[test]
    fn an_rgb8_source_is_resampled_as_it_is() {
        // Nothing to flatten, so nothing may differ from resampling it directly — not even by the rounding a round
        // trip through 16 bits would add.
        let source = DynamicImage::ImageRgb8(pattern(97, 41));

        for filter in [FilterType::Triangle, FilterType::Lanczos3] {
            let direct = source.resize_exact(32, 32, filter);

            assert_eq!(stretched(&source, 32, filter), direct, "{filter:?} did not resample the source as it is");
        }
    }

    #[test]
    fn a_translucent_source_is_the_stretch_of_its_own_flattening() {
        // The flattening written out independently: each channel times its alpha, as the premultiplied 16-bit sample
        // `Sampler` answers, then the nearest byte to that over 257.
        let source = translucent(97, 41);
        let flattened = ImageBuffer::from_fn(97, 41, |x, y| {
            let Rgba([r, g, b, a]) = *source.get_pixel(x, y);
            let premultiplied = |c: u8| (u32::from(c) * 257 * u32::from(a) * 257) / 65535;

            Rgb([r, g, b].map(|c| ((premultiplied(c) + 128) / 257) as u8))
        });

        for filter in [FilterType::Triangle, FilterType::Lanczos3] {
            assert_eq!(
                stretched(&DynamicImage::ImageRgba8(source.clone()), 32, filter),
                DynamicImage::ImageRgb8(flattened.clone()).resize_exact(32, 32, filter),
                "{filter:?} resampled the straight alpha rather than the composited photograph"
            );
        }

        // And the point of it: colour stored under a fully transparent pixel reaches nothing. Two sources that differ
        // only there stretch to one picture.
        let mut hidden = source.clone();
        for pixel in hidden.pixels_mut() {
            if pixel.0[3] == 0 {
                pixel.0 = [255, 0, 255, 0];
            }
        }

        assert_eq!(
            stretched(&DynamicImage::ImageRgba8(hidden), 32, FilterType::Lanczos3),
            stretched(&DynamicImage::ImageRgba8(source), 32, FilterType::Lanczos3),
            "the colour under a transparent pixel bled into the stretch"
        );
    }

    #[test]
    fn a_sixteen_bit_source_comes_back_sixteen_bit() {
        let opaque = ImageBuffer::from_fn(40, 30, |x, y| Rgb([x as u16 * 1000, y as u16 * 2000, 12345_u16]));
        let alpha = ImageBuffer::from_fn(40, 30, |x, y| Rgba([x as u16 * 1000, y as u16 * 2000, 12345_u16, 40000]));
        let gray = ImageBuffer::from_fn(40, 30, |x, y| Luma([(x + y) as u16 * 900]));

        for source in [
            DynamicImage::ImageRgb16(opaque),
            DynamicImage::ImageRgba16(alpha),
            DynamicImage::ImageLuma16(gray),
            DynamicImage::ImageRgb32F(DynamicImage::ImageRgb8(pattern(40, 30)).to_rgb32f()),
        ] {
            let result = stretched(&source, 16, FilterType::Triangle);

            assert!(
                matches!(result, DynamicImage::ImageRgb16(_)),
                "a {:?} source was stretched to {:?}",
                source.color(),
                result.color()
            );
        }
    }

    #[test]
    fn an_eight_bit_source_comes_back_eight_bit() {
        let gray = ImageBuffer::from_fn(40, 30, |x, y| Luma([((x + y) % 256) as u8]));

        for source in [
            DynamicImage::ImageRgb8(pattern(40, 30)),
            DynamicImage::ImageRgba8(translucent(40, 30)),
            DynamicImage::ImageLuma8(gray),
            DynamicImage::ImageLumaA8(DynamicImage::ImageRgba8(translucent(40, 30)).to_luma_alpha8()),
        ] {
            let result = stretched(&source, 16, FilterType::Lanczos3);

            assert!(
                matches!(result, DynamicImage::ImageRgb8(_)),
                "a {:?} source was stretched to {:?}",
                source.color(),
                result.color()
            );
        }
    }

    #[test]
    fn an_opaque_eight_bit_pixel_flattens_to_exactly_itself() {
        // Why an opaque `Rgba8` PNG and an `Rgb8` JPEG of one picture stretch to one picture.
        let opaque = pattern(23, 19);
        let with_alpha = ImageBuffer::from_fn(23, 19, |x, y| {
            let Rgb([r, g, b]) = *opaque.get_pixel(x, y);
            Rgba([r, g, b, 255])
        });

        let source = DynamicImage::ImageRgba8(with_alpha);

        assert_eq!(flattened::<u8>(&Sampler::new(&source), 23, 19), opaque);
    }
}
