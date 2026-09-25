//! The align-centres bilinear read of a square plane back onto a photograph: the per-axis tables that map each of the
//! photograph's columns and rows onto the plane, and the read of one plane through them.
//!
//! Two callers from two families, which is why it is here rather than in either: São Paulo reads its weight map back
//! over the photograph (`models::color_balance::saopaulo::process`), and colorization reads its predicted chroma back
//! onto the photograph's own lightness (`models::colorization::compose`).

// Hand-rolled rather than `image`'s `Triangle`, because they are not the same filter. `Triangle` is a support-based
// resampler with its own normalisation and its own edge handling, which is a different answer at every pixel — and it
// would materialise a full-resolution plane per read, three of them and some 290 MB at 24 megapixels for São Paulo's
// weight map, for a value the fused pass consumes immediately. The four-tap read below is the exact inverse of one
// particular resize, which is a property of the pipelines that call it rather than of a general image library.
//
// Not `face_recovery::composite`'s `Taps` either, which is also a bilinear read of a square plane and is a *different
// function*: it resolves its geometry per pixel from an arbitrary `(x, y)`, because a face lands at a sub-pixel offset
// under an affine warp. This mapping is a pure axis-aligned scale, so both axes are separable and are resolved once
// for the whole photograph. Folding either into the other would put per-pixel `floor`s back into a loop that exists to
// have hoisted them.
//
// The reference's `sampleAxis` is a copy of its colorization helper, and the two agree only because nobody has yet
// changed one of them. Here there is one, and both families read through it.

/// The four-tap read of one destination position on one axis: the two source indices and how far between them it
/// falls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tap {
    /// The lower source index, clamped into the source.
    lo: usize,
    /// The upper source index, clamped to the last source index — which is what makes the last destination pixel
    /// read one sample rather than run off the end.
    hi: usize,
    /// How far between `lo` and `hi` the destination position falls, in `[0, 1]`.
    frac: f32,
}

/// One axis's bilinear table: a [`Tap`] per destination position, resolved once for the whole photograph.
#[derive(Debug)]
pub struct Axis {
    /// One entry per destination index, in order.
    taps: Vec<Tap>,
}

impl Axis {
    /// The table mapping `dst_len` destination positions back onto `src_len` source positions, aligned at pixel
    /// centres: destination `i` reads source coordinate `(i + 0.5) * src_len / dst_len - 0.5`, clamped at zero, with
    /// both taps clamped to the last source index.
    ///
    /// # Panics
    ///
    /// Panics where either length is zero, which only a caller that skipped its pipeline's zero-area refusal passes.
    pub fn new(src_len: u32, dst_len: u32) -> Self {
        assert!(src_len > 0 && dst_len > 0, "an axis of {src_len} source and {dst_len} destination positions");

        let scale = f64::from(src_len) / f64::from(dst_len);
        let last = src_len as usize - 1;

        let taps = (0..dst_len as usize)
            .map(|index| {
                // Centres rather than corners: both callers' planes are the output of a graph fed through a
                // pixel-centre resize (`image`'s `resize_exact`, reached through `present::presented` for
                // São Paulo and `models::colorization::stretch` for colorization), and this is that resize's inverse.
                // Align-corners would shift the plane by up to half a pixel against the photograph, which on a sharp
                // illuminant or colour boundary is a displaced edge. A graph's own convention may be the opposite, to
                // match its training, but nothing downstream of the graph is trained.
                //
                // The reference's own expression, in `f64` as it is there: at 24 megapixels the destination index
                // runs to five figures and the scale is a ratio of two of them, so the product is where a `f32`
                // would start losing the fraction this whole table is about. Written plainly rather than as a
                // `mul_add`, so it rounds twice exactly as the reference's unfused expression does.
                let source = (((index as f64) + 0.5) * scale - 0.5).max(0.0);

                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "the coordinate is clamped at zero above and truncates below the source length, which \
                              `min` then bounds"
                )]
                let lo = (source as usize).min(last);
                let hi = (lo + 1).min(last);

                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the fraction is the distance to the sample below, which is in [0, 1] by construction"
                )]
                let frac = (source - lo as f64) as f32;

                Tap { lo, hi, frac }
            })
            .collect();

        Self { taps }
    }

    /// The tap for destination position `index`.
    ///
    /// # Panics
    ///
    /// Panics where `index` is outside the axis this was built for.
    #[inline]
    pub fn tap(&self, index: usize) -> Tap {
        self.taps[index]
    }
}

