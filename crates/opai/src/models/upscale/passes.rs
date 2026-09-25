//! The convolutional upscale pipeline: native passes back to back, and one correcting resample at the end.

use image::DynamicImage;
use image::imageops::FilterType;

use super::conv::PassShape;
use crate::error::InferenceError;
use crate::models::{ArtifactId, Pass, Scale};
use crate::pipeline::Backend;
use crate::pipeline::{ImagePipeline, Model, reporter};
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;
use imaging::{ChannelDepth, RunTile, TilingError, run_tiled};

/// What one convolutional upscale operation runs: its passes, the scale they may overshoot, and the one profile they
/// share.
///
/// Built by [`Upscale::pipeline`](crate::models::Upscale::pipeline), the family's contract seam. The chain driver holds
/// it as an [`ImagePipeline`] and never learns which of the three models it is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PassSequence {
    /// The operation as a user would see it, for naming it in the failure this pipeline folds its own error into.
    name: String,
    /// The scale that was requested, which the passes may overshoot and which the correction resamples back to.
    requested: Scale,
    // Asked of the operation rather than counted off the passes: a repeated pass is one artifact served twice — Tokyo
    // at 8x is two passes of one set of weights.
    /// The distinct artifacts this operation needs on disk, in the order it first needs them.
    required: Vec<ArtifactId>,
    // One entry per pass rather than per artifact, because a repeated pass takes a second handle on the one entry.
    // Held as the `Pass` the caller already had rather than split into two index-aligned lists, which is one fewer
    // correspondence to state in prose and keep true.
    /// Each pass, in the order they run: its native scale and the artifact serving it.
    passes: Vec<Pass>,
    // **One** profile rather than one per artifact, and that is the substance of the difference from the diffusion
    // contract rather than a shortcut: the passes genuinely share it, because Kyoto's declaration was measured across
    // both its 2x and 4x graphs and an 8x request carries it through each pass.
    /// The execution-provider tuning every session for this operation is built with.
    profile: EpProfile,
    /// The tile shape the model accepts and the range it was trained against, carried from its row.
    shape: PassShape,
}

impl PassSequence {
    /// The pipeline running `passes` for the operation named `name`, correcting back to `requested`, with every
    /// session built under `profile` and `required` on disk first.
    pub(crate) fn new(
        name: String,
        passes: Vec<Pass>,
        requested: Scale,
        required: Vec<ArtifactId>,
        profile: EpProfile,
        shape: PassShape,
    ) -> Self {
        Self { name, requested, required, passes, profile, shape }
    }
}

impl<B: Backend> Model<B> for PassSequence {
    fn required(&self) -> &[ArtifactId] {
        &self.required
    }

    /// Each pass's artifact against the operation's **one** profile.
    fn sessions(&self) -> Vec<(&ArtifactId, &EpProfile)> {
        self.passes.iter().map(|pass| (&pass.artifact, &self.profile)).collect()
    }
}

impl<B: Backend> ImagePipeline<B> for PassSequence {
    fn stages(&self) -> usize {
        self.passes.len()
    }

    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        depth: ChannelDepth,
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DynamicImage, InferenceError> {
        // By position, which is what `sessions` above acquired them as: one handle per pass, in pass order, so a
        // repeated pass reaches its own handle on the one cache entry.
        let mut tile = |pass: usize, input: &[f32], output: &mut [f32]| B::run_tile(&sessions[pass], input, output);

        // Collected once per operation, against a run that costs seconds — `run_passes` walks the scales alone and
        // has no use for the artifacts beside them.
        let scales: Vec<u8> = self.passes.iter().map(|pass| pass.scale).collect();

        let plan = PassPlan { scales: &scales, requested: self.requested, shape: self.shape };

        run_passes(input, plan, depth, progress, cancelled, &mut tile)
            .map_err(|error| InferenceError::from_tiling(&self.name, error))
    }
}

