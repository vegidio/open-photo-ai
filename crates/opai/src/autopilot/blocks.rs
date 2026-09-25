//! The second pass over a photograph's pixels, which the noise and sharpness signals both read: square blocks at the
//! photograph's own resolution, on a regular grid spread over the frame.
//!
//! Every sample is read through [`Sampler`], so a 16-bit source is measured at 16 bits, and is kept as gamma-encoded
//! Rec. 709 luma and the two chroma differences `B - Y` and `R - Y`, each an `f32` on the encoded scale.

// Blocks rather than the strided grid the light and colour signals read. Grain and blur exist only between neighbouring
// pixels: a stride skips the neighbours, and a resize averages grain away and makes blur look sharp. Blocks read every
// neighbour inside them, and a budget of blocks bounds the cost as the stride does.
//
// A regular grid rather than random blocks, so that the same photograph always gets the same answer, which the run
// cache and the tests both need.
//
// No cancellation inside the loop, for the reason the strided pass gives: at most a million samples, and the caller
// checks before and after.

#[cfg(test)]
use image::DynamicImage;
use imaging::tensor::Sampler;

use super::pass::{OPAQUE_ENOUGH, clipped};

/// The side of a block, in pixels.
pub(super) const BLOCK: u32 = 64;

/// The most blocks one pass reads: with [`BLOCK`], the million samples the strided pass reads.
pub(super) const MAX_BLOCKS: u32 = 256;

/// The spacing block origins are rounded to, so that a JPEG's 8x8 compression blocks sit at the same place in every
/// block.
pub(super) const ALIGN: u32 = 8;

/// One block's samples, row by row.
pub(super) struct Block {
    /// Encoded Rec. 709 luma, on `[0, 1]`.
    pub(super) luma: Vec<f32>,
    /// `B - Y` and `R - Y`, each on `[-1, 1]`.
    pub(super) chroma: [Vec<f32>; 2],
    /// Whether any channel of the sample is at either end of its range.
    pub(super) clipped: Vec<bool>,
}

/// Everything the block pass gathered, for the two signals to read.
pub(super) struct Blocks {
    /// The side of every block, in pixels.
    pub(super) size: u32,
    /// The blocks read, in raster order of their origins. Blocks touching transparency are not here.
    pub(super) blocks: Vec<Block>,
}

/// The origins of the blocks along one axis of `extent` pixels: every whole block where `count` of them tile it, and
/// otherwise `count` spread evenly from one edge to the other, each at a multiple of [`ALIGN`].
fn origins(extent: u32, size: u32, count: u32) -> Vec<u32> {
    if count == extent / size {
        return (0..count).map(|i| i * size).collect();
    }
    if count == 1 {
        // Centred, so a frame one block wide is not read at its edge.
        return vec![(extent - size) / 2 / ALIGN * ALIGN];
    }

    // In steps of `ALIGN`. Since `count` whole blocks fit, the step between two origins is at least a block, and the
    // floor of each keeps it so: no two blocks overlap and the last one ends inside the frame.
    let slots = u64::from((extent - size) / ALIGN);
    let gaps = u64::from(count - 1);

    (0..u64::from(count)).map(|i| (i * slots / gaps) as u32 * ALIGN).collect()
}

/// How many blocks of `size` a frame of `width` by `height` is read in, across and down.
///
/// Every whole block where they number at most `budget`. Otherwise the grid holding the most blocks within the budget,
/// and of those the one whose cells are closest to square, so that blocks are spread as evenly one way as the other.
pub(super) fn grid(width: u32, height: u32, size: u32, budget: u32) -> (u32, u32) {
    let (across, down) = (width / size, height / size);

    if u64::from(across) * u64::from(down) <= u64::from(budget) {
        return (across, down);
    }

    let squareness = |columns: u32, rows: u32| {
        let (cell_width, cell_height) = (f64::from(width) / f64::from(columns), f64::from(height) / f64::from(rows));
        (cell_width / cell_height).ln().abs()
    };

    (1..=across.min(budget))
        .map(|columns| (columns, down.min(budget / columns)))
        .max_by(|a, b| {
            (a.0 * a.1)
                .cmp(&(b.0 * b.1))
                .then_with(|| squareness(b.0, b.1).total_cmp(&squareness(a.0, a.1)))
        })
        .unwrap_or((0, 0))
}

