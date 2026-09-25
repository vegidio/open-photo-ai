//! Writing a finished tile into the picture without leaving a seam where it meets the last one.
//!
//! **One of this crate's two ways of combining tiles**, and the one for a model whose output is deterministic: each
//! tile is written *over* what is already in the buffer, at the caller's channel depth. A model drawing its own noise
//! per tile uses [`canvas`](crate::canvas) instead.

use image::{ImageBuffer, Rgb};

use crate::tensor::Channel;

// Derived from the channel's depth rather than written as one figure, because what "indistinguishable" means is half a
// level, and a level is 1/255 of the range at eight bits and 1/65535 at sixteen. Above this weight the blend moves the
// incoming value by less than half a level, so rounding lands it back on that value exactly. A fixed 0.999 held at
// eight bits and not at sixteen, where the 0.001 it waved through is 65 levels. Naming it keeps the threshold and the
// fast path that tests it from drifting apart, and it is what makes an unblended edge a byte-for-byte copy rather than
// a round trip through a float.
/// The weight above which a blend at `T`'s depth is indistinguishable from a straight write.
fn opaque<T: Channel>() -> f32
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    1.0 - 0.5 / T::MAX_VALUE
}

/// The incoming tile's weight at `offset` into a ramp `width` pixels wide, and 1 once past it — a zero width being
/// "no neighbour on this side". A raised cosine, strictly positive from the first offset.
#[inline]
pub fn ramp_weight(offset: u32, width: u32) -> f32 {
    // Usable without `blend_tile`, because `canvas` feathers the diffusion upscaler's tiles into a weighted float
    // accumulator with this same curve. The two agreeing on it is load-bearing — a tuning change applied to only one
    // of them shows up as a seam.
    //
    // A raised cosine rather than a straight line. Both are continuous, but a linear ramp's slope jumps at each end of
    // the band, and on a smooth gradient that reads as a visible edge exactly where the blend starts and stops.
    if width == 0 || offset >= width {
        return 1.0;
    }

    // The half-pixel offset is what keeps the first weight strictly positive: at offset 0 a bare cosine is exactly 0,
    // which throws the incoming tile's outermost column away entirely rather than mixing it in.
    let position = (f64::from(offset) + 0.5) / f64::from(width);

    (0.5 - 0.5 * (std::f64::consts::PI * position).cos()) as f32
}