// The pass index is what distinguishes this from the driver's own `RunTile`: each pass runs a different set of weights,
// so the caller needs to know which session to reach for — and a test needs to know which pass it is standing in for,
// which is what makes the whole pipeline exercisable against a tile function that is arithmetic.
/// The per-tile step a pass sequence is given: which pass it belongs to, the tile already converted and padded, and
/// the buffer the model's output is to be written into.
pub(crate) type RunPass<'a, E> = dyn FnMut(usize, &[f32], &mut [f32]) -> Result<(), E> + 'a;

/// What one pass-sequence run covers, and the shape it runs at.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PassPlan<'a> {
    // One argument rather than three because they travel together: a scale list is meaningless without the request it
    // may overshoot, and neither says anything without the tensor shape the weights are frozen at.
    /// The native scale of each pass, in the order they run.
    pub(crate) scales: &'a [u8],
    /// The scale that was requested, which the passes may overshoot and which the correction resamples back to.
    pub(crate) requested: Scale,
    /// The tile geometry and input range the model declares, from its row.
    pub(crate) shape: PassShape,
}

/// Runs `scales` in order over `source` and corrects any overshoot once, at the end.
///
/// Each pass takes the previous pass's result, and **nothing is resampled between passes**.
///
/// `progress` is reported over the whole operation, `0.0..=1.0` from an initial `0.0`, with the passes weighted by
/// input pixel area — see [`PassWeights`].
///
/// # Errors
///
/// Whatever the driver reports, on the pass that reported it: [`TilingError::Untileable`] for an image with no area,
/// [`TilingError::Cancelled`] where the caller stopped it, and [`TilingError::Tile`] where the model failed. A failed
/// or cancelled run returns **no image at all**.
///
/// # Panics
///
/// Panics on an empty `scales`, which the caller refuses before anything is installed.
pub(crate) fn run_passes<E>(
    source: &DynamicImage,
    plan: PassPlan<'_>,
    depth: ChannelDepth,
    progress: Option<&dyn Fn(f64)>,
    cancelled: &dyn Fn() -> bool,
    run_tile: &mut RunPass<'_, E>,
) -> Result<DynamicImage, TilingError<E>> {
    let PassPlan { scales, requested, shape } = plan;
    let PassShape { tiles, range } = shape;

    assert!(!scales.is_empty(), "a pass sequence covering no scale is refused before a run starts");

    let (width, height) = (source.width(), source.height());
    let weights = PassWeights::new(width, height, scales);

    // This function's to emit, because the driver's `0.0` is per pass and a multi-pass run must not return to the
    // start at each of them.
    reporter(progress)(0.0);

    // Nothing is resampled between passes, and that is the decision rather than an implementation detail: a model's
    // input being the previous model's output is what a multi-pass upscale *is*, and a resize in between would hand the
    // next pass an interpolation to restore rather than a photograph — visibly softer, for no reason a user could see.
    //
    // The source is borrowed for the first pass and each later pass reads the one before it, so the previous pass's
    // image is dropped as soon as the next has produced its own — which at 8x is the difference between holding two
    // results and three.
    let mut result: Option<DynamicImage> = None;

    for (index, &scale) in scales.iter().enumerate() {
        let input = result.as_ref().unwrap_or(source);
        let (base, span) = weights.share(index);

        // Composed here rather than inside the driver, which reports its own `0.0..=1.0` per pass and knows nothing
        // about how many passes there are.
        let pass_progress = progress.map(|report| move |fraction: f64| report(base + fraction * span));
        let pass_progress = pass_progress.as_ref().map(|report| report as &dyn Fn(f64));

        let mut tile = |input: &[f32], output: &mut [f32]| run_tile(index, input, output);

        let produced = run_tiled(
            input,
            tiles,
            u32::from(scale),
            depth,
            range,
            pass_progress,
            cancelled,
            &mut tile as &mut RunTile<'_, E>,
        )?;

        // At most two per operation — the longest pass sequence this project resolves is two passes, as 8x is 4x
        // then 2x — so this is the deepest a record goes and it is `debug`. **Nothing below here records anything**,
        // which is the bound the log's size actually rests on: `run_tiled` is per tile and its grid is per region, and
        // a 12,000 × 8,000 photograph is hundreds of tiles per pass.
        tracing::debug!(
            pass = index + 1,
            passes = scales.len(),
            scale,
            width = produced.width(),
            height = produced.height(),
            "upscale pass finished"
        );

        result = Some(produced);
    }

    let produced = result.expect("a non-empty pass sequence produces an image");

    Ok(correct_overshoot(produced, width, height, requested))
}

