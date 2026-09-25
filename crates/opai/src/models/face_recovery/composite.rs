//! Writing one restored face back into the photograph, feathered through the blend mask.
//!
//! The destination is walked rather than the aligned square, and each destination pixel is mapped **forward** into
//! the square. That is the reference's own optimisation and its reasoning holds unchanged: the mask lives in the
//! aligned square's coordinates, which is exactly where the forward transform lands, so neither the mask nor the
//! restoration has to be pre-warped across the whole frame.
//!
//! One buffer, read and written in the same pass. Each destination pixel is read as the background and written in
//! the same iteration, so a second buffer would buy nothing — and reading the destination is what makes two
//! overlapping faces composite in the order the operation carries them, which is why a run over one selection
//! produces one deterministic image.
//!
//! Nothing here names a variant, and the square, the transform, the normalisation and the channel are all the
//! caller's.

use image::{ImageBuffer, Rgb};

use super::transform::Affine;

use crate::models::face::Rect;
use imaging::tensor::{Channel, Normalisation};

/// How far past its own bounding box a face's blend window reaches, as a fraction of the box's longest side.
///
/// The mask is feathered out to the aligned square's boundary, which is well outside the box the detector drew, so a
/// window clipped to the box alone would cut the falloff off partway and draw the edge the feathering exists to
/// remove.
const MASK_MARGIN_FACTOR: f32 = 0.5;

/// The lowest mask weight still worth blending. Below it the restoration's contribution is under half a level of an
/// 8-bit channel, and the read, the four-tap sample and the write buy nothing.
const MIN_BLEND_ALPHA: f32 = 0.001;

/// Composites the restored face in `restored` into `dest`, weighted by `mask`, through `transform`.
///
/// `transform` is the **forward** alignment, from the photograph's coordinates into the aligned square's — the same
/// value [`warp::align`](super::warp::align) was given, used here as it stands rather than inverted. `restored` is
/// the graph's own planar CHW output at `tile` x `tile` under `norm`, and `mask` is the `tile`-square alpha plane.
///
/// Writing is bounded to `bounding_box` expanded by [`MASK_MARGIN_FACTOR`] of its longest side and clipped to the
/// picture; everything outside that window is left exactly as it was, and so is every pixel inside it whose mask
/// weight is below [`MIN_BLEND_ALPHA`].
pub(crate) fn blend<T: Channel>(
    dest: &mut ImageBuffer<Rgb<T>, Vec<T>>,
    restored: &[f32],
    mask: &[f32],
    transform: Affine,
    bounding_box: Rect,
    tile: u32,
    norm: Normalisation,
) where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    let plane = (tile as usize) * (tile as usize);

    debug_assert_eq!(restored.len(), 3 * plane, "the restoration is not three planes of the square it was run at");
    debug_assert_eq!(mask.len(), plane, "the mask is not the square the restoration was run at");

    let Some(window) = window(bounding_box, dest.width(), dest.height()) else {
        return;
    };

    let [[a00, a01, a02], [a10, a11, a12]] = transform.rows;

    let extent = tile as f32;
    let (red, green, blue) = (&restored[..plane], &restored[plane..2 * plane], &restored[2 * plane..]);

    for y in window.top..window.bottom {
        // Hoisted out of the row, as the reference's blend hoists it and unlike the warp's own loop: this is the
        // composite rather than the model's input, so a one-ulp regrouping lands under the mask's own weighting
        // rather than in the tensor a graph is judged against.
        let aligned_x_base = a01 * y as f32 + a02;
        let aligned_y_base = a11 * y as f32 + a12;

        for x in window.left..window.right {
            let aligned_x = a00 * x as f32 + aligned_x_base;
            let aligned_y = a10 * x as f32 + aligned_y_base;

            // Outside the aligned square there is no restoration to read: the window is the box plus a margin, and
            // a rotated square does not cover all of it.
            if aligned_x < 0.0 || aligned_x >= extent || aligned_y < 0.0 || aligned_y >= extent {
                continue;
            }

            // The mask and the three colour planes are read at the same point, so the geometry of that read is
            // resolved once here and used four times below.
            let taps = Taps::at(tile, aligned_x, aligned_y);
            let alpha = taps.read(mask);

            if alpha <= MIN_BLEND_ALPHA {
                continue;
            }

            let remainder = 1.0 - alpha;

            // The restoration is sampled as **floats** and quantised once, at the write. Decoding the tensor to an
            // image and sampling that would quantise it to the output depth before the blend instead of after,
            // which is the same choice the diffusion upscaler's canvas already makes.
            let mixed = [red, green, blue].map(|source| taps.read(source));

            // One borrow rather than a `get_pixel` and a `put_pixel`, which bounds-check and index the same pixel
            // twice over.
            let pixel = dest.get_pixel_mut(x, y);

            *pixel = Rgb([
                T::from_unit(norm.decode(mixed[0]) * alpha + pixel.0[0].to_unit() * remainder),
                T::from_unit(norm.decode(mixed[1]) * alpha + pixel.0[1].to_unit() * remainder),
                T::from_unit(norm.decode(mixed[2]) * alpha + pixel.0[2].to_unit() * remainder),
            ]);
        }
    }
}