/// [`read_from`] over the whole of `source`, for a test that has no sampler to share.
#[cfg(test)]
pub(super) fn read(source: &DynamicImage) -> Blocks {
    read_from(&Sampler::new(source), source.width(), source.height())
}

/// Reads the blocks out of a `width` by `height` photograph through `sampler`, which the strided pass shares: at most
/// [`MAX_BLOCKS`] of them, skipping any block with a sample that is mostly transparent.
pub(super) fn read_from(sampler: &Sampler<'_>, width: u32, height: u32) -> Blocks {
    // 64 rather than 32, which was measured at the same number of samples and detected no more of either fault.
    let size = BLOCK;
    let (across, down) = grid(width, height, size, MAX_BLOCKS);

    let columns = origins(width, size, across);
    let rows = origins(height, size, down);

    let blocks = rows
        .iter()
        .flat_map(|y| columns.iter().map(move |x| (*x, *y)))
        .filter_map(|(x, y)| block(sampler, x, y, size))
        .collect();

    Blocks { size, blocks }
}

/// The block of `size` whose top-left corner is at `(x0, y0)`, or `None` where any of its samples is mostly
/// transparent.
fn block(sampler: &Sampler<'_>, x0: u32, y0: u32, size: u32) -> Option<Block> {
    // Skipped whole rather than sample by sample: a transparent edge inside a block reads as a hard edge to the
    // sharpness signal and as a smooth area to the noise signal, and neither is the photograph.
    let area = (size * size) as usize;
    let mut block = Block {
        luma: Vec::with_capacity(area),
        chroma: [Vec::with_capacity(area), Vec::with_capacity(area)],
        clipped: Vec::with_capacity(area),
    };

    for y in y0..y0 + size {
        for x in x0..x0 + size {
            let (rgb, alpha) = sampler.rgb_and_alpha(x, y);

            if alpha < OPAQUE_ENOUGH {
                return None;
            }

            let [r, g, b] = rgb.map(|sample| f32::from(sample) / f32::from(u16::MAX));
            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;

            block.luma.push(luma);
            block.chroma[0].push(b - luma);
            block.chroma[1].push(r - luma);
            // Both ends, because either one flattens the noise that would otherwise be there: a blown sky reads as
            // clean, and so does a crushed shadow.
            block.clipped.push(clipped(rgb));
        }
    }

    Some(block)
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{ImageBuffer, Luma, Rgb, Rgba};

    fn grey(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_pixel(width, height, Rgb([90, 90, 90])))
    }

    #[test]
    fn a_photograph_within_the_budget_is_tiled_with_every_whole_block() {
        // 16 by 16 blocks exactly, then a frame with a partial block at each edge.
        for (width, height, want) in [(1024, 1024, 256), (1000, 700, 15 * 10), (64, 64, 1)] {
            let blocks = read(&grey(width, height));

            assert_eq!(blocks.blocks.len(), want, "{width}x{height}");
        }

        assert_eq!(origins(1000, BLOCK, 15), (0..15).map(|i| i * 64).collect::<Vec<_>>());
    }

    #[test]
    fn a_larger_photograph_is_read_in_at_most_a_million_samples() {
        for (width, height) in [(1100, 1100), (6000, 4000), (30_000, 20_000), (100_000, 64), (64, 100_000)] {
            let (across, down) = grid(width, height, BLOCK, MAX_BLOCKS);
            let samples = u64::from(across * down) * u64::from(BLOCK * BLOCK);

            assert!(samples <= 1_048_576, "{width}x{height} is {samples} samples");
            // And the budget is used: at least nine tenths of it, or every block the frame has.
            assert!(
                across * down >= (MAX_BLOCKS * 9 / 10).min((width / BLOCK) * (height / BLOCK)),
                "{width}x{height}"
            );

            let columns = origins(width, BLOCK, across);
            let rows = origins(height, BLOCK, down);
            for axis in [&columns, &rows] {
                assert!(axis.iter().all(|origin| origin % ALIGN == 0), "an origin off the 8-pixel grid: {axis:?}");
                assert!(axis.windows(2).all(|pair| pair[1] - pair[0] >= BLOCK), "blocks overlap: {axis:?}");
            }
            assert!(
                columns.last().is_some_and(|x| x + BLOCK <= width) && rows.last().is_some_and(|y| y + BLOCK <= height)
            );
        }

        let blocks = read(&grey(6000, 4000));
        assert!(blocks.blocks.len() as u64 * u64::from(BLOCK * BLOCK) <= 1_048_576);
    }

    #[test]
    fn the_blocks_of_a_large_photograph_are_spread_over_the_whole_frame() {
        let (across, down) = grid(6000, 4000, BLOCK, MAX_BLOCKS);
        let columns = origins(6000, BLOCK, across);
        let rows = origins(4000, BLOCK, down);

        assert_eq!((columns[0], rows[0]), (0, 0));
        assert!(6000 - (columns[columns.len() - 1] + BLOCK) < ALIGN, "{columns:?}");
        assert!(4000 - (rows[rows.len() - 1] + BLOCK) < ALIGN, "{rows:?}");
    }

    #[test]
    fn a_photograph_with_no_whole_block_yields_no_blocks() {
        for (width, height) in [(63, 1000), (1000, 63), (10, 10), (1, 1)] {
            assert!(read(&grey(width, height)).blocks.is_empty(), "{width}x{height}");
        }
    }

    #[test]
    fn a_block_touching_transparency_is_skipped() {
        // Two blocks across; one mostly transparent pixel in the second.
        let mut buffer = ImageBuffer::from_pixel(128, 64, Rgba([90, 90, 90, 255]));
        buffer.put_pixel(100, 30, Rgba([90, 90, 90, 127]));

        let blocks = read(&DynamicImage::ImageRgba8(buffer));

        assert_eq!(blocks.blocks.len(), 1);
    }

    #[test]
    fn a_sixteen_bit_gradient_is_measured_at_more_than_eight_bits() {
        let gradient = ImageBuffer::from_fn(4096, 64, |x, _| Luma([(x * 16) as u16]));

        let blocks = read(&DynamicImage::ImageLuma16(gradient));
        let mut levels: Vec<u32> =
            blocks.blocks.iter().flat_map(|block| block.luma.iter().map(|luma| luma.to_bits())).collect();
        levels.sort_unstable();
        levels.dedup();

        assert!(levels.len() > 256, "a 16-bit gradient landed in only {} levels", levels.len());
    }

    #[test]
    fn a_sample_is_kept_as_luma_and_two_chroma_differences() {
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(64, 64, Rgb([200, 140, 80])));

        let blocks = read(&source);
        let block = &blocks.blocks[0];
        let [r, g, b] = [200.0_f32, 140.0, 80.0].map(|v| v / 255.0);
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;

        assert!((block.luma[0] - luma).abs() < 1e-6);
        assert!((block.chroma[0][0] - (b - luma)).abs() < 1e-6);
        assert!((block.chroma[1][0] - (r - luma)).abs() < 1e-6);
        assert!(!block.clipped[0]);
    }

    #[test]
    fn a_sample_at_either_end_of_its_range_is_clipped() {
        let mut buffer = ImageBuffer::from_pixel(64, 64, Rgb([120, 120, 120]));
        buffer.put_pixel(1, 0, Rgb([255, 200, 200]));
        buffer.put_pixel(2, 0, Rgb([30, 0, 30]));

        let blocks = read(&DynamicImage::ImageRgb8(buffer));
        let clipped = &blocks.blocks[0].clipped;

        assert_eq!(clipped[..3], [false, true, true]);
    }
}
