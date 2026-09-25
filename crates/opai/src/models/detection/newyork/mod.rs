//! New York: the RetinaFace face detector, and the one model whose result is not an image.
//!
//! One graph, one run, three output tensors. Progress is reported at three phase boundaries — 0.2 once the tensor is
//! built, 0.6 once the graph returns, 1.0 once the decode finishes — and cancellation is checked at the same three
//! points.

// The whole of it is here and in the four files beside it, because none of it is shared with anything: it is
// RetinaFace's arithmetic, general-purpose to nothing, and a second detection model is a directory of its own plus one
// arm in `Detection`'s contract seam. A run:
//
//   input::letterbox    the photograph fitted into a 640 square, BGR planar CHW, minus the per-channel means
//   run_named_outputs   one graph run -> loc, conf, landmarks, addressed by name
//   anchors::generate   the 16,800 priors the three outputs are indexed by
//   filter::faces       threshold, decode the survivors, suppress overlaps, rescale into the source's pixels
//
// The progress fractions are the reference's own phase boundaries rather than measurements; only the graph run takes
// any real time. Three cancellation checks are the only schedule a single whole-image run offers.

pub(crate) mod anchors;
pub(crate) mod decode;
pub(crate) mod filter;
pub(crate) mod input;

use std::sync::Arc;

use image::DynamicImage;

use super::Detection;

use crate::error::InferenceError;
use crate::models::ArtifactId;
use crate::models::face::Faces;
use crate::pipeline::Backend;
use crate::pipeline::session::{GraphShape, NamedOutput};
use crate::pipeline::{DataPipeline, OnOneGraph, SharedData, SingleGraph, checkpoint, reporter};
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;

// Not a tunable: the anchor grid, the output tensor lengths and the whole of the pre- and post-processing are derived
// from this value, and the weights are exported at a static `[1, 3, 640, 640]`.
//
// That export, rather than any provider setting, is what made this model fast. Anyone re-exporting these weights must
// keep the input shape static: with dynamic batch, height and width axes CoreML takes 10 of 176 nodes and the other
// 166 run on CPU kernels, 158ms a run against 12.3ms at the fixed shape — a 13x cost that reports nothing but a slow
// run.
/// The fixed square input resolution this graph accepts.
pub(crate) const TARGET_SIZE: u32 = 640;

// Addressed by name through `Backend::run_named_outputs`, unlike every other model in this project — see that method
// for why three outputs invert the argument for taking them positionally.
/// The graph's three output tensors, by the names it declares them under.
const OUTPUTS: [&str; 3] = ["loc", "conf", "landmarks"];

/// How many values each of [`OUTPUTS`] carries per anchor: four box offsets, two class scores, ten landmark
/// coordinates.
const WIDTHS: [usize; 3] = [4, 2, 10];

/// The fractions a run reports at.
const AFTER_INPUT: f64 = 0.2;
const AFTER_GRAPH: f64 = 0.6;
const AFTER_DECODE: f64 = 1.0;

/// The pipeline running `operation`, as the family's contract seam hands it back.
pub(crate) fn pipeline<B: Backend>(operation: Detection) -> SharedData<B, Faces> {
    Arc::new(NewYork::new(operation.artifact(), operation.profile(), operation.display_name()))
}

/// One New York run: the graph it opens, the settings it opens it under, and what it is called in a failure.
struct NewYork {
    /// The graph this operation runs, and the name it reports a failure in.
    graph: SingleGraph,
}

impl NewYork {
    /// The pipeline for `artifact` under `profile`, named `name`.
    fn new(artifact: ArtifactId, profile: EpProfile, name: String) -> Self {
        Self { graph: SingleGraph::new(name, artifact, profile) }
    }
}

impl OnOneGraph for NewYork {
    fn graph(&self) -> &SingleGraph {
        &self.graph
    }
}

