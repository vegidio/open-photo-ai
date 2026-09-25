//! The contract every image filter runs: the photograph through a tile grid at its own resolution, then moved towards
//! the result by the run's strength.

// Cross-family, because denoise and sharpen both run it unchanged: the reference's `sharpen/process.go` and
// `sharpen/variant.go` are its denoise files line for line with the prefix changed. What stays with each family is its
// range, which of its variants are guarded, and the measured profiles. What stays with each model is its guard
// threshold and the prose behind it.
//
// What a run is:
//
//   tile        the whole photograph through `run_tiled` at the default 256/16 geometry, scale 1, in the caller's range
//   guard       per tile, where the variant has one: a raw output past the threshold keeps the tile's own input
//   blend       the photograph moved towards — or past — the tiled result by the run's strength
//
// **Only the guard is this file's own.** The grid is `imaging`'s and the blend is `imaging::mix`. The reference's
// `RunPipeline` then `BlendWithIntensity`, in that order.
//
// The range and the strength arrive from the caller rather than as one family's `RANGE` and `*Params`: both families
// read `[0, 1]` because each of the reference's two `process.go` files passes `standardize: false`, and a shared file
// that hard-coded it would be right for two families by coincidence.

use std::sync::Arc;

use image::DynamicImage;

use crate::error::InferenceError;
use crate::models::ArtifactId;
use crate::models::strength::Strength;
use crate::pipeline::Backend;
use crate::pipeline::{ImagePipeline, OnOneGraph, Shared, SingleGraph, reporter};
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;
use imaging::mix::blended;
use imaging::tensor::{Channel, Normalisation, Sampler};
use imaging::{ChannelDepth, RunTile, TileGeometry, run_tiled};

// A fixed share rather than one fitted to a measured split, for the reason `light_adjustment::process::STEPS` gives:
// a constant correct for one image size on one provider is a number nobody can re-derive from the code. The blend is
// one full-resolution loop against hundreds of model runs, so the tiles take nearly all of it; what the share buys is
// that the end of the range is reported only once the result exists, rather than on the last tile with the blend
// still to run behind a bar that already says finished — which is what the reference does.
/// How much of the operation's progress range the tile grid reports across. The blend reports the rest.
const TILES: f64 = 0.95;

/// Whether a tile's raw model output has blown up: any value whose magnitude is **strictly** past `threshold`.
///
/// Strict, as the reference's comparison is, so a tile whose largest value is exactly the threshold is kept.
fn diverged(output: &[f32], threshold: f32) -> bool {
    // Read off the raw output, before the decode: `from_unit` clamps every value into the channel, so a check made
    // against the decoded pixels could not tell a blow-up from a legitimately bright tile.
    output.iter().any(|value| value.abs() > threshold)
}

// Test-only, and read by each family's live check to report whether its guard fired on a real photograph. A static
// rather than a field because a live check goes through `Opai::process`, which builds its pipeline where the test
// cannot reach it. Shared by every family that runs this file, and the fake-backend suite below trips the guard too,
// which is why each live check is `#[ignore]`d, run alone, and counts only what its own runs added.
/// How many tiles the guard has kept since the process started.
#[cfg(test)]
pub(crate) static GUARDED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The pipeline running one filter operation, as a family's contract seam hands it back.
pub(crate) fn pipeline<B: Backend>(
    name: String,
    artifact: ArtifactId,
    profile: EpProfile,
    range: Normalisation,
    strength: Strength,
    guard: Option<f32>,
) -> Shared<B> {
    Arc::new(Filter::new(name, artifact, profile, range, strength, guard))
}

/// One filter run: the graph it opens, the settings it opens it under, the range it feeds it, the guard its model
/// needs, and how much of the result lands.
struct Filter {
    /// The graph this operation runs, and the name it reports a failure in.
    graph: SingleGraph,
    /// The range the graph reads and writes, as its family states it.
    range: Normalisation,
    // An `Option` rather than the reference's zero-means-off `float32`, so "no guard" is not a sentinel a later reader
    // has to know about.
    /// The magnitude past which a tile's raw output is discarded, for the models that need it.
    guard: Option<f32>,
    /// How far the photograph is moved towards the tiled result: past it above 1.
    strength: f32,
}

impl Filter {
    fn new(
        name: String,
        artifact: ArtifactId,
        profile: EpProfile,
        range: Normalisation,
        strength: Strength,
        guard: Option<f32>,
    ) -> Self {
        Self { graph: SingleGraph::new(name, artifact, profile), range, guard, strength: strength.as_f32() }
    }
}

