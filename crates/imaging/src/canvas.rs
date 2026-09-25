//! The second way of combining tiles, for a model drawing its own noise per region: weight every contribution, sum
//! them, and divide once.

// Not `blend::blend_tile`, which blends each tile against **whatever is already in the buffer** and so can average two
// tiles and no more. Where four tiles meet, its result depends on the order they were written. For a convolutional
// model that corner is invisible, because two tiles covering one pixel very nearly agree; for a model drawing its own
// noise per region they genuinely disagree, and that corner is exactly where a seam shows. Here every contribution is
// weighted into a sum and a weight plane and the division happens once, so a pixel's value is order-independent by
// construction.
//
// They also differ in what they hold. `blend_tile` writes into an `ImageBuffer` at the caller's channel depth, so each
// tile is quantised on its way in; this holds planar float as the decoder produces it and the depth is applied once,
// to the finished image. Quantising a stochastic tile before averaging it throws away precision exactly where the
// averaging needs it.

use crate::blend::ramp_weight;
use crate::grid::{Tile, TileGrid};

// Feathering the real width is what makes each pair of abutting ramps a partition of unity **before** the division
// rather than leaving the division to rescue it, and it is the answer the convolutional path already gives to the
// same question. A border edge is not ramped because that would drive the accumulated weight towards zero along the
// outside of the picture and amplify whatever survived the division.
//
// A named struct rather than four `u32`s in a row, which is an argument order a caller can get wrong in a way that
// compiles, runs, and puts the seam on the wrong side of the picture.
/// How many pixels a region actually shares with the region on each side of it.
///
/// **Not the configured overlap** — see [`Layout::overlaps_x`](crate::grid::Layout::overlaps_x).
///
/// A zero means *there is no region on that side*: the edge lies on the border of the image, and it is written at
/// full weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shared {
    /// With the region to the left.
    pub left: u32,
    /// With the region to the right.
    pub right: u32,
    /// With the region above.
    pub top: u32,
    /// With the region below.
    pub bottom: u32,
}

/// Every region of `grid`, paired with what it shares with each of its neighbours.
pub fn placements(grid: TileGrid) -> Vec<(Tile, Shared)> {
    // The one spelling of the tile-index-to-overlap mapping for a `Canvas`, shared rather than copied: a pipeline
    // places regions the same way `Canvas` expects them to be placed, and two spellings of that are free to drift
    // apart into a disagreement no test can see — an off-by-one on `column + 1` or a swapped pair of axes is invisible
    // to every reconstruction test. `run_tiled`, which blends through `blend_tile` rather than a `Canvas`, indexes the
    // same `Layout` itself.
    let layout = grid.layout();
    let (tiles, columns) = (layout.tiles(), layout.columns());
    let (overlaps_x, overlaps_y) = (layout.overlaps_x(), layout.overlaps_y());

    tiles
        .iter()
        .enumerate()
        .map(|(index, tile)| {
            let (column, row) = (index % columns, index / columns);

            (
                *tile,
                Shared {
                    left: overlaps_x[column],
                    // What the *next* region shares with this one, which is this edge's own width — and zero past the
                    // last column, which is how "this edge is on the border of the image" is spelled.
                    right: overlaps_x.get(column + 1).copied().unwrap_or(0),
                    top: overlaps_y[row],
                    bottom: overlaps_y.get(row + 1).copied().unwrap_or(0),
                },
            )
        })
        .collect()
}

/// A planar float sum over the whole image, plus the weight plane that turns it into an average.
pub struct Canvas {
    /// The width of the extent being covered.
    width: u32,
    /// The height of the extent being covered.
    height: u32,
    /// Three planes of `width * height`: every contribution, each already multiplied by its weight.
    sum: Vec<f32>,
    // One weight plane for all three channels rather than three: the weight a region carries at a pixel is geometry,
    // and geometry does not differ per channel. At a 4x output over a 12-megapixel photograph that is 770 MB not held.
    /// One plane of `width * height`: the weights those contributions were multiplied by.
    weight: Vec<f32>,
}