/// The half-open pixel window a face is composited into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Window {
    /// The first column written.
    left: u32,
    /// The first row written.
    top: u32,
    /// One past the last column written.
    right: u32,
    /// One past the last row written.
    bottom: u32,
}

/// `bounding_box` expanded by its margin and clipped to a `width` by `height` picture, or `None` where the clip
/// leaves nothing.
///
/// Truncated towards zero on each side rather than rounded, which is what the reference's `int()` conversion does,
/// and then clipped — so a box that starts before the picture begins or ends after it does writes inside it and
/// nowhere else.
fn window(bounding_box: Rect, width: u32, height: u32) -> Option<Window> {
    let box_width = bounding_box.max.x - bounding_box.min.x;
    let box_height = bounding_box.max.y - bounding_box.min.y;
    let margin = box_width.max(box_height) * MASK_MARGIN_FACTOR;

    let left = clip(bounding_box.min.x - margin, width);
    let top = clip(bounding_box.min.y - margin, height);
    let right = clip(bounding_box.max.x + margin, width);
    let bottom = clip(bounding_box.max.y + margin, height);

    if left >= right || top >= bottom {
        return None;
    }

    Some(Window { left, top, right, bottom })
}

/// One edge of the window: truncated towards zero as the reference's `int()` is, then clipped into `0..=length`.
fn clip(coordinate: f32, length: u32) -> u32 {
    if !coordinate.is_finite() || coordinate <= 0.0 {
        return 0;
    }

    (coordinate as u32).min(length)
}

/// The four samples and weights one bilinear read of a `stride`-square plane is made of, resolved once.
///
/// **Built per pixel and used four times.** The mask and the three colour planes are all read at the same
/// `(x, y)`, and the geometry of that read — which two rows, which two columns, and the four weights — is the same
/// for all of them; only the values loaded differ. Resolving it once turns four copies of two `floor`s, four
/// weights, four clamps and two row multiplies into one.
///
/// **Clamped** at the edges rather than reflected, which is `blendFaceInto`'s own choice and the opposite of the
/// warp's: every read here is inside the square by construction — the caller has already skipped the pixels that
/// land outside it — so the clamp only ever catches the last fraction of a pixel at the far edge, where a
/// reflection would fold the square's opposite side in.
struct Taps {
    /// The index of the top-left sample.
    top_left: usize,
    /// The index of the top-right sample.
    top_right: usize,
    /// The index of the bottom-left sample.
    bottom_left: usize,
    /// The index of the bottom-right sample.
    bottom_right: usize,
    /// The weight of the left column, `1 - wx`.
    wx0: f32,
    /// The weight of the right column.
    wx: f32,
    /// The weight of the top row, `1 - wy`.
    wy0: f32,
    /// The weight of the bottom row.
    wy: f32,
}

impl Taps {
    /// The taps a read of a `stride`-square plane at `(x, y)` is made of.
    fn at(stride: u32, x: f32, y: f32) -> Self {
        let x0 = x.floor();
        let y0 = y.floor();

        let wx = x - x0;
        let wy = y - y0;

        let last = stride.saturating_sub(1);
        let clamped = |value: f32| -> u32 {
            if value <= 0.0 {
                return 0;
            }

            (value as u32).min(last)
        };

        let left = clamped(x0) as usize;
        let right = clamped(x0 + 1.0) as usize;
        let top = (clamped(y0) * stride) as usize;
        let bottom = (clamped(y0 + 1.0) * stride) as usize;

        Self {
            top_left: top + left,
            top_right: top + right,
            bottom_left: bottom + left,
            bottom_right: bottom + right,
            wx0: 1.0 - wx,
            wx,
            wy0: 1.0 - wy,
            wy,
        }
    }

