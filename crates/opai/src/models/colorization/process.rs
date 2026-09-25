//! The colorization pipeline all three of this family's models run: the whole photograph stretched to the model's
//! square, one graph run, and the predicted chroma put onto the photograph's own lightness.

// Family tier: every variant colorizes through it unchanged. What differs between them is which of the two graph
// contracts it speaks, and `Contract` is that difference stated as data: the square, the output's channel count, the
// input builder and the chroma reader. The pipeline body reads all four off the arm and nothing else, so nothing
// contract-specific reaches the shared steps.
//
// What a run is:
//
//   refuse      a photograph with no area, before anything is allocated
//   input       stretched to the square, then gray-Lab (Ab) or ITU-601 luma (Rgb) on all three planes
//   run         one graph run: the square in, the chroma (Ab) or an RGB rendering (Rgb) out
//   compose     the chroma read and put onto the photograph's own full-resolution L, at the run's depth
//
// **No arithmetic is written here.** Every step is `stretch`'s, `ab`'s, `jaipur::rgb`'s or `compose`'s, and each is
// pinned there. This file orders them, and its tests check the ordering: which input each contract builds, the shapes
// the graph is run at, and that the photograph's lightness and the model's colour both arrive.
//
// **Not tiled.** A model colours what it sees from the scene's context, so tiles coloured separately would disagree
// about the regions they share, and blending their overlap does not resolve a disagreement. The square costs the same
// for a thumbnail and a 24-megapixel photograph, and the photograph keeps its own detail because only chroma comes
// from the model.

use std::sync::Arc;

use image::{DynamicImage, ImageBuffer, Rgb};

use super::compose::{self, Encoded};
use super::{ab, jaipur::rgb};

use crate::error::InferenceError;
use crate::models::ArtifactId;
use crate::pipeline::Backend;
use crate::pipeline::session::GraphShape;
use crate::pipeline::{ImagePipeline, OnOneGraph, Shared, SingleGraph, checkpoint, reporter};
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;
use imaging::ChannelDepth;
use imaging::tensor::{Channel, Sampler};

/// Which of the family's two graph contracts a variant speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Contract {
    /// Delhi's and Mumbai's: a gray rendering in at 512, the Lab a/b planes out.
    Ab,
    /// Jaipur's: ITU-601 luma in at 560, a colorized RGB rendering out, whose a/b is extracted.
    Rgb,
}

impl Contract {
    /// The square this contract's graphs accept, and the only one they accept.
    pub(super) const fn side(self) -> u32 {
        match self {
            Self::Ab => ab::SIDE,
            Self::Rgb => rgb::SIDE,
        }
    }

    /// How many planes this contract's graphs return.
    pub(super) const fn channels(self) -> usize {
        match self {
            Self::Ab => ab::CHANNELS,
            Self::Rgb => rgb::CHANNELS,
        }
    }

    /// The graph's input: `source` stretched to [`side`](Self::side) and rendered the way this contract's model was
    /// trained on, on all three planes.
    fn input(self, source: &DynamicImage) -> Vec<f32> {
        match self {
            Self::Ab => ab::input(source),
            Self::Rgb => rgb::input(source),
        }
    }

    /// The photograph `source` at `extent`, recoloured with the chroma this contract reads out of the graph's
    /// `output`.
    fn composed<T: Encoded>(
        self,
        source: &Sampler<'_>,
        output: &[f32],
        extent: (u32, u32),
    ) -> ImageBuffer<Rgb<T>, Vec<T>>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        // The chroma read and the compose folded into one method, which is where the two contracts' different
        // ownership is absorbed: the Ab planes are borrowed straight out of the output, and Jaipur's are computed and
        // owned. A common return type for "two planes" would either copy 2 MB out of every Ab output or be a `Cow`.
        match self {
            Self::Ab => compose::composed(source, ab::planes(output), ab::SIDE, extent),
            Self::Rgb => {
                let (a, b) = rgb::chroma(output);
                compose::composed(source, (&a, &b), rgb::SIDE, extent)
            }
        }
    }
}

