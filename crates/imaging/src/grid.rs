//! Where the tiles are.

// The partitioning rule lives here and nowhere else, because it is shared by more than the driver beside it: the
// diffusion upscaler needs exactly this partitioning and a completely different per-tile pipeline, so the grid has to
// be usable without `run_tiled`. A second copy of the rule is a second copy of the two non-obvious decisions on
// `TileGrid`, and the moment they disagree the seam between two tiles moves.

/// The rectangle one tile covers, in source pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    /// The left edge, in source pixels.
    pub x: u32,
    /// The top edge, in source pixels.
    pub y: u32,
    /// How wide the tile is. Equal to [`TileGrid::size`] except on an axis shorter than one tile.
    pub width: u32,
    /// How tall the tile is. Equal to [`TileGrid::size`] except on an axis shorter than one tile.
    pub height: u32,
}

/// The tiles covering a `width` x `height` image at a given geometry, and the overlaps they actually produce.
///
/// Two rules in it are not obvious and both are load-bearing:
///
/// - **A tile that would overhang the edge is moved back, not shrunk.**
/// - **The moved tile is dropped when it lands exactly on its predecessor.** At a 512 tile with 128 overlap over a
///   1280-pixel side that is three tiles rather than four.
///
/// The consequence of the first rule is what [`overlaps_x`](Layout::overlaps_x) exists for; see its documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileGrid {
    /// The side of the square shape the model accepts.
    pub size: u32,
    /// How far each tile is asked to overlap the one before it. What it *actually* overlaps is
    /// [`overlaps_x`](Layout::overlaps_x).
    pub overlap: u32,
    /// The width of the image being covered.
    pub width: u32,
    /// The height of the image being covered.
    pub height: u32,
}

impl TileGrid {
    /// The partitioning this geometry produces over this image, computed once.
    ///
    /// An invalid *geometry* — a zero size, an overlap that leaves no forward progress, a tile at least as big as what
    /// it covers — yields a single tile covering the whole image, so no geometry can produce an empty partitioning. A
    /// zero *dimension* is different: no rectangle covers a zero-width image, so the layout is empty. A caller must
    /// treat that as a failure rather than iterating zero tiles, or it hands back an untouched buffer as though the
    /// work had succeeded — which is exactly what [`run_tiled`](crate::run_tiled) refuses to do.
    pub fn layout(&self) -> Layout {
        // The one way in. Every question a caller asks about a grid — where the tiles are, how many are in a row, how
        // far each actually overlaps its neighbour — is answered from the same two offset lists, so they are built
        // here and read off the `Layout` rather than re-derived per question: deriving each one separately is how two
        // of them end up disagreeing about where a seam is.
        let xs = self.offsets(self.width);
        let ys = self.offsets(self.height);

        Layout { extent_x: self.extent(self.width), extent_y: self.extent(self.height), xs, ys }
    }

    /// The tile start positions along one axis.
    fn offsets(&self, length: u32) -> Vec<u32> {
        // The last tile is moved back to end flush with the image rather than being shrunk: a fixed-shape graph
        // accepts one shape and rejects anything else, so a shrunken final tile could not be run at all. The move can
        // put it on top of the previous one; when it lands on exactly the same position it is dropped, because
        // running the model twice over identical pixels costs a full run and produces what is already there — for a
        // diffusion model, tens of seconds.
        if length == 0 {
            return Vec::new();
        }

        if !self.partitions(length) {
            return vec![0];
        }

        let stride = self.size - self.overlap;
        let last = length - self.size;

        let mut out = Vec::with_capacity(length.div_ceil(stride) as usize);
        let mut p = 0;

        while p < length {
            let start = p.min(last);

            if out.last() == Some(&start) {
                break;
            }

            out.push(start);

            if start + self.size >= length {
                break;
            }

            p += stride;
        }

        out
    }

    /// The tile size along one axis: the configured size, or the whole length where the geometry does not partition
    /// it.
    fn extent(&self, length: u32) -> u32 {
        if self.partitions(length) { self.size } else { length }
    }

    /// Whether the configured geometry actually partitions an axis of this length: a positive tile, an overlap that
    /// leaves forward progress, and a tile smaller than what it is covering.
    fn partitions(&self, length: u32) -> bool {
        // One predicate rather than three copies, so `extent` and `offsets` cannot drift into disagreeing about what a
        // degenerate geometry means.
        self.size > 0 && self.overlap < self.size && self.size < length
    }
}

