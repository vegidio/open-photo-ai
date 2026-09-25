//! The prior boxes the detector predicts against: 16,800 of them for a 640 square, generated per run.

// RetinaFace does not predict absolute positions. It predicts an *offset* from a fixed reference box at every
// position, scale and shape it considers, and this is the grid of those references — three feature-pyramid levels at
// strides 8, 16 and 32, two anchor sizes at each, over every cell.

/// The feature-pyramid strides, in the order the levels are laid out.
const STRIDES: [u32; 3] = [8, 16, 32];

/// The two anchor sizes at each level, in pixels of the detector's square.
const MIN_SIZES: [[u32; 2]; 3] = [[16, 32], [64, 128], [256, 512]];

/// One prior box: where it is and how big it is, every value normalised to `0..=1` over the detector's square.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Prior {
    // Normalised rather than in pixels because the decode adds a variance-scaled offset to these and then scales the
    // whole result by the target size once — which is the reference's arithmetic, and keeping it means the two agree
    // value for value rather than approximately.
    /// The centre's distance from the left edge.
    pub(super) cx: f32,
    /// The centre's distance from the top edge.
    pub(super) cy: f32,
    /// The box's width.
    pub(super) sx: f32,
    /// The box's height.
    pub(super) sy: f32,
}

/// Every prior for a `size` square, in the order the model's outputs are indexed by: level-major, then row-major
/// within a level, then by size within a cell.
///
/// Built per run rather than memoised.
pub(super) fn generate(size: u32) -> Vec<Prior> {
    // **The order is the contract.** The model's `loc`, `conf` and `landmarks` outputs are indexed by an anchor's
    // *position in this list*, so a reordering here does not fail loudly — it shifts every detection by a fraction of
    // the image and reads as a model that got worse. It is the reference's own order and the order the graph was
    // exported against.
    //
    // The reference memoises them behind a `sync.Once`, but 16,800 priors is 268 KB and a few hundred microseconds
    // against a run costing about 12 milliseconds, so both are defensible and this is the one with no shared mutable
    // state to reason about. If a measurement ever says otherwise it is a `LazyLock` here and a change to nothing else.
    let image_size = size as f32;

    let total: usize = STRIDES
        .iter()
        .zip(MIN_SIZES)
        .map(|(stride, sizes)| {
            let side = side_of(size, *stride);
            side * side * sizes.len()
        })
        .sum();

    let mut priors = Vec::with_capacity(total);

    for (stride, sizes) in STRIDES.iter().zip(MIN_SIZES) {
        let side = side_of(size, *stride);
        let step = *stride as f32 / image_size;
        let normalised = sizes.map(|min| min as f32 / image_size);

        for row in 0..side {
            // The centre of the cell, not its corner, which is what the decode assumes when it adds an offset.
            let cy = (row as f32 + 0.5) * step;

            for column in 0..side {
                let cx = (column as f32 + 0.5) * step;

                for size in &normalised {
                    priors.push(Prior { cx, cy, sx: *size, sy: *size });
                }
            }
        }
    }

    priors
}

/// The feature map's side for `size` at `stride`, **rounded up**.
const fn side_of(size: u32, stride: u32) -> usize {
    // Rounding down would leave the last row and column of the image with no anchor at all, and a face there would
    // never be found. It divides exactly at 640, which is the only size this model runs; the rounding is here because
    // getting it wrong at any other size is silent.
    size.div_ceil(stride) as usize
}

#[cfg(test)]
mod tests {
    use super::super::TARGET_SIZE;
    use super::*;

    /// The tolerance the reference's own anchor test uses.
    fn close(got: f32, want: f32) -> bool {
        (f64::from(got) - f64::from(want)).abs() < 1e-6
    }

    #[test]
    fn the_grid_for_the_detectors_square_is_exactly_sixteen_thousand_eight_hundred_priors() {
        // Three levels at strides 8/16/32, two sizes each. For 640 the feature maps are 80, 40 and 20 squares, so the
        // total is 2*(80² + 40² + 20²) = 16800 — the figure the decode path is written against, and what sizes the
        // model's three output tensors.
        assert_eq!(generate(TARGET_SIZE).len(), 2 * (80 * 80 + 40 * 40 + 20 * 20));
        assert_eq!(generate(TARGET_SIZE).len(), 16_800);
    }

