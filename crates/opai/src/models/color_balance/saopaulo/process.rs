//! São Paulo's contract, and São Paulo's alone: one fixed square, one graph run answering three weight planes and
//! two synthesized renderings, one mapping fitted per rendering, and a per-pixel blend of three illuminants
//! evaluated over the photograph's own pixels.

// Model tier rather than the family's, for the reason `rio::process` gives: Rio's pipeline answers *fit one mapping
// and evaluate it*; this one answers *fit one mapping per rendering and blend them by a predicted weight map*. What the
// two **do** share sits a tier up: `samples` and `mapping`.
//
// What a run is:
//
//   present     Lanczos3 to the planned size, then reflection-padded into the 656 square as planar CHW
//   run         one graph run: three planes in, nine out
//   fit         the extension dropped from both tensors, and one 11-term polynomial fitted per rendering
//   blend       the three illuminants weighted per pixel by the upsampled map, at the photograph's resolution
//   bias        the photograph moved towards — or away from — that blend by the run's bias
//
// Two of those five are nobody's family: the plan and the presentation are `imaging::present` and the bias is
// `imaging::mix`.
//
// The blend is one fused pass. At 12 megapixels each rendering would be a ~144 MB float buffer, and there is nothing
// to do with them afterwards but multiply and add — which is what the loop already does. The reference measures the
// fusion at 1008 ms and 672 MB down to 56 ms and 48 MB, where the 48 MB is the output image: the working set becomes
// the result rather than three intermediates plus it.
//
// What this run does **not** do: the reference splits the fused pass across cores in row bands. That is not carried,
// and it changes no pixel: the rows are strictly independent and each pixel's arithmetic is unchanged, so it is a
// wall-clock property alone — roughly 0.5-1 s on a 24-megapixel photograph. What kept it out was the tier rule: the
// only row split was Osaka's, at model tier. It has since been promoted to `imaging::rows::for_each_row` for
// colorization's compose, so nothing now stands between this pass and the reference's banding but a change of its own.
// That remains an open parity gap: the promotion was a move that left São Paulo's output untouched, and banding this
// pass is a separate decision.

use std::sync::Arc;

use image::{DynamicImage, ImageBuffer, Rgb};

use super::super::ColorBalanceParams;
use super::super::mapping::{self, Mapping};
use super::super::samples::Samples;
use super::{CHANNELS, RANGE, RENDERINGS, SETTINGS, rendering_offset};

use crate::error::InferenceError;
use crate::models::ArtifactId;
use crate::pipeline::Backend;
use crate::pipeline::session::GraphShape;
use crate::pipeline::{ImagePipeline, OnOneGraph, Shared, SingleGraph, checkpoint, reporter};
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;
use imaging::ChannelDepth;
use imaging::bilinear::{self, Axis};
use imaging::mix::blended;
use imaging::present::{plan, presented};
use imaging::tensor::{Channel, Sampler};

// One step per stage, and what each costs:
//
//   1  presented    fixed cost: the square, whatever the photograph is
//   2  run          fixed cost: one graph run
//   3  fit          fixed cost: bounded by the canvas — two destinations, one elimination
//   4  blend        proportional to the photograph: the fused weighted pass
//   5  bias         proportional to the photograph: `imaging::mix::blended`
//
// Five, as `rio::process`'s are, and the same count for a different reason: Rio's apply and blend are two passes over
// the photograph, while here the apply *is* the weighted blend and the fifth step is the bias. Counted rather than
// accumulated, for the reason Rio's `STEPS` gives. The boundaries double as the cancellation points.
/// How many progress steps one run is: presented, run, fitted, blended, biased.
const STEPS: usize = 5;

/// The pipeline running one São Paulo colour balance operation, as the family's contract seam hands it back.
pub(crate) fn pipeline<B: Backend>(
    name: String,
    artifact: ArtifactId,
    profile: EpProfile,
    params: ColorBalanceParams,
    canvas: u32,
) -> Shared<B> {
    Arc::new(Mixed::new(name, artifact, profile, params, canvas))
}

/// One São Paulo run: the graph it opens, the settings it opens it under, the square it runs at, and how far the
/// photograph is moved towards the blend the weight map describes.
struct Mixed {
    /// The graph this operation runs, and the name it reports a failure in.
    graph: SingleGraph,
    /// The square this variant's graph was exported at, asked of the variant rather than fixed here.
    canvas: u32,
    /// How far, and in which direction, the photograph is moved towards the blend.
    bias: f32,
}

impl Mixed {
    fn new(name: String, artifact: ArtifactId, profile: EpProfile, params: ColorBalanceParams, canvas: u32) -> Self {
        Self { graph: SingleGraph::new(name, artifact, profile), canvas, bias: params.bias.as_f32() }
    }
}

impl OnOneGraph for Mixed {
    fn graph(&self) -> &SingleGraph {
        &self.graph
    }
}

