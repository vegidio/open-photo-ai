//! The colour mapping this family fits and evaluates: an 11-term polynomial in RGB, solved by least squares from
//! what a graph was shown to what it produced.

// Family tier rather than Rio's, because it is what both of this family's contracts do with what their graph returns.
// Rio fits one mapping from one rendering; São Paulo fits one per rendering and blends them by a predicted weight map.
// The solver takes several destinations for that reason, and Rio passes one.
//
// A polynomial rather than the model's output, because the graph runs at a fixed square — 656 for both of this
// family's variants — which is smaller than most photographs. Handing its output back enlarged would cap every
// photograph's detail at that square. What is taken from it instead is the *colour transform* it describes, fitted
// across a few hundred thousand samples and then evaluated over the photograph's own pixels at the photograph's own
// resolution. An 11-term fit barely moves with the resolution its samples were taken at, which is the whole of why
// the sampling resolution can be traded and the photograph's detail cannot.
//
// The arithmetic is the reference's, term for term. The feature order, the `f64` accumulation, the `1e-8` ridge, the
// partial pivoting and the elimination order are all transcribed. None of them is a matter of taste: `XᵀX` sums up to
// 430,336 products of features that include `r²g²`-scale terms, and accumulating that in `f32` loses the low-order
// bits the ridge is supposed to be the only perturbation of. Changing any of them changes which photograph comes out,
// with no error anywhere and no measurement in this project that would catch a small difference.
//
// `mul_add` is not used here, and that is the reference's behaviour rather than a divergence from it. Go permits an
// implementation to contract `x + y*z` into one rounding, and the reference's `mixed.go` writes explicit conversions
// specifically to forbid it: a contracted product can fall on the other side of an exact integer once truncated to 8
// bits, and change the byte. Rust never contracts implicitly, so writing the accumulation plainly reaches the
// behaviour the reference works to get, by doing nothing.
//
// The rule, stated once for the families that follow: **an accumulation of many products is written plainly; a
// single fused term may use `mul_add`.** `imaging::mix::blended` is the single-term case and keeps its `mul_add`.

use super::samples::Samples;

use crate::error::InferenceError;

/// How many terms the polynomial carries.
pub(super) const TERMS: usize = 11;

/// The 11-feature polynomial vector Deep_White_Balance's fit is taken over.
///
/// **The order is significant and is pinned as a literal by the test below.**
pub(super) fn kernel(r: f32, g: f32, b: f32) -> [f32; TERMS] {
    [r, g, b, r * g, r * b, g * b, r * r, g * g, b * b, r * g * b, 1.0]
}

/// One fitted colour mapping: the weight each of the eleven features carries into each of the three channels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Mapping {
    /// `[feature][channel]`, as the reference's `[11][3]float32` is.
    weights: [[f32; 3]; TERMS],
}

impl Mapping {
    /// The colour at `rgb` mapped through this fit, in `[0, 1]` unit space and unbounded: the caller's own channel
    /// conversion is what bounds it, as [`Channel::from_unit`](imaging::tensor::Channel::from_unit) clamps
    /// to the range the channel can carry.
    ///
    /// Exactly [`kernel`] and then [`evaluate_kernel`](Self::evaluate_kernel).
    pub(super) fn evaluate(&self, rgb: [f32; 3]) -> [f32; 3] {
        // Unbounded rather than clamped: a second clamp here would be a copy of `from_unit`'s rule that can disagree
        // with it.
        let [r, g, b] = rgb;

        self.evaluate_kernel(kernel(r, g, b))
    }

    /// This fit applied to an **already-built** feature vector.
    ///
    /// For a caller evaluating several mappings at one pixel, as São Paulo's fused pass does: building [`kernel`] is
    /// most of the arithmetic — eleven terms including three multiplies — so calling [`evaluate`](Self::evaluate)
    /// once per mapping would very nearly double the loop for nothing.
    pub(super) fn evaluate_kernel(&self, features: [f32; TERMS]) -> [f32; 3] {
        // One definition of the polynomial rather than two, which is the point of the split rather than a consequence
        // of it: a second copy of the accumulation somewhere else is a copy that can disagree with this one, and the
        // disagreement would be a photograph rather than a failure.
        //
        // The accumulation is written plainly. See this module's header.
        let mut out = [0.0_f32; 3];

        for (feature, weights) in features.into_iter().zip(self.weights) {
            for (channel, weight) in out.iter_mut().zip(weights) {
                *channel += feature * weight;
            }
        }

        out
    }
}