/// One `side`-square `plane` read at `row` and `column`, bilinearly.
///
/// `plane` is one square plane rather than the whole tensor, so a caller slicing the wrong plane out of a
/// many-channel output is a slice index rather than an offset buried in this arithmetic.
///
/// The read is a convex combination of four samples, so it commutes with the sum over the planes: a weight vector
/// that is a partition of unity at each of the four taps is still one after interpolation.
///
/// # Panics
///
/// Panics where `plane` does not hold the `side` square.
#[inline]
pub fn sample(plane: &[f32], side: u32, row: Tap, column: Tap) -> f32 {
    let side = side as usize;
    assert_eq!(plane.len(), side * side, "a plane that is not the {side} square it was read at");

    // The reference's grouping: summed along x within each row first, then between the rows. Written plainly, with no
    // `mul_add`, for the reason `models::color_balance::mapping`'s header states.
    let top = plane[row.lo * side + column.lo] * (1.0 - column.frac) + plane[row.lo * side + column.hi] * column.frac;
    let bottom =
        plane[row.hi * side + column.lo] * (1.0 - column.frac) + plane[row.hi * side + column.hi] * column.frac;

    top * (1.0 - row.frac) + bottom * row.frac
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four weights a pair of taps puts on the four samples it reads, in the order the sum above applies them.
    fn quadrature(row: Tap, column: Tap) -> [f32; 4] {
        [
            (1.0 - row.frac) * (1.0 - column.frac),
            (1.0 - row.frac) * column.frac,
            row.frac * (1.0 - column.frac),
            row.frac * column.frac,
        ]
    }

    #[test]
    fn a_destination_landing_on_a_source_centre_reads_that_sample_alone() {
        // The exact-integer case, and the one a hand-computed number can be written down for. At an integer scale
        // the destination centres that land on source centres are the ones where the fraction is zero, and there
        // the read is the `lo` sample with nothing mixed into it.
        //
        // 2 -> 4: `s = (i + 0.5) * 0.5 - 0.5` gives -0.25, 0.25, 0.75, 1.25 — clamped to 0, then 0.25, 0.75, 1.25.
        let axis = Axis::new(2, 4);

        assert_eq!(axis.tap(0), Tap { lo: 0, hi: 1, frac: 0.0 }, "the first destination did not clamp at zero");
        assert_eq!(axis.tap(1), Tap { lo: 0, hi: 1, frac: 0.25 });
        assert_eq!(axis.tap(2), Tap { lo: 0, hi: 1, frac: 0.75 });
        assert_eq!(axis.tap(3), Tap { lo: 1, hi: 1, frac: 0.25 }, "the last destination did not clamp at the end");

        // And the identity case, which is the one every 1:1 axis takes: every destination is its own source and
        // nothing is interpolated at all.
        let same = Axis::new(5, 5);
        for index in 0..5 {
            assert_eq!(same.tap(index), Tap { lo: index, hi: (index + 1).min(4), frac: 0.0 }, "at {index}");
        }
    }

    #[test]
    fn the_mapping_is_align_centres_rather_than_align_corners() {
        // Its own test rather than folded into the table above, because it fails on exactly this mistake and nothing
        // else. Align-corners maps index `i` to `i * (src_len - 1) / (dst_len - 1)`: corner to corner, so the first
        // and last destination positions land exactly on the first and last source positions and everything between
        // shifts by up to half a pixel.
        //
        // A 3 -> 6 upsample separates them at almost every position, which a 1:1 or an integer-centred case does
        // not — the two conventions agree at the ends of the axis whatever they do in between.
        let axis = Axis::new(3, 6);
        let corners = |index: usize| (index as f64) * 2.0 / 5.0;

        // Centres: `s = (i + 0.5) * 0.5 - 0.5` -> -0.25, 0.25, 0.75, 1.25, 1.75, 2.25 -> clamped, floored, fracted.
        let expected = [
            Tap { lo: 0, hi: 1, frac: 0.0 },
            Tap { lo: 0, hi: 1, frac: 0.25 },
            Tap { lo: 0, hi: 1, frac: 0.75 },
            Tap { lo: 1, hi: 2, frac: 0.25 },
            Tap { lo: 1, hi: 2, frac: 0.75 },
            Tap { lo: 2, hi: 2, frac: 0.25 },
        ];

        for (index, want) in expected.into_iter().enumerate() {
            assert_eq!(axis.tap(index), want, "destination {index} is not the align-centres tap");
        }

        // And the two conventions genuinely disagree here, so the table above is a statement rather than a
        // coincidence: at four of the six positions the corner mapping lands somewhere else entirely.
        let disagreements = (0..6)
            .filter(|index| {
                let tap = axis.tap(*index);
                let centred = f64::from(tap.lo as u32) + f64::from(tap.frac);

                (centred - corners(*index)).abs() > 1e-9
            })
            .count();

        assert!(disagreements >= 4, "only {disagreements} of six positions tell the two conventions apart");
    }

    #[test]
    fn both_edges_clamp_rather_than_reading_outside_the_source() {
        // Half a source pixel falls outside the map at each end whatever the scale is, and there is no sample
        // there. Clamping is what makes the first and last destination rows read the map's own edge; an unclamped
        // `lo` would index below zero and an unclamped `hi` past the plane.
        for (src, dst) in [(1_u32, 9_u32), (4, 17), (656, 4000), (656, 3), (7, 7)] {
            let axis = Axis::new(src, dst);
            let last = src as usize - 1;

            for index in 0..dst as usize {
                let tap = axis.tap(index);

                assert!(tap.lo <= last && tap.hi <= last, "{src}->{dst} at {index} read outside the source");
                assert!(tap.hi >= tap.lo, "{src}->{dst} at {index} has its taps the wrong way round");
                assert!((0.0..=1.0).contains(&tap.frac), "{src}->{dst} at {index} has a fraction of {}", tap.frac);
            }

            // A single-sample source is the degenerate end of the clamp: both taps land on the one sample there
            // is, whatever the fraction between them works out to — so every destination position reads it.
            if src == 1 {
                assert!(
                    (0..dst as usize).all(|index| axis.tap(index).lo == 0 && axis.tap(index).hi == 0),
                    "a one-sample source was read somewhere other than at its one sample"
                );
            }
        }
    }

    #[test]
    fn the_four_weights_sum_to_one() {
        // What makes the read a convex combination, and therefore what makes the blend need no renormalising: the
        // weights are a partition of unity at every destination position, so a weight vector that is one stays one.
        let rows = Axis::new(9, 40);
        let columns = Axis::new(13, 31);

        for y in 0..40 {
            for x in 0..31 {
                let total: f32 = quadrature(rows.tap(y), columns.tap(x)).iter().sum();

                assert!((total - 1.0).abs() < 1e-6, "({x}, {y}) weights summed to {total}");
            }
        }
    }

    #[test]
    fn an_upsample_of_a_constant_plane_is_that_constant_everywhere() {
        // The property that falls out of the one above, stated over the read rather than over the table: a weight
        // map that says the same thing everywhere says it at the photograph's resolution too, whatever the scale
        // and whatever the clamps did at the edges.
        let canvas = 8_u32;
        let plane = vec![0.375_f32; (canvas as usize) * (canvas as usize)];
        let rows = Axis::new(5, 23);
        let columns = Axis::new(6, 17);

        for y in 0..23 {
            for x in 0..17 {
                let read = sample(&plane, canvas, rows.tap(y), columns.tap(x));

                assert!((read - 0.375).abs() < 1e-6, "({x}, {y}) read {read} from a plane that is 0.375 everywhere");
            }
        }
    }

    #[test]
    fn the_read_is_the_four_taps_it_names_and_nothing_else() {
        // The arithmetic against hand-computed numbers, on a plane whose every element is distinct so a sample read
        // from the wrong row or the wrong plane stride is a different number rather than a plausible one.
        //
        // The crop is 4x3 of an 8 square, so the stride between rows is the **square's** width rather than the
        // crop's — which is the mistake this catches: a read strided by the crop lands in the extension.
        let canvas = 8_u32;
        let plane: Vec<f32> = (0..64).map(|index| (index / 8) as f32 * 10.0 + (index % 8) as f32).collect();

        // 3 -> 6 vertically and 4 -> 8 horizontally, both of which put a destination between two sources.
        let rows = Axis::new(3, 6);
        let columns = Axis::new(4, 8);

        let (row, column) = (rows.tap(3), columns.tap(5));
        assert_eq!(row, Tap { lo: 1, hi: 2, frac: 0.25 });
        assert_eq!(column, Tap { lo: 2, hi: 3, frac: 0.25 });

        // The four samples: (1,2)=12, (1,3)=13, (2,2)=22, (2,3)=23, weighted 0.5625, 0.1875, 0.1875, 0.0625.
        let expected = 12.0 * 0.5625 + 13.0 * 0.1875 + 22.0 * 0.1875 + 23.0 * 0.0625;

        let read = sample(&plane, canvas, row, column);
        assert!((read - expected).abs() < 1e-4, "read {read} against the {expected} the four taps imply");

        // And a destination on a source centre reads that one sample, with the other three multiplied by nothing.
        let exact = sample(&plane, canvas, Axis::new(4, 4).tap(2), Axis::new(4, 4).tap(1));
        assert!((exact - 21.0).abs() < 1e-6, "a tap on a source centre read {exact} rather than that sample");
    }

    #[test]
    #[should_panic(expected = "a plane that is not the")]
    fn a_plane_that_is_not_the_square_is_refused_rather_than_read() {
        // The caller slices this out of a tensor it allocated at the square, so a disagreement is that caller
        // contradicting itself — and an unchecked read of a short plane is a silent index into whatever follows it
        // in the tensor, which for this layout is a rendering.
        let plane = vec![0.0_f32; 63];

        let _ = sample(&plane, 8, Axis::new(8, 8).tap(0), Axis::new(8, 8).tap(0));
    }
}