impl<B: Backend> ImagePipeline<B> for Mixed {
    /// One graph run, whatever the photograph is.
    fn stages(&self) -> usize {
        // The fixed square is the whole of why this is a constant rather than something derived from the image.
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
        // Dispatched once at the top, as Rio's `corrected` and `Adjust::adjusted` dispatch it. The fused pass below
        // walks the photograph's own resolution — twenty-four million iterations on a 24-megapixel source, each
        // evaluating two polynomials — and it is the clearest case in this crate for hoisting the branch. The fit is
        // `f32`/`f64` throughout and is not monomorphised.
        //
        // This is also the divergence from the reference, deliberately: its `blendWeighted` writes into an
        // `image.NewRGBA`, so a 16-bit photograph comes back at 256 levels per channel whatever was asked for.
        match depth {
            ChannelDepth::Eight => self.rendered::<u8, B>(input, sessions, progress, cancelled).map(u8::into_dynamic),
            ChannelDepth::Sixteen => {
                self.rendered::<u16, B>(input, sessions, progress, cancelled).map(u16::into_dynamic)
            }
        }
    }
}

impl Mixed {
    /// [`ImagePipeline::run`]'s body, once, at whichever channel the caller asked for.
    fn rendered<T: Channel, B: Backend>(
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
        // fit a mapping from, and no destination for a weight map to be carried up to.
        if width == 0 || height == 0 {
            return Err(InferenceError::Untileable { width, height });
        }

        let report = reporter(progress);
        let planned = plan(width, height, self.canvas);
        let side = self.canvas as usize;
        let shown = GraphShape::new(3, side, side);
        let produced = GraphShape::new(CHANNELS, side, side);
        let crop = (planned.scaled_width, planned.scaled_height);

        // Checked at each of the five step boundaries. A cancelled run returns no image at all — not the
        // photograph, and not the partly blended buffer the cancellation landed in, which a caller has no way to
        // tell from a finished correction.
        if cancelled() {
            return Err(InferenceError::Cancelled);
        }

        let mut tensor = vec![0.0_f32; shown.len()];
        // `expect` rather than a folded error, as Rio's presentation does and for the same reason: the scratch is
        // allocated here at exactly the shape the graph is run at, so a disagreement is this function contradicting
        // itself rather than anything a caller could have caused.
        //
        // The resampled photograph is **dropped**. Nothing here divides by what the model was shown: the fit reads
        // it back out of the tensor at the graph's own range, and the blend reads the photograph's own pixels.
        presented(input, planned, RANGE, &mut tensor)
            .expect("the scratch is allocated at the square the graph accepts");

        checkpoint(&report, cancelled, 1.0 / STEPS as f64)?;

        // **Nine planes out of three in**, which is the whole of what makes this a second contract rather than a
        // second set of weights. `GraphShape` carries a channel count and `run_graph` takes the two shapes
        // separately, which is all the shared seam needs for it.
        let mut output = vec![0.0_f32; produced.len()];
        B::run_graph(&sessions[0], &tensor, shown, &mut output, produced)
            .map_err(InferenceError::run(&self.graph.name, 0))?;

        checkpoint(&report, cancelled, 2.0 / STEPS as f64)?;

        // Every view cropped to the plan, which is where the extension leaves this run: it must stay out of these
        // **global** fits, for the reason, and the 4.8 dB, that `samples` gives.
        //
        // **One solve for both renderings**, which pays the `XᵀX` accumulation once; see `mapping::fit`.
        let mappings = {
            let source = Samples::new(&tensor, 0, self.canvas, crop, RANGE)
                .expect("the scratch is the square the graph was run at");
            let renderings: Vec<Samples<'_>> = (0..RENDERINGS)
                .map(|index| {
                    Samples::new(&output, rendering_offset(index), self.canvas, crop, RANGE)
                        .expect("the output is the square the graph was run at")
                })
                .collect();

            mapping::fit(&self.graph.name, &source, &renderings.iter().collect::<Vec<_>>())?
        };

        // The input scratch goes; **the output does not.** Its first three planes are the weight map, which the
        // fused pass reads at every pixel, so the nine-plane tensor lives until the result is built.
        drop(tensor);

        checkpoint(&report, cancelled, 3.0 / STEPS as f64)?;

        let sampler = Sampler::new(input);
        let map = &output[..SETTINGS * side * side];
        let weighted = weighted::<T>(&sampler, &mappings, map, self.canvas, crop, (width, height));

        drop(output);

        checkpoint(&report, cancelled, 4.0 / STEPS as f64)?;

        let biased = blended(&sampler, weighted, self.bias);

        report(1.0);

        Ok(biased)
    }
}