    /// `plane`'s value at the point these taps were resolved for.
    ///
    /// Summed in the order the reference sums it — along x within each row, then between the rows — because a
    /// regrouping here is a one-ulp difference in a value the mask then weights.
    fn read(&self, plane: &[f32]) -> f32 {
        let upper = plane[self.top_left] * self.wx0 + plane[self.top_right] * self.wx;
        let lower = plane[self.bottom_left] * self.wx0 + plane[self.bottom_right] * self.wx;

        upper * self.wy0 + lower * self.wy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::face::Point;

    /// The bilinear read as one function per plane, the shape `Taps` replaced.
    ///
    /// Kept as the definition the tap set is measured against: resolving the geometry once and reusing it is a
    /// performance change and nothing else, so any value it alters is a defect.
    fn sample_unoptimised(plane: &[f32], stride: u32, x: f32, y: f32) -> f32 {
        let x0 = x.floor();
        let y0 = y.floor();

        let wx = x - x0;
        let wy = y - y0;
        let wx0 = 1.0 - wx;
        let wy0 = 1.0 - wy;

        let last = stride.saturating_sub(1);
        let clamped = |value: f32| -> u32 {
            if value <= 0.0 {
                return 0;
            }

            (value as u32).min(last)
        };

        let left = clamped(x0);
        let right = clamped(x0 + 1.0);
        let top = clamped(y0);
        let bottom = clamped(y0 + 1.0);

        let row = |y: u32| (y * stride) as usize;

        let upper = plane[row(top) + left as usize] * wx0 + plane[row(top) + right as usize] * wx;
        let lower = plane[row(bottom) + left as usize] * wx0 + plane[row(bottom) + right as usize] * wx;

        upper * wy0 + lower * wy
    }

    #[test]
    fn a_reused_tap_set_reads_exactly_what_a_per_plane_sample_read() {
        let stride = 8_u32;
        let plane: Vec<f32> =
            (0..stride * stride).map(|index| ((index as f32) * 1.3).cos().mul_add(0.5, 0.5)).collect();

        // Sub-pixel positions across the square, including both edges — where the clamp is the only thing keeping
        // the read inside the plane — and a little past them, which the caller's window check normally excludes.
        for step_y in 0..40 {
            for step_x in 0..40 {
                let x = (step_x as f32) * 0.2 - 0.2;
                let y = (step_y as f32) * 0.2 - 0.2;

                let taps = Taps::at(stride, x, y);

                // Exact equality: the same four loads combined in the same order.
                assert_eq!(taps.read(&plane), sample_unoptimised(&plane, stride, x, y), "drifted at ({x}, {y})");
            }
        }
    }

    /// A destination filled with one flat background, so any pixel that differs from it was written.
    fn background(width: u32, height: u32, value: u8) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        ImageBuffer::from_pixel(width, height, Rgb([value, value, value]))
    }

    /// A restoration of one flat colour, as three planes of a `tile`-square planar CHW tensor under `Unit`.
    fn restoration(tile: u32, value: f32) -> Vec<f32> {
        vec![value; 3 * (tile * tile) as usize]
    }

