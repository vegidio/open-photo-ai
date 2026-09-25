//! Rio's contract, and Rio's alone: one fixed square, one graph run, one global colour mapping fitted from what the
//! graph was shown to what it produced, and that mapping evaluated over the photograph's own pixels.

// Model tier rather than the family's, unlike `light_adjustment::process`, and the difference is what the two
// families' variants share. Paris and Lyon adjust through one contract unchanged. Rio and São Paulo do not: Rio's
// graph returns one corrected rendering and its pipeline answers *fit one mapping and evaluate it*; São Paulo's returns
// three weight planes and two renderings, and its pipeline answers *fit one mapping per rendering and blend them by a
// predicted weight map*. They differ at the graph's output shape and at everything after it, so a shared `process.rs`
// would carry the general name for one of two contracts and the first reader looking for São Paulo's would find Rio's
// under it. What they **do** share sits a tier up: `samples` and `mapping`.
//
// What a run is:
//
//   present     Lanczos3 to the planned size, then reflection-padded into the 656 square as planar CHW
//   run         one graph run: the square in, the square out
//   fit         the extension dropped from both tensors, and one 11-term polynomial fitted between them
//   apply       that mapping evaluated over the photograph's own pixels, at its own resolution
//   blend       the photograph moved towards — or away from — the corrected image by the run's bias
//
// **Two of those five are nobody's family.** The plan and the presentation are `imaging::present` and the blend is
// `imaging::mix`. The model's output is never what comes back; `mapping`'s header says why.

use std::sync::Arc;

use image::{DynamicImage, ImageBuffer, Rgb};

use super::super::ColorBalanceParams;
use super::super::mapping::{self, Mapping};
use super::super::samples::Samples;

use crate::error::InferenceError;
use crate::models::ArtifactId;
use crate::pipeline::Backend;
use crate::pipeline::session::GraphShape;
use crate::pipeline::{ImagePipeline, OnOneGraph, Shared, SingleGraph, checkpoint, reporter};
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;
use imaging::ChannelDepth;
use imaging::mix::blended;
use imaging::present::{plan, presented};
use imaging::tensor::{Channel, Normalisation, Sampler};

// Deep_White_Balance is trained on the unit range, against the `[-1, 1]` face recovery's restorers were trained on.
//
// Stated in this model's file rather than the family's because it is a property of the **graph**, and the two graphs
// this family publishes are two exports. They agree today, and the second one says so where it is written rather than
// by inheriting it.
/// The range this family's graphs read and write: `[0, 1]`.
pub(super) const RANGE: Normalisation = Normalisation::Unit;

// One step per stage, and what each costs:
//
//   1  presented    fixed cost: the square, whatever the photograph is
//   2  run          fixed cost: one graph run
//   3  fit          fixed cost: bounded by the canvas, not the photograph
//   4  apply        proportional to the photograph
//   5  blend        proportional to the photograph
//
// Counted rather than accumulated, so the last lands on exactly the end of the operation's share instead of on
// whatever a run of additions of a fifth drifted to. That is `light_adjustment::process`'s own schedule and
// `face_recovery::restore`'s, for the same reason.
//
// **Five rather than light adjustment's four because there is a real fifth stage**, and the boundaries double as the
// cancellation points. The reference's `0 / 0.9 / 1` is not carried: three constants describe no run when three of
// the five stages are a fixed cost and two scale with the photograph.
/// How many progress steps one run is: presented, run, fitted, applied, blended.
const STEPS: usize = 5;

/// The pipeline running one Rio colour balance operation, as the family's contract seam hands it back.
pub(crate) fn pipeline<B: Backend>(
    name: String,
    artifact: ArtifactId,
    profile: EpProfile,
    params: ColorBalanceParams,
    canvas: u32,
) -> Shared<B> {
    Arc::new(Balance::new(name, artifact, profile, params, canvas))
}

/// One Rio run: the graph it opens, the settings it opens it under, the square it runs at, and how far the
/// photograph is moved towards what the fit produced.
struct Balance {
    /// The graph this operation runs, and the name it reports a failure in.
    graph: SingleGraph,
    /// The square this variant's graph was exported at, asked of the variant rather than fixed here.
    canvas: u32,
    /// How far, and in which direction, the photograph is moved towards the corrected image.
    bias: f32,
}

impl Balance {
    fn new(name: String, artifact: ArtifactId, profile: EpProfile, params: ColorBalanceParams, canvas: u32) -> Self {
        Self { graph: SingleGraph::new(name, artifact, profile), canvas, bias: params.bias.as_f32() }
    }
}

impl OnOneGraph for Balance {
    fn graph(&self) -> &SingleGraph {
        &self.graph
    }
}