/// Resamples `produced` to exactly the requested scale, where the passes overshot it, keeping the depth the run
/// produced.
///
/// The final dimensions are the *input's* multiplied by the requested scale and rounded, whether or not any pass
/// overshot. Where `produced` already has them it is returned untouched.
fn correct_overshoot(produced: DynamicImage, width: u32, height: u32, requested: Scale) -> DynamicImage {
    // The reference implementation's arithmetic exactly, and `Lanczos3` below matches the filter it resamples with.
    let (intended_width, intended_height) = requested.applied_to(width, height);

    // A 4x request served by the 4x weights is not passed through an identity transform, which at 24 megapixels is
    // seconds of work producing the image it was given.
    if produced.width() == intended_width && produced.height() == intended_height {
        return produced;
    }

    // `DynamicImage::resize_exact` rather than `imageops::resize` over the image directly. The latter would go through
    // `DynamicImage`'s `GenericImageView`, which is typed `Rgba<u8>` — so correcting a 1.5x request would be where a
    // 16-bit result the caller asked for silently became an 8-bit one, on the last line of the pipeline. This
    // dispatches per variant and keeps the depth the run produced.
    produced.resize_exact(intended_width, intended_height, FilterType::Lanczos3)
}

// Here rather than beside the chain's progress reporting, because the pass sequence is the only thing that has passes
// to weight: a family-tier file's arithmetic, used by nothing outside it.
/// How one operation's passes divide the run half of its range.
///
/// **Weighted by input pixel area**, so an 8x run's 4x pass — which covers a sixteenth of the pixels its 2x successor
/// does — takes a proportionate share rather than half of it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PassWeights {
    /// Each pass's `(base, span)` within `0.0..=1.0`, in pass order.
    shares: Vec<(f64, f64)>,
}

impl PassWeights {
    /// The shares of a run over `width` x `height` whose passes scale by `scales` in turn.
    pub(crate) fn new(width: u32, height: u32, scales: &[u8]) -> Self {
        let mut areas = Vec::with_capacity(scales.len());
        let (mut w, mut h) = (f64::from(width), f64::from(height));

        for &scale in scales {
            // The *input* area rather than the output's, because the input's area is what decides the tile count, and
            // the tile count is what the driver reports against.
            areas.push(w * h);
            w *= f64::from(scale);
            h *= f64::from(scale);
        }

        let total: f64 = areas.iter().sum();
        let mut shares = Vec::with_capacity(areas.len());
        let mut done = 0.0;

        for area in areas {
            // An image with no area is the driver's own refusal, and a zero total here would be a division producing
            // NaN reports on the way to it. Equal shares instead, which is what a caller sees for the moment before
            // the refusal arrives.
            let span = if total > 0.0 { area / total } else { 1.0 / scales.len().max(1) as f64 };
            shares.push((done, span));
            done += span;
        }

        Self { shares }
    }