    #[test]
    fn a_size_that_is_not_a_multiple_of_the_stride_rounds_the_feature_map_up() {
        // 641 is not a size this model runs, which is why the property is checked rather than left to the one that
        // divides.
        let expected: usize = STRIDES
            .iter()
            .map(|stride| {
                let side = 641_u32.div_ceil(*stride) as usize;
                side * side * 2
            })
            .sum();

        assert_eq!(generate(641).len(), expected);
    }

    #[test]
    fn the_first_priors_are_the_stride_eight_cell_at_the_origin_at_its_two_sizes() {
        // Transcribed from the reference's `anchors_test.go`. Both facts are assumptions the decode makes when it
        // adds an offset to a prior's centre: the values are normalised to 0-1, and a centre sits at the middle of
        // its cell rather than at its corner.
        let priors = generate(TARGET_SIZE);

        let first = priors[0];
        assert!(
            close(first.cx, 0.5 * 8.0 / 640.0),
            "the first centre is not the middle of the first stride-8 cell"
        );
        assert!(close(first.cy, 0.5 * 8.0 / 640.0));
        assert!(close(first.sx, 16.0 / 640.0), "the first prior is not 16/640 square");
        assert!(close(first.sy, 16.0 / 640.0));

        // The second is the same cell at the level's other size, which is what makes the ordering size-innermost.
        let second = priors[1];
        assert!(close(second.sx, 32.0 / 640.0), "the second prior is not the same cell at 32/640");
        assert!(close(second.cx, first.cx), "the second prior moved to another cell");
        assert!(close(second.cy, first.cy));
    }

    #[test]
    fn the_last_prior_is_the_coarsest_levels_final_cell_at_its_largest_size() {
        // The other end of the traversal, which is what pins the ordering as level-major: the last anchor belongs to
        // the stride-32 level, at cell (19, 19) of a 20-square feature map, at 512/640.
        let priors = generate(TARGET_SIZE);
        let last = priors[priors.len() - 1];

        assert!(close(last.cx, 19.5 * 32.0 / 640.0), "the last prior is not the final stride-32 cell");
        assert!(close(last.cy, 19.5 * 32.0 / 640.0));
        assert!(close(last.sx, 512.0 / 640.0), "the last prior is not at the coarsest level's largest size");
        assert!(close(last.sy, 512.0 / 640.0));
    }

    #[test]
    fn the_ordering_is_level_major_then_row_major_then_size() {
        // The contract the decode rests on, stated as the three boundaries rather than as a spot check.
        let priors = generate(TARGET_SIZE);

        // Each level's span, in the order the levels are laid out.
        let spans = [2 * 80 * 80, 2 * 40 * 40, 2 * 20 * 20];
        let mut start = 0;

        for (level, span) in spans.iter().enumerate() {
            let stride = STRIDES[level] as f32;
            let step = stride / 640.0;

            // The first prior of the level is its cell (0,0) at its smaller size.
            let first = priors[start];
            assert!(close(first.cx, 0.5 * step), "level {level} does not begin at its own origin");
            assert!(close(first.sx, MIN_SIZES[level][0] as f32 / 640.0), "level {level}'s first size is wrong");

            // Within a level, the position advances by *column* before it advances by row: priors 2 and 3 are the
            // next cell across, at the same row.
            let next_cell = priors[start + 2];
            assert!(close(next_cell.cy, first.cy), "level {level} advanced a row before it advanced a column");
            assert!(close(next_cell.cx, 1.5 * step), "level {level} did not advance one column");

            start += span;
        }

        assert_eq!(start, priors.len(), "the levels do not account for every prior");
    }

    #[test]
    fn every_prior_is_inside_the_square_and_has_a_positive_extent() {
        // A centre outside 0-1 or a non-positive size would decode into a box nothing downstream could use, and the
        // exponential in the decode would carry either straight through.
        for (index, prior) in generate(TARGET_SIZE).iter().enumerate() {
            assert!((0.0..=1.0).contains(&prior.cx), "prior {index} has a centre outside the square: {prior:?}");
            assert!((0.0..=1.0).contains(&prior.cy), "prior {index} has a centre outside the square: {prior:?}");
            assert!(prior.sx > 0.0 && prior.sy > 0.0, "prior {index} has a non-positive size: {prior:?}");
        }
    }
}
