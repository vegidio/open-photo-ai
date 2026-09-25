//! The last step of every colorization run: the predicted chroma put onto the photograph's own lightness, at the
//! photograph's resolution and at the run's depth.

// Family tier: both contracts end here. The Ab contract hands over the two planes its graph returned (`ab::planes`), and
// the Rgb contract the two it extracted (`jaipur::rgb::chroma`). Past that point the two are the same.
//
// **One fused pass**, where the obvious form is four full-resolution ones: extract L, upsample a, upsample b, combine.
// Each pass but the last writes a float plane at the photograph's resolution that is read once and thrown away. At 12
// megapixels that is some 144 MB, and the reference measured the fusion, together with its lookup tables, at 2812 ms
// and 192 MB down to 231 ms and 48 MB, where the 48 MB is the output image. Fusing also keeps each source pixel in cache
// for the whole of its own computation. The separate passes survive as the oracle in this file's tests, and the fused
// pass is equal to them **bit for bit**.
//
// Rows are split across cores (`imaging::rows`), as the reference splits them. Every row reads the photograph, the
// planes and the two axis tables immutably and writes only its own samples, so the split changes nothing in the result.

use image::{ImageBuffer, Rgb};

use super::lab;

use imaging::bilinear::{self, Axis};
use imaging::rows::for_each_row;
use imaging::tensor::{Channel, Sampler};

// A trait rather than a function-pointer parameter, which would put an indirect call in the innermost loop. It lets
// the pipeline dispatch the depth once, as light adjustment's `adjusted::<T>` does. As visible as `composed`, whose
// bound it is, and no more.
/// A channel the compose can encode linear light into: 8 bits through a threshold search, 16 through the transfer.
pub(super) trait Encoded: Channel + Send
where
    Rgb<Self>: image::Pixel<Subpixel = Self>,
{
    /// A linear-light channel gamma-encoded to this depth.
    fn encode(linear: f64) -> Self;
}

impl Encoded for u8 {
    fn encode(linear: f64) -> Self {
        lab::srgb_u8(linear)
    }
}

impl Encoded for u16 {
    fn encode(linear: f64) -> Self {
        lab::srgb_u16(linear)
    }
}