impl Canvas {
    /// An empty accumulator over a `width` x `height` extent.
    pub fn new(width: u32, height: u32) -> Self {
        let plane = (width as usize) * (height as usize);

        Self { width, height, sum: vec![0.0; 3 * plane], weight: vec![0.0; plane] }
    }

    /// Accumulates one region's planar output at `at`, ramped over the widths it shares with its neighbours.
    ///
    /// `region` is three planes of `at.width` x `at.height`, in the same planar CHW order every conversion in this
    /// crate uses. A region that runs past the canvas is accumulated only as far as the canvas goes.
    ///
    /// # Panics
    ///
    /// Panics when `region` is not three planes of `at`'s dimensions.
    pub fn add(&mut self, region: &[f32], at: Tile, shared: Shared) {
        let (tile_width, tile_height) = (at.width as usize, at.height as usize);
        let tile_plane = tile_width * tile_height;

        assert_eq!(region.len(), 3 * tile_plane, "the region is not three planes of {}x{}", at.width, at.height);

        if tile_plane == 0 {
            return;
        }

        // Tabulated once per region rather than evaluated per pixel, for the reason `blend_tile` gives.
        let across = axis_weights(at.width, shared.left, shared.right);
        let down = axis_weights(at.height, shared.top, shared.bottom);

        let columns = self.width.saturating_sub(at.x).min(at.width) as usize;
        let rows = self.height.saturating_sub(at.y).min(at.height) as usize;
        let canvas_plane = (self.width as usize) * (self.height as usize);
        let canvas_width = self.width as usize;

        for (row, vertical) in down.iter().enumerate().take(rows) {
            let source_row = row * tile_width;
            let dest_row = ((at.y as usize) + row) * canvas_width + (at.x as usize);

            for (column, horizontal) in across.iter().enumerate().take(columns) {
                let weight = vertical * horizontal;
                let (source, dest) = (source_row + column, dest_row + column);

                self.sum[dest] += region[source] * weight;
                self.sum[canvas_plane + dest] += region[tile_plane + source] * weight;
                self.sum[2 * canvas_plane + dest] += region[2 * tile_plane + source] * weight;
                self.weight[dest] += weight;
            }
        }
    }

    /// Divides the accumulated sums by their weights and hands back the planar result.
    ///
    /// **In place**: the accumulator *is* the result, and the weight plane is dropped with the rest of `self`.
    ///
    /// A pixel no region covered is left at its accumulated zero rather than divided by it.
    pub fn resolve(mut self) -> Vec<f32> {
        // In place because a second buffer would double the peak float held — around 2.3 GB on a 4x pass over a
        // 12-megapixel photograph — on a machine already holding a 7 GB model. Dropping the weight plane with `self`
        // is what keeps the pipeline's peak at three planes rather than four.
        let plane = (self.width as usize) * (self.height as usize);

        for index in 0..plane {
            let weight = self.weight[index];

            // Unreachable while the grid covers every pixel, which it does. Left at zero because a zero keeps a hole
            // visible, where an infinity is one the channel conversion would clamp into ordinary-looking white.
            if weight <= 0.0 {
                continue;
            }

            // One reciprocal and three multiplies rather than three divisions. `1.0 / weight` is not exactly
            // representable, so LLVM will not hoist it without fast-math, and a divide is several times the latency
            // of a multiply — over the padded extent of a 4x pass that is hundreds of millions of them.
            let inv = 1.0 / weight;
            self.sum[index] *= inv;
            self.sum[plane + index] *= inv;
            self.sum[2 * plane + index] *= inv;
        }

        self.sum
    }
}

/// The weight every offset along one axis of a region carries: a raised cosine rising over the width it shares with
/// the region before it, and falling over the width it shares with the one after.
///
/// **Not clamped to half the extent, unlike [`blend_tile`](crate::blend::blend_tile)'s.** Where the two
/// ramps of one region do meet — a middle region whose successor was moved a long way back — they multiply.
pub fn axis_weights(length: u32, before: u32, after: u32) -> Vec<f32> {
    // Unclamped because every weight here is normalised away at the end, so clamping would buy nothing and cost the
    // partition of unity: two abutting ramps of the shared width sum to exactly one, and narrowing one of them is what
    // would make the division load-bearing again. Where two ramps meet, their product stays positive and smooth and
    // is normalised like anything else.
    (0..length)
        .map(|offset| {
            // `ramp_weight` answers 1 for a zero width, which is how "this edge is on the border of the image" is
            // spelled — so a border edge needs no branch of its own here.
            ramp_weight(offset, before) * ramp_weight(length - 1 - offset, after)
        })
        .collect()
}