/// Where a [`TileGrid`] actually put its tiles.
///
/// Built once by [`TileGrid::layout`] and then only read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The tile start positions across the image.
    xs: Vec<u32>,
    /// The tile start positions down the image.
    ys: Vec<u32>,
    /// The tile width: the configured size, or the whole width where the geometry does not partition it.
    extent_x: u32,
    /// The tile height, as [`extent_x`](Self::extent_x) is for the other axis.
    extent_y: u32,
}

impl Layout {
    /// The tiles, in row-major order: every tile of the first row left to right, then the second row, and so on.
    ///
    /// Empty where either axis has no length; see [`TileGrid::layout`].
    pub fn tiles(&self) -> Vec<Tile> {
        if self.xs.is_empty() || self.ys.is_empty() {
            return Vec::new();
        }

        let mut tiles = Vec::with_capacity(self.xs.len() * self.ys.len());
        for y in &self.ys {
            for x in &self.xs {
                tiles.push(Tile { x: *x, y: *y, width: self.extent_x, height: self.extent_y });
            }
        }

        tiles
    }

    /// How many tiles [`tiles`](Self::tiles) puts in each row.
    ///
    /// The driver needs it to turn a tile's index into its column and row, and so into which overlap applies to it.
    pub fn columns(&self) -> usize {
        self.xs.len()
    }

    /// Per column, how many pixels that tile actually overlaps the one to its left. The first entry is 0 — nothing
    /// precedes it.
    ///
    /// **This is not [`TileGrid::overlap`]**, and assuming it is has a name: it is the hard edge down the last column
    /// of the picture. `TileGrid::offsets` moves the last tile back so it ends flush with the image rather than
    /// shrinking it, so the final column can overlap its predecessor by far more than was configured. At size 256 and
    /// overlap 16 over a 600-pixel side the offsets are `[0, 240, 344]` and the last tile overlaps by **152**. Ramping
    /// that seam over 16 pixels leaves the other 136 written at full strength, which is the edge.
    pub fn overlaps_x(&self) -> Vec<u32> {
        Self::overlaps(&self.xs, self.extent_x)
    }

    /// Per row, how many pixels that tile actually overlaps the one above it. The vertical counterpart of
    /// [`overlaps_x`](Self::overlaps_x), and it carries the same warning.
    pub fn overlaps_y(&self) -> Vec<u32> {
        Self::overlaps(&self.ys, self.extent_y)
    }

