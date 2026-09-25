//! Sampling a face out of the photograph and straight into the graph's input tensor.
//!
//! The reference warps into an 8-bit NRGBA image and then converts that image to CHW, which quantises a 16-bit
//! source **twice**: once narrowing it to eight bits inside the warp, and once widening it back to a float. Fusing
//! the two loops removes the intermediate image, the allocation it costs per face, and the quantisation — and the
//! arithmetic is otherwise the reference's, [`reflect_coord`] included.
//!
//! Nothing here names a variant. The destination square, the transform, the normalisation and the source are all the
//! caller's, which is what both of the family's models need from this file.

use super::transform::Affine;

use imaging::tensor::{Normalisation, Sampler, TensorShape};

/// Writes the face `transform` aligns into `dest` as the planar CHW `f32` the graph reads: the whole red plane, then
/// the whole green one, then the whole blue one, each `tile` x `tile` and row-major.
///
/// `source` is the photograph's own dimensions, which is what the reflection at its borders is measured against.
///
/// `transform` is the **forward** alignment — from the photograph's coordinates into the aligned square's — and is
/// inverted here rather than by the caller. That is the reference's shape and it removes the one mistake this file
/// can make invisibly: an inverse applied in the forward direction produces a plausible square of the wrong part of
/// the photograph, with nothing at any layer able to notice.
///
/// Where the square reaches past the photograph's border, the picture's own edge pixels are **reflected** back
/// inwards. A face at an edge is exactly the case this covers: a constant fill would put a hard edge through the
/// model's input, which it would restore as picture content.
///
/// # Errors
///
/// Returns [`TensorShape`] when `dest` is not exactly `3 * tile * tile` floats long, having written nothing.
pub(crate) fn align(
    dest: &mut [f32],
    sampler: &Sampler<'_>,
    source: (u32, u32),
    transform: Affine,
    tile: u32,
    norm: Normalisation,
) -> Result<(), TensorShape> {
    let plane = (tile as usize) * (tile as usize);
    let expected = 3 * plane;

    if dest.len() != expected {
        return Err(TensorShape { expected, actual: dest.len(), width: tile, height: tile });
    }

    let (green, blue) = (plane, 2 * plane);

    let (width, height) = (source.0 as i32, source.1 as i32);

    let inverse = transform.inverse();
    let [[m00, m01, m02], [m10, m11, m12]] = inverse.rows;

    for y in 0..tile {
        let base = (y as usize) * (tile as usize);

        for x in 0..tile {
            // Destination coordinates to source coordinates, kept as a single expression rather than hoisting the
            // `y` terms out of the row. Float addition is not associative, so regrouping these can shift a result by
            // one ulp and change an output pixel — which would make a pixel-level comparison against the reference
            // meaningless, and the reference's own comment says the same of its loop.
            let source_x = m00 * x as f32 + m01 * y as f32 + m02;
            let source_y = m10 * x as f32 + m11 * y as f32 + m12;

            let [r, g, b] = bilinear(sampler, source_x, source_y, width, height);

            let index = base + x as usize;

            dest[index] = norm.encode_unit(r);
            dest[green + index] = norm.encode_unit(g);
            dest[blue + index] = norm.encode_unit(b);
        }
    }

    Ok(())
}

/// The `[0, 1]` RGB fraction at the fractional coordinate `(x, y)`, over four reflected reads of `sampler`.
///
/// The interpolation happens in **channel space and in floating point**, over the 16-bit values [`Sampler`] reports,
/// and is converted to a fraction once at the end. The reference lerps the same four 16-bit reads and then narrows
/// the result to eight bits, which is the quantisation this file exists to remove; the weights and the order of the
/// two lerps are its own.
fn bilinear(sampler: &Sampler<'_>, x: f32, y: f32, width: i32, height: i32) -> [f32; 3] {
    let x0 = x.floor();
    let y0 = y.floor();

    // Before the reflection, so the weights describe where the sample sits between two source pixels rather than
    // where it sits between the two the reflection happened to land on.
    let wx = x - x0;
    let wy = y - y0;
    let wx0 = 1.0 - wx;
    let wy0 = 1.0 - wy;

    let x0 = x0 as i32;
    let y0 = y0 as i32;

    let left = reflect_coord(x0, width);
    let right = reflect_coord(x0 + 1, width);
    let top = reflect_coord(y0, height);
    let bottom = reflect_coord(y0 + 1, height);

    let top_left = sampler.rgb(left, top);
    let top_right = sampler.rgb(right, top);
    let bottom_left = sampler.rgb(left, bottom);
    let bottom_right = sampler.rgb(right, bottom);

    let mut channels = [0.0_f32; 3];

    for (channel, value) in channels.iter_mut().enumerate() {
        // Along x first, then along y, which is the reference's own order.
        let upper = f32::from(top_left[channel]) * wx0 + f32::from(top_right[channel]) * wx;
        let lower = f32::from(bottom_left[channel]) * wx0 + f32::from(bottom_right[channel]) * wx;

        *value = (upper * wy0 + lower * wy) / f32::from(u16::MAX);
    }

    channels
}