/// Fits one mapping per destination from a shared source, solving them all in one elimination.
///
/// `W = (XᵀX + λI)⁻¹XᵀY`, where each row of `X` is [`kernel`] over a source sample and each row of `Y` is the
/// destination sample at the same index.
///
/// # Errors
///
/// Returns [`InferenceError::ColourMapping`] where a pivot is zero, naming `operation` and the column. The ridge
/// makes `XᵀX` positive-definite, so after partial pivoting that should never happen — and *should* is not a
/// guarantee in `f64`.
///
/// # Panics
///
/// Panics where `destinations` is empty, or where any destination does not hold as many samples as `source`.
pub(super) fn fit(
    operation: &str,
    source: &Samples<'_>,
    destinations: &[&Samples<'_>],
) -> Result<Vec<Mapping>, InferenceError> {
    assert!(!destinations.is_empty(), "a colour mapping was fitted to no destination");

    for (index, destination) in destinations.iter().enumerate() {
        assert_eq!(
            destination.len(),
            source.len(),
            "destination {index} holds {} samples against the source's {}",
            destination.len(),
            source.len()
        );
    }

    // Several destinations rather than a loop around one: `XᵀX` depends only on the source, so it is the same matrix
    // for every destination — and building it is the expensive half, `O(N·121)` over a few hundred thousand samples
    // against a fixed 11x11 elimination afterwards. Accumulating it once is therefore very nearly the whole saving: the
    // reference's own benchmark measures 32 ms for two destinations fitted together against 53 ms fitted separately, at
    // this family's canvas.
    //
    // One block of three columns per destination, laid out end to end so a single elimination solves all of them.
    let columns = 3 * destinations.len();
    let width = TERMS + columns;

    // The augmented matrix `[XᵀX | XᵀY]`, flat and row-major: eleven rows of `width`. Flat rather than nested
    // because the row swap below is an index swap either way and the elimination walks one row at a time.
    let mut augmented = vec![0.0_f64; TERMS * width];

    for index in 0..source.len() {
        let [r, g, b] = source.triple(index);
        let features = kernel(r, g, b).map(f64::from);

        for (row, feature) in features.into_iter().enumerate() {
            let base = row * width;

            for (column, other) in features.into_iter().enumerate() {
                augmented[base + column] += feature * other;
            }

            for (destination, samples) in destinations.iter().enumerate() {
                let [dr, dg, db] = samples.triple(index);
                let block = base + TERMS + 3 * destination;

                augmented[block] += feature * f64::from(dr);
                augmented[block + 1] += feature * f64::from(dg);
                augmented[block + 2] += feature * f64::from(db);
            }
        }
    }

    // The ridge, on the diagonal only. `1e-8` is the reference's, and it is what keeps a degenerate source — a flat
    // colour, a photograph of one wall — from producing a system with no answer at all.
    for term in 0..TERMS {
        augmented[term * width + term] += 1e-8;
    }

    eliminate(operation, &mut augmented, width)?;

    Ok((0..destinations.len())
        .map(|destination| {
            let base = TERMS + 3 * destination;
            let mut weights = [[0.0_f32; 3]; TERMS];

            for (term, row) in weights.iter_mut().enumerate() {
                let element = term * width + base;

                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the fit accumulates in f64 and the weights are carried at the f32 the apply reads them at, \
                              which is the reference's own arithmetic"
                )]
                for (channel, value) in row.iter_mut().enumerate() {
                    *value = augmented[element + channel] as f32;
                }
            }

            Mapping { weights }
        })
        .collect())
}