/// Fixtures the tests of this module and of the pipelines that drive a [`Canvas`] both build on.
#[cfg(any(test, feature = "test-support"))]
pub mod fixtures {
    // The placement itself is not here: it is `placements`, which the driver runs too, so the tests exercise the
    // expression that ships rather than a second copy of it.

    /// A region of `width` x `height` whose every value in every channel is `level`.
    pub fn flat(width: u32, height: u32, level: f32) -> Vec<f32> {
        vec![level; 3 * (width as usize) * (height as usize)]
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::flat;
    use super::*;
    use crate::grid::TileGrid;

    #[test]
    fn a_region_shares_the_width_the_grid_actually_left_with_each_neighbour_it_has() {
        // The one property no reconstruction test can catch. `resolve` divides by the accumulated weight plane, so a
        // pair of abutting ramps sums to unity whatever width they are given — which means an exact round trip stays
        // exact with the axes swapped, the columns off by one, or the border ramped. Every other test here is blind
        // to this mapping by construction, so it is pinned directly against the geometry instead.
        //
        // The numbers are the worked example in `Layout::overlaps_x`: at size 256 and overlap 16 over a 600-pixel
        // side the offsets are [0, 240, 344], so the last column overlaps its predecessor by 152, not by 16.
        let placed = placements(TileGrid { size: 256, overlap: 16, width: 600, height: 300 });

        let shared: Vec<Shared> = placed.iter().map(|(_, shared)| *shared).collect();
        assert_eq!(shared.len(), 6, "three columns over 600 and two rows over 300");

        // Top-left: both borders of the image, both interior edges shared with the neighbour that follows.
        assert_eq!(shared[0], Shared { left: 0, right: 16, top: 0, bottom: 212 });
        // Top-right: `right` is 0 because nothing follows it, and `left` is the 152 the grid really left.
        assert_eq!(shared[2], Shared { left: 152, right: 0, top: 0, bottom: 212 });
        // Bottom-right: the far corner, where both trailing edges are borders.
        assert_eq!(shared[5], Shared { left: 152, right: 0, top: 212, bottom: 0 });
    }

    /// The red plane of a resolved canvas, indexed by `(x, y)`.
    fn red(resolved: &[f32], width: u32, x: u32, y: u32) -> f32 {
        resolved[(y * width + x) as usize]
    }

    #[test]
    fn a_pixel_four_regions_cover_is_the_weighted_average_of_all_four_whatever_order_they_arrived_in() {
        // The corner `blend_tile` cannot serve: four regions that genuinely disagree. A small synthetic geometry
        // rather than any real model's, because what is being checked is the accumulator's arithmetic and regions
        // of several hundred pixels would only make it slower to read. A driver checks it at its own geometry,
        // beside the geometry.
        let grid = TileGrid { size: 12, overlap: 6, width: 18, height: 18 };
        let placed = placements(grid);
        assert_eq!(placed.len(), 4, "the geometry chosen does not put four regions over one corner");

        // Four levels far enough apart that an average of all four cannot be mistaken for an average of two.
        let levels = [0.1_f32, 0.4, 0.7, 1.0];
        let resolve = |order: &[usize]| {
            let mut canvas = Canvas::new(18, 18);
            for &which in order {
                let (tile, shared) = placed[which];
                canvas.add(&flat(tile.width, tile.height, levels[which]), tile, shared);
            }

            canvas.resolve()
        };

        let forward = resolve(&[0, 1, 2, 3]);
        let backward = resolve(&[3, 2, 1, 0]);
        let shuffled = resolve(&[2, 0, 3, 1]);

        // The centre pixel of the shared corner, which all four cover.
        let (x, y) = (9, 9);
        let value = red(&forward, 18, x, y);

        assert!(value > levels[0] + 0.05, "the corner is at {value}, at or below the lowest of the four");
        assert!(value < levels[3] - 0.05, "the corner is at {value}, at or above the highest of the four");

        // Every region's weight at that pixel, computed the same way `add` does, so the expected value is the
        // weighted average rather than the unweighted one — which they are not equal to here.
        let mut weighted = 0.0_f32;
        let mut total = 0.0_f32;
        for (which, (tile, shared)) in placed.iter().enumerate() {
            let across = axis_weights(tile.width, shared.left, shared.right);
            let down = axis_weights(tile.height, shared.top, shared.bottom);
            let weight = across[(x - tile.x) as usize] * down[(y - tile.y) as usize];

            weighted += levels[which] * weight;
            total += weight;
        }

        assert!(
            (value - weighted / total).abs() < 1e-5,
            "the corner is {value}, not the weighted average of four"
        );

        // And the property the accumulator exists for: the answer does not depend on the order they were produced in.
        //
        // To within the last bits of a float sum rather than bit for bit — adding the same four products in three
        // orders is three roundings, and float addition is not associative. What "order-independent" rules out is the
        // thing `blend_tile` actually does: a pixel taking the value of whichever tile was written last, which is a
        // difference of the levels themselves rather than of their sixth decimal. One 16-bit channel level is
        // 1.5e-5, so the bound below is well inside what any depth could represent.
        let largest =
            |left: &[f32], right: &[f32]| left.iter().zip(right).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);

        assert!(largest(&forward, &backward) < 1e-6, "reversing the order changed the picture");
        assert!(largest(&forward, &shuffled) < 1e-6, "shuffling the order changed the picture");
    }