/// The source coordinate supplying `coord` along an axis `size` pixels long, mirroring the picture's own pixels back
/// inwards past either end.
///
/// The reference's `reflectCoord` with `min` at zero, which is every call it has. It is not
/// [`pad::source_offset`](imaging::pad::source_offset) and cannot be: that function extends a **region** to
/// a tile shape, where an offset is a `u32` and only the far end is ever past the edge, while an aligned square
/// reaches past the **near** edge too — a face at the top-left corner samples negative coordinates, which that
/// signature cannot express.
///
/// One reflection is not enough to get back inside for a coordinate more than a full axis away, so the mirrored
/// value is clamped into the picture — the same fallback `source_offset` documents, and reachable here only from a
/// transform no fitted alignment produces.
fn reflect_coord(coord: i32, size: i32) -> u32 {
    debug_assert!(size > 0, "a warp read an axis with no pixels, which its driver refuses before allocating");

    let mirrored = if coord < 0 {
        -coord - 1
    } else if coord >= size {
        2 * size - coord - 1
    } else {
        coord
    };

    mirrored.clamp(0, size - 1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{DynamicImage, ImageBuffer, Rgb};

    use imaging::tensor::Sampler;

    /// An image's own dimensions, as the warp takes them.
    fn dimensions(image: &DynamicImage) -> (u32, u32) {
        (image.width(), image.height())
    }

    /// A source whose every channel is a different function of the coordinates, so a pixel read from the wrong place
    /// or a plane written to the wrong offset is a value no correct read produces.
    fn source(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([((x * 7 + y * 3) % 256) as u8, ((x * 13 + y * 11) % 256) as u8, ((x * 29 + y * 17) % 256) as u8])
        }))
    }

    /// The one channel of `dest` at `(x, y)` in a `tile`-square planar CHW buffer.
    fn at(dest: &[f32], tile: u32, x: u32, y: u32, channel: usize) -> f32 {
        let plane = (tile * tile) as usize;

        dest[channel * plane + (y * tile + x) as usize]
    }

    /// A `[0, 1]` fraction for an 8-bit channel value, which is what a widened 8-bit sample normalises to.
    fn unit(value: u8) -> f32 {
        f32::from(u16::from(value) * 257) / f32::from(u16::MAX)
    }

    #[test]
    fn an_identity_transform_reproduces_the_sources_own_pixels_in_the_tensor() {
        // The base case the rest of the geometry is read against: under the identity every destination pixel lands
        // exactly on a source pixel, both interpolation weights are zero, and the tensor is the image — which also
        // pins that the three planes are written in red, green, blue order at the offsets a graph expects.
        let tile = 16;
        let image = source(tile, tile);
        let sampler = Sampler::new(&image);

        let mut dest = vec![f32::NAN; 3 * (tile * tile) as usize];
        align(&mut dest, &sampler, dimensions(&image), Affine::IDENTITY, tile, Normalisation::Unit)
            .expect("the buffer fits");

        let buffer = image.as_rgb8().expect("the source is 8-bit RGB");
        for y in 0..tile {
            for x in 0..tile {
                let Rgb(expected) = *buffer.get_pixel(x, y);

                for (channel, value) in expected.iter().enumerate() {
                    assert!(
                        (at(&dest, tile, x, y, channel) - unit(*value)).abs() < 1e-6,
                        "({x}, {y}) channel {channel} was {} rather than {}",
                        at(&dest, tile, x, y, channel),
                        unit(*value)
                    );
                }
            }
        }
    }

    #[test]
    fn a_half_pixel_offset_interpolates_between_the_two_neighbours_it_sits_between() {
        // What makes the sampling bilinear rather than nearest-neighbour. A face is aligned at whatever sub-pixel
        // scale and rotation its landmarks ask for, so almost every destination pixel lands between source pixels,
        // and a nearest-neighbour read would hand the model an aliased face.
        let tile = 8;
        let image = source(tile, tile);
        let sampler = Sampler::new(&image);
        let buffer = image.as_rgb8().expect("the source is 8-bit RGB");

        // Half a pixel to the right: destination (x, y) is to read source (x + 0.5, y). `align` samples through the
        // transform's inverse, so the sampling map is what this test names and the transform is its inverse.
        let sampling = Affine { rows: [[1.0, 0.0, 0.5], [0.0, 1.0, 0.0]] };

        let mut dest = vec![f32::NAN; 3 * (tile * tile) as usize];
        align(&mut dest, &sampler, dimensions(&image), sampling.inverse(), tile, Normalisation::Unit)
            .expect("the buffer fits");

        for channel in 0..3 {
            let left = unit(buffer.get_pixel(2, 3).0[channel]);
            let right = unit(buffer.get_pixel(3, 3).0[channel]);
            let midpoint = (left + right) / 2.0;

            let sampled = at(&dest, tile, 2, 3, channel);

            assert!(
                (sampled - midpoint).abs() < 1e-5,
                "channel {channel} sampled {sampled} rather than the midpoint {midpoint} of {left} and {right}"
            );
            // And it is genuinely between them rather than one of them, which is what a nearest-neighbour read or a
            // weight computed after the reflection would produce.
            assert!(
                (sampled - left).abs() > 1e-5 && (sampled - right).abs() > 1e-5,
                "channel {channel} sampled a neighbour rather than interpolating"
            );
        }
    }

    #[test]
    fn a_square_reaching_past_the_border_reflects_the_picture_rather_than_clamping_or_filling_it() {
        // A face near an edge aligns against a mirrored continuation of the photograph. The two alternatives are
        // both visible in the restoration: a clamp smears one edge pixel across the margin, and a constant fill puts
        // a hard edge through the model's input, which it restores as picture content.
        let tile = 8;
        let image = source(tile, tile);
        let sampler = Sampler::new(&image);
        let buffer = image.as_rgb8().expect("the source is 8-bit RGB");

        // Destination (0, y) reads source (-1.5, y): its neighbours are -2 and -1, which reflect to 1 and 0.
        let sampling = Affine { rows: [[1.0, 0.0, -1.5], [0.0, 1.0, 0.0]] };

        let mut dest = vec![f32::NAN; 3 * (tile * tile) as usize];
        align(&mut dest, &sampler, dimensions(&image), sampling.inverse(), tile, Normalisation::Unit)
            .expect("the buffer fits");

        for channel in 0..3 {
            let first = unit(buffer.get_pixel(0, 4).0[channel]);
            let second = unit(buffer.get_pixel(1, 4).0[channel]);
            let reflected = (first + second) / 2.0;

            let sampled = at(&dest, tile, 0, 4, channel);

            assert!(
                (sampled - reflected).abs() < 1e-5,
                "channel {channel} sampled {sampled} rather than the reflection {reflected}"
            );
            assert!((sampled - first).abs() > 1e-5, "channel {channel} clamped to the edge pixel instead");
            assert!(sampled > 0.0 || first > 0.0, "channel {channel} was filled with a colour the picture has not");
        }

        // The far edge mirrors the same way, which is the case `pad::source_offset` covers for a tile and this
        // function has to cover for a square that overhangs on either side.
        // Destination (0, y) reads source (8.5, y) of an 8-pixel axis: its neighbours are 8 and 9, which reflect to
        // 7 and 6.
        let sampling = Affine { rows: [[1.0, 0.0, tile as f32 + 0.5], [0.0, 1.0, 0.0]] };
        let mut dest = vec![f32::NAN; 3 * (tile * tile) as usize];
        align(&mut dest, &sampler, dimensions(&image), sampling.inverse(), tile, Normalisation::Unit)
            .expect("the buffer fits");

        for channel in 0..3 {
            let last = unit(buffer.get_pixel(tile - 1, 4).0[channel]);
            let previous = unit(buffer.get_pixel(tile - 2, 4).0[channel]);
            let sampled = at(&dest, tile, 0, 4, channel);

            assert!(
                (sampled - (last + previous) / 2.0).abs() < 1e-5,
                "channel {channel} sampled {sampled} past the right edge rather than the reflection"
            );
        }
    }

    #[test]
    fn a_sixteen_bit_source_is_not_narrowed_on_the_way_into_the_tensor() {
        // The whole of why this writes the tensor directly rather than warping into an image first. The reference
        // narrows to eight bits inside the warp and widens back to a float afterwards, so a 16-bit RAW reaches the
        // model with 256 levels; here it reaches it with the levels it has.
        let tile = 4;
        // 1000 is not a multiple of 257, so it is a value no widened 8-bit channel can produce: narrowing it and
        // widening it back lands on 1028.
        let image = DynamicImage::ImageRgb16(ImageBuffer::from_fn(tile, tile, |_, _| Rgb([1000_u16, 40000, 65535])));
        let sampler = Sampler::new(&image);

        let mut dest = vec![f32::NAN; 3 * (tile * tile) as usize];
        align(&mut dest, &sampler, dimensions(&image), Affine::IDENTITY, tile, Normalisation::Unit)
            .expect("the buffer fits");

        let expected = [1000.0, 40000.0, 65535.0].map(|value: f32| value / f32::from(u16::MAX));
        let narrowed = 1028.0 / f32::from(u16::MAX);

        for (channel, value) in expected.iter().enumerate() {
            let sampled = at(&dest, tile, 1, 1, channel);

            assert!((sampled - value).abs() < 1e-7, "channel {channel} sampled {sampled} rather than {value}");
        }

        assert!(
            (at(&dest, tile, 1, 1, 0) - narrowed).abs() > 1e-7,
            "the red channel came back at the value an 8-bit round trip produces"
        );
    }

    #[test]
    fn the_tensor_the_graph_is_fed_is_inside_the_range_it_was_normalised_over() {
        // Athens' graph is trained against `[-1, 1]`, and a warp that wrote outside it — through a weight that did
        // not sum to one, or a reflection that read past the buffer — is not an error a runtime reports.
        let tile = 12;
        let image = source(32, 20);
        let sampler = Sampler::new(&image);

        // A rotation and a scale, so most destination pixels interpolate and the square reaches past two borders.
        let fitted = Affine { rows: [[0.6, -0.4, 3.0], [0.4, 0.6, -5.0]] };

        let mut dest = vec![f32::NAN; 3 * (tile * tile) as usize];
        align(&mut dest, &sampler, dimensions(&image), fitted, tile, Normalisation::Signed).expect("the buffer fits");

        for (index, value) in dest.iter().enumerate() {
            assert!((-1.0..=1.0).contains(value), "the tensor holds {value} at {index}, outside [-1, 1]");
        }

        // And the range is genuinely the signed one rather than `[0, 1]` happening to fit inside it.
        assert!(
            dest.iter().any(|value| *value < 0.0),
            "nothing was negative, so the normalisation was not signed"
        );
    }

    #[test]
    fn a_buffer_the_square_does_not_fit_is_refused_with_nothing_written() {
        // A caller's scratch and the tile size it believes the graph accepts having drifted apart. Writing what fits
        // would feed the model the previous face's pixels in the region left over.
        let tile = 8;
        let image = source(tile, tile);
        let sampler = Sampler::new(&image);
        let needed = 3 * (tile * tile) as usize;

        for length in [needed - 1, needed + 1, 0] {
            let mut dest = vec![f32::NAN; length];

            let error = align(&mut dest, &sampler, dimensions(&image), Affine::IDENTITY, tile, Normalisation::Unit)
                .expect_err("a buffer of the wrong length was accepted");

            assert_eq!((error.expected, error.actual), (needed, length));
            assert!(dest.iter().all(|value| value.is_nan()), "a refused call wrote to the buffer");
        }
    }
}