    /// A box from `(min_x, min_y)` to `(max_x, max_y)`.
    fn boxed(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Rect {
        Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y))
    }

    /// Every pixel of `image` that differs from `value`, as a window, or `None` where none does.
    fn written(image: &ImageBuffer<Rgb<u8>, Vec<u8>>, value: u8) -> Option<Window> {
        let mut found: Option<Window> = None;

        for (x, y, pixel) in image.enumerate_pixels() {
            if pixel.0 == [value, value, value] {
                continue;
            }

            found = Some(match found {
                None => Window { left: x, top: y, right: x + 1, bottom: y + 1 },
                Some(window) => Window {
                    left: window.left.min(x),
                    top: window.top.min(y),
                    right: window.right.max(x + 1),
                    bottom: window.bottom.max(y + 1),
                },
            });
        }

        found
    }

    #[test]
    fn the_written_window_is_the_box_plus_its_margin_clipped_to_the_picture() {
        // The bound the whole file rests on: a run over one face in a group photograph touches that face's
        // neighbourhood and nothing else, so everything more than a margin from a bounding box is the photograph the
        // caller handed over.
        let tile = 64;
        let mut dest = background(64, 64, 10);

        // 20 wide, 10 tall, so the margin is half of 20.
        let face = boxed(20.0, 25.0, 40.0, 35.0);
        blend(
            &mut dest,
            &restoration(tile, 1.0),
            &vec![1.0; (tile * tile) as usize],
            Affine::IDENTITY,
            face,
            tile,
            Normalisation::Unit,
        );

        // The box expanded by ten on every side: 10..50 across and 15..45 down.
        assert_eq!(written(&dest, 10), Some(Window { left: 10, top: 15, right: 50, bottom: 45 }));
    }

    #[test]
    fn nothing_outside_the_window_is_touched() {
        // Stated as its own property rather than read off the one above, because the failure is different: a write
        // one pixel outside the window is a mask sampled in the wrong space, and it would show as a smear across a
        // photograph rather than as a misplaced face.
        let tile = 64;
        let mut dest = background(64, 64, 10);
        let face = boxed(20.0, 25.0, 40.0, 35.0);

        blend(
            &mut dest,
            &restoration(tile, 1.0),
            &vec![1.0; (tile * tile) as usize],
            Affine::IDENTITY,
            face,
            tile,
            Normalisation::Unit,
        );

        for (x, y, pixel) in dest.enumerate_pixels() {
            let inside = (10..50).contains(&x) && (15..45).contains(&y);

            if !inside {
                assert_eq!(pixel.0, [10, 10, 10], "({x}, {y}) is outside the window and was written");
            }
        }
    }

    #[test]
    fn an_opaque_centre_writes_the_restoration_and_a_zero_rim_leaves_the_background() {
        // The two ends of the blend, checked against a mask whose shape is known rather than against the feathered
        // one: what is asserted is that the weight is applied at all and applied the right way round. A mask read
        // inverted would restore the background over the face and leave the face untouched at its centre.
        let tile = 32;
        let mut dest = background(32, 32, 0);

        // Opaque over the middle quarter of the square, nothing outside it.
        let mut mask = vec![0.0_f32; (tile * tile) as usize];
        for y in 12..20 {
            for x in 12..20 {
                mask[(y * tile + x) as usize] = 1.0;
            }
        }

        let face = boxed(0.0, 0.0, 32.0, 32.0);
        blend(&mut dest, &restoration(tile, 1.0), &mask, Affine::IDENTITY, face, tile, Normalisation::Unit);

        assert_eq!(dest.get_pixel(16, 16).0, [255, 255, 255], "the opaque centre did not take the restoration");
        assert_eq!(dest.get_pixel(2, 2).0, [0, 0, 0], "a zero-alpha pixel took a contribution");
        assert_eq!(dest.get_pixel(31, 31).0, [0, 0, 0], "a zero-alpha pixel took a contribution");

        // And a half weight is half of each, which is what makes this a blend rather than a stencil.
        let mut half = background(32, 32, 0);
        blend(
            &mut half,
            &restoration(tile, 1.0),
            &vec![0.5; (tile * tile) as usize],
            Affine::IDENTITY,
            face,
            tile,
            Normalisation::Unit,
        );

        assert_eq!(half.get_pixel(16, 16).0, [128, 128, 128], "a half weight did not mix the two halves");
    }

    #[test]
    fn a_second_face_composites_over_what_the_first_one_left() {
        // What makes a run over one selection deterministic: the destination is read as the background on every
        // face, so two overlapping neighbourhoods land in the order the operation carries them rather than in
        // whichever order the buffer happened to be written in.
        let tile = 32;
        let mut dest = background(32, 32, 0);
        let face = boxed(0.0, 0.0, 32.0, 32.0);

        // A half-weight restoration of white over black leaves mid grey; a second half-weight pass of white over
        // that grey leaves three quarters — 192 rather than 191 because the grey it reads back is 128/255, half a
        // level above a half. A composite that read the original background twice would leave 128.
        let half = vec![0.5_f32; (tile * tile) as usize];

        blend(&mut dest, &restoration(tile, 1.0), &half, Affine::IDENTITY, face, tile, Normalisation::Unit);
        assert_eq!(dest.get_pixel(16, 16).0, [128, 128, 128]);

        blend(&mut dest, &restoration(tile, 1.0), &half, Affine::IDENTITY, face, tile, Normalisation::Unit);
        assert_eq!(dest.get_pixel(16, 16).0, [192, 192, 192], "the second face did not composite over the first");

        // Ignoring the mask's own contribution, the two orders are not the same image either, which is the fact the
        // order-sensitivity is for: the restorations differ, so the later one wins the larger share.
        let mut other = background(32, 32, 0);
        blend(&mut other, &restoration(tile, 1.0), &half, Affine::IDENTITY, face, tile, Normalisation::Unit);
        blend(&mut other, &restoration(tile, 0.0), &half, Affine::IDENTITY, face, tile, Normalisation::Unit);
        assert_eq!(other.get_pixel(16, 16).0, [64, 64, 64], "the two faces did not composite in the order given");
    }

    #[test]
    fn a_face_at_the_pictures_edge_writes_nothing_outside_it() {
        // The clip, and the reason it is a clip rather than a bounds check per pixel: a face at a border has a
        // neighbourhood that runs off the picture on two sides, and both the negative coordinate and the one past
        // the far edge have to become no write rather than a panic or a wrapped index.
        let tile = 64;
        let mask = vec![1.0_f32; (tile * tile) as usize];

        let mut dest = background(40, 40, 10);
        blend(
            &mut dest,
            &restoration(tile, 1.0),
            &mask,
            Affine::IDENTITY,
            boxed(-30.0, -20.0, 10.0, 10.0),
            tile,
            Normalisation::Unit,
        );

        // The window's own edges, clipped: nothing before the picture starts and nothing past where it ends.
        assert_eq!(written(&dest, 10), Some(Window { left: 0, top: 0, right: 30, bottom: 30 }));

        let mut dest = background(40, 40, 10);
        blend(
            &mut dest,
            &restoration(tile, 1.0),
            &mask,
            Affine::IDENTITY,
            boxed(30.0, 30.0, 70.0, 70.0),
            tile,
            Normalisation::Unit,
        );

        assert_eq!(written(&dest, 10), Some(Window { left: 10, top: 10, right: 40, bottom: 40 }));

        // And a box entirely off the picture writes nothing at all rather than an empty range that underflows.
        let mut dest = background(40, 40, 10);
        blend(
            &mut dest,
            &restoration(tile, 1.0),
            &mask,
            Affine::IDENTITY,
            boxed(200.0, 200.0, 210.0, 210.0),
            tile,
            Normalisation::Unit,
        );

        assert_eq!(written(&dest, 10), None, "a face off the picture was composited into it");
    }

    #[test]
    fn a_pixel_the_square_does_not_cover_is_left_as_it_was() {
        // The window is the box plus a margin in the *photograph's* coordinates, and a rotated or scaled square does
        // not cover all of it. Those pixels are the photograph, not a restoration sampled from the square's edge.
        let tile = 8;
        let mut dest = background(32, 32, 10);

        // The square covers only the picture's top-left 8 by 8 under the identity.
        blend(
            &mut dest,
            &restoration(tile, 1.0),
            &vec![1.0; (tile * tile) as usize],
            Affine::IDENTITY,
            boxed(0.0, 0.0, 32.0, 32.0),
            tile,
            Normalisation::Unit,
        );

        assert_eq!(written(&dest, 10), Some(Window { left: 0, top: 0, right: 8, bottom: 8 }));
    }

    #[test]
    fn a_restoration_is_sampled_in_the_squares_own_space_rather_than_the_pictures() {
        // The forward mapping, checked where getting it backwards would still produce an image: a translated
        // transform has to move which part of the *square* each destination pixel reads, not merely where the
        // writing happens.
        let tile = 16;
        let mut mask = vec![0.0_f32; (tile * tile) as usize];
        // Opaque in the square's left half alone, so where the opacity lands in the picture says which way the
        // transform was applied.
        for y in 0..tile {
            for x in 0..tile / 2 {
                mask[(y * tile + x) as usize] = 1.0;
            }
        }

        // The photograph's x = 8 maps to the square's x = 0, so the square's opaque left half lands on the
        // photograph's columns 8 to 15.
        let shifted = Affine { rows: [[1.0, 0.0, -8.0], [0.0, 1.0, 0.0]] };

        let mut dest = background(16, 16, 10);
        blend(
            &mut dest,
            &restoration(tile, 1.0),
            &mask,
            shifted,
            boxed(0.0, 0.0, 16.0, 16.0),
            tile,
            Normalisation::Unit,
        );

        assert_eq!(written(&dest, 10), Some(Window { left: 8, top: 0, right: 16, bottom: 16 }));
    }
}