    fn overlaps(offsets: &[u32], extent: u32) -> Vec<u32> {
        if offsets.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(offsets.len());
        out.push(0);

        for pair in offsets.windows(2) {
            // `pair[0] + extent` is where the previous tile ends and `pair[1]` is where this one starts. Both are
            // within the axis, so the subtraction cannot go below zero.
            out.push(pair[0] + extent - pair[1]);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every geometry and dimension the sweep below covers. Small enough to be exhaustive and wide enough to include
    /// an axis shorter than a tile, an axis exactly one tile, and axes that land on and off a stride boundary.
    const SIZES: [u32; 4] = [4, 16, 64, 256];
    const OVERLAPS: [u32; 4] = [0, 1, 16, 128];
    const LENGTHS: [u32; 7] = [1, 3, 16, 17, 100, 600, 1280];

    #[test]
    fn every_pixel_is_covered_by_a_tile_that_stays_inside_the_image() {
        for size in SIZES {
            for overlap in OVERLAPS {
                for width in LENGTHS {
                    for height in LENGTHS {
                        let grid = TileGrid { size, overlap, width, height };
                        let tiles = grid.layout().tiles();

                        assert!(!tiles.is_empty(), "{grid:?} partitioned into nothing");

                        let mut covered = vec![false; (width * height) as usize];
                        let mut origins = Vec::new();

                        for tile in &tiles {
                            assert!(
                                tile.x + tile.width <= width && tile.y + tile.height <= height,
                                "{tile:?} escapes {grid:?}"
                            );

                            origins.push((tile.x, tile.y));

                            for y in tile.y..tile.y + tile.height {
                                for x in tile.x..tile.x + tile.width {
                                    covered[(y * width + x) as usize] = true;
                                }
                            }
                        }

                        assert!(covered.iter().all(|seen| *seen), "{grid:?} left a pixel uncovered");

                        let mut distinct = origins.clone();
                        distinct.sort_unstable();
                        distinct.dedup();
                        assert_eq!(distinct.len(), origins.len(), "{grid:?} emitted a position twice");
                    }
                }
            }
        }
    }

    #[test]
    fn an_image_smaller_than_one_tile_is_covered_by_exactly_one() {
        let grid = TileGrid { size: 256, overlap: 16, width: 100, height: 40 };

        assert_eq!(grid.layout().tiles(), vec![Tile { x: 0, y: 0, width: 100, height: 40 }]);
        assert_eq!(grid.layout().columns(), 1);
    }

    #[test]
    fn a_moved_tile_that_lands_on_its_predecessor_is_dropped() {
        // 1280 at 512/128 is a stride of 384: 0, 384, 768, and the fourth would be moved back to 768 as well. Three
        // tiles rather than four, with nothing left uncovered — which the coverage assertion below is what proves.
        let grid = TileGrid { size: 512, overlap: 128, width: 1280, height: 512 };

        assert_eq!(grid.layout().columns(), 3);
        assert_eq!(grid.offsets(1280), vec![0, 384, 768]);

        let last = grid.layout().tiles().last().copied().unwrap();
        assert_eq!(last.x + last.width, 1280, "the last tile does not end flush with the edge");
    }

    #[test]
    fn the_last_tile_reports_the_overlap_it_actually_has_rather_than_the_configured_one() {
        // The worked case from `overlaps_x`: size 256, overlap 16, a 600-pixel side. The offsets are [0, 240, 344],
        // so the last tile was moved back by 104 and overlaps its predecessor by 152 — not by 16. A blend given 16
        // here writes the other 136 columns at full strength, which is the hard edge this figure exists to prevent.
        let grid = TileGrid { size: 256, overlap: 16, width: 600, height: 600 };

        assert_eq!(grid.offsets(600), vec![0, 240, 344]);
        assert_eq!(grid.layout().overlaps_x(), vec![0, 16, 152]);
        assert_eq!(grid.layout().overlaps_y(), vec![0, 16, 152]);
    }

    #[test]
    fn every_reported_overlap_is_the_one_the_offsets_produce() {
        for grid in [
            TileGrid { size: 256, overlap: 16, width: 600, height: 1000 },
            TileGrid { size: 512, overlap: 128, width: 1280, height: 1281 },
            TileGrid { size: 64, overlap: 0, width: 200, height: 65 },
            TileGrid { size: 256, overlap: 16, width: 100, height: 40 },
        ] {
            for (length, reported) in
                [(grid.width, grid.layout().overlaps_x()), (grid.height, grid.layout().overlaps_y())]
            {
                let offsets = grid.offsets(length);
                let extent = grid.extent(length);

                assert_eq!(reported.len(), offsets.len(), "{grid:?} reported a different number of overlaps");
                assert_eq!(reported[0], 0, "{grid:?} gave the first tile a predecessor");

                for i in 1..offsets.len() {
                    let shared = offsets[i - 1] + extent - offsets[i];
                    assert_eq!(reported[i], shared, "{grid:?} misreported the overlap at {i}");
                }
            }
        }
    }

    #[test]
    fn an_axis_with_no_length_yields_no_tiles() {
        for grid in [
            TileGrid { size: 256, overlap: 16, width: 0, height: 100 },
            TileGrid { size: 256, overlap: 16, width: 100, height: 0 },
            TileGrid { size: 256, overlap: 16, width: 0, height: 0 },
        ] {
            assert!(grid.layout().tiles().is_empty(), "{grid:?} partitioned an image with no area");
        }
    }

    #[test]
    fn an_invalid_geometry_still_yields_one_tile_covering_the_whole_axis() {
        for grid in [
            TileGrid { size: 0, overlap: 0, width: 100, height: 100 },
            TileGrid { size: 16, overlap: 16, width: 100, height: 100 },
            TileGrid { size: 16, overlap: 32, width: 100, height: 100 },
            TileGrid { size: 256, overlap: 16, width: 256, height: 256 },
        ] {
            let whole = Tile { x: 0, y: 0, width: grid.width, height: grid.height };
            assert_eq!(grid.layout().tiles(), vec![whole], "{grid:?} did not fall back to one whole-axis tile");
        }
    }
}