impl<B: Backend> ImagePipeline<B> for Balance {
    /// One graph run, whatever the photograph is.
    fn stages(&self) -> usize {
        // The fixed square is the whole of why this is a constant rather than something derived from the image: a
        // thumbnail and a 24-megapixel photograph are both one run.
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
        // Dispatched once at the top, as `Adjust::adjusted` and `restore` dispatch it. The apply below walks the
        // photograph's own resolution — twenty-four million iterations on a 24-megapixel source — and a per-pixel
        // branch on the depth there is the one place this pattern is load-bearing rather than stylistic. The fit
        // is `f32`/`f64` throughout and is not monomorphised.
        match depth {
            ChannelDepth::Eight => self.corrected::<u8, B>(input, sessions, progress, cancelled).map(u8::into_dynamic),
            ChannelDepth::Sixteen => {
                self.corrected::<u16, B>(input, sessions, progress, cancelled).map(u16::into_dynamic)
            }
        }
    }
}

impl Balance {
    /// [`ImagePipeline::run`]'s body, once, at whichever channel the caller asked for.
    fn corrected<T: Channel, B: Backend>(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImageBuffer<Rgb<T>, Vec<T>>, InferenceError>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        let (width, height) = (input.width(), input.height());

        // Before anything is allocated. There is no scaling of an empty photograph onto the square, no samples to
        // fit a mapping from, and a graph run over a square holding nothing but a reflection of nothing is not a
        // correction of anything.
        if width == 0 || height == 0 {
            return Err(InferenceError::Untileable { width, height });
        }

        let report = reporter(progress);
        let planned = plan(width, height, self.canvas);
        let shape = GraphShape::new(3, self.canvas as usize, self.canvas as usize);
        let crop = (planned.scaled_width, planned.scaled_height);

        // Checked at each of the five step boundaries. A cancelled run returns no image at all — not the
        // photograph, and not the partly corrected buffer the cancellation landed in, which a caller has no way to
        // tell from a finished correction.
        if cancelled() {
            return Err(InferenceError::Cancelled);
        }

        let mut tensor = vec![0.0_f32; shape.len()];
        // `expect` rather than a folded error, as `run_tiled` does with its own conversion and for the same reason:
        // the scratch is allocated here at exactly the shape the graph is run at, so a disagreement is this
        // function contradicting itself rather than anything a caller could have caused or acted on.
        //
        // The resampled photograph is **dropped**. It is what light adjustment's gain map needs as a denominator;
        // nothing here divides by what the model was shown, because the fit reads it back out of the tensor at the
        // graph's own range rather than as an image.
        presented(input, planned, RANGE, &mut tensor)
            .expect("the scratch is allocated at the square the graph accepts");

        checkpoint(&report, cancelled, 1.0 / STEPS as f64)?;

        let mut output = vec![0.0_f32; shape.len()];
        B::run_graph(&sessions[0], &tensor, shape, &mut output, shape)
            .map_err(InferenceError::run(&self.graph.name, 0))?;

        checkpoint(&report, cancelled, 2.0 / STEPS as f64)?;

        // Both views cropped to the plan, which is where the extension leaves this run: it must stay out of a
        // **global** fit, for the reason, and the 4.8 dB, that `samples` gives.
        //
        // Scoped so that both views and both tensors are gone before the full-resolution buffer is allocated: the
        // peak is then the photograph's buffers rather than the sum of them and 5 MB of square.
        let mapping = {
            let shown = Samples::new(&tensor, 0, self.canvas, crop, RANGE)
                .expect("the scratch is the square the graph was run at");
            let produced = Samples::new(&output, 0, self.canvas, crop, RANGE)
                .expect("the output is the square the graph was run at");

            mapping::fit(&self.graph.name, &shown, &[&produced])?
                .pop()
                .expect("one destination fits exactly one mapping")
        };

        drop(tensor);
        drop(output);

        checkpoint(&report, cancelled, 3.0 / STEPS as f64)?;

        let sampler = Sampler::new(input);
        let mapped = mapped::<T>(&sampler, &mapping, (width, height));

        checkpoint(&report, cancelled, 4.0 / STEPS as f64)?;

        // One extra pass over a buffer this run already owns, rather than the fused apply-and-blend the reference's
        // São Paulo `blendWeighted` does. Fusing them here would mean not calling `blended`, the cross-family blend
        // this run shares, and the fusion's payoff in the reference is avoiding three materialised full-resolution
        // renderings — a problem Rio, with one mapping, does not have.
        let blended = blended(&sampler, mapped, self.bias);

        report(1.0);

        Ok(blended)
    }
}