impl<B: Backend> DataPipeline<B> for NewYork {
    type Output = Faces;

    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Faces, InferenceError> {
        let report = reporter(progress);

        if cancelled() {
            return Err(InferenceError::Cancelled);
        }

        let fitted = input::letterbox(input, TARGET_SIZE)?;

        // The priors are what the three outputs are indexed by, so their count is what the outputs are sized to —
        // which keeps the two coupled rather than agreeing by coincidence.
        let priors = anchors::generate(TARGET_SIZE);

        checkpoint(&report, cancelled, AFTER_INPUT)?;

        let mut buffers: [Vec<f32>; 3] = WIDTHS.map(|width| vec![0.0_f32; priors.len() * width]);
        let [loc, conf, landmarks] = &mut buffers;

        let mut outputs = [
            NamedOutput { name: OUTPUTS[0], buffer: loc },
            NamedOutput { name: OUTPUTS[1], buffer: conf },
            NamedOutput { name: OUTPUTS[2], buffer: landmarks },
        ];

        let shape = GraphShape::new(3, TARGET_SIZE as usize, TARGET_SIZE as usize);

        // Named rather than a tile index, because there is one run rather than a grid: the failure this can report is
        // "the model failed", and the tile it names is the only one there is.
        B::run_named_outputs(&sessions[0], &fitted.tensor, shape, &mut outputs)
            .map_err(InferenceError::run(&self.graph.name, 0))?;

        checkpoint(&report, cancelled, AFTER_GRAPH)?;

        let [loc, conf, landmarks] = &buffers;
        let found = filter::faces(loc, conf, landmarks, &priors, &fitted, TARGET_SIZE);

        report(AFTER_DECODE);

        Ok(found)
    }
}

