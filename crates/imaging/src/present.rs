//! The front half of a whole-image run on a fixed square: the geometry that fits a photograph onto it, and the
//! presentation that writes the photograph into a graph's tensor.

// Cross-family tier rather than any one family's, because running at a fixed square is not one family's arrangement.
// Light adjustment presents at 1024 and colour balance at 656, and both reach their graph through this same plan, this
// same Lanczos resample and this same reflection extension. What each family keeps for itself is the square its graphs
// were exported at, the range they read — which `presented` takes rather than picks — and everything it does with what
// comes back.
//
// **The square is a cost, not an answer.** A fixed-shape graph accepts exactly one size, so a photograph smaller than
// the square is enlarged onto it as readily as a larger one is shrunk, and the output is produced at the square
// whatever the photograph was. Handing that back enlarged would cap every photograph's detail at the square; what a
// family takes from it is the *change* it describes, applied to pixels the photograph already had. Nothing here does
// that — it is why nothing here is asked to.

use image::DynamicImage;
use image::imageops::FilterType;

use crate::tensor::{self, Normalisation, Sampler, TensorShape};

// A value of its own, and produced by a pure function, so the geometry is something a test can check rather than
// something a reader has to take on trust — which is also why the reference splits `planCanvas` out of `Process`.
// Every way this can be wrong produces a plausible photograph and no error.
/// The geometry one run uses: the size the photograph is resampled to, and the extension that fills the square.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// The width the photograph is resampled to before it is padded.
    pub scaled_width: u32,
    /// The height the photograph is resampled to before it is padded.
    pub scaled_height: u32,
    /// How many columns of reflected edge fill the square to the right of the photograph.
    pub pad_width: u32,
    /// How many rows of reflected edge fill the square below it.
    pub pad_height: u32,
}

impl Plan {
    /// The side of the square this plan fills: the photograph and its extension together.
    pub const fn canvas(self) -> u32 {
        self.scaled_width + self.pad_width
    }
}

/// The geometry for a `width` by `height` photograph on a `canvas` square.
///
/// A fixed-shape graph accepts exactly one size, so the longer side always lands on `canvas` — enlarging a
/// photograph smaller than the square as readily as shrinking one larger than it — and the shorter side is extended
/// out to fill it.
///
/// # Panics
///
/// Panics when either dimension is zero.
pub fn plan(width: u32, height: u32, canvas: u32) -> Plan {
    let (scaled_width, scaled_height) = fit_rounded(width, height, canvas);

    Plan {
        scaled_width,
        scaled_height,
        // Saturating rather than plain subtraction: the fit cannot exceed the canvas, so both of these are the
        // extension's real width, and an underflow here would be this function contradicting itself rather than
        // anything a caller could have caused.
        pad_width: canvas.saturating_sub(scaled_width),
        pad_height: canvas.saturating_sub(scaled_height),
    }
}

/// The whole-pixel dimensions a `width` by `height` photograph is scaled to so that its longer side is `size`,
/// **rounding** to the nearest pixel. The shorter side is floored to at least one pixel.
fn fit_rounded(width: u32, height: u32, size: u32) -> (u32, u32) {
    // Not `newyork::input::fit`, which does the same thing. Detection already has a function that scales a
    // photograph's longer side onto a square, and nothing here may call it. New York's **truncates**, because the
    // reference's detection preprocessing truncates and the coordinate rescale that undoes it has to undo exactly what
    // it did. `utils.FitLongSide`, which every family presented on a fixed square goes through, **rounds**.
    //
    // They differ by a pixel on most aspect ratios, and a pixel here is a slightly different scale factor, a slightly
    // different set of pixels shown to the model, and a slightly different photograph out — with no error and no
    // diagnostic anywhere. So there is a second one, named for the difference rather than for the job, and a test
    // pins a ratio on which the two disagree so that neither can be folded into the other silently.
    //
    // The tier rule is not being bent by the duplication, and the two sitting at different tiers is not an argument
    // for folding them: a `use` reaching from here into `detection/newyork/` would be this module naming a model,
    // which it never does, and promoting New York's would give one function two callers wanting two answers. There is
    // no promotion to make, because the two are not the same function.
    assert!(width > 0 && height > 0, "a {width}x{height} photograph has no scale onto the square");

    let ratio = f64::from(size) / f64::from(width.max(height));

    (scaled(width, ratio), scaled(height, ratio))
}

/// One axis through the fit: rounded to the nearest pixel, and never scaled out of existence.
fn scaled(length: u32, ratio: f64) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the ratio is the square over the longer side, so neither product exceeds the square and both are \
                  positive"
    )]
    let rounded = (f64::from(length) * ratio).round() as u32;

    // A 4000x1 photograph scales its height to zero, and a resize to no height is a picture with nothing in it rather
    // than a very wide one.
    rounded.max(1)
}