/// The photograph `source` at `width` × `height`, recoloured with the `side`-square chroma planes `a` and `b`.
///
/// Per pixel: the photograph's own Lab L, the a and b planes read bilinearly at that pixel, and the three converted
/// back to sRGB at `T`'s depth. The photograph's alpha has already been composited against black by
/// [`Sampler::rgb`], so the result carries none.
///
/// # Panics
///
/// Panics where `width` or `height` is zero, through [`Axis::new`]: the pipeline refuses a photograph with no area
/// before anything is allocated, as every whole-image pipeline does. Also panics where either plane is not the `side`
/// square.
pub(super) fn composed<T: Encoded>(
    source: &Sampler<'_>,
    (a, b): (&[f32], &[f32]),
    side: u32,
    (width, height): (u32, u32),
) -> ImageBuffer<Rgb<T>, Vec<T>>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // Neither table depends on the pixels, so both are built once rather than per pixel.
    let columns = Axis::new(side, width);
    let rows = Axis::new(side, height);

    let mut out = ImageBuffer::<Rgb<T>, Vec<T>>::new(width, height);

    for_each_row(&mut out, 3 * width as usize, height as usize, |y, samples| {
        let row = rows.tap(y);

        for (x, pixel) in samples.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            let column = columns.tap(x);

            #[expect(
                clippy::cast_possible_truncation,
                reason = "x and y index a row and a column of a buffer whose dimensions are u32"
            )]
            let l = lab::lightness(source.rgb(x as u32, y as u32));
            let linear = lab::lab_to_linear_rgb(
                l,
                bilinear::sample(a, side, row, column),
                bilinear::sample(b, side, row, column),
            );

            *pixel = linear.map(T::encode);
        }
    });

    out
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Luma, Rgba};

    use super::*;

    // The oracle: the compose as the reference wrote it before fusing it, ported from its `process_reference_test.go`.
    // Three passes over whole planes, each through the general conversion rather than a fast path, and a resize with
    // its own coordinate arithmetic rather than `Axis`, so the equality below also proves the promoted `Axis` against
    // the reference's form.

    /// The Lab L of every pixel, through the general forward conversion on unit floats.
    fn l_plane(source: &Sampler<'_>, (width, height): (u32, u32)) -> Vec<f32> {
        let mut out = Vec::with_capacity((width as usize) * (height as usize));

        for y in 0..height {
            for x in 0..width {
                out.push(lab::rgb_to_lab(source.rgb(x, y).map(|sample| f32::from(sample) / 65535.0))[0]);
            }
        }

        out
    }

    /// One plane resized bilinearly, aligning pixel centres and clamping at the edges; a plain copy at the same size.
    fn resize_plane(src: &[f32], (src_w, src_h): (usize, usize), (dst_w, dst_h): (usize, usize)) -> Vec<f32> {
        if (src_w, src_h) == (dst_w, dst_h) {
            return src.to_vec();
        }

        let scale_x = src_w as f64 / dst_w as f64;
        let scale_y = src_h as f64 / dst_h as f64;
        let mut out = vec![0.0_f32; dst_w * dst_h];

        for y in 0..dst_h {
            let sy = ((y as f64 + 0.5) * scale_y - 0.5).max(0.0);
            let y0 = (sy as usize).min(src_h - 1);
            let y1 = (y0 + 1).min(src_h - 1);
            let fy = (sy - y0 as f64) as f32;

            for x in 0..dst_w {
                let sx = ((x as f64 + 0.5) * scale_x - 0.5).max(0.0);
                let x0 = (sx as usize).min(src_w - 1);
                let x1 = (x0 + 1).min(src_w - 1);
                let fx = (sx - x0 as f64) as f32;

                let top = src[y0 * src_w + x0] * (1.0 - fx) + src[y0 * src_w + x1] * fx;
                let bottom = src[y1 * src_w + x0] * (1.0 - fx) + src[y1 * src_w + x1] * fx;
                out[y * dst_w + x] = top * (1.0 - fy) + bottom * fy;
            }
        }

        out
    }

    /// The photograph from full-resolution L, a and b, through the general inverse conversion and this crate's
    /// rounding.
    fn compose_lab<T: Encoded>(
        l: &[f32],
        a: &[f32],
        b: &[f32],
        (width, height): (u32, u32),
    ) -> ImageBuffer<Rgb<T>, Vec<T>>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        ImageBuffer::from_fn(width, height, |x, y| {
            let index = (y as usize) * (width as usize) + x as usize;

            Rgb(lab::lab_to_rgb(l[index], a[index], b[index]).map(T::from_unit))
        })
    }

    /// The whole pre-fusion tail: the three passes `composed` replaces with one.
    fn reference<T: Encoded>(
        source: &Sampler<'_>,
        (a, b): (&[f32], &[f32]),
        side: u32,
        extent: (u32, u32),
    ) -> ImageBuffer<Rgb<T>, Vec<T>>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let square = (side as usize, side as usize);
        let full = (extent.0 as usize, extent.1 as usize);

        compose_lab(&l_plane(source, extent), &resize_plane(a, square, full), &resize_plane(b, square, full), extent)
    }

    /// The reference's `synthChroma`: deterministic planes spanning ±120, so the compose runs far from the neutral
    /// axis.
    fn synth_chroma(side: u32) -> (Vec<f32>, Vec<f32>) {
        let side = side as usize;

        (0..side * side)
            .map(|index| {
                let (x, y) = (index % side, index / side);
                (((x * 13 + y * 7) % 241) as f32 - 120.0, ((x * 29 + y * 17) % 241) as f32 - 120.0)
            })
            .unzip()
    }

    /// The reference's `synth` photograph, whose every pixel differs from its neighbours.
    fn synth(width: u32, height: u32) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([((x * 7 + y * 3) % 256) as u8, ((x * 13 + y * 11) % 256) as u8, ((x * 29 + y * 17) % 256) as u8])
        })
    }

    /// The alpha the translucent layouts carry: the whole range, fully transparent included.
    fn alpha(x: u32, y: u32) -> u8 {
        ((x * 11 + y * 5) % 256) as u8
    }

    /// `synth` in each of the layouts `Sampler` tells apart: its borrowed arms at both depths, both premultiply paths,
    /// and its owned conversion.
    fn layouts(width: u32, height: u32) -> Vec<(&'static str, DynamicImage)> {
        let rgb8 = synth(width, height);
        // Off the `v * 257` grid, so the 16-bit layouts carry samples no 8-bit source can.
        let deep = |v: u8, x: u32| u16::from(v) * 257 - u16::from(v > 0) * ((x % 200) as u16);

        let with_alpha = |a: fn(u32, u32) -> u8| {
            ImageBuffer::from_fn(width, height, |x, y| {
                let Rgb([r, g, b]) = *rgb8.get_pixel(x, y);
                Rgba([r, g, b, a(x, y)])
            })
        };

        vec![
            ("Rgb8", DynamicImage::ImageRgb8(rgb8.clone())),
            ("opaque Rgba8", DynamicImage::ImageRgba8(with_alpha(|_, _| 255))),
            ("translucent Rgba8", DynamicImage::ImageRgba8(with_alpha(alpha))),
            (
                "Rgb16",
                DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
                    Rgb(rgb8.get_pixel(x, y).0.map(|v| deep(v, x)))
                })),
            ),
            (
                "translucent Rgba16",
                DynamicImage::ImageRgba16(ImageBuffer::from_fn(width, height, |x, y| {
                    let [r, g, b] = rgb8.get_pixel(x, y).0.map(|v| deep(v, x));
                    Rgba([r, g, b, u16::from(alpha(x, y)) * 251])
                })),
            ),
            (
                "L8",
                DynamicImage::ImageLuma8(ImageBuffer::from_fn(width, height, |x, y| Luma([rgb8.get_pixel(x, y).0[1]]))),
            ),
        ]
    }

    /// Asserts `composed` equal to the oracle at every pixel and channel of every layout, at `T`'s depth.
    fn assert_matches_reference<T: Encoded + std::fmt::Debug>(side: u32, extent: (u32, u32))
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let (a, b) = synth_chroma(side);

        for (name, image) in layouts(extent.0, extent.1) {
            let sampler = Sampler::new(&image);

            let fused = composed::<T>(&sampler, (&a, &b), side, extent);
            let separate = reference::<T>(&sampler, (&a, &b), side, extent);

            for (index, (got, want)) in fused.as_raw().iter().zip(separate.as_raw()).enumerate() {
                let pixel = index / 3;
                let (x, y) = (pixel % extent.0 as usize, pixel / extent.0 as usize);

                assert_eq!(
                    got,
                    want,
                    "{name} at {}x{} from {side}: pixel ({x}, {y}) channel {} differs",
                    extent.0,
                    extent.1,
                    index % 3
                );
            }
        }
    }

    /// The reference's five shapes, and one tall enough that `for_each_row` splits it on any machine with two cores.
    const SHAPES: [(u32, (u32, u32)); 6] =
        [(16, (64, 48)), (16, (16, 16)), (16, (7, 5)), (32, (129, 31)), (8, (1, 1)), (16, (40, 300))];

    #[test]
    fn the_fused_compose_is_the_separate_passes_bit_for_bit_at_eight_bits() {
        // Not within a level: equal. A fused pass that drifted from the separate steps would render a slightly wrong
        // photograph with no error anywhere.
        for (side, extent) in SHAPES {
            assert_matches_reference::<u8>(side, extent);
        }
    }

    #[test]
    fn the_fused_compose_is_the_separate_passes_bit_for_bit_at_sixteen_bits() {
        for (side, extent) in SHAPES {
            assert_matches_reference::<u16>(side, extent);
        }
    }

    #[test]
    fn resizing_a_constant_plane_keeps_it_constant() {
        // Within float rounding rather than exactly: the weights are applied as `a·(1 - f) + b·f`, whose two products
        // round independently. That form is deliberate, because the fused pass uses it verbatim and has to agree with
        // this one bit for bit.
        let out = resize_plane(&[7.5_f32; 16], (4, 4), (11, 5));

        for (index, value) in out.iter().enumerate() {
            assert!((value - 7.5).abs() <= 1e-5, "{index}: {value}");
        }
    }

    #[test]
    fn resizing_two_by_two_to_four_by_four_aligns_centres() {
        let out = resize_plane(&[0.0, 10.0, 20.0, 30.0], (2, 2), (4, 4));

        // Row 0 samples -0.25, clamped to row 0; the columns sample -0.25, 0.25, 0.75, 1.25.
        for (index, want) in [0.0, 2.5, 7.5, 10.0].into_iter().enumerate() {
            assert!((out[index] - want).abs() <= 1e-5, "row 0, {index}: {} rather than {want}", out[index]);
        }

        // The last row clamps to source row 1.
        for (index, want) in [20.0, 22.5, 27.5, 30.0].into_iter().enumerate() {
            assert!((out[12 + index] - want).abs() <= 1e-5, "row 3, {index}: {} rather than {want}", out[12 + index]);
        }
    }

    /// Asserts that composing a gray photograph with zero chroma gives it back, at `T`'s depth.
    fn assert_zero_chroma_preserves_gray<T: Encoded + Into<i64>>(source: &DynamicImage)
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let side = source.width();
        let zero = vec![0.0_f32; (side as usize) * (side as usize)];
        let sampler = Sampler::new(source);

        let out = composed::<T>(&sampler, (&zero, &zero), side, (side, side));

        for (x, y, pixel) in out.enumerate_pixels() {
            let [r, g, b] = pixel.0.map(Into::into);
            // The source's gray at this depth: every channel of a gray pixel is the same sample.
            let want: i64 = T::from_unit(u16::to_unit(sampler.rgb(x, y)[0])).into();

            // One level of tolerance, for the reference's reason: the conversion's constants keep the neutral axis
            // gray only to about 1e-5, so a channel can straddle a rounding boundary.
            assert!((r - want).abs() <= 1, "({x}, {y}): {r} rather than {want}");
            assert!((r - g).abs() <= 1 && (r - b).abs() <= 1, "({x}, {y}): zero chroma gave ({r}, {g}, {b})");
        }
    }

    #[test]
    fn zero_chroma_preserves_gray_at_eight_bits() {
        let gray = ImageBuffer::from_fn(16, 16, |x, y| Luma([((x * 16 + y) % 256) as u8]));

        assert_zero_chroma_preserves_gray::<u8>(&DynamicImage::ImageLuma8(gray));
    }

    #[test]
    fn zero_chroma_preserves_gray_at_sixteen_bits() {
        let gray = ImageBuffer::from_fn(16, 16, |x, y| Rgb([((x * 16 + y) * 257 - x) as u16; 3]));

        assert_zero_chroma_preserves_gray::<u16>(&DynamicImage::ImageRgb16(gray));
    }
}