/// The photograph's own pixels, each blended across the three illuminants by the weight that pixel carries.
///
/// `map` is the first [`SETTINGS`] planes of the graph's output — the weight map at the canvas's own resolution,
/// with only its `crop` region meaning anything — in the graph's channel order, so plane 0 multiplies the
/// photograph and plane `i + 1` multiplies `mappings[i]`.
///
/// Each rendering is bounded to `[0, 1]` before its weight multiplies it, and the weighted sum is bounded again by
/// [`Channel::from_unit`].
fn weighted<T: Channel>(
    source: &Sampler<'_>,
    mappings: &[Mapping],
    map: &[f32],
    canvas: u32,
    crop: (u32, u32),
    extent: (u32, u32),
) -> ImageBuffer<Rgb<T>, Vec<T>>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    let (width, height) = extent;

    // Resolved once per axis, before the pixel loop: neither table depends on pixel data, and each entry is re-read
    // once per row or once per column.
    let columns = Axis::new(crop.0, width);
    let rows = Axis::new(crop.1, height);
    // Sliced once rather than per pixel: daylight's plane first, then one per fitted rendering.
    let planes: Vec<&[f32]> = map.chunks_exact((canvas as usize) * (canvas as usize)).collect();

    ImageBuffer::from_fn(width, height, |x, y| {
        let (row, column) = (rows.tap(y as usize), columns.tap(x as usize));

        let [r, g, b] = source.rgb(x, y).map(u16::to_unit);
        // Built once per pixel and reused across both mappings, which is most of the arithmetic in this loop.
        let features = mapping::kernel(r, g, b);

        // The weights are not renormalised, and need not be: `bilinear::sample` keeps a partition of unity one (see
        // its docs). Renormalising defensively would hide a layout error rather than fix one.
        //
        // Daylight is the photograph itself (see `RENDERINGS`), and the one term that is **not** clamped, its
        // channels being in range by construction.
        let daylight = bilinear::sample(planes[0], canvas, row, column);
        let mut out = [daylight * r, daylight * g, daylight * b];

        for (index, fitted) in mappings.iter().enumerate() {
            let weight = bilinear::sample(planes[index + 1], canvas, row, column);
            let rendering = fitted.evaluate_kernel(features);

            // Each rendering bounded before its weight multiplies it, not the blend afterwards. The eleven-term
            // polynomial extrapolates well outside `[0, 1]` on saturated pixels, and bounding only at the end lets a
            // blown highlight pull the blend somewhere no rendering goes, by however much its own weight allows. This
            // is **not** the same bound as `from_unit`'s, which comes after and bounds the *sum*; both are needed and
            // they are not redundant.
            //
            // The accumulation is written plainly, with no `mul_add`, for the reason `mapping`'s header states.
            for (channel, value) in out.iter_mut().zip(rendering) {
                *channel += weight * value.clamp(0.0, 1.0);
            }
        }

        Rgb(out.map(T::from_unit))
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
    // that had to run at 656 would be fifteen megabytes of output tensor and a 430,336-sample fit per case.
    /// A canvas small enough for a test to run a whole pipeline over, in place of the 656 São Paulo ships at.
    const SMALL: u32 = 16;

    /// What the fake graph's weight planes say, at the canvas's own resolution.
    #[derive(Clone, Copy)]
    enum Map {
        /// One weight vector over the whole square.
        Uniform([f32; SETTINGS]),
        /// One weight vector over the square's left half and another over its right, split at the canvas's own
        /// middle column — which for a square photograph is the photograph's own middle.
        Split([f32; SETTINGS], [f32; SETTINGS]),
        /// One weight vector over the square's top half and another over its bottom, split at the canvas's own
        /// middle **row**. On a wide photograph that row is inside the extension, which is what makes this the
        /// fixture that separates a map read over the crop from one read across the whole square.
        Rows([f32; SETTINGS], [f32; SETTINGS]),
    }

    /// What a fake session answers a graph run with: a weight map, and one per-channel cast per rendering.
    ///
    /// The casts are **not** bounded by the fake graph, deliberately. A cast is exactly inside the fit's feature
    /// set, so the mapping recovered from one is that cast and the rendered photograph is a number a test can write
    /// down — including where that number is above one, which is the case the per-rendering clamp exists for.
    #[derive(Clone, Copy)]
    struct Answer {
        /// What the three weight planes carry.
        map: Map,
        /// The per-channel factor each synthesized rendering applies to what the graph was shown.
        casts: [[f32; 3]; RENDERINGS],
    }

    /// The two casts used wherever a test needs the two renderings told apart: different in every channel and in
    /// opposite directions, so a rendering read at the other's offset is a **different colour** rather than a
    /// slightly different one.
    const TWO_CASTS: [[f32; 3]; RENDERINGS] = [[0.55, 1.0, 1.30], [1.25, 0.85, 0.45]];

    /// `answer` applied to `input`, written into `output`. Shared by the fake backend and by the tests that
    /// reproduce what it did, so the two cannot drift apart.
    fn answer(answer: Answer, input: &[f32], shape: GraphShape, output: &mut [f32]) {
        let side = shape.width;
        let plane = side * shape.height;

        for setting in 0..SETTINGS {
            for y in 0..shape.height {
                for x in 0..side {
                    let weights = match answer.map {
                        Map::Uniform(weights) => weights,
                        Map::Split(left, right) => {
                            if x < side / 2 {
                                left
                            } else {
                                right
                            }
                        }
                        Map::Rows(top, bottom) => {
                            if y < shape.height / 2 {
                                top
                            } else {
                                bottom
                            }
                        }
                    };

                    output[setting * plane + y * side + x] = weights[setting];
                }
            }
        }

        for (index, cast) in answer.casts.iter().enumerate() {
            let base = rendering_offset(index) as usize * plane;

            for (channel, factor) in cast.iter().enumerate() {
                for element in 0..plane {
                    output[base + channel * plane + element] = input[channel * plane + element] * factor;
                }
            }
        }
    }

    /// What the fake graph was asked to do, so a test can say what reached it rather than what came back.
    #[derive(Default)]
    struct Log {
        /// The two shapes each run declared, so a run at the wrong square or the wrong channel count is a failure
        /// rather than a silent resize.
        shapes: Vec<(GraphShape, GraphShape)>,
        /// How many times the graph was run, which for this contract must be once.
        runs: usize,
    }

    /// A session standing in for São Paulo's graph: it records what it was given and answers nine planes.
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

        /// The single-input call, which is the only one this family's graphs take — with nine planes out of three
        /// in, which is the only thing about it that differs from Rio's.
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

        stub_backend_runs!("São Paulo's graph takes one output"; run_named_outputs);
        stub_backend_runs!("São Paulo's graph takes no second input"; run_weighted);
    }

    /// A handle on a fake session answering with `answer`, and the log it records into.
    fn session(answer: Answer) -> (SessionHandle<FakeSession>, Arc<Mutex<Log>>) {
        let log: Arc<Mutex<Log>> = Arc::default();
        let handle = SessionHandle::held(FakeSession { log: Arc::clone(&log), answer }, ExecutionProvider::Cpu);

        (handle, log)
    }

    /// The pipeline at [`SMALL`], at `bias`.
    fn balancing(bias: f64) -> Shared<Fake> {
        // Built off a real operation, for the reason Rio's `balancing` gives.
        let bias = Bias::new(bias).expect("the test supplied a bias in range");
        let operation = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp32), bias);

        pipeline::<Fake>(operation.display_name(), operation.artifact(), operation.profile(), operation.params(), SMALL)
    }

    /// Runs `source` through the pipeline at `bias` against a session answering with `answered`.
    fn run(source: &DynamicImage, bias: f64, answered: Answer, depth: ChannelDepth) -> DynamicImage {
        let (handle, _) = session(answered);

        balancing(bias)
            .run(source, std::slice::from_ref(&handle), depth, None, &|| false)
            .expect("a colour balance over a photograph")
    }

    /// The two tensors one run builds — what was presented and what the fake graph answered — plus the crop the
    /// plan gives them.
    ///
    /// The same presentation and the same graph the run makes, reproduced here so a test can state what the run's
    /// answer *should* be rather than compare it to itself.
    fn tensors(source: &DynamicImage, answered: Answer) -> (Vec<f32>, Vec<f32>, (u32, u32)) {
        let planned = plan(source.width(), source.height(), SMALL);
        let shown = GraphShape::new(3, SMALL as usize, SMALL as usize);
        let produced = GraphShape::new(CHANNELS, SMALL as usize, SMALL as usize);

        let mut tensor = vec![0.0_f32; shown.len()];
        presented(source, planned, RANGE, &mut tensor).expect("a square scratch");

        let mut output = vec![0.0_f32; produced.len()];
        answer(answered, &tensor, shown, &mut output);

        (tensor, output, (planned.scaled_width, planned.scaled_height))
    }

    /// The mappings fitted from `tensor` to the renderings `output` carries at `offsets`.
    fn fitted(tensor: &[f32], output: &[f32], crop: (u32, u32), offsets: [u32; RENDERINGS]) -> Vec<Mapping> {
        let source = Samples::new(tensor, 0, SMALL, crop, RANGE).expect("a square");
        let renderings: Vec<Samples<'_>> = offsets
            .iter()
            .map(|offset| Samples::new(output, *offset, SMALL, crop, RANGE).expect("a square"))
            .collect();

        mapping::fit("São Paulo (FP32)", &source, &renderings.iter().collect::<Vec<_>>())
            .expect("a determined system fits")
    }

    /// The offsets the layout declares, as the array [`fitted`] takes.
    fn layout_offsets() -> [u32; RENDERINGS] {
        [rendering_offset(0), rendering_offset(1)]
    }

    #[test]
    fn one_run_asks_for_one_artifact_under_its_own_profile() {
        // The `Model` half of the contract, which is what the driver reads before anything is installed. One
        // artifact, because the whole of this model's fixed-resolution half — the editing network, the weight
        // predictor and its internal ensemble — is in a single graph.
        let operation = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16), Bias::clamped(0.5));
        let built = pipeline::<Fake>(
            operation.display_name(),
            operation.artifact(),
            operation.profile(),
            operation.params(),
            SMALL,
        );

        assert_eq!(
            Model::<Fake>::required(built.as_ref()).iter().map(ArtifactId::as_str).collect::<Vec<_>>(),
            vec!["cb_saopaulo_fp16"],
            "a São Paulo run asked for something other than its own one graph"
        );

        let sessions = Model::<Fake>::sessions(built.as_ref());
        assert_eq!(sessions.len(), 1, "a single-graph contract asked for {} sessions", sessions.len());
        assert_eq!(sessions[0].0.as_str(), "cb_saopaulo_fp16");
        assert_eq!(*sessions[0].1, operation.profile(), "the session would be opened under another model's tuning");
    }

    #[test]
    fn one_run_is_one_graph_run_of_nine_planes_out_of_three_in() {
        // The contract's whole shape, and the half that separates it from Rio's: the output shape declares nine
        // channels where the input declares three. A pipeline that had declared three out would be handed a buffer
        // a third of the size, and the weight planes would be read out of a rendering.
        let (handle, log) = session(Answer { map: Map::Uniform([1.0, 0.0, 0.0]), casts: TWO_CASTS });

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
                GraphShape::new(CHANNELS, SMALL as usize, SMALL as usize)
            ),
            "the graph was not run at the variant's square with this model's channel layout"
        );

        // And `stages` agrees with what actually happened, which is what the driver's per-step record reports.
        assert_eq!(ImagePipeline::<Fake>::stages(balancing(1.0).as_ref()), log.runs);
    }

    #[test]
    fn an_image_with_no_area_is_refused_before_anything_is_allocated() {
        // The same refusal the tiling of a zero-area image makes, plus the two this contract adds: there are no
        // samples to fit a mapping from, and no destination for a weight map to be carried up to. Checked on both
        // axes, because a guard on one of them would let the other reach the plan's own assertion as a panic.
        for (width, height) in [(0, 8), (8, 0), (0, 0)] {
            let (handle, log) = session(Answer { map: Map::Uniform([1.0, 0.0, 0.0]), casts: TWO_CASTS });
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
    fn both_renderings_are_recovered_and_each_is_what_it_would_be_fitted_alone() {
        // Two *different* casts, so a fit that recovered one of them twice — the plausible mistake, and a
        // photograph rather than an error — fails here. And the generalisation is stated as well: fitting both
        // together must give what fitting each alone gives, because `XᵀX` depends only on the source and the
        // elimination touches each destination's columns with the same factors.
        let source = photograph(48, 30);
        let answered = Answer { map: Map::Uniform([0.0, 1.0, 0.0]), casts: TWO_CASTS };
        let (tensor, output, crop) = tensors(&source, answered);

        let together = fitted(&tensor, &output, crop, layout_offsets());
        assert_eq!(together.len(), RENDERINGS, "one solve did not answer one mapping per rendering");

        for (index, offset) in layout_offsets().into_iter().enumerate() {
            let alone = fitted(&tensor, &output, crop, [offset, offset]);

            assert_eq!(together[index], alone[0], "rendering {index} fitted together is not what it fits alone");
        }

        assert_ne!(together[0], together[1], "two different renderings were recovered as one mapping");

        // And the mappings are the casts they were built from, read off the photograph the run produces: an
        // all-shade map renders shade's cast and an all-tungsten map renders tungsten's.
        for (index, cast) in TWO_CASTS.into_iter().enumerate() {
            let mut weights = [0.0_f32; SETTINGS];
            weights[index + 1] = 1.0;

            let produced =
                run(&source, 1.0, Answer { map: Map::Uniform(weights), casts: TWO_CASTS }, ChannelDepth::Eight);
            let (original, produced) = (Sampler::new(&source), Sampler::new(&produced));

            for y in 0..30 {
                for x in 0..48 {
                    let before = original.rgb(x, y);
                    let after = produced.rgb(x, y);

                    for channel in 0..3 {
                        let expected = (before[channel].to_unit() * cast[channel]).clamp(0.0, 1.0);

                        assert!(
                            (after[channel].to_unit() - expected).abs() < 0.02,
                            "rendering {index} at ({x}, {y}) channel {channel}: {} against the {expected} its cast \
                             implies",
                            after[channel].to_unit()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_rendering_read_at_the_wrong_offset_is_a_different_photograph() {
        // **What makes the offsets worth pinning as literals**, and the risk this model adds that Rio's contract does
        // not have: half this graph's output is *shown*. A rendering read one plane early takes
        // its red from the last weight plane and its green and blue from shade's red and green — which is a fit, a
        // solve and a photograph, with nothing anywhere to say so.
        let source = photograph(48, 30);
        let answered = Answer { map: Map::Uniform([0.0, 1.0, 0.0]), casts: TWO_CASTS };
        let (tensor, output, crop) = tensors(&source, answered);

        let right = fitted(&tensor, &output, crop, layout_offsets());
        let wrong = fitted(&tensor, &output, crop, [rendering_offset(0) - 1, rendering_offset(1) - 1]);

        assert_ne!(right, wrong, "the fixture stopped separating the declared offsets from shifted ones");

        let sampler = Sampler::new(&source);
        let plane = (SMALL as usize) * (SMALL as usize);
        let map = &output[..SETTINGS * plane];

        let correct = weighted::<u8>(&sampler, &right, map, SMALL, crop, (48, 30));
        let shifted = weighted::<u8>(&sampler, &wrong, map, SMALL, crop, (48, 30));

        // Counted rather than asserted per pixel: the two mappings are different functions, but they still agree
        // wherever both of them clip to the same end of a channel. What is being stated is that the mistake is
        // visible in the **picture**.
        let differing = correct.pixels().zip(shifted.pixels()).filter(|(left, right)| left.0 != right.0).count();

        assert!(
            differing * 2 > (48 * 30),
            "only {differing} of {} pixels tell a rendering read at the right offset from one read a plane early",
            48 * 30
        );
    }

    #[test]
    fn an_all_daylight_map_returns_the_photograph() {
        // The identity the whole arrangement rests on, and the spec's own scenario: daylight is the photograph
        // itself rather than something the graph returned, so a map that selects it everywhere returns the
        // photograph's own pixels. Byte for byte, because the weight is exactly one and `to_unit` and `from_unit`
        // are exactly inverse.
        //
        // A pipeline that had taken daylight from a plane of the graph's output — the mistake the layout's three
        // weight planes and two renderings invite — returns something else entirely here.
        let source = photograph(37, 15);

        let produced = run(
            &source,
            1.0,
            Answer { map: Map::Uniform([1.0, 0.0, 0.0]), casts: TWO_CASTS },
            ChannelDepth::Eight,
        );
        let original = Sampler::new(&source);
        let produced = produced.as_rgb8().expect("an eight-bit run");

        for (x, y, pixel) in produced.enumerate_pixels() {
            let [r, g, b] = original.rgb(x, y);

            assert_eq!(
                pixel.0,
                [r, g, b].map(|value| u8::from_unit(value.to_unit())),
                "an all-daylight map changed ({x}, {y})"
            );
        }
    }

    #[test]
    fn an_all_shade_map_is_that_renderings_mapping_applied_globally() {
        // The spec's own scenario: a weight map that selects a single rendering across the whole photograph gives
        // that rendering's mapping applied globally, as a single-output model would apply it. Which is to say this
        // contract contains Rio's as the degenerate case, and a run through it lands where Rio's own apply would.
        let source = photograph(40, 24);
        let cast = TWO_CASTS[0];

        let produced = run(
            &source,
            1.0,
            Answer { map: Map::Uniform([0.0, 1.0, 0.0]), casts: TWO_CASTS },
            ChannelDepth::Eight,
        );
        let (original, produced) = (Sampler::new(&source), Sampler::new(&produced));

        for y in 0..24 {
            for x in 0..40 {
                let before = original.rgb(x, y);
                let after = produced.rgb(x, y);

                for channel in 0..3 {
                    let expected = (before[channel].to_unit() * cast[channel]).clamp(0.0, 1.0);

                    assert!(
                        (after[channel].to_unit() - expected).abs() < 0.02,
                        "({x}, {y}) channel {channel}: {} against the {expected} shade's cast implies",
                        after[channel].to_unit()
                    );
                }
            }
        }
    }

    #[test]
    fn a_map_split_down_the_middle_produces_the_boundary_at_the_photographs_own_middle() {
        // **The spec's weight-boundary scenario**, and the assertion that says the map was carried up to the
        // photograph's resolution rather than the photograph reduced to the map's. A square photograph, so the crop
        // is the whole square and the canvas's middle column is the photograph's own middle.
        //
        // The left half selects daylight — the photograph — and the right half selects shade, whose cast darkens
        // red by well over a third. The weight each column ended up with is then read back **out of the picture**,
        // which is the only place it is observable at all.
        let source = photograph(64, 64);
        let cast = [0.35_f32, 1.0, 1.0];
        let answered = Answer { map: Map::Split([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), casts: [cast, [1.0; 3]] };

        let produced = run(&source, 1.0, answered, ChannelDepth::Eight);
        let (original, produced) = (Sampler::new(&source), Sampler::new(&produced));

        // The recovered daylight weight per column, averaged over the rows where the two candidates are far enough
        // apart for 8-bit quantisation not to dominate the ratio.
        let recovered = |x: u32| {
            let mut total = 0.0_f64;
            let mut counted = 0_u32;

            for y in 0..64 {
                let daylight = original.rgb(x, y)[0].to_unit();
                let shade = (daylight * cast[0]).clamp(0.0, 1.0);

                if daylight - shade < 0.1 {
                    continue;
                }

                let measured = produced.rgb(x, y)[0].to_unit();
                total += f64::from((measured - shade) / (daylight - shade));
                counted += 1;
            }

            assert!(counted > 8, "column {x} had too few separable pixels to read a weight from");
            total / f64::from(counted)
        };

        // The interpolation spans four of the photograph's columns at this scale, so the two flat regions are
        // stated where they are flat and the crossing is stated where it crosses.
        for x in [0_u32, 7, 18, 29] {
            assert!(recovered(x) > 0.98, "column {x} is not entirely the photograph: {}", recovered(x));
        }
        for x in [34_u32, 40, 55, 63] {
            assert!(recovered(x) < 0.02, "column {x} is not entirely the shade rendering: {}", recovered(x));
        }

        // And the boundary itself: the first column where the shade rendering carries more than half the weight is
        // the photograph's own middle. A map read across the whole square, or read at the wrong scale, lands
        // somewhere else — and on a smooth map nothing else in this suite would notice.
        let crossing = (0..64).find(|x| recovered(*x) < 0.5).expect("the map changes rendering somewhere");

        assert_eq!(crossing, 32, "the boundary landed at column {crossing} rather than at the photograph's middle");
    }

    #[test]
    fn a_rendering_that_extrapolates_past_one_does_not_pull_the_blend_past_what_it_spans() {
        // **The spec's blown-highlight scenario**, and the one property that separates a clamp per rendering from a
        // clamp on the blend afterwards. The two agree everywhere the polynomial stays in gamut, which is most of
        // any photograph — so the fixture is a rendering that doubles every channel, and the assertion is taken on
        // the pixels where doubling leaves the range.
        //
        // Half daylight and half shade: a pixel at 0.8 renders `0.5 * 0.8 + 0.5 * clamp01(1.6)` = 0.9, where a
        // blend clamped only at the end renders `clamp01(0.5 * 0.8 + 0.5 * 1.6)` = 1.0. The second is a highlight
        // pulled somewhere no rendering goes.
        let source = photograph(40, 24);
        let answered = Answer { map: Map::Uniform([0.5, 0.5, 0.0]), casts: [[2.0; 3], [1.0; 3]] };

        let produced = run(&source, 1.0, answered, ChannelDepth::Eight);
        let (original, produced) = (Sampler::new(&source), Sampler::new(&produced));

        let mut separated = 0_usize;

        for y in 0..24 {
            for x in 0..40 {
                for channel in 0..3 {
                    let before = original.rgb(x, y)[channel].to_unit();
                    let after = produced.rgb(x, y)[channel].to_unit();

                    let per_rendering = 0.5 * before + 0.5 * (2.0 * before).clamp(0.0, 1.0);
                    let at_the_end = (0.5 * before + 0.5 * 2.0 * before).clamp(0.0, 1.0);

                    assert!(
                        (after - per_rendering).abs() < 0.02,
                        "({x}, {y}) channel {channel}: {after} against the {per_rendering} a per-rendering clamp \
                         implies"
                    );

                    if (per_rendering - at_the_end).abs() > 0.02 {
                        separated += 1;
                    }
                }
            }
        }

        // The fixture actually reaches the case, rather than passing because nothing in it saturates.
        assert!(separated > 100, "only {separated} channel values tell the two clamps apart");
    }

    #[test]
    fn a_run_produces_the_channel_depth_that_was_asked_for_and_no_alpha() {
        // The depth dispatch, which is the divergence from the reference this pipeline is built around: its
        // `blendWeighted` renders into an 8-bit RGBA buffer, so a 16-bit photograph would have both polynomials
        // evaluated over 256 levels and that answer written into a 16-bit result.
        let source = DynamicImage::ImageRgb16(ImageBuffer::from_fn(40, 25, |x, y| {
            Rgb([(x * 700 + y * 11) as u16, (x * 13 + y * 900) as u16, 30_000])
        }));
        let answered = Answer { map: Map::Uniform([0.4, 0.6, 0.0]), casts: TWO_CASTS };

        for (depth, sixteen) in [(ChannelDepth::Eight, false), (ChannelDepth::Sixteen, true)] {
            let produced = run(&source, 1.0, answered, depth);

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
        // finer than 256 levels, which the reference's own output buffer cannot do. An all-daylight map, so what is
        // being measured is the blend's own arithmetic rather than a fit over a grey ramp.
        let gradient = DynamicImage::ImageRgb16(ImageBuffer::from_fn(600, 1, |x, _| Rgb([(x as u16) * 100; 3])));
        let answered = Answer { map: Map::Uniform([1.0, 0.0, 0.0]), casts: TWO_CASTS };

        let produced = run(&gradient, 1.0, answered, ChannelDepth::Sixteen);
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
        let answered = Answer { map: Map::Uniform([0.3, 0.4, 0.3]), casts: TWO_CASTS };

        let produced = run(&translucent, 1.0, answered, ChannelDepth::Eight);

        assert!(produced.as_rgb8().is_some(), "a translucent photograph did not come back as colour alone");
    }

    #[test]
    fn a_bias_of_zero_returns_the_photograph_byte_for_byte() {
        // The spec's own scenario, through the whole pipeline: the presentation, the graph, the crop, both fits and
        // the weighted blend all run and are then multiplied by nothing. Byte for byte rather than within a level,
        // because the blend's own endpoint is exact — anything that reached the pixels by another route shows here.
        let source = photograph(30, 22);
        let answered = Answer { map: Map::Split([0.2, 0.8, 0.0], [0.1, 0.0, 0.9]), casts: TWO_CASTS };

        let produced = run(&source, 0.0, answered, ChannelDepth::Eight);
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
        // because what this family moves is a cast rather than a level, and the blend here warms the picture.
        let source = photograph(30, 22);
        let answered = Answer { map: Map::Uniform([0.0, 1.0, 0.0]), casts: [[1.3, 1.0, 0.75], [1.0; 3]] };

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

        let up = run(&source, 1.0, answered, ChannelDepth::Eight);
        let down = run(&source, -1.0, answered, ChannelDepth::Eight);

        let (before, up, down) = (warmth(&source), warmth(&up), warmth(&down));

        assert!(up > before, "a positive bias did not warm the picture: {up} against {before}");
        assert!(down < before, "a negative bias did not cool it: {down} against {before}");
    }

    #[test]
    fn detail_finer_than_the_square_survives_the_blend() {
        // The requirement the fixed square is affordable under, and the one that separates this arrangement from
        // returning the model's output enlarged. A one-pixel checkerboard is entirely lost by the downscale onto
        // the square, so a result carrying it can only have come from the photograph's own pixels — which is what
        // the requirement means by the model contributing a colour transform rather than the pixels.
        let checker = DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 64, |x, y| {
            Rgb(if (x + y) % 2 == 0 { [230_u8, 200, 180] } else { [40, 60, 70] })
        }));
        let answered = Answer { map: Map::Uniform([0.2, 0.8, 0.0]), casts: [[1.1, 1.0, 0.9], [1.0; 3]] };

        let produced = run(&checker, 1.0, answered, ChannelDepth::Eight);
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
    fn progress_reaches_exactly_the_end_of_the_range_and_never_goes_backwards() {
        // Counted rather than accumulated, which is what makes the last report exactly the end rather than whatever
        // five additions of a fifth drifted to — and the property the spec states, that the report advances during
        // the full-resolution work rather than only around the graph.
        let (handle, _) = session(Answer { map: Map::Uniform([0.3, 0.7, 0.0]), casts: TWO_CASTS });
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
        // `0 / 0.9 / 1` does not have: on a large photograph the weighted blend and the bias are most of the run.
        assert!(reported.iter().filter(|fraction| **fraction > 0.4).count() >= 3, "{reported:?}");
    }

    #[test]
    fn a_cancellation_at_any_of_the_five_boundaries_produces_no_image() {
        // Every boundary, separately, rather than one of them: a caller handed a partly blended photograph with no
        // error has no way to tell it from a finished correction, and the later boundaries are the ones where such
        // a buffer exists to be handed back.
        for boundary in 0..STEPS {
            let (handle, _) = session(Answer { map: Map::Uniform([0.3, 0.7, 0.0]), casts: TWO_CASTS });
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
        let (handle, _) = session(Answer { map: Map::Uniform([0.3, 0.7, 0.0]), casts: TWO_CASTS });
        assert!(
            balancing(1.0)
                .run(&photograph(33, 21), std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .is_ok()
        );
    }

    #[test]
    fn the_weight_map_is_read_over_the_photographs_region_alone() {
        // **The spec's per-pixel-map scenario.** The map covers the whole square and only its top-left crop region
        // was predicted for anything: read across the whole square it would be stretched against the photograph it
        // is meant to line up with, which displaces every value in it rather than only those near the extended
        // edge.
        //
        // A wide photograph, so the extension is three quarters of the square's height, and a map split at the
        // canvas's middle **row** — which is inside the extension. Read over the crop, every row of the photograph
        // falls in the top half and the result is the photograph itself. Read across the square, everything below
        // the photograph's own middle picks up the bottom half's rendering instead, and the cast below is far
        // enough from the identity that it cannot be mistaken for rounding.
        let source = photograph(64, 15);
        let planned = plan(64, 15, SMALL);

        assert!(
            planned.scaled_height * 2 < SMALL,
            "the fixture stopped putting the canvas's middle row inside the extension"
        );

        let answered = Answer { map: Map::Rows([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), casts: [[0.35, 1.0, 1.0], [1.0; 3]] };

        let produced = run(&source, 1.0, answered, ChannelDepth::Eight);
        let original = Sampler::new(&source);
        let produced = produced.as_rgb8().expect("an eight-bit run");

        assert_eq!((produced.width(), produced.height()), (64, 15), "the extension reached the result's shape");

        for (x, y, pixel) in produced.enumerate_pixels() {
            let [r, g, b] = original.rgb(x, y);

            assert_eq!(
                pixel.0,
                [r, g, b].map(|value| u8::from_unit(value.to_unit())),
                "({x}, {y}) took its weights from the extension rather than from the region predicted for it"
            );
        }

        // And the fixture reaches the case rather than passing because the two readings agree: the same map read
        // across the whole square puts the bottom half of the photograph on the other rendering.
        let (tensor, output, crop) = tensors(&source, answered);
        let mappings = fitted(&tensor, &output, crop, layout_offsets());
        let plane = (SMALL as usize) * (SMALL as usize);
        let map = &output[..SETTINGS * plane];
        let sampler = Sampler::new(&source);

        let stretched = weighted::<u8>(&sampler, &mappings, map, SMALL, (SMALL, SMALL), (64, 15));
        let differing = produced.pixels().zip(stretched.pixels()).filter(|(left, right)| left.0 != right.0).count();

        assert!(
            differing * 4 > 64 * 15,
            "only {differing} of {} pixels tell a map read over the crop from one read across the square",
            64 * 15
        );
    }
}