/// Gauss-Jordan elimination with partial pivoting over the augmented matrix, in place.
///
/// # Errors
///
/// Returns [`InferenceError::ColourMapping`] on a zero pivot. See [`fit`].
fn eliminate(operation: &str, augmented: &mut [f64], width: usize) -> Result<(), InferenceError> {
    // Split out of `fit` because it is the half that can fail and the half whose order is transcribed. Nothing here is
    // an improvement on the reference and nothing here should become one.
    for column in 0..TERMS {
        let mut pivot = column;
        let mut largest = augmented[column * width + column].abs();

        for row in column + 1..TERMS {
            let candidate = augmented[row * width + column].abs();

            if candidate > largest {
                largest = candidate;
                pivot = row;
            }
        }

        if pivot != column {
            for element in 0..width {
                augmented.swap(column * width + element, pivot * width + element);
            }
        }

        // Raised rather than rendered: dividing by a zero pivot fills every weight with NaN, which the apply renders
        // as a destroyed photograph with nothing anywhere to say why. Falling back to the identity mapping is the
        // tempting alternative and is worse: it returns the photograph unchanged from a run the user asked to change
        // it, which is indistinguishable from a bias of zero and from a model that found nothing to correct.
        let divisor = augmented[column * width + column];
        if divisor == 0.0 {
            return Err(InferenceError::ColourMapping { operation: operation.to_string(), column });
        }

        for element in column..width {
            augmented[column * width + element] /= divisor;
        }

        for row in 0..TERMS {
            if row == column {
                continue;
            }

            let factor = augmented[row * width + column];
            if factor == 0.0 {
                continue;
            }

            for element in column..width {
                augmented[row * width + element] -= factor * augmented[column * width + element];
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use imaging::tensor::Normalisation;

    /// The range this family's graphs are written in, which is what the views below are read through.
    const UNIT: Normalisation = Normalisation::Unit;

    /// A one-plane-triple tensor of `side` square holding `colours` in its top-left row-major run, as the planar CHW
    /// a graph produces.
    fn planar(side: u32, colours: &[[f32; 3]]) -> Vec<f32> {
        // Planar rather than triples, because that is the only thing `Samples` reads — so a fixture built the other
        // way would be testing a reader this family does not have.
        let plane = (side as usize) * (side as usize);
        let mut tensor = vec![0.0_f32; 3 * plane];

        for (index, colour) in colours.iter().enumerate() {
            let element = (index / (side as usize)) * (side as usize) + index % (side as usize);

            for (channel, value) in colour.iter().enumerate() {
                tensor[channel * plane + element] = *value;
            }
        }

        tensor
    }

    /// A spread of colours wide enough for an 11-term fit to be determined: no two share a channel value, and the
    /// products that make up the higher terms are distinct as well.
    fn spread() -> Vec<[f32; 3]> {
        let mut colours = Vec::new();

        for r in 0..4_u8 {
            for g in 0..4_u8 {
                for b in 0..4_u8 {
                    colours.push([f32::from(r) / 3.2 + 0.03, f32::from(g) / 3.5 + 0.05, f32::from(b) / 3.7 + 0.07]);
                }
            }
        }

        colours
    }

    /// The square the colour fixtures below are laid into.
    const SIDE: u32 = 8;

    /// The crop a run of `colours` occupies in a [`SIDE`] square: whole rows, so a rectangle inside the square rather
    /// than a run that wraps.
    fn crop(colours: &[[f32; 3]]) -> (u32, u32) {
        // Whole rows because a rectangle is what the view reads and is the geometry a real run has. A fixture that
        // wrapped would be testing a reader this family does not have.
        let rows = colours.len() / (SIDE as usize);

        assert_eq!(colours.len() % (SIDE as usize), 0, "the fixture is not whole rows of the square");
        assert!(rows <= SIDE as usize, "the fixture outgrew the square");

        (SIDE, rows as u32)
    }

    /// The mapping fitted from `source` to `destination`, both given as colour lists.
    fn fitted(source: &[[f32; 3]], destination: &[[f32; 3]]) -> Mapping {
        let source_tensor = planar(SIDE, source);
        let destination_tensor = planar(SIDE, destination);

        let source_view = Samples::new(&source_tensor, 0, SIDE, crop(source), UNIT).expect("a square");
        let destination_view = Samples::new(&destination_tensor, 0, SIDE, crop(destination), UNIT).expect("a square");

        fit("Rio (FP32)", &source_view, &[&destination_view])
            .expect("a determined system fits")
            .pop()
            .expect("one destination fits one mapping")
    }

    #[test]
    fn splitting_the_evaluation_in_two_did_not_move_what_it_answers() {
        // The constraint the split is held to, stated where the split is rather than only in São Paulo's fused pass,
        // which calls the kernel half. Rio's apply calls `evaluate`, so a refactor that moved its answer would be a
        // regression in a **shipped** model.
        //
        // Bit for bit rather than within a tolerance: the two paths are the same accumulation in the same order,
        // so anything else is a different function however close it lands.
        let mapping =
            fitted(&spread(), &spread().iter().map(|[r, g, b]| [r * 0.8, g * 1.1, b * 0.95]).collect::<Vec<_>>());

        for colour in [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.2, 0.7, 0.4], [0.93, 0.01, 0.55]] {
            let [r, g, b] = colour;

            assert_eq!(
                mapping.evaluate(colour),
                mapping.evaluate_kernel(kernel(r, g, b)),
                "the two halves disagree at {colour:?}"
            );
        }
    }

    #[test]
    fn the_feature_order_is_the_one_the_weights_were_fitted_against() {
        // Pinned as a literal, because every other way of getting it wrong is a photograph rather than an error: a
        // permuted order still builds a full-rank system, still solves, and still renders. The order is
        // Deep_White_Balance's and the weights are fitted against it.
        let features = kernel(0.2, 0.3, 0.5);

        assert_eq!(
            features,
            [
                0.2,
                0.3,
                0.5,
                0.2 * 0.3,
                0.2 * 0.5,
                0.3 * 0.5,
                0.2 * 0.2,
                0.3 * 0.3,
                0.5 * 0.5,
                0.2 * 0.3 * 0.5,
                1.0
            ],
            "the kernel's term order moved"
        );
        assert_eq!(features.len(), TERMS);
        assert_eq!(features[TERMS - 1], 1.0, "the constant term is not last");
    }

    #[test]
    fn an_exactly_linear_colour_map_is_recovered_to_round_off() {
        // The strongest statement a test can make about a least-squares fit: where the destination really is a
        // function inside the feature set, the fit has to find *that* function rather than something close to it.
        // A non-symmetric map, so a solver that transposed the weight block — three channels read out of one
        // column — fails here rather than reproducing a symmetric answer.
        let source = spread();
        let destination: Vec<[f32; 3]> = source
            .iter()
            .map(|[r, g, b]| [0.8 * r + 0.1 * g + 0.02, 0.05 * r + 1.1 * g - 0.03 * b, 0.2 * g + 0.9 * b + 0.01])
            .collect();

        let mapping = fitted(&source, &destination);

        for (colour, expected) in source.iter().zip(&destination) {
            let produced = mapping.evaluate(*colour);

            for channel in 0..3 {
                assert!(
                    (produced[channel] - expected[channel]).abs() < 1e-3,
                    "{colour:?} channel {channel}: {} against the exact {}",
                    produced[channel],
                    expected[channel]
                );
            }
        }
    }

    #[test]
    fn the_fit_runs_from_the_source_to_the_destination_and_not_the_other_way() {
        // The mistake that produces a photograph, and on a mild cast a nearly plausible one. Stated on a map that
        // is not its own inverse: fitting it backwards recovers roughly `1/0.6` rather than `0.6`, so the recovered
        // weight is read off and compared rather than the residual alone.
        let source = spread();
        let destination: Vec<[f32; 3]> = source.iter().map(|[r, g, b]| [0.6 * r, *g, *b]).collect();

        let mapping = fitted(&source, &destination);

        // Feature 0 is `r`, channel 0 is red: the scale the map applies, read directly out of the fit.
        assert!(
            (mapping.weights[0][0] - 0.6).abs() < 1e-3,
            "the red-from-red weight is {} rather than the 0.6 the map applies",
            mapping.weights[0][0]
        );

        let reversed = fitted(&destination, &source);
        assert!(
            (reversed.weights[0][0] - 1.0 / 0.6).abs() < 1e-2,
            "fitting the other way did not recover the inverse: {}",
            reversed.weights[0][0]
        );
    }

    #[test]
    fn a_feature_order_permuted_by_one_does_not_reproduce_the_fit() {
        // What the literal above is *for*, stated as the consequence rather than as the spelling: evaluating a
        // fitted mapping against a kernel whose terms are rotated by one produces a different colour. A reader who
        // "tidied" the order into something alphabetical would pass every other test in this file.
        let source = spread();
        let destination: Vec<[f32; 3]> =
            source.iter().map(|[r, g, b]| [0.9 * r + 0.05 * g, 1.05 * g, 0.85 * b + 0.04 * r]).collect();

        let mapping = fitted(&source, &destination);
        let colour = [0.42_f32, 0.31, 0.63];

        let mut rotated = kernel(colour[0], colour[1], colour[2]);
        rotated.rotate_left(1);

        let mut permuted = [0.0_f32; 3];
        for (feature, weights) in rotated.into_iter().zip(mapping.weights) {
            for (channel, weight) in permuted.iter_mut().zip(weights) {
                *channel += feature * weight;
            }
        }

        let produced = mapping.evaluate(colour);
        assert!(
            (0..3).any(|channel| (produced[channel] - permuted[channel]).abs() > 1e-3),
            "a rotated feature order produced the same colour: {produced:?} against {permuted:?}"
        );
    }

    #[test]
    fn two_destinations_fitted_together_give_what_each_gives_alone() {
        // The generalisation's own claim, which is what makes it safe for Rio to pass one destination: `XᵀX`
        // depends only on the source, so adding a second destination adds three columns to the elimination and
        // changes nothing about the first three.
        let source = spread();
        let first: Vec<[f32; 3]> = source.iter().map(|[r, g, b]| [0.7 * r, 1.2 * g, 0.9 * b]).collect();
        let second: Vec<[f32; 3]> = source.iter().map(|[r, g, b]| [1.1 * r, 0.8 * g, 1.3 * b]).collect();

        let extent = crop(&source);
        let (source_tensor, first_tensor, second_tensor) =
            (planar(SIDE, &source), planar(SIDE, &first), planar(SIDE, &second));

        let source_view = Samples::new(&source_tensor, 0, SIDE, extent, UNIT).expect("a square");
        let first_view = Samples::new(&first_tensor, 0, SIDE, extent, UNIT).expect("a square");
        let second_view = Samples::new(&second_tensor, 0, SIDE, extent, UNIT).expect("a square");

        let together = fit("Rio (FP32)", &source_view, &[&first_view, &second_view]).expect("a determined system");

        assert_eq!(together.len(), 2, "two destinations did not produce two mappings");
        assert_eq!(together[0], fitted(&source, &first), "the first mapping moved when a second was fitted with it");
        assert_eq!(together[1], fitted(&source, &second), "the second mapping is not the one fitted alone");
    }

    #[test]
    fn a_degenerate_source_still_fits_rather_than_dividing_by_nothing() {
        // What the ridge is for, on the input that reaches it: a photograph of one flat colour gives a rank-1
        // system, and without the `1e-8` on the diagonal the elimination divides by zero. With it the system is
        // positive-definite and answers — which is why the refusal below is the unreachable case rather than the
        // ordinary one.
        let flat = vec![[0.4_f32, 0.4, 0.4]; 16];
        let destination = vec![[0.5_f32, 0.3, 0.2]; 16];

        let mapping = fitted(&flat, &destination);
        let produced = mapping.evaluate([0.4, 0.4, 0.4]);

        for channel in 0..3 {
            assert!(produced[channel].is_finite(), "a degenerate fit produced {produced:?}");
        }
    }

    #[test]
    fn a_singular_system_is_refused_by_name_rather_than_filled_with_nan() {
        // The refusal, reached by handing the elimination a matrix the accumulation cannot produce — every element
        // zero, so the ridge is not there to save it. That is the point: the variant exists for the case the ridge
        // was supposed to have made impossible, and the only honest way to exercise it is to remove the ridge.
        let width = TERMS + 3;
        let mut augmented = vec![0.0_f64; TERMS * width];

        let error = eliminate("Rio (FP16)", &mut augmented, width).expect_err("a zero matrix has no pivot");

        let InferenceError::ColourMapping { operation, column } = error else {
            panic!("a singular system was refused as something else");
        };

        assert_eq!(operation, "Rio (FP16)", "the refusal did not name the operation");
        assert_eq!(column, 0, "the refusal did not name the column the pivot failed at");
    }

    #[test]
    fn the_refusal_names_the_column_the_elimination_actually_reached() {
        // Not the first one: the column is carried so a report says where the system went degenerate, and a variant
        // that always said zero would be carrying a constant. An identity in the first two columns and nothing
        // after them fails at the third.
        let width = TERMS + 3;
        let mut augmented = vec![0.0_f64; TERMS * width];
        augmented[0] = 1.0;
        augmented[width + 1] = 1.0;

        let error = eliminate("Rio (FP32)", &mut augmented, width).expect_err("a rank-2 matrix has no third pivot");

        assert!(
            matches!(error, InferenceError::ColourMapping { column: 2, .. }),
            "the refusal named the wrong column: {error:?}"
        );
    }

    #[test]
    fn the_identity_map_is_recovered_as_the_identity() {
        // The endpoint the whole arrangement rests on: a graph that returned what it was shown produces a mapping
        // that changes nothing, so the run returns the photograph. Every other property in this file is about a
        // correction; this one is about there being none.
        let source = spread();
        let mapping = fitted(&source, &source);

        for colour in [[0.1_f32, 0.2, 0.3], [0.5, 0.5, 0.5], [0.9, 0.05, 0.44]] {
            let produced = mapping.evaluate(colour);

            for channel in 0..3 {
                assert!(
                    (produced[channel] - colour[channel]).abs() < 1e-3,
                    "{colour:?} channel {channel} came back as {}",
                    produced[channel]
                );
            }
        }
    }
}