// Counted rather than accumulated, so the last report lands on exactly the end of the operation's share. That is light
// adjustment's schedule, for light adjustment's reason.
//
// Three even steps rather than the reference's `0 / 1`. The graph is a **fixed** cost whatever the photograph is, from
// about 100 to 900 ms by variant and provider, while the compose after it is proportional to the photograph: 86 ms at
// 12 megapixels. A bar that moved only at the start and the end would sit still through the compose on a large
// photograph, and a report on each side of the graph run is what shows that it has run. A weighting fitted to the
// measured split is rejected, because it would be a constant correct for one image size and one provider.
/// How many progress steps one run is: input built, graph run, result composed.
const STEPS: usize = 3;

/// The pipeline running one colorization operation, as the family's contract seam hands it back.
pub(crate) fn pipeline<B: Backend>(
    name: String,
    artifact: ArtifactId,
    profile: EpProfile,
    contract: Contract,
) -> Shared<B> {
    Arc::new(Colorize { graph: SingleGraph::new(name, artifact, profile), contract })
}

/// One colorization run: the graph it opens, the settings it opens it under, and the contract that graph speaks.
struct Colorize {
    /// The graph this operation runs, and the name it reports a failure in.
    graph: SingleGraph,
    /// The contract the graph speaks: its square, its output and how the chroma is read out of it.
    contract: Contract,
}

impl OnOneGraph for Colorize {
    fn graph(&self) -> &SingleGraph {
        &self.graph
    }
}

impl<B: Backend> ImagePipeline<B> for Colorize {
    /// One graph run, whatever the photograph is.
    fn stages(&self) -> usize {
        // The fixed square is the whole of why this is a constant: a thumbnail and a 24-megapixel photograph are both
        // one run.
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
        // Dispatched once at the top, so the full-resolution compose is compiled per channel type rather than
        // branching per pixel.
        match depth {
            ChannelDepth::Eight => self.colorized::<u8, B>(input, sessions, progress, cancelled).map(u8::into_dynamic),
            ChannelDepth::Sixteen => {
                self.colorized::<u16, B>(input, sessions, progress, cancelled).map(u16::into_dynamic)
            }
        }
    }
}