// Synthetic rather than invented inference. What it stands in for is the graph's **shape** — three named tensors of
// three lengths, and a confidence layout of interleaved background/face pairs — so that everything around the run is
// exercised without an ONNX Runtime. Nothing it writes is presented as a detection.
//
// `pub(crate)` and outside the test module below because it has a second caller: `autopilot` drives a whole analysis
// through `execute`, which reaches this pipeline for real, and it needs the graph to answer. Copying it there instead
// would be two fixtures that can quietly stop agreeing about what this graph's output looks like, at which point one
// of the two suites is testing a shape the model does not have. It stays here, with the model whose tensor names and
// confidence layout it encodes, rather than moving to `pipeline::test_support`, which holds only what no model owns.
/// Answers this graph's named-output call with synthetic anchor tensors: one confident face at `face_at`, or none.
///
/// Zero offsets for `loc` and `landmarks`, so every decoded box is its own prior and the arithmetic stays checkable
/// by hand.
#[cfg(test)]
pub(crate) fn answer_graph(outputs: &mut [NamedOutput<'_>], face_at: Option<usize>) {
    for output in outputs.iter_mut() {
        match output.name {
            "loc" | "landmarks" => output.buffer.fill(0.0),
            "conf" => {
                // Interleaved background/face pairs, every anchor below the threshold except the named one.
                for anchor in 0..output.buffer.len() / 2 {
                    let score = if face_at == Some(anchor) { 0.99 } else { 0.01 };
                    output.buffer[anchor * 2] = 1.0 - score;
                    output.buffer[anchor * 2 + 1] = score;
                }
            }
            other => panic!("the detector asked for an output named {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    use crate::models::detection::DetectionVariant;
    use crate::models::precision::FloatPrecision;
    use crate::pipeline::Model;
    use crate::pipeline::test_support::stub_backend_runs;
    use crate::providers::ExecutionProvider;
    use crate::providers::profile::{CoreMlComputeUnits, CoreMlSpecialization, ExecutionMode};
    use crate::sessions::SessionHandle;

    /// What a run of the fake found, so a test can say which of the three phases it was stopped at.
    #[derive(Default)]
    struct Log {
        /// Every set of output names the graph was asked for, in order.
        asked: Vec<Vec<String>>,
    }

    /// A session standing in for the graph, answering with synthetic anchor tensors.
    struct FakeSession {
        log: Arc<Mutex<Log>>,
        /// Which anchor, if any, is a confident face. Every other anchor scores below the threshold.
        face_at: Option<usize>,
    }

    /// A backend with no ONNX Runtime that answers the named-output call with tensors of the right lengths.
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
        ) -> Result<SessionHandle<Self::Session>, crate::error::SessionError> {
            unreachable!("the pipeline is handed its sessions; it never acquires one")
        }

        /// Records which outputs were asked for, then writes them through [`answer_graph`].
        fn run_named_outputs(
            handle: &SessionHandle<Self::Session>,
            input: &[f32],
            input_shape: GraphShape,
            outputs: &mut [NamedOutput<'_>],
        ) -> Result<(), Self::Error> {
            let session = handle.session();
            session
                .log
                .lock()
                .unwrap()
                .asked
                .push(outputs.iter().map(|output| output.name.to_string()).collect());

            assert_eq!(input.len(), input_shape.len(), "the detector was fed a buffer its declared shape does not fit");

            answer_graph(outputs, session.face_at);

            Ok(())
        }

        // The three seams a detection run never takes — its graph is fed the letterboxed image and nothing beside
        // it, and it asks for several outputs back. See `stub_backend_runs`.
        stub_backend_runs!("a detection run takes the named-output seam"; run_tile, run_graph, run_weighted);
    }

    /// A handle on a fake session with a confident face at `face_at`, and the log it records into.
    fn session(face_at: Option<usize>) -> (SessionHandle<FakeSession>, Arc<Mutex<Log>>) {
        let log: Arc<Mutex<Log>> = Arc::default();
        let handle = SessionHandle::held(FakeSession { log: Arc::clone(&log), face_at }, ExecutionProvider::Cpu);

        (handle, log)
    }

    /// The pipeline under test, at FP32.
    fn newyork() -> NewYork {
        let operation = Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp32));

        NewYork::new(operation.artifact(), operation.profile(), operation.display_name())
    }

    /// A source image of a known size, with nothing in it that matters — the fake's answer does not read the pixels.
    fn source(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(width, height, image::Rgb([128, 128, 128])))
    }

    #[test]
    fn a_run_over_a_known_image_produces_the_face_the_graph_reported() {
        // End to end with no ONNX Runtime: the letterbox, the named call, the anchor grid, the threshold, the
        // suppression and the rescale, driven by a graph whose answer is known.
        //
        // Anchor 0 is the stride-8 cell at the origin at 16/640, so with zero offsets its box decodes back to itself:
        // centred at 0.5*8/640 with an extent of 16/640, scaled by 640 and then into the source's pixels.
        let (handle, _log) = session(Some(0));
        let found =
            DataPipeline::<Fake>::run(&newyork(), &source(1280, 640), &[handle], None, &|| false).expect("a run");

        assert_eq!(found.len(), 1, "one confident anchor did not produce one face");

        let face = found.as_slice()[0];
        let centre = 0.5_f32 * 8.0;
        let half = 16.0_f32 / 2.0;
        // A 1280x640 source letterboxes to 640x320, so both axes scale by exactly 2.
        let expected_min = (centre - half) * 2.0;
        let expected_max = (centre + half) * 2.0;

        assert!((f64::from(face.bounding_box().min.x) - f64::from(expected_min)).abs() < 1e-3, "{face:?}");
        assert!((f64::from(face.bounding_box().max.x) - f64::from(expected_max)).abs() < 1e-3, "{face:?}");
        assert_eq!(face.confidence().get(), 0.99, "the confidence was not carried through unrounded");
    }

    #[test]
    fn an_image_the_graph_finds_nothing_in_produces_an_empty_set_rather_than_an_error() {
        let (handle, _log) = session(None);

        let found =
            DataPipeline::<Fake>::run(&newyork(), &source(800, 600), &[handle], None, &|| false).expect("a run");

        assert!(found.is_empty(), "an image with no face produced something other than an empty set");
    }

    #[test]
    fn the_graph_is_asked_for_its_three_outputs_by_name_and_in_the_declared_order() {
        // The one model in this project that spells tensor names, and the reason it does: taking three outputs
        // positionally makes the result depend on the order a re-export happened to declare them in.
        let (handle, log) = session(Some(0));

        DataPipeline::<Fake>::run(&newyork(), &source(640, 640), &[handle], None, &|| false).expect("a run");

        let asked = &log.lock().unwrap().asked;
        assert_eq!(asked.len(), 1, "a whole-image detection is one graph run");
        assert_eq!(asked[0], vec!["loc", "conf", "landmarks"]);
    }

    #[test]
    fn the_three_phase_fractions_are_reported_in_order() {
        let reported: Arc<Mutex<Vec<f64>>> = Arc::default();
        let sink = Arc::clone(&reported);
        let (handle, _log) = session(Some(0));

        DataPipeline::<Fake>::run(
            &newyork(),
            &source(640, 640),
            &[handle],
            Some(&|fraction: f64| sink.lock().unwrap().push(fraction)),
            &|| false,
        )
        .expect("a run");

        assert_eq!(*reported.lock().unwrap(), vec![AFTER_INPUT, AFTER_GRAPH, AFTER_DECODE]);
    }

    #[test]
    fn a_run_cancelled_at_any_of_the_three_points_produces_no_faces() {
        // Cancellation is checked before the tensor is built, after it, and after the graph returns. Each is driven by
        // a token that turns on after a given number of checks, so all three points are reached rather than only the
        // first.
        for after in 0..3 {
            let checks = std::cell::Cell::new(0_usize);
            let (handle, _log) = session(Some(0));

            let outcome = DataPipeline::<Fake>::run(&newyork(), &source(640, 640), &[handle], None, &|| {
                let seen = checks.get();
                checks.set(seen + 1);
                seen >= after
            });

            assert!(
                matches!(outcome, Err(InferenceError::Cancelled)),
                "a run cancelled at check {after} produced something other than a cancellation"
            );
        }
    }

    #[test]
    fn an_image_with_no_area_is_refused_before_the_graph_is_run() {
        // The letterbox's refusal, reaching the caller as the pipeline's own — and before the session is touched, so
        // a run over a square of nothing but padding never happens.
        let (handle, log) = session(Some(0));
        let empty = DynamicImage::ImageRgb8(image::ImageBuffer::new(0, 480));

        let outcome = DataPipeline::<Fake>::run(&newyork(), &empty, &[handle], None, &|| false);

        assert!(matches!(outcome, Err(InferenceError::Untileable { width: 0, height: 480 })), "{outcome:?}");
        assert!(log.lock().unwrap().asked.is_empty(), "the graph was run for an image with no area");
    }

    #[test]
    fn the_pipeline_names_its_one_artifact_and_the_profile_measured_for_it() {
        for precision in FloatPrecision::ALL {
            let operation = Detection::new(DetectionVariant::NewYork(precision));
            let pipeline = NewYork::new(operation.artifact(), operation.profile(), operation.display_name());

            assert_eq!(Model::<Fake>::required(&pipeline), &[operation.artifact()]);

            let sessions = Model::<Fake>::sessions(&pipeline);
            assert_eq!(sessions.len(), 1, "a detection run opens one session");
            assert_eq!(*sessions[0].0, operation.artifact());
            assert_eq!(*sessions[0].1, operation.profile(), "the graph was paired with another profile");
        }
    }

    #[test]
    fn new_york_declares_nhwc_and_sequential_execution_at_both_precisions() {
        // Mirrors the reference's `TestVariantRunsNewyorkInNHWCAtBothPrecisions`. The FP32 half is the one at risk, as
        // `DetectionVariant::profile` records: tidying it for symmetry with Athens is silent.
        for precision in FloatPrecision::ALL {
            let profile = DetectionVariant::NewYork(precision).profile();

            assert!(profile.cuda_prefer_nhwc, "at {precision:?}: the graph must run in NHWC on CUDA");
            assert_eq!(profile.execution_mode, ExecutionMode::Sequential, "at {precision:?}");
        }
    }

    #[test]
    fn new_york_is_left_on_the_coreml_defaults_at_both_precisions() {
        // Mirrors `TestVariantLeavesNewyorkOnTheCoreMLDefaults`, and the failure it guards is silent. Athens keeps
        // itself off the Neural Engine and Santorini asks for fast prediction, so those are the obvious things to add
        // here for symmetry — and the model would still load and still return the same faces, only slower. See
        // `DetectionVariant::profile` for what each costs.
        for precision in FloatPrecision::ALL {
            let profile = DetectionVariant::NewYork(precision).profile();

            assert_eq!(
                profile.coreml_compute_units,
                CoreMlComputeUnits::All,
                "at {precision:?}: keeping this graph off the Neural Engine costs the FP16 graph 20%"
            );
            assert_eq!(
                profile.coreml_specialization,
                CoreMlSpecialization::Default,
                "at {precision:?}: fast prediction measured as a tie"
            );
        }
    }

    #[test]
    fn new_york_declares_the_two_measured_settings_and_nothing_else() {
        // The whole profile, so a setting added for symmetry with another model is a failing test rather than a
        // slower run nothing reports. Both CUDA defaults the reference names as load-bearing are among what is left
        // alone: TF32 stays on, and convolution-bias fusion stays off.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                DetectionVariant::NewYork(precision).profile(),
                EpProfile { cuda_prefer_nhwc: true, execution_mode: ExecutionMode::Sequential, ..EpProfile::default() },
                "at {precision:?}"
            );
        }
    }
}