/// The photograph's own pixels, each evaluated through the fitted colour mapping.
fn mapped<T: Channel>(source: &Sampler<'_>, mapping: &Mapping, extent: (u32, u32)) -> ImageBuffer<Rgb<T>, Vec<T>>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // The reference carries two paths here — a fast one over a raw pixel buffer and a generic one through `image.At`.
    // **There is one path here, and that is not a divergence.** `Sampler` already is that fast path for every variant
    // the loader produces: the four the decoder yields are borrowed and indexed directly and everything else is
    // converted up to 16 bits once.
    let (width, height) = extent;

    ImageBuffer::from_fn(width, height, |x, y| {
        let [r, g, b] = source.rgb(x, y);

        // `from_unit` is what bounds each channel to the range it can carry. The polynomial is unconstrained — an
        // extrapolating fit can put a bright pixel above one or a dark one below zero — and without the bound those
        // would wrap to the opposite end of the channel, which is the worst-looking failure this arithmetic has.
        Rgb(mapping.evaluate([r.to_unit(), g.to_unit(), b.to_unit()]).map(T::from_unit))
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    use super::super::super::{ColorBalance, ColorBalanceVariant};

    use crate::error::SessionError;
    use crate::models::Bias;
    use crate::models::precision::FloatPrecision;
    use crate::pipeline::Model;
    use crate::pipeline::test_support::stub_backend_runs;
    use crate::providers::ExecutionProvider;
    use imaging::test_support::photograph;

    // The square is the variant's rather than the contract's, which is exactly what makes this substitutable: a test
    // that had to run at 656 would be five megabytes of scratch and a 430,336-sample fit per case.
    /// A canvas small enough for a test to run a whole pipeline over, in place of the 656 Rio ships at.
    const SMALL: u32 = 16;

    /// What the fake graph was asked to do, so a test can say what reached it rather than what came back.
    #[derive(Default)]
    struct Log {
        /// The two shapes each run declared, so a run at the wrong square is a failure rather than a silent resize.
        shapes: Vec<(GraphShape, GraphShape)>,
        /// How many times the graph was run, which for this contract must be once.
        runs: usize,
    }

    /// What a fake session answers a graph run with.
    #[derive(Clone, Copy)]
    enum Answer {
        /// Every channel scaled by its own factor and bounded, which is a colour cast a polynomial can represent
        /// exactly — so the fit's answer is a number a test can write down.
        Cast([f32; 3]),
        /// A transform that is **not** in the feature set: a scale, hard-clipped. What comes out of a least-squares
        /// fit against it depends on which samples were fitted, which is what makes the crop visible.
        Clipped(f32),
        /// A cast whose strength depends on the pixel's own brightness: each channel scaled by `low` where it is
        /// dark and by `high` where it is bright, interpolated by the channel's own value.
        ///
        /// `c * (low + (high - low) * c)` is `low·c + (high - low)·c²` — a term the kernel carries and a
        /// per-channel gain cannot express, which is what makes it the answer the richer-than-linear feature set
        /// is stated against. Monotone over `[0, 1]` and landing on `high` at 1, so a `high` of at most 1 never
        /// clips and the fit is exact rather than approximate.
        Graded { low: f32, high: f32 },
        /// Not a number, in every element.
        Nan,
    }

    /// A session standing in for Rio's graph: it records what it was given and answers with a known function of it.
    struct FakeSession {
        log: Arc<Mutex<Log>>,
        answer: Answer,
    }

    /// A backend with **no ONNX Runtime**.
    struct Fake;

    impl Backend for Fake {
        type Session = FakeSession;
        type Error = std::io::Error;

        async fn acquire(
            &self,
            _artifact: &ArtifactId,
            _profile: &EpProfile,
            _requested: ExecutionProvider,
            _interest: &crate::sessions::Interest,
        ) -> Result<SessionHandle<Self::Session>, SessionError> {
            unreachable!("the pipeline is handed its sessions; it never acquires one")
        }

        stub_backend_runs!("a colour balance run takes a declared-shape seam"; run_tile);

        /// The single-input call, which is the only one this family's graphs take.
        fn run_graph(
            handle: &SessionHandle<Self::Session>,
            input: &[f32],
            input_shape: GraphShape,
            output: &mut [f32],
            output_shape: GraphShape,
        ) -> Result<(), Self::Error> {
            let session = handle.session();

            {
                let mut log = session.log.lock().unwrap();
                log.shapes.push((input_shape, output_shape));
                log.runs += 1;
            }

            // Refused exactly as the real seam does: a buffer the declared shape does not describe is the mistake
            // it exists to prevent.
            assert_eq!(input.len(), input_shape.len(), "the graph was fed a buffer {input_shape} does not describe");
            assert_eq!(output.len(), output_shape.len(), "the output buffer is not {output_shape}");

            answer(session.answer, input, input_shape, output);

            Ok(())
        }

        stub_backend_runs!("Rio's graph takes one output"; run_named_outputs);
        stub_backend_runs!("Rio's graph takes no second input"; run_weighted);
    }

    /// `answer` applied to `input`, written into `output`. Shared by the fake backend and by the tests that
    /// reproduce what it did, so the two cannot drift apart.
    fn answer(answer: Answer, input: &[f32], shape: GraphShape, output: &mut [f32]) {
        let plane = shape.height * shape.width;

        match answer {
            Answer::Cast(factors) => {
                for (index, (value, source)) in output.iter_mut().zip(input).enumerate() {
                    *value = (source * factors[index / plane]).clamp(0.0, 1.0);
                }
            }
            Answer::Clipped(factor) => {
                for (value, source) in output.iter_mut().zip(input) {
                    *value = (source * factor).min(0.6);
                }
            }
            Answer::Graded { low, high } => {
                for (value, source) in output.iter_mut().zip(input) {
                    *value = (source * (low + (high - low) * source)).clamp(0.0, 1.0);
                }
            }
            Answer::Nan => output.fill(f32::NAN),
        }
    }

    /// A handle on a fake session answering with `answer`, and the log it records into.
    fn session(answer: Answer) -> (SessionHandle<FakeSession>, Arc<Mutex<Log>>) {
        let log: Arc<Mutex<Log>> = Arc::default();
        let handle = SessionHandle::held(FakeSession { log: Arc::clone(&log), answer }, ExecutionProvider::Cpu);

        (handle, log)
    }

    /// The pipeline at [`SMALL`], at `bias`.
    fn balancing(bias: f64) -> Shared<Fake> {
        // The name, the artifact and the profile come off a real operation rather than being written here, so what
        // these tests hold is what the family's seam hands over — the square alone is substituted, and it is the one
        // thing a variant is entitled to differ in.
        let bias = Bias::new(bias).expect("the test supplied a bias in range");
        let operation = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp32), bias);

        pipeline::<Fake>(operation.display_name(), operation.artifact(), operation.profile(), operation.params(), SMALL)
    }

    /// Runs `source` through the pipeline at `bias` against a session answering with `answer`.
    fn run(source: &DynamicImage, bias: f64, answer: Answer, depth: ChannelDepth) -> DynamicImage {
        let (handle, _) = session(answer);

        balancing(bias)
            .run(source, std::slice::from_ref(&handle), depth, None, &|| false)
            .expect("a colour balance over a photograph")
    }

    #[test]
    fn the_graph_is_fed_the_unit_range_rather_than_the_signed_one_face_recovery_uses() {
        // The literal, pinned rather than left to be read off a call. The two ranges differ in nothing a runtime
        // can report — a graph trained on `[0, 1]` and fed `[-1, 1]` loads, runs and returns a worse photograph —
        // and this family sits two directories from one that genuinely takes the other.
        assert_eq!(RANGE, Normalisation::Unit, "Rio's graph was switched to the signed range");

        // And the property behind the literal: a presented photograph lands inside `[0, 1]`, where `Signed` would
        // have put half of it below zero — and the fit's features are products of those values, so the sign
        // reaching them is a different polynomial rather than a shifted one.
        let mut tensor = vec![0.0_f32; 3 * (SMALL as usize) * (SMALL as usize)];
        presented(&photograph(20, 11), plan(20, 11, SMALL), RANGE, &mut tensor).expect("a square scratch");

        assert!(
            tensor.iter().all(|value| (0.0..=1.0).contains(value)),
            "a presented photograph left the range the graph was trained on"
        );
    }

    #[test]
    fn one_run_asks_for_one_artifact_under_its_own_profile() {
        // The `Model` half of the contract, which is what the driver reads before anything is installed: one
        // artifact, opened under the tuning measured for the graph it names. A pipeline that had answered with the
        // family's default would still correct the right photograph and cost 3% more on an M2 Max.
        let operation = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), Bias::clamped(0.5));
        let built = pipeline::<Fake>(
            operation.display_name(),
            operation.artifact(),
            operation.profile(),
            operation.params(),
            SMALL,
        );

        assert_eq!(
            Model::<Fake>::required(built.as_ref()).iter().map(ArtifactId::as_str).collect::<Vec<_>>(),
            vec!["cb_rio_fp16"],
            "a Rio run asked for something other than its own one graph"
        );

        let sessions = Model::<Fake>::sessions(built.as_ref());
        assert_eq!(sessions.len(), 1, "a single-graph contract asked for {} sessions", sessions.len());
        assert_eq!(sessions[0].0.as_str(), "cb_rio_fp16");
        assert_eq!(*sessions[0].1, operation.profile(), "the session would be opened under another model's tuning");
    }

    #[test]
    fn one_run_is_one_graph_run_at_the_variants_own_square() {
        // The contract's whole shape, and the two halves a caller cannot see: the graph is run once whatever the
        // photograph is, and it is run at the square the variant declares rather than at anything derived from the
        // image. A pipeline that had passed the photograph's own dimensions would still produce a picture here.
        let (handle, log) = session(Answer::Cast([1.0; 3]));

        let produced = balancing(1.0)
            .run(&photograph(53, 31), std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("a colour balance over a photograph");

        assert_eq!((produced.width(), produced.height()), (53, 31), "the result is not the photograph's own size");

        let log = log.lock().unwrap();
        assert_eq!(log.runs, 1, "a fixed-canvas contract ran the graph {} times", log.runs);
        assert_eq!(
            log.shapes[0],
            (
                GraphShape::new(3, SMALL as usize, SMALL as usize),
                GraphShape::new(3, SMALL as usize, SMALL as usize)
            ),
            "the graph was not run at the variant's square"
        );

        // And `stages` agrees with what actually happened, which is what the driver's per-step record reports.
        assert_eq!(ImagePipeline::<Fake>::stages(balancing(1.0).as_ref()), log.runs);
    }

    #[test]
    fn an_image_with_no_area_is_refused_before_anything_is_allocated() {
        // The same refusal the tiling of a zero-area image makes, for the same reason, plus one this family adds:
        // there are no samples to fit a mapping from. Checked on both axes, because a guard on one of them would
        // let the other reach the plan's own assertion as a panic.
        for (width, height) in [(0, 8), (8, 0), (0, 0)] {
            let (handle, log) = session(Answer::Cast([1.0; 3]));
            let empty = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));

            let outcome =
                balancing(1.0).run(&empty, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false);

            assert!(
                matches!(outcome, Err(InferenceError::Untileable { .. })),
                "a {width}x{height} image produced {outcome:?}"
            );
            assert_eq!(log.lock().unwrap().runs, 0, "a refused run reached the graph");
        }
    }

    #[test]
    fn a_graph_that_scales_each_channel_is_recovered_as_that_scaling() {
        // What the fit **is**, read off the result rather than assumed. A per-channel cast is exactly inside the
        // feature set, so the least-squares answer is that cast and the corrected photograph is the photograph
        // times those factors. A fit run in the wrong direction — a plausible mistake, and a photograph rather
        // than an error — lands on the reciprocals and fails here.
        let factors = [0.8_f32, 1.0, 1.25];
        let source = photograph(40, 24);

        let produced = run(&source, 1.0, Answer::Cast(factors), ChannelDepth::Eight);
        let (original, produced) = (Sampler::new(&source), Sampler::new(&produced));

        for y in 0..24 {
            for x in 0..40 {
                let before = original.rgb(x, y);
                let after = produced.rgb(x, y);

                for channel in 0..3 {
                    let expected = (before[channel].to_unit() * factors[channel]).clamp(0.0, 1.0);

                    assert!(
                        (after[channel].to_unit() - expected).abs() < 0.02,
                        "({x}, {y}) channel {channel}: {} against the {expected} the cast implies",
                        after[channel].to_unit()
                    );
                }
            }
        }
    }

    #[test]
    fn a_cast_that_varies_with_brightness_corrects_a_dark_pixel_differently_from_a_bright_one() {
        // The spec's own scenario, and what the eleven terms are **for**. Every other fake answer in this file is a
        // per-channel scale — which is precisely what a three-parameter model could fit — so without this case the
        // richer-than-linear feature set is pinned only as a literal in `mapping`, and a solver narrowed to a
        // per-channel gain would pass every other test here.
        //
        // The graph grades each channel by its own value: `c * (LOW + (HIGH - LOW)·c)`, which is `LOW·c` plus
        // `(HIGH - LOW)·c²` — both terms the kernel carries, so the fit recovers the function rather than
        // approximating it. The correction it implies is a *ratio* that runs from LOW on black to HIGH on white.
        const LOW: f32 = 0.6;
        const HIGH: f32 = 1.0;

        // A hue per column and a brightness per row, so the two pixels compared below differ in brightness alone
        // while the fit still sees a spread wide enough to determine eleven terms.
        let hue = |x: u32| {
            let across = x as f32 / 63.0;
            [0.30 + 0.60 * across, 0.80 - 0.50 * across, 0.25 + 0.45 * (1.0 - across)]
        };
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 64, |x, y| {
            let level = (y as f32 + 1.0) / 64.0;
            Rgb(hue(x).map(|channel| u8::from_unit(channel * level)))
        }));

        let produced = run(&source, 1.0, Answer::Graded { low: LOW, high: HIGH }, ChannelDepth::Eight);
        let (original, produced) = (Sampler::new(&source), produced.as_rgb8().expect("an eight-bit run"));

        // One column, so the hue is held fixed and only the brightness moves. A dark row and a bright one, both
        // far enough from black that the ratio below is not dominated by the 8-bit quantisation of the source.
        let column = 20;
        let ratio_at = |row: u32| {
            let [r, _, _] = original.rgb(column, row);
            let before = r.to_unit();
            let after = produced.get_pixel(column, row).0[0].to_unit();

            (before, after / before)
        };

        let (dark, dark_ratio) = ratio_at(15);
        let (bright, bright_ratio) = ratio_at(63);

        // What the graph's own function implies at each of those two brightnesses, which is what the fit has to
        // have recovered for the result to be this.
        let implied = |channel: f32| LOW + (HIGH - LOW) * channel;

        for (level, ratio, row) in [(dark, dark_ratio, 15), (bright, bright_ratio, 63)] {
            assert!(
                (ratio - implied(level)).abs() < 0.05,
                "row {row}: red was corrected by {ratio} against the {} a brightness-dependent cast implies",
                implied(level)
            );
        }

        // And the point of the scenario: the same hue is corrected differently at the two brightnesses. A fit
        // narrowed to a per-channel gain lands on one ratio for both and fails here whichever one it picked.
        assert!(
            bright_ratio - dark_ratio > 0.08,
            "a dark pixel and a bright one of one hue were corrected alike: {dark_ratio} against {bright_ratio}"
        );
    }

    #[test]
    fn the_fit_is_taken_over_the_photograph_alone_and_not_over_the_extension_beside_it() {
        // **The single highest-value assertion in this file**, because the crop is worth 4.8 dB of median (see
        // `samples`) and shows up nowhere else.
        //
        // It has to be stated against a graph whose answer is **not** in the feature set. A cast, or the identity,
        // is recovered exactly whichever samples are fitted — adding mirrored ones changes nothing — so a run
        // through either comes back the same with the crop or without it, and a test built on one would pass over a
        // reader that never cropped. A clipped scale is not representable, so the least-squares answer genuinely
        // depends on which samples reached it.
        let source = photograph(37, 15);
        let answered = Answer::Clipped(1.6);
        let planned = plan(37, 15, SMALL);
        let shape = GraphShape::new(3, SMALL as usize, SMALL as usize);
        let crop = (planned.scaled_width, planned.scaled_height);

        assert_ne!(crop, (SMALL, SMALL), "the fixture stopped needing an extension at all");

        // The same presentation and the same graph the run makes, reproduced here so the two mappings below differ
        // in nothing but which samples they were fitted over.
        let mut tensor = vec![0.0_f32; shape.len()];
        presented(&source, planned, RANGE, &mut tensor).expect("a square scratch");
        let mut output = vec![0.0_f32; shape.len()];
        answer(answered, &tensor, shape, &mut output);

        let fitted = |extent| {
            let shown = Samples::new(&tensor, 0, SMALL, extent, RANGE).expect("a square");
            let produced = Samples::new(&output, 0, SMALL, extent, RANGE).expect("a square");

            mapping::fit("Rio (FP32)", &shown, &[&produced])
                .expect("a determined system")
                .pop()
                .expect("one mapping")
        };

        let (cropped, whole) = (fitted(crop), fitted((SMALL, SMALL)));
        assert_ne!(cropped, whole, "the fixture stopped separating a cropped fit from a polluted one");

        // And the run's own answer is the cropped one, pixel for pixel.
        let produced = run(&source, 1.0, answered, ChannelDepth::Eight);
        let sampler = Sampler::new(&source);
        let produced = produced.as_rgb8().expect("an eight-bit run");

        let mut separated = 0_usize;
        for (x, y, pixel) in produced.enumerate_pixels() {
            let [r, g, b] = sampler.rgb(x, y);
            let unit = [r, g, b].map(Channel::to_unit);

            assert_eq!(
                pixel.0,
                cropped.evaluate(unit).map(u8::from_unit),
                "({x}, {y}) is not what the cropped fit produces"
            );

            if pixel.0 != whole.evaluate(unit).map(u8::from_unit) {
                separated += 1;
            }
        }

        // Counted rather than asserted per pixel: the two mappings are different functions, but they still agree
        // wherever both of them clip to the same end of the channel, and a run of black pixels in the fixture is
        // not evidence of anything. What is being stated is that the polluted fit is visible in the **picture** —
        // which is the whole of what the 4.8 dB is — rather than only in the weights compared above.
        let pixels = produced.width() * produced.height();
        assert!(
            separated * 2 > pixels as usize,
            "only {separated} of {pixels} pixels tell the cropped fit from the polluted one"
        );
    }

    #[test]
    fn a_graph_that_returns_what_it_was_shown_returns_the_photograph() {
        // The identity the whole arrangement rests on: when the model changes nothing, the fitted mapping is the
        // identity and what comes back is the photograph's own pixels. A pipeline that had upsampled the model's
        // output instead of fitting a transform returns a 16-pixel-wide blur here.
        //
        // Deliberately **not** the crop's test — see above. An identity is recovered exactly from any set of
        // samples, so this passes with or without the extension in the fit; what it does catch is the extension
        // reaching the *result*, which is a different mistake and a visible one.
        let source = photograph(37, 15);

        let produced = run(&source, 1.0, Answer::Cast([1.0; 3]), ChannelDepth::Eight);
        let (original, produced) = (Sampler::new(&source), produced.as_rgb8().expect("an eight-bit run").clone());

        for (x, y, pixel) in produced.enumerate_pixels() {
            let [r, g, b] = original.rgb(x, y);
            let expected = [r, g, b].map(|value| u8::from_unit(value.to_unit()));

            for (channel, want) in expected.into_iter().enumerate() {
                let (got, want) = (i32::from(pixel.0[channel]), i32::from(want));

                assert!((got - want).abs() <= 1, "({x}, {y}) channel {channel}: {got} against the photograph's {want}");
            }
        }
    }

    #[test]
    fn detail_finer_than_the_square_survives_the_correction() {
        // The requirement the fixed square is affordable under, and the one that separates this arrangement from
        // returning the model's output enlarged. A one-pixel checkerboard is entirely lost by the downscale onto
        // the square, so a result carrying it can only have come from the photograph's own pixels.
        let checker = DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 64, |x, y| {
            Rgb(if (x + y) % 2 == 0 { [230_u8, 200, 180] } else { [40, 60, 70] })
        }));

        let produced = run(&checker, 1.0, Answer::Cast([1.1, 1.0, 0.9]), ChannelDepth::Eight);
        let produced = produced.as_rgb8().expect("an eight-bit run");

        let mut spread = 0_i32;
        for y in 0..64 {
            for x in 0..63 {
                let here = i32::from(produced.get_pixel(x, y).0[0]);
                let next = i32::from(produced.get_pixel(x + 1, y).0[0]);

                spread = spread.max((here - next).abs());
            }
        }

        assert!(spread > 100, "the checkerboard was flattened to a spread of {spread} levels");
    }

    #[test]
    fn a_run_produces_the_channel_depth_that_was_asked_for() {
        // The depth dispatch, which is the divergence from the reference this pipeline is built around: it forces
        // every intermediate to 8-bit RGBA, so a 16-bit photograph would have its polynomial evaluated over 256
        // levels and that answer written into a 16-bit result.
        let source = DynamicImage::ImageRgb16(ImageBuffer::from_fn(40, 25, |x, y| {
            Rgb([(x * 700 + y * 11) as u16, (x * 13 + y * 900) as u16, 30_000])
        }));

        for (depth, sixteen) in [(ChannelDepth::Eight, false), (ChannelDepth::Sixteen, true)] {
            let produced = run(&source, 1.0, Answer::Cast([1.1, 0.95, 1.0]), depth);

            assert_eq!((produced.width(), produced.height()), (40, 25));
            assert_eq!(produced.as_rgb16().is_some(), sixteen, "{depth:?} did not produce the depth asked for");
            assert_eq!(produced.as_rgb8().is_some(), !sixteen, "{depth:?} did not produce the depth asked for");

            // And colour only: alpha is not preserved through a model run, and a fourth channel would claim the
            // model returned an opacity it was never given.
            assert!(produced.as_rgba8().is_none() && produced.as_rgba16().is_none(), "{depth:?} carried an alpha");
        }
    }

    #[test]
    fn a_sixteen_bit_run_carries_more_than_an_eight_bit_channel_could_have_held() {
        // The claim behind the dispatch, on the input that shows it: a gradient finer than 256 levels comes back
        // finer than 256 levels. An 8-bit answer widened into a 16-bit result would land on 256 of them exactly.
        let gradient = DynamicImage::ImageRgb16(ImageBuffer::from_fn(600, 1, |x, _| Rgb([(x as u16) * 100; 3])));

        let produced = run(&gradient, 1.0, Answer::Cast([1.0, 1.0, 1.0]), ChannelDepth::Sixteen);
        let produced = produced.as_rgb16().expect("a sixteen-bit run");
        let levels: std::collections::BTreeSet<u16> = produced.pixels().map(|pixel| pixel.0[0]).collect();

        assert!(levels.len() > 256, "a 16-bit result carried only {} distinct levels", levels.len());
    }

    #[test]
    fn a_photograph_carrying_transparency_comes_back_as_colour_alone() {
        // The spec's own scenario, on the one input variant that has an alpha to lose.
        let translucent = DynamicImage::ImageRgba8(ImageBuffer::from_fn(20, 20, |x, y| {
            image::Rgba([(x * 9) as u8, (y * 9) as u8, 120, 128])
        }));

        let produced = run(&translucent, 1.0, Answer::Cast([1.0; 3]), ChannelDepth::Eight);

        assert!(produced.as_rgb8().is_some(), "a translucent photograph did not come back as colour alone");
    }

    #[test]
    fn a_bias_of_zero_returns_the_photograph_byte_for_byte() {
        // The spec's own scenario, through the whole pipeline: the presentation, the graph, the crop, the fit and
        // the apply all run and are then multiplied by nothing. Byte for byte rather than within a level, because
        // the blend's own endpoint is exact — anything that reached the pixels by another route shows here.
        let source = photograph(30, 22);

        let produced = run(&source, 0.0, Answer::Cast([0.6, 1.3, 1.1]), ChannelDepth::Eight);
        let original = Sampler::new(&source);
        let produced = produced.as_rgb8().expect("an eight-bit run");

        for (x, y, pixel) in produced.enumerate_pixels() {
            let [r, g, b] = original.rgb(x, y);

            assert_eq!(
                pixel.0,
                [r, g, b].map(|value| u8::from_unit(value.to_unit())),
                "a bias of zero changed ({x}, {y})"
            );
        }
    }

    #[test]
    fn two_biases_of_opposite_sign_move_the_colour_in_opposite_directions() {
        // The spec's own scenario, through the whole pipeline rather than the blend alone. Mean red-minus-blue,
        // because what this family moves is a cast rather than a level, and the graph here warms the picture.
        let source = photograph(30, 22);

        let warmth = |image: &DynamicImage| {
            let sampler = Sampler::new(image);
            let total: f64 = (0..22)
                .flat_map(|y| (0..30).map(move |x| (x, y)))
                .map(|(x, y)| {
                    let [r, _, b] = sampler.rgb(x, y);
                    f64::from(r) - f64::from(b)
                })
                .sum();

            total / (30.0 * 22.0)
        };

        let cast = Answer::Cast([1.3, 1.0, 0.75]);
        let up = run(&source, 1.0, cast, ChannelDepth::Eight);
        let down = run(&source, -1.0, cast, ChannelDepth::Eight);

        let (before, up, down) = (warmth(&source), warmth(&up), warmth(&down));

        assert!(up > before, "a positive bias did not warm the picture: {up} against {before}");
        assert!(down < before, "a negative bias did not cool it: {down} against {before}");
    }

    #[test]
    fn progress_reaches_exactly_the_end_of_the_range_and_never_goes_backwards() {
        // Counted rather than accumulated, which is what makes the last report exactly the end rather than whatever
        // five additions of a fifth drifted to — and the property the spec states, that the report advances during
        // the full-resolution work rather than only around the graph.
        let (handle, _) = session(Answer::Cast([1.2, 1.0, 0.9]));
        let reported: Mutex<Vec<f64>> = Mutex::default();

        balancing(0.5)
            .run(
                &photograph(40, 60),
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                Some(&|fraction| reported.lock().unwrap().push(fraction)),
                &|| false,
            )
            .expect("a colour balance over a photograph");

        let reported = reported.lock().unwrap();

        assert_eq!(reported.len(), STEPS, "a five-step run reported {} times", reported.len());
        assert!(reported.windows(2).all(|pair| pair[1] > pair[0]), "progress went backwards: {reported:?}");
        assert!(reported[0] > 0.0, "the first report was at the start of the range rather than after a step");
        assert_eq!(*reported.last().expect("a run reports"), 1.0, "the last report was not the end of the range");

        // More than one report lands **after** the graph has run, which is the half of the schedule the reference's
        // `0 / 0.9 / 1` does not have: on a large photograph the apply and the blend are most of the run.
        assert!(reported.iter().filter(|fraction| **fraction > 0.4).count() >= 3, "{reported:?}");
    }

    #[test]
    fn a_cancellation_at_any_of_the_five_boundaries_produces_no_image() {
        // Every boundary, separately, rather than one of them: a caller handed a partly corrected photograph with
        // no error has no way to tell it from a finished correction, and the later boundaries are the ones where
        // such a buffer exists to be handed back.
        for boundary in 0..STEPS {
            let (handle, _) = session(Answer::Cast([1.1, 1.0, 0.9]));
            let checked = std::sync::atomic::AtomicUsize::new(0);

            let outcome = balancing(1.0).run(
                &photograph(33, 21),
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                None,
                &|| checked.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= boundary,
            );

            assert!(
                matches!(outcome, Err(InferenceError::Cancelled)),
                "a cancellation at boundary {boundary} produced {outcome:?}"
            );
        }

        // And the run that is never cancelled still returns a photograph, so the loop above is checking a refusal
        // rather than a pipeline that fails whatever it is told.
        let (handle, _) = session(Answer::Cast([1.1, 1.0, 0.9]));
        assert!(
            balancing(1.0)
                .run(&photograph(33, 21), std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .is_ok()
        );
    }

    #[test]
    fn a_graph_that_answers_with_nothing_that_is_a_number_does_not_reach_the_singular_refusal() {
        // Recorded as the property rather than assumed either way, because it is the one place the refusal's
        // reachability could be misread. `XᵀX` is built from the **source** alone, so nothing the graph returns can
        // make a pivot zero: a NaN output lands entirely in `XᵀY`, the elimination finds its pivots as usual, and
        // the weights come out as NaN.
        //
        // So `InferenceError::ColourMapping` is not reachable from here, and the refusal is exercised where it can
        // be — against `mapping::eliminate`, with the ridge removed, which is the only way to build the system the
        // ridge exists to make impossible.
        //
        // What a NaN graph renders is black, because `f32::NAN as u8` saturates to zero. That is a bad picture from
        // a broken model rather than something this pipeline can diagnose, and it is stated here so nobody reads
        // the refusal's absence as an oversight.
        let produced = run(&photograph(20, 12), 1.0, Answer::Nan, ChannelDepth::Eight);
        let produced = produced.as_rgb8().expect("an eight-bit run");

        assert_eq!((produced.width(), produced.height()), (20, 12));
        assert!(
            produced.pixels().all(|pixel| pixel.0 == [0; 3]),
            "a NaN mapping rendered something other than black"
        );
    }
}