    #[test]
    fn an_edge_on_the_border_of_the_image_is_not_ramped() {
        // One region covering the whole canvas: every edge is a border, so every pixel keeps its own value exactly.
        let mut canvas = Canvas::new(8, 8);
        let mut region = flat(8, 8, 0.0);
        for (index, value) in region.iter_mut().enumerate() {
            *value = index as f32 / 100.0;
        }

        canvas.add(&region, Tile { x: 0, y: 0, width: 8, height: 8 }, Shared::default());
        let resolved = canvas.resolve();

        assert_eq!(resolved, region, "an unramped region did not come back as itself");

        // And the outermost column of a region whose *other* edge is shared: the border edge is still untouched.
        let mut canvas = Canvas::new(20, 8);
        canvas.add(
            &flat(12, 8, 0.6),
            Tile { x: 0, y: 0, width: 12, height: 8 },
            Shared { right: 4, ..Shared::default() },
        );
        canvas.add(
            &flat(12, 8, 0.2),
            Tile { x: 8, y: 0, width: 12, height: 8 },
            Shared { left: 4, ..Shared::default() },
        );

        let resolved = canvas.resolve();
        assert!((red(&resolved, 20, 0, 0) - 0.6).abs() < 1e-6, "the left border was weighted against nothing");
        assert!((red(&resolved, 20, 19, 0) - 0.2).abs() < 1e-6, "the right border was weighted against nothing");
    }

    #[test]
    fn a_pixel_no_region_covered_is_left_at_zero_rather_than_divided_by_it() {
        // Unreachable while the grid covers every pixel, so checked directly rather than through a grid.
        let mut canvas = Canvas::new(10, 4);
        canvas.add(&flat(4, 4, 0.75), Tile { x: 0, y: 0, width: 4, height: 4 }, Shared::default());

        let resolved = canvas.resolve();

        assert!((red(&resolved, 10, 0, 0) - 0.75).abs() < 1e-6, "the covered pixel was not resolved");
        for x in 4..10 {
            let value = red(&resolved, 10, x, 0);
            assert_eq!(value, 0.0, "the uncovered pixel at {x} resolved to {value}");
            assert!(value.is_finite(), "the uncovered pixel at {x} was divided by a zero weight");
        }
    }
}