impl Colorize {
    /// [`ImagePipeline::run`]'s body, once, at whichever channel the caller asked for.
    fn colorized<T: Encoded, B: Backend>(
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

        // Before anything is allocated. An empty photograph cannot be stretched onto a square, there is no lightness
        // to keep, and the compose's axis tables panic on a zero-length axis.
        if width == 0 || height == 0 {
            return Err(InferenceError::Untileable { width, height });
        }

        let report = reporter(progress);
        let side = self.contract.side() as usize;

        // Checked where the reference checks its context: before the stretch, after the input is built, and after the
        // graph run. Nothing is checked after the compose, because the image is finished by then. A cancelled run
        // returns no image at all.
        if cancelled() {
            return Err(InferenceError::Cancelled);
        }

        let tensor = self.contract.input(input);

        checkpoint(&report, cancelled, 1.0 / STEPS as f64)?;

        let output_shape = GraphShape::new(self.contract.channels(), side, side);
        let mut output = vec![0.0_f32; output_shape.len()];
        B::run_graph(&sessions[0], &tensor, GraphShape::new(3, side, side), &mut output, output_shape)
            .map_err(InferenceError::run(&self.graph.name, 0))?;

        checkpoint(&report, cancelled, 2.0 / STEPS as f64)?;

        // Built here rather than before the graph run. For a layout it cannot borrow, it is an owned 16-bit copy of
        // the photograph, which would otherwise be held across the graph run for nothing.
        let sampler = Sampler::new(input);
        let composed = self.contract.composed::<T>(&sampler, &output, (width, height));

        report(1.0);

        Ok(composed)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use image::{GenericImageView as _, Rgba};

    use super::*;

    use crate::error::SessionError;
    use crate::models::colorization::variant::tests::every_variant;
    use crate::models::colorization::{Colorization, ColorizationVariant, lab};
    use crate::models::precision::FloatPrecision;
    use crate::pipeline::test_support::stub_backend_runs;
    use crate::providers::ExecutionProvider;
    use imaging::test_support::photograph;

    #[test]
    fn every_variant_speaks_its_own_contract_at_its_own_square() {
        // Driven from every published variant, so one added later cannot go unpinned. The literals are the exports':
        // a variant on the wrong arm is fed the wrong rendering at the wrong size, and its output is read at the wrong
        // channel count, which is a plausible photograph rather than an error.
        for variant in every_variant() {
            let expected = match variant {
                ColorizationVariant::Delhi(_) | ColorizationVariant::Mumbai(_) => (Contract::Ab, 512, 2),
                ColorizationVariant::Jaipur(_) => (Contract::Rgb, 560, 3),
            };
            let contract = variant.contract();

            assert_eq!((contract, contract.side(), contract.channels()), expected, "{variant:?}");
        }
    }

    // The fake-backend suite: the whole pipeline — the contract dispatch, the stretch, the graph shapes, the compose,
    // the depth dispatch, the progress schedule, the cancellation and the refusal — with no ONNX Runtime, no GPU, no
    // model file and no network, which is what keeps every property below checked on every CI platform. Every test
    // runs through `ImagePipeline::run`, not through the helpers, so the wiring is what is tested.

    /// What the fake graph writes into its output, given the input it was fed.
    type Paint = Box<dyn Fn(&[f32], &mut [f32]) + Send + Sync>;

    /// What the fake graph was asked to do, so a test can say what reached it rather than what came back.
    #[derive(Default)]
    struct Log {
        /// The two shapes each run declared.
        shapes: Vec<(GraphShape, GraphShape)>,
        /// The input tensor each run was fed.
        inputs: Vec<Vec<f32>>,
    }

    /// A session standing in for a colorization graph: it records what it was given and paints what the test chose.
    struct FakeSession {
        log: Arc<Mutex<Log>>,
        paint: Paint,
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

        stub_backend_runs!("a colorization run takes a declared-shape seam"; run_tile);

        /// The single-input call, which is the only one any of this family's graphs takes.
        fn run_graph(
            handle: &SessionHandle<Self::Session>,
            input: &[f32],
            input_shape: GraphShape,
            output: &mut [f32],
            output_shape: GraphShape,
        ) -> Result<(), Self::Error> {
            let session = handle.session();

            // Refused exactly as the real seam does: a buffer the declared shape does not describe is the mistake it
            // exists to prevent.
            assert_eq!(input.len(), input_shape.len(), "the graph was fed a buffer {input_shape} does not describe");
            assert_eq!(output.len(), output_shape.len(), "the output buffer is not {output_shape}");

            {
                let mut log = session.log.lock().unwrap();
                log.shapes.push((input_shape, output_shape));
                log.inputs.push(input.to_vec());
            }

            (session.paint)(input, output);

            Ok(())
        }

        stub_backend_runs!("a colorization run takes one output"; run_named_outputs);
        stub_backend_runs!("no colorization graph takes a second input"; run_weighted);
    }

    /// A handle on a fake session painting through `paint`, and the log it records into.
    fn session(
        paint: impl Fn(&[f32], &mut [f32]) + Send + Sync + 'static,
    ) -> (SessionHandle<FakeSession>, Arc<Mutex<Log>>) {
        let log: Arc<Mutex<Log>> = Arc::default();
        let handle =
            SessionHandle::held(FakeSession { log: Arc::clone(&log), paint: Box::new(paint) }, ExecutionProvider::Cpu);

        (handle, log)
    }

    /// A graph that predicts no colour: all zeros, which is zero chroma on the Ab contract.
    fn no_colour(_: &[f32], output: &mut [f32]) {
        output.fill(0.0);
    }

    /// An Rgb graph that returns the gray it was shown, which is no colour on the Rgb contract.
    fn echo(input: &[f32], output: &mut [f32]) {
        output.copy_from_slice(input);
    }

    /// An Ab graph predicting `(a, b)` at every position.
    fn constant((a, b): (f32, f32)) -> impl Fn(&[f32], &mut [f32]) + Send + Sync + 'static {
        move |_, output| {
            let (first, second) = output.split_at_mut(output.len() / 2);
            first.fill(a);
            second.fill(b);
        }
    }