/// Writes `tile` into `dest` at `(x, y)`, ramping each edge that meets an already-written neighbour.
///
/// `overlap_x` and `overlap_y` are how many pixels this tile **actually** overlaps its left and top neighbours — not
/// the configured overlap; see [`Layout::overlaps_x`](crate::grid::Layout::overlaps_x) — and zero means there is no
/// neighbour on that side and that edge is written straight.
///
/// Where a pixel lies in a band on both axes the two weights multiply, so a corner is weighted by each.
///
/// A tile that runs past `dest` is written only as far as `dest` goes.
pub fn blend_tile<T: Channel>(
    dest: &mut ImageBuffer<Rgb<T>, Vec<T>>,
    tile: &ImageBuffer<Rgb<T>, Vec<T>>,
    x: u32,
    y: u32,
    overlap_x: u32,
    overlap_y: u32,
) where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    let columns = dest.width().saturating_sub(x).min(tile.width());
    let rows = dest.height().saturating_sub(y).min(tile.height());

    if columns == 0 || rows == 0 {
        return;
    }

    // A ramp wider than half the tile would have its two ends overlap each other and stop being monotonic, which
    // here is a visible band: this writes over what is underneath, so a weight that dips back down is the picture
    // dipping with it. `canvas::axis_weights` does **not** clamp, and says why.
    let ramp_x = overlap_x.min(columns / 2);
    let ramp_y = overlap_y.min(rows / 2);

    let opaque = opaque::<T>();

    // Tabulated once per tile rather than evaluated per pixel: the horizontal weight depends only on the column, so
    // the loop below would otherwise recompute the same cosines on every one of `rows` rows. The vertical weight is
    // already a per-row value, so it stays a direct call.
    let horizontal: Vec<f32> = (0..ramp_x).map(|offset| ramp_weight(offset, ramp_x)).collect();

    // The strides the row copy below indexes with. Read before `dest` is borrowed mutably.
    let (dest_width, tile_width) = (dest.width() as usize, tile.width() as usize);

    for row in 0..rows {
        let vertical = ramp_weight(row, ramp_y);

        // Past the vertical ramp every column at or beyond `ramp_x` carries weight 1: the tile simply replaces what
        // is underneath. That is the large majority of every tile — at the default geometry a 4x pass decodes to
        // 1024x1024, of which the ramp band is two strips 16 pixels wide — and it is a copy, so it is done as one.
        // Per pixel it would be two bounds checks and an index recomputation each, four hundred tiles to a pass.
        if vertical > opaque && ramp_x < columns {
            let span = ((columns - ramp_x) * 3) as usize;
            let from = (row as usize * tile_width + ramp_x as usize) * 3;
            let into = ((y + row) as usize * dest_width + (x + ramp_x) as usize) * 3;

            // Through the buffers' own sample slices: `ImageBuffer`'s `Index` is by coordinate, so the raw samples
            // have to be reached for explicitly. Both are `Rgb<T>` with a stride of `width * 3`, so the spans line up.
            let written: &mut [T] = dest;
            written[into..into + span].copy_from_slice(&(**tile)[from..from + span]);
        }

        // The ramp band, and — on a row still inside the vertical ramp — the whole width, since there the tile is
        // mixed with what is underneath at every column.
        let blended_to = if vertical > opaque { ramp_x } else { columns };

        for column in 0..blended_to {
            let weight = match horizontal.get(column as usize) {
                Some(across) => vertical * across,
                None => vertical,
            };

            let source = *tile.get_pixel(column, row);

            let blended = if weight > opaque {
                // The tile replaces what is underneath, so the value is carried across unchanged rather than round
                // tripped through a float.
                source
            } else {
                let under = *dest.get_pixel(x + column, y + row);
                let mut mixed = source;

                for channel in 0..3 {
                    let over = source.0[channel].to_unit();
                    mixed.0[channel] = T::from_unit(under.0[channel].to_unit().mul_add(1.0 - weight, over * weight));
                }

                mixed
            };

            dest.put_pixel(x + column, y + row, blended);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A uniform buffer at a `[0, 1]` level, so one scenario can be stated once and run at both depths.
    fn flat<T: Channel>(width: u32, height: u32, level: f32) -> ImageBuffer<Rgb<T>, Vec<T>>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let value = T::from_unit(level);
        ImageBuffer::from_pixel(width, height, Rgb([value, value, value]))
    }

    /// The red channel of one destination row, as `[0, 1]` levels — the units every assertion below is written in,
    /// so the same tolerances hold at eight bits and at sixteen.
    fn row<T: Channel>(dest: &ImageBuffer<Rgb<T>, Vec<T>>, y: u32) -> Vec<f32>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        (0..dest.width()).map(|x| dest.get_pixel(x, y).0[0].to_unit()).collect()
    }

    /// The largest jump between adjacent columns of `levels` over `range`.
    fn largest_step(levels: &[f32], range: std::ops::Range<usize>) -> f32 {
        levels[range].windows(2).map(|pair| (pair[1] - pair[0]).abs()).fold(0.0, f32::max)
    }

    /// A flat tile written over a differently-coloured background across a seam 120 pixels wide — the width the last
    /// column of a real grid actually shares, rather than the 16 it was configured with.
    fn a_wide_seam_leaves_no_step<T: Channel>()
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let mut dest = flat::<T>(400, 4, 40.0 / 255.0);
        let tile = flat::<T>(240, 4, 200.0 / 255.0);

        blend_tile(&mut dest, &tile, 100, 0, 120, 0);
        let levels = row(&dest, 0);

        // Across the whole seam — the column before the tile starts, through the ramp, to the first column past it.
        let step = largest_step(&levels, 99..222);
        assert!(step < 0.02, "a 120-pixel seam left a {step} step between adjacent columns");

        // The ramp actually spans the seam rather than ending early: it starts near the background and reaches the
        // tile's own level by the far side of the band.
        assert!((levels[100] - 40.0 / 255.0).abs() < 0.01, "the seam did not start at the background");
        assert!((levels[220] - 200.0 / 255.0).abs() < 0.01, "the seam did not reach the tile by its far side");

        // And the regression this figure exists to prevent: ramping the same seam over the *configured* 16 pixels
        // leaves a step several times larger, which is the hard edge down the last column of the picture.
        let mut narrow = flat::<T>(400, 4, 40.0 / 255.0);
        blend_tile(&mut narrow, &tile, 100, 0, 16, 0);

        let narrow_step = largest_step(&row(&narrow, 0), 99..222);
        assert!(narrow_step > step * 3.0, "ramping a 120-pixel seam over 16 pixels left no larger step");
    }

    /// An edge with no already-written neighbour is written at full strength, byte for byte.
    fn an_edge_with_no_neighbour_is_written_unchanged<T: Channel + std::fmt::Debug>()
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let mut dest = flat::<T>(64, 64, 0.0);
        let mut tile = flat::<T>(32, 32, 0.75);
        tile.put_pixel(0, 0, Rgb([T::from_unit(0.25), T::from_unit(0.5), T::from_unit(1.0)]));

        blend_tile(&mut dest, &tile, 0, 0, 0, 0);

        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(dest.get_pixel(x, y), tile.get_pixel(x, y), "the first tile was altered at ({x}, {y})");
            }
        }

        // Nothing outside the tile was touched to weight it.
        assert_eq!(*dest.get_pixel(32, 0), Rgb([T::from_unit(0.0); 3]));
    }

    /// A pixel inside a band on both axes is weighted by each of them.
    fn a_corner_is_weighted_by_both_axes<T: Channel>()
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let mut dest = flat::<T>(200, 200, 0.0);
        let tile = flat::<T>(100, 100, 1.0);

        blend_tile(&mut dest, &tile, 50, 50, 20, 20);

        // Over a black background the level a pixel reaches is its weight. Read ten pixels into both bands, where
        // the two weights are far enough from their endpoints that either depth can tell them apart.
        let corner = dest.get_pixel(60, 60).0[0].to_unit();
        let in_column_only = dest.get_pixel(60, 90).0[0].to_unit();
        let in_row_only = dest.get_pixel(90, 60).0[0].to_unit();

        let across = ramp_weight(10, 20);
        assert!(
            (corner - across * across).abs() < 0.01,
            "the corner is at {corner}, not the product of both bands"
        );
        assert!((in_column_only - across).abs() < 0.01, "a pixel in one band only carries more than that band");
        assert!(corner < in_column_only - 0.1, "the corner was not weighted by the vertical band as well");
        assert!(corner < in_row_only - 0.1, "the corner was not weighted by the horizontal band as well");

        // And the outermost corner, where both weights are at their smallest, is the product there too.
        let outermost = dest.get_pixel(50, 50).0[0].to_unit();
        let expected = ramp_weight(0, 20) * ramp_weight(0, 20);
        assert!((outermost - expected).abs() < 0.01, "the outermost corner is at {outermost}, not {expected}");

        // Past both bands the tile is at full strength.
        assert!((dest.get_pixel(90, 90).0[0].to_unit() - 1.0).abs() < 0.01);
    }

    /// A band wider than half the tile is clamped, which is what keeps each ramp monotonic.
    fn a_band_wider_than_half_the_tile_is_clamped<T: Channel>()
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let mut dest = flat::<T>(100, 100, 0.0);
        let tile = flat::<T>(40, 40, 1.0);

        blend_tile(&mut dest, &tile, 0, 0, 400, 400);
        let levels = row(&dest, 20);

        for pair in levels[..40].windows(2) {
            assert!(pair[1] >= pair[0], "an over-wide band stopped being monotonic at {pair:?}");
        }

        // Clamped to half the tile, so the far half is at full strength rather than still ramping.
        assert!(
            (levels[39] - 1.0).abs() < 0.01,
            "the clamped band did not reach full strength by the tile's end"
        );
    }

    #[test]
    fn every_blend_scenario_holds_at_eight_bits() {
        a_wide_seam_leaves_no_step::<u8>();
        an_edge_with_no_neighbour_is_written_unchanged::<u8>();
        a_corner_is_weighted_by_both_axes::<u8>();
        a_band_wider_than_half_the_tile_is_clamped::<u8>();
    }

    #[test]
    fn every_blend_scenario_holds_at_sixteen_bits() {
        a_wide_seam_leaves_no_step::<u16>();
        an_edge_with_no_neighbour_is_written_unchanged::<u16>();
        a_corner_is_weighted_by_both_axes::<u16>();
        a_band_wider_than_half_the_tile_is_clamped::<u16>();
    }

    #[test]
    fn a_weight_just_short_of_one_is_blended_at_sixteen_bits_and_copied_at_eight() {
        // The last column of a 40-pixel band weighs about 0.9996: close enough to 1 that eight bits cannot tell, and
        // 25 levels short of it at sixteen — which a fixed 0.999 cutoff used to copy straight across.
        let weight = ramp_weight(39, 40);
        assert!(weight > 0.999 && weight < opaque::<u16>(), "the fixture no longer sits between the two cutoffs");

        let mut deep = flat::<u16>(100, 1, 0.0);
        blend_tile(&mut deep, &flat::<u16>(100, 1, 1.0), 0, 0, 40, 0);
        assert_eq!(deep.get_pixel(39, 0).0[0], u16::from_unit(weight), "sixteen bits copied a blended column");

        let mut shallow = flat::<u8>(100, 1, 0.0);
        blend_tile(&mut shallow, &flat::<u8>(100, 1, 1.0), 0, 0, 40, 0);
        assert_eq!(shallow.get_pixel(39, 0).0[0], u8::MAX, "eight bits changed a column it cannot resolve");
    }

    #[test]
    fn a_tile_that_runs_past_the_destination_is_written_only_as_far_as_it_goes() {
        // Which is what crops the mirrored padding off the one tile that can carry any.
        let mut dest = flat::<u8>(10, 10, 0.0);
        let tile = flat::<u8>(16, 16, 1.0);

        blend_tile(&mut dest, &tile, 4, 4, 0, 0);

        assert_eq!(dest.dimensions(), (10, 10));
        assert_eq!(*dest.get_pixel(9, 9), Rgb([255, 255, 255]));
        assert_eq!(*dest.get_pixel(3, 3), Rgb([0, 0, 0]));
    }

    #[test]
    fn the_curve_rises_from_above_zero_to_one_across_the_band() {
        for width in [1, 2, 16, 120, 256] {
            let weights: Vec<f32> = (0..width).map(|offset| ramp_weight(offset, width)).collect();

            assert!(weights[0] > 0.0, "the first weight of a {width}-pixel band throws the outermost column away");

            for pair in weights.windows(2) {
                assert!(pair[1] > pair[0], "a {width}-pixel band is not monotonic at {pair:?}");
            }

            let last = weights[weights.len() - 1];
            assert!(last < 1.0, "a {width}-pixel band reaches full strength inside the band");
            assert!(last > 1.0 - 2.0 / width as f32, "a {width}-pixel band ends at {last}, short of full strength");
        }
    }

    #[test]
    fn past_the_band_the_incoming_tile_is_at_full_strength() {
        assert_eq!(ramp_weight(16, 16), 1.0);
        assert_eq!(ramp_weight(17, 16), 1.0);
        assert_eq!(ramp_weight(u32::MAX, 16), 1.0);
    }

    #[test]
    fn a_band_of_no_width_is_full_strength_everywhere() {
        // Which is what "this edge has no already-written neighbour" is spelt as.
        for offset in [0, 1, 255] {
            assert_eq!(ramp_weight(offset, 0), 1.0, "a zero-width band weighted the tile at {offset}");
        }
    }

    #[test]
    fn the_curve_is_symmetric_about_the_middle_of_the_band() {
        // The property that makes it a *raised cosine* rather than any other rising curve: the weight it gives the
        // incoming tile and the weight the same offset from the far end gives what is underneath sum to 1.
        for offset in 0..120 {
            let rising = ramp_weight(offset, 120);
            let falling = ramp_weight(119 - offset, 120);

            assert!((rising + falling - 1.0).abs() < 1e-6, "the band is not symmetric at {offset}");
        }
    }
}