    /// Pass `index`'s `(base, span)` of the operation's run range.
    pub(crate) fn share(&self, index: usize) -> (f64, f64) {
        self.shares.get(index).copied().unwrap_or((0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use image::GenericImageView as _;

    use super::*;

    use imaging::TileGeometry;
    use imaging::tensor::Normalisation;

    /// The shape the three shipped convolutional rows all declare, which is what these tests drive the pass driver
    /// at: they exercise the sequencing and the correction, not any one model's declaration.
    const DEFAULT_SHAPE: PassShape = PassShape { tiles: TileGeometry::DEFAULT, range: Normalisation::Unit };
    use imaging::test_support::{gradient, nearest_neighbour};

    /// What one run of the pipeline did: which pass each tile belonged to.
    #[derive(Debug, Default)]
    struct Runs {
        /// The pass index of every tile run, in order.
        tiles: Vec<usize>,
    }

    impl Runs {
        /// How many passes actually ran, counted from the tiles rather than from what was asked for.
        fn passes(&self) -> usize {
            let mut seen: Vec<usize> = Vec::new();

            for &index in &self.tiles {
                if !seen.contains(&index) {
                    seen.push(index);
                }
            }

            seen.len()
        }
    }

    /// Runs the pipeline over `source` at `requested`, covered by `scales`, against a tile function that enlarges each
    /// pixel into a block — the arithmetic an upscaler's shape does, without any of its detail.
    fn run(source: &DynamicImage, scales: &[u8], requested: f64) -> (DynamicImage, Runs) {
        let mut runs = Runs::default();

        let result = {
            let runs = &mut runs;
            let mut tile = |pass: usize, input: &[f32], output: &mut [f32]| -> Result<(), Infallible> {
                runs.tiles.push(pass);
                nearest_neighbour(input, output);

                Ok(())
            };

            run_passes(
                source,
                PassPlan { scales, requested: Scale::new(requested).unwrap(), shape: DEFAULT_SHAPE },
                ChannelDepth::Eight,
                None,
                &|| false,
                &mut tile,
            )
            .unwrap()
        };

        (result, runs)
    }

    #[test]
    fn a_two_times_request_runs_one_pass_and_is_not_resampled() {
        let source = gradient(300, 200);
        let (result, runs) = run(&source, &[2], 2.0);

        assert_eq!(runs.passes(), 1);
        assert_eq!(result.dimensions(), (600, 400));
    }

    #[test]
    fn a_four_times_request_runs_one_pass_and_is_not_resampled() {
        // The property that makes this more than a dimension check: a nearest-neighbour tile function produces blocks
        // of identical pixels, and a Lanczos resample through the same dimensions would not preserve them. So an
        // unchanged block is the check that no identity resample ran.
        let source = gradient(300, 200);
        let (result, runs) = run(&source, &[4], 4.0);

        assert_eq!(runs.passes(), 1);
        assert_eq!(result.dimensions(), (1200, 800));

        let pixels = result.to_rgb8();
        assert_eq!(pixels.get_pixel(0, 0), pixels.get_pixel(1, 1), "a 4x request was resampled after its pass");
    }

    #[test]
    fn an_eight_times_request_runs_two_passes_each_over_the_one_before_it() {
        let source = gradient(300, 200);
        let (result, runs) = run(&source, &[4, 2], 8.0);

        assert_eq!(runs.passes(), 2, "an 8x request did not run two passes");
        assert_eq!(result.dimensions(), (2400, 1600));
    }

    #[test]
    fn each_pass_is_given_the_dimensions_the_pass_before_it_produced() {
        // Counted through the tiles: at 256/16 the tile count is a function of the input's dimensions, so a second
        // pass handed the *source* rather than the first pass's result would run the first pass's tile count again.
        let source = gradient(300, 200);
        let (_, runs) = run(&source, &[4, 2], 8.0);

        let first = runs.tiles.iter().filter(|&&pass| pass == 0).count();
        let second = runs.tiles.iter().filter(|&&pass| pass == 1).count();

        // 300x200 is two tiles by one; 1200x800 is five by four.
        assert_eq!(first, 2, "the first pass did not run over the source");
        assert_eq!(second, 20, "the second pass did not run over the first pass's 1200x800 result");
    }

    #[test]
    fn a_scale_no_whole_factor_serves_is_resampled_exactly_once_after_the_last_pass() {
        // A 1.5x request served by the 2x weights. The pass overshoots to 2x and one resample brings it back.
        let source = gradient(300, 200);
        let (result, runs) = run(&source, &[2], 1.5);

        assert_eq!(runs.passes(), 1, "the correction ran a second pass instead of a resample");
        assert_eq!(result.dimensions(), (450, 300), "the result is not the request's own dimensions");
    }

    #[test]
    fn the_final_dimensions_round_the_requested_scale_rather_than_truncating_it() {
        // 301 * 1.5 is 451.5, which rounds to 452 — the reference's arithmetic, and the half that a truncation gets
        // wrong by a pixel on every odd dimension.
        let source = gradient(301, 201);
        let (result, _) = run(&source, &[2], 1.5);

        assert_eq!(result.dimensions(), (452, 302));
    }

    #[test]
    fn the_passes_report_one_range_that_never_returns_to_the_start() {
        let source = gradient(300, 200);
        let reported = std::cell::RefCell::new(Vec::new());

        let mut tile = |_pass: usize, _input: &[f32], output: &mut [f32]| -> Result<(), Infallible> {
            output.fill(0.0);
            Ok(())
        };

        run_passes(
            &source,
            PassPlan { scales: &[4, 2], requested: Scale::new(8.0).unwrap(), shape: DEFAULT_SHAPE },
            ChannelDepth::Eight,
            Some(&|fraction| reported.borrow_mut().push(fraction)),
            &|| false,
            &mut tile,
        )
        .unwrap();

        let reported = reported.into_inner();
        assert_eq!(reported.first().copied(), Some(0.0), "the run did not open on zero");
        assert_eq!(reported.last().copied(), Some(1.0), "the run did not land on exactly one");
        for pair in reported.windows(2) {
            assert!(pair[1] >= pair[0], "the passes reported backwards: {} then {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn a_failing_pass_produces_no_image_at_all() {
        let source = gradient(300, 200);

        let mut tile =
            |_pass: usize, _input: &[f32], _output: &mut [f32]| Err(std::io::Error::other("the model failed"));

        let outcome = run_passes(
            &source,
            PassPlan { scales: &[4, 2], requested: Scale::new(8.0).unwrap(), shape: DEFAULT_SHAPE },
            ChannelDepth::Eight,
            None,
            &|| false,
            &mut tile,
        );

        assert!(matches!(outcome, Err(TilingError::Tile { index: 0, .. })), "a failed pass returned {outcome:?}");
    }

    #[test]
    fn a_sixteen_bit_request_keeps_its_depth_through_the_correcting_resample() {
        // The resample is the last thing that touches the pixels, and going through `DynamicImage`'s own
        // `GenericImageView` there would narrow a 16-bit result to `Rgba<u8>` — silently, on a run the caller
        // explicitly asked for sixteen bits.
        let source = gradient(300, 200);
        let mut tile = |_pass: usize, input: &[f32], output: &mut [f32]| -> Result<(), Infallible> {
            nearest_neighbour(input, output);
            Ok(())
        };

        let result = run_passes(
            &source,
            PassPlan { scales: &[2], requested: Scale::new(1.5).unwrap(), shape: DEFAULT_SHAPE },
            ChannelDepth::Sixteen,
            None,
            &|| false,
            &mut tile,
        )
        .unwrap();

        assert!(
            matches!(result, DynamicImage::ImageRgb16(_)),
            "the correction narrowed the result to {result:?}"
        );
    }

    #[test]
    fn a_large_image_and_a_small_one_produce_the_same_number_of_records() {
        // **The bound the level discipline exists for**, pinned where it can actually be checked: `run_passes` and
        // everything under it — `run_tiled`, the grid, the tile loop — run synchronously on this thread, so a record
        // added anywhere in them is visible here. The chain drives this on a blocking thread, where a thread-bound
        // test subscriber would not see one and the test would pass while the file filled up.
        //
        // 64x64 is one tile at the default geometry; 900x600 is several, so the two differ in tile count by an order
        // of magnitude and in pixel count by a hundred.
        let count = |width, height| {
            let (log, ()) = crate::logging::records_of_blocking("debug", || {
                let mut tile = |_: usize, input: &[f32], output: &mut [f32]| -> Result<(), Infallible> {
                    nearest_neighbour(input, output);
                    Ok(())
                };

                run_passes(
                    &gradient(width, height),
                    PassPlan { scales: &[2], requested: Scale::new(2.0).unwrap(), shape: DEFAULT_SHAPE },
                    ChannelDepth::Eight,
                    None,
                    &|| false,
                    &mut tile,
                )
                .unwrap();
            });

            (log.lines().filter(|line| line.contains("msg=")).count(), log)
        };

        let (small, small_log) = count(64, 64);
        let (large, large_log) = count(900, 600);

        assert_eq!(
            small, large,
            "the number of records grew with the size of the image:\n--- small ---\n{small_log}\n--- large ---\n{large_log}"
        );

        // One per pass, so the equality above is not two empty files — and it is below the default level.
        assert_eq!(large, 1, "a one-pass upscale wrote {large} records:\n{large_log}");
        assert!(large_log.contains("level=DEBUG"), "{large_log}");
        assert!(large_log.contains("msg=\"upscale pass finished\""), "{large_log}");
        assert!(large_log.contains("width=1800") && large_log.contains("height=1200"), "{large_log}");
    }

    #[test]
    fn a_multi_pass_run_records_one_line_per_pass_and_numbers_them() {
        let (log, ()) = crate::logging::records_of_blocking("debug", || {
            let mut tile = |_: usize, input: &[f32], output: &mut [f32]| -> Result<(), Infallible> {
                nearest_neighbour(input, output);
                Ok(())
            };

            run_passes(
                &gradient(64, 64),
                PassPlan { scales: &[4, 2], requested: Scale::new(8.0).unwrap(), shape: DEFAULT_SHAPE },
                ChannelDepth::Eight,
                None,
                &|| false,
                &mut tile,
            )
            .unwrap();
        });

        let passes: Vec<&str> = log.lines().filter(|line| line.contains("upscale pass finished")).collect();

        assert_eq!(passes.len(), 2, "{log}");
        assert!(passes[0].contains("pass=1") && passes[0].contains("passes=2"), "{}", passes[0]);
        assert!(passes[1].contains("pass=2") && passes[1].contains("passes=2"), "{}", passes[1]);
        // The dimensions each pass produced, which is what makes the sequence readable: 64 → 256 → 512.
        assert!(passes[0].contains("width=256"), "{}", passes[0]);
        assert!(passes[1].contains("width=512"), "{}", passes[1]);
    }

    #[test]
    fn passes_are_weighted_by_the_pixels_they_cover_rather_than_equally() {
        // An 8x Kyoto run: a 4x pass over the source, then a 2x pass over sixteen times as many pixels. Equal shares
        // would stall the bar at the halfway mark for the whole of the second pass.
        let weights = PassWeights::new(100, 100, &[4, 2]);

        let (first_base, first_span) = weights.share(0);
        let (second_base, second_span) = weights.share(1);

        assert_eq!(first_base, 0.0);
        assert!((first_span - 1.0 / 17.0).abs() < 1e-9, "the 4x pass took {first_span} of the range");
        assert!((second_base - 1.0 / 17.0).abs() < 1e-9, "the 2x pass did not start where the 4x one finished");
        assert!((second_span - 16.0 / 17.0).abs() < 1e-9, "the 2x pass took {second_span} of the range");
        assert!((second_base + second_span - 1.0).abs() < 1e-9, "the passes do not fill the range");
    }

    #[test]
    fn a_single_pass_owns_the_whole_range() {
        let weights = PassWeights::new(640, 480, &[4]);

        assert_eq!(weights.share(0), (0.0, 1.0));
    }
}