    /// An Ab graph predicting `inside` where `region(x, y)` holds on its square and `outside` elsewhere.
    fn split(
        region: impl Fn(usize, usize) -> bool + Send + Sync + 'static,
        inside: (f32, f32),
        outside: (f32, f32),
    ) -> impl Fn(&[f32], &mut [f32]) + Send + Sync + 'static {
        move |_, output| {
            let side = ab::SIDE as usize;
            let plane = side * side;

            for y in 0..side {
                for x in 0..side {
                    let (a, b) = if region(x, y) { inside } else { outside };
                    output[y * side + x] = a;
                    output[plane + y * side + x] = b;
                }
            }
        }
    }

    /// A moderate warm colour, in Lab a/b.
    const WARM: (f32, f32) = (20.0, 20.0);

    /// A moderate cool colour, in Lab a/b.
    const COOL: (f32, f32) = (-10.0, -30.0);

    /// The pipeline for `variant`, built through the family's own seam, so what these tests hold is what
    /// `Operation::pipeline` hands the chain driver.
    fn colorizing(variant: ColorizationVariant) -> Shared<Fake> {
        Colorization::new(variant).pipeline::<Fake>()
    }

    /// One variant of each contract.
    const DELHI: ColorizationVariant = ColorizationVariant::Delhi(FloatPrecision::Fp32);
    const JAIPUR: ColorizationVariant = ColorizationVariant::Jaipur(FloatPrecision::Fp32);

    /// `source` colorized by `variant` over `handle` at `depth`, with no progress and no cancellation.
    fn run(
        variant: ColorizationVariant,
        handle: &SessionHandle<FakeSession>,
        source: &DynamicImage,
        depth: ChannelDepth,
    ) -> DynamicImage {
        colorizing(variant)
            .run(source, std::slice::from_ref(handle), depth, None, &|| false)
            .expect("a colorization over a photograph")
    }

    /// A photograph of one colour.
    fn flat(width: u32, height: u32, rgb: [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_pixel(width, height, Rgb(rgb)))
    }

    /// Every pixel of `image` as a 16-bit RGB triple, whatever depth it carries.
    fn pixels(image: &DynamicImage) -> Vec<(u32, u32, [u16; 3])> {
        let sampler = Sampler::new(image);

        (0..image.height())
            .flat_map(|y| (0..image.width()).map(move |x| (x, y)))
            .map(|(x, y)| (x, y, sampler.rgb(x, y)))
            .collect()
    }

    #[test]
    fn each_variant_runs_its_graph_once_at_its_own_shapes_whatever_the_photograph_size() {
        // Larger than either square and smaller than both: the graph is run once at the square either way, never at
        // the photograph's own size, and the result is the photograph's size either way.
        for variant in every_variant() {
            let contract = variant.contract();
            let side = contract.side() as usize;

            for (width, height) in [(700, 600), (40, 30)] {
                let (handle, log) = session(no_colour);
                let result = run(variant, &handle, &photograph(width, height), ChannelDepth::Eight);

                assert_eq!(result.dimensions(), (width, height), "{variant:?} did not keep the photograph's size");

                let log = log.lock().unwrap();
                assert_eq!(log.shapes.len(), 1, "{variant:?} ran its graph {} times", log.shapes.len());
                assert_eq!(
                    log.shapes[0],
                    (GraphShape::new(3, side, side), GraphShape::new(contract.channels(), side, side)),
                    "{variant:?} was not run at its own shapes over a {width}x{height} photograph"
                );
            }

            assert_eq!(ImagePipeline::<Fake>::stages(colorizing(variant).as_ref()), 1, "{variant:?}");
        }
    }

    #[test]
    fn each_model_is_shown_its_own_contracts_gray_rendering() {
        // Bit for bit, on a colour photograph, where gray-Lab and ITU-601 luma are different renderings: a swapped
        // contract, or a pipeline that fed the photograph unstretched or ungrayed, fails here.
        let source = photograph(97, 61);
        let (gray, luma) = (ab::input(&source), rgb::input(&source));

        for variant in every_variant() {
            let expected = match variant.contract() {
                Contract::Ab => &gray,
                Contract::Rgb => &luma,
            };
            let (handle, log) = session(no_colour);

            run(variant, &handle, &source, ChannelDepth::Eight);

            assert!(log.lock().unwrap().inputs[0] == *expected, "{variant:?} was not shown its own contract's input");
        }
    }

    #[test]
    fn an_image_with_no_area_is_refused_before_the_graph_runs() {
        // Both axes separately: a guard on one of them would let the other reach the compose's axis tables as a
        // panic.
        for variant in [DELHI, JAIPUR] {
            for (width, height) in [(0, 8), (8, 0), (0, 0)] {
                let (handle, log) = session(no_colour);
                let empty = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));

                let outcome =
                    colorizing(variant)
                        .run(&empty, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false);

                assert!(
                    matches!(outcome, Err(InferenceError::Untileable { .. })),
                    "{variant:?} over a {width}x{height} image produced {outcome:?}"
                );
                assert!(log.lock().unwrap().shapes.is_empty(), "a refused run reached the graph");
            }
        }
    }

    #[test]
    fn a_model_that_predicts_no_colour_gives_the_photographs_own_gray() {
        // Zero chroma on each contract: the Ab graph writes zeros, and the Rgb graph returns the gray it was shown,
        // whose extracted chroma is zero. What comes back is then the photograph's own lightness and nothing else, at
        // full resolution, which is the property that says the lightness is the photograph's rather than the model's.
        let source = photograph(83, 47);

        for depth in [ChannelDepth::Eight, ChannelDepth::Sixteen] {
            // One level of the result's depth, in the 16-bit samples `pixels` answers.
            let level = if depth == ChannelDepth::Eight { 257 } else { 1 };
            let expected = |gray: f32| match depth {
                ChannelDepth::Eight => i32::from(u8::from_unit(gray)) * 257,
                ChannelDepth::Sixteen => i32::from(u16::from_unit(gray)),
            };

            for (variant, handle) in [(DELHI, session(no_colour).0), (JAIPUR, session(echo).0)] {
                let result = run(variant, &handle, &source, depth);
                let original = Sampler::new(&source);

                for (x, y, pixel) in pixels(&result) {
                    let want = expected(lab::gray(original.rgb(x, y)));
                    let [r, g, b] = pixel.map(i32::from);

                    for got in [r, g, b] {
                        assert!(
                            (got - want).abs() <= level,
                            "{variant:?} at {depth:?}, ({x}, {y}): {pixel:?} is not the photograph's gray {want}"
                        );
                    }
                    assert!(
                        r.max(g).max(b) - r.min(g).min(b) <= level,
                        "{variant:?} at {depth:?}, ({x}, {y}): {pixel:?} is not neutral"
                    );
                }
            }
        }
    }

    #[test]
    fn a_predicted_warm_colour_arrives_and_the_photographs_lightness_is_kept() {
        let source = flat(40, 30, [128; 3]);
        let lightness = lab::lightness(Sampler::new(&source).rgb(0, 0));

        // The rounding of each depth, in L: half a level on each channel moves an 8-bit mid-gray by about 0.2.
        for (depth, rounding) in [(ChannelDepth::Eight, 0.5), (ChannelDepth::Sixteen, 0.01)] {
            let (handle, _) = session(constant(WARM));

            for (x, y, [r, g, b]) in pixels(&run(DELHI, &handle, &source, depth)) {
                assert!(r > b, "{depth:?}, ({x}, {y}): ({r}, {g}, {b}) is not warm");

                let kept = lab::lightness([r, g, b]);
                assert!(
                    (kept - lightness).abs() <= rounding,
                    "{depth:?}, ({x}, {y}): lightness {kept} against the photograph's {lightness}"
                );
            }
        }
    }

    #[test]
    fn a_colour_photograph_is_recoloured_rather_than_blended() {
        // A saturated blue photograph and a model predicting warm: the result is warm, so the photograph's own colour
        // did not survive into it.
        let (handle, _) = session(constant(WARM));

        for (x, y, [r, _, b]) in pixels(&run(DELHI, &handle, &flat(40, 30, [20, 40, 220]), ChannelDepth::Eight)) {
            assert!(r > b, "({x}, {y}): the blue photograph kept its colour ({r} against {b})");
        }
    }

    #[test]
    fn a_photograph_far_from_square_gets_each_region_its_own_colour() {
        // A 64x8 photograph is stretched eight times as much on one axis as the other, so the colour read back lands
        // on the right region only if the compose inverts that stretch axis by axis.
        let source = flat(64, 8, [128; 3]);
        let half = ab::SIDE as usize / 2;
        let quarter = ab::SIDE as usize / 4;

        // Left half of the square warm and right half cool: a transposed axis puts all eight rows' worth of colour on
        // the wrong side.
        let (handle, _) = session(split(move |x, _| x < half, WARM, COOL));
        for (x, y, [r, _, b]) in pixels(&run(DELHI, &handle, &source, ChannelDepth::Eight)) {
            let warm = x < 32;
            assert_eq!(r > b, warm, "({x}, {y}): ({r}, {b}) is on the wrong side of the square's split");
        }

        // Top quarter warm, off the centre line: the photograph's first two rows are warm and the rest cool. A fit
        // that letterboxed the photograph into the middle of the square would read all eight rows as cool.
        let (handle, _) = session(split(move |_, y| y < quarter, WARM, COOL));
        for (x, y, [r, _, b]) in pixels(&run(DELHI, &handle, &source, ChannelDepth::Eight)) {
            let warm = y < 2;
            assert_eq!(r > b, warm, "({x}, {y}): ({r}, {b}) is on the wrong side of the square's split");
        }
    }

    #[test]
    fn a_large_photograph_keeps_its_own_size_and_detail() {
        // A one-pixel checkerboard in the lightness is gone entirely at the square, so a result carrying it can only
        // have taken its lightness from the photograph at full resolution, never from the model's output enlarged.
        let checker = DynamicImage::ImageRgb8(ImageBuffer::from_fn(1600, 1200, |x, y| {
            Rgb(if (x + y) % 2 == 0 { [230_u8; 3] } else { [40; 3] })
        }));

        for (variant, handle) in [(DELHI, session(no_colour).0), (JAIPUR, session(echo).0)] {
            let result = run(variant, &handle, &checker, ChannelDepth::Eight);
            assert_eq!(result.dimensions(), (1600, 1200), "{variant:?} did not keep the photograph's size");

            let sampler = Sampler::new(&result);
            for y in (0..1200).step_by(37) {
                for x in 0..1599 {
                    let step = (lab::lightness(sampler.rgb(x, y)) - lab::lightness(sampler.rgb(x + 1, y))).abs();
                    assert!(step > 50.0, "{variant:?}, ({x}, {y}): the checkerboard was flattened to a step of {step}");
                }
            }
        }
    }

    #[test]
    fn a_sixteen_bit_photograph_is_colorized_at_sixteen_bits() {
        // A gradient finer than an 8-bit channel can hold: a run that narrowed the photograph or the result to 8 bits
        // anywhere would come back with at most 256 levels.
        let gradient = DynamicImage::ImageRgb16(ImageBuffer::from_fn(600, 2, |x, _| Rgb([(x as u16) * 100; 3])));

        for (variant, handle) in [(DELHI, session(no_colour).0), (JAIPUR, session(echo).0)] {
            let result = run(variant, &handle, &gradient, ChannelDepth::Sixteen);
            let result = result.as_rgb16().unwrap_or_else(|| panic!("{variant:?} did not come back as Rgb16"));

            let levels: std::collections::BTreeSet<u16> = result.pixels().map(|pixel| pixel.0[0]).collect();
            assert!(levels.len() > 256, "{variant:?} carried only {} distinct levels", levels.len());
        }
    }

    #[test]
    fn a_photograph_with_transparency_is_colorized_as_its_composite_against_black() {
        let translucent = DynamicImage::ImageRgba8(ImageBuffer::from_fn(37, 23, |x, y| {
            Rgba([(x * 7) as u8, (y * 11) as u8, 120, ((x * 11 + y * 5) % 256) as u8])
        }));

        // The composite at full precision: the premultiplied 16-bit samples `Sampler` answers, which is what the
        // compose reads L from. Flattening to 8 bits instead would round the lightness the comparison is about.
        let composite =
            DynamicImage::ImageRgb16(ImageBuffer::from_fn(37, 23, |x, y| Rgb(Sampler::new(&translucent).rgb(x, y))));

        let warm_rgb = |_: &[f32], output: &mut [f32]| {
            let plane = output.len() / 3;
            for (channel, value) in [0.8, 0.5, 0.3].into_iter().enumerate() {
                output[channel * plane..(channel + 1) * plane].fill(value);
            }
        };

        for (variant, paint) in [(DELHI, Box::new(constant(WARM)) as Paint), (JAIPUR, Box::new(warm_rgb) as Paint)] {
            let (handle, _) = session(paint);
            let result = run(variant, &handle, &translucent, ChannelDepth::Eight);
            let flattened = run(variant, &handle, &composite, ChannelDepth::Eight);

            assert!(
                result.as_rgb8().is_some(),
                "{variant:?} did not come back as colour alone: {:?}",
                result.color()
            );
            assert_eq!(result, flattened, "{variant:?} did not colorize the photograph composited against black");
        }
    }

    #[test]
    fn progress_is_reported_after_each_of_the_three_steps() {
        // Counted, so the last report is exactly the end of the range, and one report lands between the graph run and
        // the finished compose — the half of the schedule the reference's `0 / 1` does not have.
        let (handle, _) = session(no_colour);
        let reported: Mutex<Vec<f64>> = Mutex::default();

        colorizing(DELHI)
            .run(
                &photograph(40, 60),
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                Some(&|fraction| reported.lock().unwrap().push(fraction)),
                &|| false,
            )
            .expect("a colorization over a photograph");

        let reported = reported.into_inner().unwrap();
        assert_eq!(reported, [1.0 / 3.0, 2.0 / 3.0, 1.0], "the reports are not the three counted steps");
        assert!(reported.windows(2).all(|pair| pair[1] >= pair[0]), "progress went backwards: {reported:?}");
    }

    #[test]
    fn a_cancellation_at_any_of_the_three_boundaries_produces_no_image() {
        // Every boundary separately: before the stretch, after the input, and after the graph run. A caller handed a
        // photograph or a partly composed result with no error could not tell it from a finished colorization.
        for boundary in 0..STEPS {
            let (handle, log) = session(no_colour);
            let checked = AtomicUsize::new(0);

            let outcome = colorizing(DELHI).run(
                &photograph(33, 21),
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                None,
                &|| checked.fetch_add(1, Ordering::Relaxed) >= boundary,
            );

            assert!(
                matches!(outcome, Err(InferenceError::Cancelled)),
                "a cancellation at boundary {boundary} produced {outcome:?}"
            );

            let ran = log.lock().unwrap().shapes.len();
            assert_eq!(
                ran,
                usize::from(boundary == 2),
                "a cancellation at boundary {boundary} ran the graph {ran} times"
            );
        }

        // And a run that is never cancelled still returns a photograph, so the loop above is checking a refusal rather
        // than a pipeline that fails whatever it is told.
        let (handle, _) = session(no_colour);
        assert!(
            colorizing(DELHI)
                .run(&photograph(33, 21), std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .is_ok()
        );
    }
}