/// Resamples `source` to the planned size and writes it into `dest` as the reflection-padded planar CHW the graph
/// accepts, handing back the resampled photograph the correction will need again.
///
/// `norm` is the range the family's graphs were trained on; there is no default.
///
/// # Errors
///
/// Returns [`TensorShape`] when `dest` is not exactly three planes of the planned square, having written nothing.
pub fn presented(
    source: &DynamicImage,
    planned: Plan,
    norm: Normalisation,
    dest: &mut [f32],
) -> Result<DynamicImage, TensorShape> {
    // Two steps that look like one and are not, as in the reference. **Fitting the longer side onto the square is a
    // resample, because that is the point of it. Filling the rest of the square is reflection padding, because it is
    // not** — the pixels the model was going to see must not change because the photograph needed more columns to
    // make up a square.
    //
    // Resizing to fill the square instead does change them, and visibly: measured on a 1000x750 photograph through the
    // real Paris graph, that path left the last column differing from its neighbour by 40.8 levels on average against
    // an interior column-to-column gradient of 3.9 — a hard one-pixel line down the side of the picture. Reflecting
    // brings it to 2.9, which is the interior.
    //
    // `norm` is stated rather than chosen because the families disagree: light adjustment and colour balance read
    // `[0, 1]`, face recovery's restorers `[-1, 1]`. Each family pins its own beside the graphs that read it.
    //
    // `DynamicImage::resize_exact` rather than `imageops::resize` over the image directly, which is this crate's
    // established idiom and for the reason `upscale::passes` gives: the latter goes through `DynamicImage`'s
    // `GenericImageView`, typed `Rgba<u8>`, so a 16-bit photograph would be quantised to eight bits here — in the
    // one step that exists to show the model what the photograph actually holds.
    let resized = source.resize_exact(planned.scaled_width, planned.scaled_height, FilterType::Lanczos3);

    tensor::padded_to_chw(
        dest,
        &Sampler::new(&resized),
        (planned.scaled_width, planned.scaled_height),
        (planned.canvas(), planned.canvas()),
        norm,
    )?;

    // Returned rather than dropped because a caller correcting the photograph by the ratio between what the model
    // produced and what it was shown needs it as that ratio's denominator — light adjustment's gain map does.
    // Resampling it a second time there would be the same arithmetic done twice on a photograph-sized buffer, and one
    // of the two copies could drift from the other. A caller with no use for it drops it; the resample happens either
    // way, so only the binding differs.
    Ok(resized)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::photograph;

    /// The canvas both light adjustment variants are exported at, which is what the geometry below is checked on.
    const CANVAS: u32 = 1024;

    // Named here rather than reached for out of a family, because the point of the parameter is that this module has
    // no range of its own — a test that read light adjustment's constant would be asserting that family's choice from
    // a file that does not make it. Which range these use is arbitrary; that the function writes whichever it is given
    // is the separate test below.
    /// The range the presentation tests below hand the function.
    const UNIT: Normalisation = Normalisation::Unit;

    /// `newyork::input::fit`, transcribed rather than imported.
    fn truncating_fit(width: u32, height: u32, size: u32) -> (u32, u32) {
        // The import would be a `use` across a model boundary, which the tier rule names as a bug, and it would mean
        // widening that function's visibility for a test. What is wanted here is not detection's answer but the
        // **truncating** answer, so the truncation is written out and the test below asserts the two disagree.
        let ratio = f64::from(height) / f64::from(width);

        if ratio > 1.0 {
            (((f64::from(size) / ratio) as u32).max(1), size)
        } else {
            (size, ((f64::from(size) * ratio) as u32).max(1))
        }
    }

    #[test]
    fn the_longer_side_lands_on_the_square_and_the_shorter_keeps_the_aspect_ratio() {
        // Landscape, portrait and square, because the geometry is symmetric and a test of one orientation would
        // pass over a fit that had swapped the axes.
        assert_eq!(
            plan(2048, 1024, CANVAS),
            Plan { scaled_width: 1024, scaled_height: 512, pad_width: 0, pad_height: 512 }
        );
        assert_eq!(
            plan(1024, 2048, CANVAS),
            Plan { scaled_width: 512, scaled_height: 1024, pad_width: 512, pad_height: 0 }
        );
        assert_eq!(
            plan(1500, 1500, CANVAS),
            Plan { scaled_width: 1024, scaled_height: 1024, pad_width: 0, pad_height: 0 }
        );
    }

    #[test]
    fn a_square_photograph_needs_no_extension_at_all() {
        // The spec's own scenario, and the one case where the padding path must not run: a square fills the canvas.
        for side in [1, 17, 512, 1024, 4000] {
            let planned = plan(side, side, CANVAS);

            assert_eq!(planned.scaled_width, CANVAS, "a {side}x{side} photograph did not fill the width");
            assert_eq!(planned.scaled_height, CANVAS, "a {side}x{side} photograph did not fill the height");
            assert_eq!((planned.pad_width, planned.pad_height), (0, 0), "a square photograph was extended");
        }
    }

    #[test]
    fn a_photograph_smaller_than_the_square_is_enlarged_onto_it_rather_than_run_at_its_own_size() {
        // A fixed-shape graph accepts one size, so there is no path on which a thumbnail runs at its own resolution.
        let planned = plan(400, 300, CANVAS);

        assert_eq!(planned.scaled_width, CANVAS);
        assert_eq!(planned.scaled_height, 768, "the aspect ratio was not preserved through the enlargement");
        assert_eq!((planned.pad_width, planned.pad_height), (0, 256));
    }

    #[test]
    fn an_extreme_aspect_ratio_still_leaves_a_pixel_on_the_short_side() {
        // A resize to zero height is a picture with nothing in it. The reference's `FitLongSide` has the same floor,
        // and unlike detection's it is load-bearing here rather than defensive: nothing downstream would report it.
        for (width, height) in [(4000_u32, 1_u32), (1, 4000)] {
            let planned = plan(width, height, CANVAS);

            assert!(planned.scaled_width >= 1 && planned.scaled_height >= 1, "{width}x{height} lost an axis");
            assert_eq!(planned.scaled_width.max(planned.scaled_height), CANVAS);
            assert_eq!(planned.scaled_width.min(planned.scaled_height), 1, "the short side was not floored to a pixel");
        }

        // And the extension then fills the whole of the rest of the square, which is the case `pad::source_offset`
        // reflects more than once to satisfy.
        assert_eq!(plan(4000, 1, CANVAS).pad_height, CANVAS - 1);
    }

    #[test]
    fn the_planned_size_and_its_extension_always_compose_to_the_square() {
        // The invariant everything downstream assumes: the tensor is one square, and the picture plus its extension
        // is exactly that square on both axes. A fit that overshot would make the padding wrap rather than extend.
        for (width, height) in [(1, 1), (4000, 3000), (3000, 4000), (1023, 1024), (1025, 1024), (6000, 17), (7, 5000)] {
            let planned = plan(width, height, CANVAS);

            assert_eq!(planned.scaled_width + planned.pad_width, CANVAS, "{width}x{height} did not fill the width");
            assert_eq!(planned.scaled_height + planned.pad_height, CANVAS, "{width}x{height} did not fill the height");
        }
    }

    #[test]
    fn the_fit_rounds_where_detections_truncates_and_the_two_can_never_be_folded() {
        // The divergence `fit_rounded` records, pinned on a ratio that actually separates them. Both functions look
        // identical and neither reports anything when the wrong one is used: what a reader would see is a
        // photograph adjusted at a very slightly different scale, which is a different photograph and no error.
        //
        // 1000x687 at 1024: the height scales to 703.488, which rounds to 703 and truncates to 703 — so the ratio
        // has to be chosen rather than assumed. 1000x669 gives 685.056 both ways. The pair below is searched for
        // rather than written down, so the test states the property instead of one example of it.
        let disagreeing = (1..=2000_u32)
            .filter_map(|height| {
                let rounded = fit_rounded(1000, height, CANVAS);
                let truncated = truncating_fit(1000, height, CANVAS);

                (rounded != truncated).then_some((height, rounded, truncated))
            })
            .next()
            .expect("some aspect ratio must separate rounding from truncation");

        let (height, rounded, truncated) = disagreeing;
        assert_ne!(rounded, truncated, "1000x{height} scaled alike under both rules");

        // And the direction, so a fit that had been "fixed" to truncate fails here rather than merely differing:
        // rounding is never below truncation, and on this ratio it is a pixel above it.
        assert!(
            rounded.0 >= truncated.0 && rounded.1 >= truncated.1,
            "1000x{height}: rounding landed below truncation, {rounded:?} against {truncated:?}"
        );
    }

    /// The red plane's value at `(x, y)` in a tensor of `canvas` square.
    fn red(tensor: &[f32], canvas: u32, x: u32, y: u32) -> f32 {
        tensor[(y as usize) * (canvas as usize) + x as usize]
    }

    #[test]
    fn the_tensors_interior_is_the_resampled_photograph_and_its_margin_is_the_mirror_beside_it() {
        // A small canvas, so the whole tensor can be compared pixel by pixel rather than sampled.
        let canvas = 16;
        let source = photograph(40, 20);
        let planned = plan(40, 20, canvas);
        assert_eq!((planned.scaled_width, planned.scaled_height), (16, 8), "the fixture stopped exercising padding");

        let mut tensor = vec![0.0_f32; 3 * (canvas as usize) * (canvas as usize)];
        let resized = presented(&source, planned, UNIT, &mut tensor).expect("a square scratch is accepted");

        assert_eq!((resized.width(), resized.height()), (16, 8), "the resample did not land on the planned size");

        // The interior: every pixel the photograph itself supplied, unchanged by the presence of the extension.
        let sampler = Sampler::new(&resized);
        for y in 0..planned.scaled_height {
            for x in 0..planned.scaled_width {
                let expected = UNIT.encode_unit(f32::from(sampler.rgb(x, y)[0]) / f32::from(u16::MAX));

                assert!(
                    (red(&tensor, canvas, x, y) - expected).abs() < 1e-6,
                    "the interior at ({x}, {y}) is not the resampled photograph"
                );
            }
        }

        // The margin: the mirror of the edge beside it, on each axis independently. Row 8 mirrors row 7, row 9
        // mirrors row 6, and so on — which is what keeps the model from reading a hard edge as picture content.
        for y in planned.scaled_height..canvas {
            let mirrored = 2 * planned.scaled_height - y - 1;

            for x in 0..planned.scaled_width {
                assert_eq!(
                    red(&tensor, canvas, x, y),
                    red(&tensor, canvas, x, mirrored),
                    "the extension at ({x}, {y}) is not the mirror of row {mirrored}"
                );
            }
        }
    }

    #[test]
    fn a_square_photograph_is_presented_with_no_margin_to_mirror() {
        // The spec's own scenario, checked on the tensor rather than on the plan: every row of the square is the
        // photograph, so there is no offset at which a mirror could have been written.
        let canvas = 8;
        let source = photograph(24, 24);
        let planned = plan(24, 24, canvas);

        let mut tensor = vec![0.0_f32; 3 * (canvas as usize) * (canvas as usize)];
        let resized = presented(&source, planned, UNIT, &mut tensor).expect("a square scratch is accepted");

        let sampler = Sampler::new(&resized);
        for y in 0..canvas {
            for x in 0..canvas {
                let expected = UNIT.encode_unit(f32::from(sampler.rgb(x, y)[0]) / f32::from(u16::MAX));

                assert!((red(&tensor, canvas, x, y) - expected).abs() < 1e-6, "({x}, {y}) is not the photograph");
            }
        }
    }

    #[test]
    fn the_range_the_tensor_is_written_in_is_whichever_one_the_caller_handed_over() {
        // The two ranges are a different photograph to a graph and neither is reportable, so a parameter silently
        // ignored would be a silently worse result for every family but the one whose value happened to be baked in.
        // Checked as the relation between the two tensors rather than against either of them alone, because
        // `encode_unit` is what both the function and an expectation written here would call.
        let canvas = 8;
        let source = photograph(20, 11);
        let planned = plan(20, 11, canvas);

        let mut unit = vec![0.0_f32; 3 * (canvas as usize) * (canvas as usize)];
        let mut signed = vec![0.0_f32; 3 * (canvas as usize) * (canvas as usize)];

        presented(&source, planned, Normalisation::Unit, &mut unit).expect("a square scratch");
        presented(&source, planned, Normalisation::Signed, &mut signed).expect("a square scratch");

        assert_ne!(unit, signed, "the normalisation passed in was ignored and one range was written twice");

        // And it is the range asked for rather than merely some other one: `Signed` is `Unit` mapped onto
        // `[-1, 1]`, so every value lands where that mapping puts it.
        for (index, (unit, signed)) in unit.iter().zip(signed.iter()).enumerate() {
            assert!((0.0..=1.0).contains(unit), "element {index} left the unit range");
            assert!(
                (signed - unit.mul_add(2.0, -1.0)).abs() < 1e-6,
                "element {index}: {signed} is not {unit} written in the signed range"
            );
        }
    }

    #[test]
    fn a_scratch_that_is_not_the_square_is_refused_rather_than_partly_written() {
        // The one failure the presentation can report, and it is the caller's scratch disagreeing with the canvas it
        // planned against rather than anything about the photograph.
        let canvas = 8;
        let mut tensor = vec![0.0_f32; 3 * 7 * 8];
        let error = presented(&photograph(20, 11), plan(20, 11, canvas), UNIT, &mut tensor)
            .expect_err("a short scratch was accepted");

        assert_eq!(error.expected, 3 * 8 * 8);
        assert_eq!(error.actual, 3 * 7 * 8);
        assert!(tensor.iter().all(|value| *value == 0.0), "a refused presentation wrote into the scratch");
    }

    #[test]
    #[should_panic(expected = "has no scale onto the square")]
    fn a_photograph_with_no_area_has_no_plan() {
        // The refusal belongs to the pipeline, which makes it before anything is allocated; this is the assertion
        // that keeps a caller that skipped it from silently planning a zero-width canvas.
        let _ = plan(0, 100, CANVAS);
    }
}