impl OnOneGraph for Filter {
    fn graph(&self) -> &SingleGraph {
        &self.graph
    }
}

impl<B: Backend> ImagePipeline<B> for Filter {
    /// One graph, run over a tile grid.
    fn stages(&self) -> usize {
        1
    }

    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        depth: ChannelDepth,
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DynamicImage, InferenceError> {
        let report = reporter(progress);

        // The driver's to leave to its caller, so that a multi-pass run does not return to the start at each pass.
        report(0.0);

        let tile_progress = progress.map(|report| move |fraction: f64| report(fraction * TILES));
        let tile_progress = tile_progress.as_ref().map(|report| report as &dyn Fn(f64));

        let session = &sessions[0];
        let guard = self.guard;

        // The guard lives in the per-tile step rather than in `run_tiled`, which takes the step as a parameter so a
        // caller can do exactly this. At scale 1 the tile the model was shown and the tensor it returned have one
        // shape, so "keep this tile's input" is a copy — and the driver then decodes and blends it like any other,
        // so a kept tile meets its neighbours through the same ramp and at the run's own depth. The reference keeps
        // an 8-bit copy of the tile instead, which quantises a guarded region inside a 16-bit result.
        let mut tile = |input: &[f32], output: &mut [f32]| -> Result<(), B::Error> {
            B::run_tile(session, input, output)?;

            if let Some(threshold) = guard
                && diverged(output, threshold)
            {
                output.copy_from_slice(input);

                #[cfg(test)]
                GUARDED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }

            Ok(())
        };

        let produced = run_tiled(
            input,
            TileGeometry::DEFAULT,
            1,
            depth,
            self.range,
            tile_progress,
            cancelled,
            &mut tile as &mut RunTile<'_, B::Error>,
        )
        .map_err(|error| InferenceError::from_tiling(&self.graph.name, error))?;

        // Once more after the last tile: the blend is a full-resolution loop, and a run cancelled during the grid's
        // last tile would otherwise hand back a finished photograph the caller had already asked it to drop.
        if cancelled() {
            return Err(InferenceError::Cancelled);
        }

        // Dispatched once rather than per pixel. The driver's result is already at the depth it was asked for, so
        // `into_rgb8` and `into_rgb16` hand its buffer over rather than converting it, and `blended` works in place
        // on it — no third full-resolution buffer. Not skipped at a strength of 1: the loop returns the tiled result
        // exactly there already, and one pass over the pixels is nothing beside the grid that produced them.
        let sampler = Sampler::new(input);
        let result = match depth {
            ChannelDepth::Eight => u8::into_dynamic(blended(&sampler, produced.into_rgb8(), self.strength)),
            ChannelDepth::Sixteen => u16::into_dynamic(blended(&sampler, produced.into_rgb16(), self.strength)),
        };

        report(1.0);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_at_the_threshold_is_not_a_blow_up() {
        // Strict, as the reference's `v > t || v < -t` is, on both signs.
        assert!(!diverged(&[3.0], 3.0));
        assert!(!diverged(&[-3.0], 3.0));
        assert!(!diverged(&[0.5, 3.0, -3.0, 1.0], 3.0));
    }

    #[test]
    fn a_value_past_the_threshold_is_a_blow_up_on_either_side_of_zero() {
        assert!(diverged(&[3.0001], 3.0));
        assert!(diverged(&[-3.0001], 3.0));
    }

    #[test]
    fn one_exploding_value_among_thousands_of_ordinary_ones_is_enough() {
        // What a blow-up looks like in practice: most of the tile is fine and one region goes to four figures.
        let mut output = vec![0.5_f32; 3 * 256 * 256];
        assert!(!diverged(&output, 3.0), "an ordinary tile read as a blow-up");

        output[123_456] = 1200.0;
        assert!(diverged(&output, 3.0), "one exploded value was missed");
    }

    #[test]
    fn an_empty_output_has_not_diverged() {
        assert!(!diverged(&[], 3.0));
    }

    // The fake-backend suite: the whole pipeline — the tile grid at scale 1, the guard, the blend, the progress
    // schedule, the depth dispatch, the cancellation and the refusal — with no ONNX Runtime, no GPU, no model file and
    // no network, which is what keeps every property above checked on every CI platform.
    //
    // Built here directly rather than through a family's seam: a shared module's tests do not reach up into one family
    // to construct what they test. Which family hands which guard to which model is each family's to prove.

    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use image::{ImageBuffer, Rgb};

    use crate::pipeline::test_support::{DARKENS, Fake, FakeSession, PAIR, darkened, original, session};

    use crate::models::artifact::Family;
    use crate::models::precision::Precision;
    use imaging::test_support::photograph;

    /// The guard every guarded model this project ships runs at.
    const THRESHOLD: f32 = 3.0;

    /// A filter over `range` at `strength`, guarded at `guard`.
    fn filtering_in(range: Normalisation, strength: f64, guard: Option<f32>) -> Shared<Fake> {
        let strength = Strength::new(strength).expect("the test supplied a strength in range");
        let artifact = ArtifactId::new(Family::Denoise, "fake", None, Precision::Fp32);

        pipeline::<Fake>("Fake (FP32)".into(), artifact, EpProfile::default(), range, strength, guard)
    }

    /// A filter over `[0, 1]` at `strength`, guarded at `guard`.
    fn filtering(strength: f64, guard: Option<f32>) -> Shared<Fake> {
        filtering_in(Normalisation::Unit, strength, guard)
    }

    /// A filter whose model is guarded.
    fn guarded(strength: f64) -> Shared<Fake> {
        filtering(strength, Some(THRESHOLD))
    }

    /// Runs `pipeline` over `source` with nothing watching and nothing cancelling it.
    fn run(
        pipeline: &Shared<Fake>,
        source: &DynamicImage,
        handle: &SessionHandle<FakeSession>,
        depth: ChannelDepth,
    ) -> DynamicImage {
        pipeline
            .run(source, std::slice::from_ref(handle), depth, None, &|| false)
            .expect("a filter over a photograph")
    }

    #[test]
    fn a_photograph_of_many_tiles_comes_back_at_its_own_size() {
        // 600x400 at 256/16 is three columns by two rows, which is also what the progress test below relies on.
        let handle = session(None);
        let produced = run(&guarded(1.0), &photograph(600, 400), &handle, ChannelDepth::Eight);

        assert_eq!((produced.width(), produced.height()), (600, 400));
        assert_eq!(handle.session().runs.load(Ordering::Relaxed), 6, "the grid was not the six tiles expected");
    }

    #[test]
    fn a_photograph_smaller_than_one_tile_comes_back_at_its_own_size_rather_than_the_tiles() {
        let handle = session(None);
        let produced = run(&guarded(1.0), &photograph(40, 17), &handle, ChannelDepth::Eight);

        assert_eq!((produced.width(), produced.height()), (40, 17));
    }

    #[test]
    fn a_strength_of_zero_returns_the_photograph_exactly() {
        let source = photograph(300, 280);
        let handle = session(None);
        let produced = run(&guarded(0.0), &source, &handle, ChannelDepth::Eight);

        let sampler = Sampler::new(&source);
        for (x, y, pixel) in produced.as_rgb8().expect("an eight-bit run").enumerate_pixels() {
            assert_eq!(pixel.0, original::<u8>(&sampler, x, y), "a strength of zero changed ({x}, {y})");
        }
    }

    #[test]
    fn a_strength_of_one_returns_the_models_output_exactly() {
        // Exact rather than within a level, which is what the refusal of a fast path at 1 rests on: the blend loop
        // already lands on the model's own output there.
        let source = photograph(300, 280);
        let handle = session(None);
        let produced = run(&guarded(1.0), &source, &handle, ChannelDepth::Eight);

        let sampler = Sampler::new(&source);
        for (x, y, pixel) in produced.as_rgb8().expect("an eight-bit run").enumerate_pixels() {
            assert_eq!(pixel.0, darkened::<u8>(&sampler, x, y), "a strength of one moved ({x}, {y}) off the output");
        }
    }

    #[test]
    fn a_strength_past_one_moves_further_along_the_same_line_and_stays_in_range() {
        // The model darkens, so the result at 2 must be darker than at 1 wherever the photograph had anything to
        // darken, and never lighter. `u8` cannot leave its range, so what "in range" rules out is a wrap: a dark pixel
        // pushed below zero coming back as a bright one, which would read as lighter than the run at 1.
        let source = photograph(300, 280);
        let (once, twice) = (session(None), session(None));

        let one = run(&guarded(1.0), &source, &once, ChannelDepth::Eight);
        let two = run(&guarded(2.0), &source, &twice, ChannelDepth::Eight);

        let (one, two) = (one.as_rgb8().expect("eight-bit"), two.as_rgb8().expect("eight-bit"));
        let sampler = Sampler::new(&source);
        let mut moved_further = 0;

        for (x, y, at_two) in two.enumerate_pixels() {
            let at_one = one.get_pixel(x, y);
            let before = original::<u8>(&sampler, x, y);

            for (channel, before) in before.into_iter().enumerate() {
                assert!(at_two.0[channel] <= at_one.0[channel], "({x}, {y}) moved the other way past a strength of 1");

                let (near, far) = (
                    i32::from(before) - i32::from(at_one.0[channel]),
                    i32::from(before) - i32::from(at_two.0[channel]),
                );
                assert!(far >= near, "({x}, {y}) moved less at 2 than at 1");
                moved_further += usize::from(far > near);
            }
        }

        assert!(moved_further > 0, "a strength of 2 produced exactly what 1 did");
    }

    #[test]
    fn a_tile_the_guard_trips_on_keeps_the_photographs_own_pixels_and_the_rest_are_the_models() {
        // The mechanism test, at both depths. A 16-bit source whose values are not multiples of 257 is what shows the
        // kept pixels are the photograph's at the run's own depth rather than an 8-bit copy of them.
        let eight = photograph(PAIR.0, PAIR.1);
        let sixteen = DynamicImage::ImageRgb16(ImageBuffer::from_fn(PAIR.0, PAIR.1, |x, y| {
            Rgb([(x * 131 + y * 7) as u16, (x * 3 + y * 311) as u16, 40_001])
        }));

        for (source, depth) in [(&eight, ChannelDepth::Eight), (&sixteen, ChannelDepth::Sixteen)] {
            let handle = session(Some((0, 4.0)));
            let produced = run(&guarded(1.0), source, &handle, depth);
            let sampler = Sampler::new(source);

            assert_eq!((produced.width(), produced.height()), PAIR, "the guarded run did not complete at full size");

            for y in 0..PAIR.1 {
                for x in (0..224).chain(256..PAIR.0) {
                    let kept = x < 224;
                    let (got, want) = match depth {
                        ChannelDepth::Eight => {
                            let got = produced.as_rgb8().expect("eight-bit").get_pixel(x, y).0.map(u16::from);
                            let want =
                                if kept { original::<u8>(&sampler, x, y) } else { darkened::<u8>(&sampler, x, y) };

                            (got, want.map(u16::from))
                        }
                        ChannelDepth::Sixteen => {
                            let got = produced.as_rgb16().expect("sixteen-bit").get_pixel(x, y).0;
                            let want =
                                if kept { original::<u16>(&sampler, x, y) } else { darkened::<u16>(&sampler, x, y) };

                            (got, want)
                        }
                    };

                    assert_eq!(
                        got,
                        want,
                        "{depth:?} ({x}, {y}): the {} tile's pixels are wrong",
                        ["second", "guarded"][usize::from(kept)]
                    );
                }
            }
        }
    }

    #[test]
    fn a_tile_whose_largest_value_is_exactly_the_threshold_is_the_models() {
        let source = photograph(PAIR.0, PAIR.1);
        let handle = session(Some((0, 3.0)));
        let produced = run(&guarded(1.0), &source, &handle, ChannelDepth::Eight);

        let sampler = Sampler::new(&source);
        let produced = produced.as_rgb8().expect("eight-bit");

        // Anywhere in the first tile but the one pixel the fake set to 3.0, which decodes to white rather than to the
        // darkened photograph.
        for (x, y) in [(0, 0), (10, 150), (200, 30)] {
            assert_eq!(produced.get_pixel(x, y).0, darkened::<u8>(&sampler, x, y), "({x}, {y}) was kept");
        }
    }

    #[test]
    fn an_unguarded_model_passes_an_exploding_tile_through() {
        // The same backend that trips the guard, and nothing is kept: every region is the model's.
        let source = photograph(PAIR.0, PAIR.1);
        let sampler = Sampler::new(&source);

        let handle = session(Some((0, 4.0)));
        let produced = run(&filtering(1.0, None), &source, &handle, ChannelDepth::Eight);
        let produced = produced.as_rgb8().expect("eight-bit");

        for (x, y) in [(0, 0), (10, 150), (200, 30), (400, 100)] {
            assert_eq!(
                produced.get_pixel(x, y).0,
                darkened::<u8>(&sampler, x, y),
                "the unguarded model kept ({x}, {y}) rather than using the model's output"
            );
        }
    }

    #[test]
    fn the_range_the_caller_hands_over_is_the_one_the_graph_is_fed() {
        // The range is the caller's, so a filter that had gone back to hard-coding one would pass every test above
        // and feed a signed-range family the unit range. The fake halves what it is shown, and halving is not the
        // same operation in the two: under `[-1, 1]` a value is halved about the middle of the channel, not about
        // black, so the two runs differ at every pixel that is not mid-grey.
        let source = photograph(300, 280);
        let sampler = Sampler::new(&source);

        for range in [Normalisation::Unit, Normalisation::Signed] {
            let produced = run(&filtering_in(range, 1.0, None), &source, &session(None), ChannelDepth::Eight);
            let produced = produced.as_rgb8().expect("eight-bit");

            for (x, y, pixel) in produced.enumerate_pixels() {
                let want = sampler
                    .rgb(x, y)
                    .map(|value| u8::from_unit(range.decode(range.encode_unit(value.to_unit()) * DARKENS)));

                assert_eq!(pixel.0, want, "{range:?} ({x}, {y}) was not fed the range it was handed");
            }
        }
    }

    #[test]
    fn progress_opens_on_zero_never_goes_backwards_and_reaches_one_only_after_the_blend() {
        let handle = session(None);
        let reported: Mutex<Vec<f64>> = Mutex::default();

        guarded(0.5)
            .run(
                &photograph(600, 400),
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                Some(&|fraction| reported.lock().unwrap().push(fraction)),
                &|| false,
            )
            .expect("a filter over a photograph");

        let reported = reported.into_inner().unwrap();

        // The opening zero, six tiles, and the end.
        assert_eq!(reported.len(), 8, "{reported:?}");
        assert_eq!(reported[0], 0.0, "the run did not open on zero");
        assert!(reported.windows(2).all(|pair| pair[1] >= pair[0]), "progress went backwards: {reported:?}");
        assert_eq!(*reported.last().expect("a run reports"), 1.0, "the last report was not the end of the range");

        // Every report before the last is short of the end, so the end cannot have been reached on a tile.
        assert!(reported[..reported.len() - 1].iter().all(|fraction| *fraction <= TILES), "{reported:?}");
    }

    #[test]
    fn a_cancellation_between_tiles_or_before_the_blend_produces_no_image() {
        // 600x400 is six tiles, so the driver asks six times and the pipeline asks a seventh just before the blend.
        // A cancellation at the second question lands between tiles; at the seventh, after every tile has run.
        for (boundary, tiles_run) in [(2, 2), (6, 6)] {
            let handle = session(None);
            let asked = AtomicUsize::new(0);
            let reported: Mutex<Vec<f64>> = Mutex::default();

            let outcome = guarded(1.0).run(
                &photograph(600, 400),
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                Some(&|fraction| reported.lock().unwrap().push(fraction)),
                &|| asked.fetch_add(1, Ordering::Relaxed) >= boundary,
            );

            assert!(matches!(outcome, Err(InferenceError::Cancelled)), "at {boundary}: {outcome:?}");
            assert_eq!(handle.session().runs.load(Ordering::Relaxed), tiles_run, "at {boundary}");

            // And the bar never claimed a finished run it then dropped.
            assert!(!reported.into_inner().unwrap().contains(&1.0), "a cancelled run reported the end at {boundary}");
        }
    }

    #[test]
    fn an_image_with_no_area_is_refused() {
        for (width, height) in [(0, 8), (8, 0), (0, 0)] {
            let handle = session(None);
            let empty = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));

            let outcome = guarded(1.0).run(&empty, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false);

            assert!(
                matches!(outcome, Err(InferenceError::Untileable { .. })),
                "a {width}x{height} image produced {outcome:?}"
            );
            assert_eq!(handle.session().runs.load(Ordering::Relaxed), 0, "a refused run reached the graph");
        }
    }

    #[test]
    fn a_run_produces_the_channel_depth_that_was_asked_for() {
        for (depth, sixteen) in [(ChannelDepth::Eight, false), (ChannelDepth::Sixteen, true)] {
            let handle = session(None);
            let produced = run(&guarded(1.5), &photograph(300, 90), &handle, depth);

            assert_eq!(produced.as_rgb16().is_some(), sixteen, "{depth:?} did not produce the depth asked for");
            assert_eq!(produced.as_rgb8().is_some(), !sixteen, "{depth:?} did not produce the depth asked for");
        }
    }

    #[test]
    fn the_pipeline_is_one_stage() {
        assert_eq!(ImagePipeline::<Fake>::stages(guarded(1.0).as_ref()), 1);
    }
}
