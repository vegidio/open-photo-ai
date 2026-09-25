//! The restoration contract both of this family's models run: one aligned square per face, one graph run, one
//! feathered composite back into the photograph.
//!
//! Progress runs in two steps per face over the whole of `0.0..=1.0` — one when its restoration returns, one when its
//! composite lands — with three cancellation checks per face.

// Family tier rather than either model's, because both variants restore through it. The geometry it sequences —
// `transform`, `warp`, `mask` and `composite` — sits beside it for the same reason. What is left in a model's own
// directory is its measured profile and the prose behind it. A run, per face:
//
//   transform::alignment      the five landmarks fitted onto the ArcFace template, scaled to the tile
//   warp::align               the photograph sampled through the inverse, straight into the graph's tensor
//   run_weighted / run_graph  one graph run: the aligned face, and the fidelity beside it where the model takes one
//   composite::blend          the restoration feathered back into the frame through the forward transform
//
// **None of `imaging`'s drivers fits this**, which is why there is no `run_tiled` call anywhere below. Its unit of
// work is not a region of a grid but a square that exists only after an affine transform; its edge handling is
// reflection inside that transform rather than a mirrored pad of a short region; and its combination step is a
// feathered alpha composite in the aligned square's own space rather than a ramped overlap between neighbours. The
// one seam it does need is the session, and that is `Backend::run_weighted` or `Backend::run_graph` — the family's two
// models differ at exactly that call and nowhere else, and which one a run takes is resolved before it starts by
// `FaceRecoveryVariant::weight`.
//
// Three cancellation checks per face are the only schedule a per-face loop offers. No fifth of the bar is reserved:
// the reference reserves `progressAfterDetect = 0.2` because its `facerecovery` package acquires the detector itself.
// Here detection is a separate operation with its own report, already finished before this one starts, so keeping
// the constant would leave the bar sitting at 20% before the first face and would make an empty selection report 0.2
// and stop.

use std::sync::Arc;

use image::{DynamicImage, ImageBuffer, Rgb};

use super::{FaceRecoveryParams, composite, mask, transform, warp};

use crate::error::InferenceError;
use crate::models::ArtifactId;
use crate::models::face::Faces;
use crate::pipeline::Backend;
use crate::pipeline::session::GraphShape;
use crate::pipeline::{ImagePipeline, OnOneGraph, Shared, SingleGraph, reporter};
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;
use imaging::ChannelDepth;
use imaging::tensor::{Channel, Normalisation, Sampler};

// Not a tunable: the weights are exported at a static `[1, 3, 512, 512]`, and the whole of the alignment — the
// template's scaling, the mask's extent and the composite's bound — is derived from this value.
/// The square each aligned face is restored at.
pub(crate) const TILE: u32 = 512;

// The reference's `standardize: true` on both halves of the conversion. Feeding it the other range is not an error a
// runtime reports; it is a worse restoration.
/// The range this graph's input and output are normalised over: `[-1, 1]`.
const RANGE: Normalisation = Normalisation::Signed;

// Two rather than one because the composite is the slower of the pair on a large photograph — it walks a window in
// the picture's own resolution rather than a fixed square — and folding them together would make that invisible in
// the report.
/// How many progress steps each face is: its restoration, and its composite.
const STEPS_PER_FACE: usize = 2;

/// The pipeline running one face recovery operation, as the family's contract seam hands it back.
///
/// `params` is what [`FaceRecoveryParams`] publishes as a run's own parameters.
///
/// `weight` is what the run binds to the graph's second input, or `None` for a model whose graph takes the image
/// alone. It is resolved by the **variant** rather than read off `params` — see
/// [`FaceRecoveryVariant::weight`](super::FaceRecoveryVariant::weight) for why the two are not the same question.
pub(crate) fn pipeline<B: Backend>(
    name: String,
    artifact: ArtifactId,
    profile: EpProfile,
    params: FaceRecoveryParams,
    weight: Option<f32>,
) -> Shared<B> {
    Arc::new(Restore::new(name, artifact, profile, params, weight))
}

/// One restoration run: the graph it opens, the settings it opens it under, the faces it restores and how
/// closely.
struct Restore {
    /// The graph this operation runs, and the name it reports a failure in.
    graph: SingleGraph,
    /// The faces to restore, in the order the operation carries them.
    faces: Faces,
    /// The weight bound to the graph's second input, or `None` where the graph takes the image alone.
    ///
    /// Resolved once, by the variant, before this pipeline is built — **not** read off
    /// `FaceRecoveryParams::fidelity`; see [`FaceRecoveryVariant::weight`](super::FaceRecoveryVariant::weight).
    weight: Option<f32>,
}

impl Restore {
    /// The pipeline for `artifact` under `profile`, named `name`, over what `params` carries, binding `weight`.
    fn new(
        name: String,
        artifact: ArtifactId,
        profile: EpProfile,
        params: FaceRecoveryParams,
        weight: Option<f32>,
    ) -> Self {
        Self { graph: SingleGraph::new(name, artifact, profile), faces: params.faces, weight }
    }
}

impl OnOneGraph for Restore {
    fn graph(&self) -> &SingleGraph {
        &self.graph
    }
}

impl<B: Backend> ImagePipeline<B> for Restore {
    /// One graph run per face, and none for an empty selection — which is what the driver's per-step record calls a
    /// model run.
    fn stages(&self) -> usize {
        self.faces.len()
    }

    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        depth: ChannelDepth,
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DynamicImage, InferenceError> {
        // Dispatched once at the top, as `run_tiled` dispatches it, so the body below is written once and
        // monomorphised for `u8` and `u16` rather than branching per pixel over the whole photograph.
        match depth {
            ChannelDepth::Eight => self.restored::<u8, B>(input, sessions, progress, cancelled).map(u8::into_dynamic),
            ChannelDepth::Sixteen => {
                self.restored::<u16, B>(input, sessions, progress, cancelled).map(u16::into_dynamic)
            }
        }
    }
}

impl Restore {
    /// [`ImagePipeline::run`]'s body, once, at whichever channel the caller asked for.
    ///
    /// **The photograph is carried across at the requested depth whether or not there is a face**, which is the
    /// empty-selection behaviour the spec pins.
    fn restored<T: Channel, B: Backend>(
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

        // Before anything is allocated.
        if width == 0 || height == 0 {
            return Err(InferenceError::Untileable { width, height });
        }

        let report = reporter(progress);

        // Carried across even for an empty selection. The reference returns its input untouched there and 8-bit NRGBA
        // otherwise; carrying that over would make face recovery the one operation whose output *form* depends on the
        // photograph's contents — a caller asking for sixteen bits would get eight from a chain of one Restore over an
        // image with no face in it, and would get whatever variant the decoder happened to produce rather than the RGB
        // every other operation returns. It costs one full-image conversion for a run that composites nothing.
        let sampler = Sampler::new(input);
        let mut canvas = carried::<T>(&sampler, width, height);

        // A legitimate request rather than an error: a chain applied across a batch meets images with no face in
        // them, and refusing one would fail an export that is otherwise correct. It reports the end of its range
        // rather than sitting partway through it waiting for work that will not happen.
        if self.faces.is_empty() {
            report(1.0);

            return Ok(canvas);
        }

        let mask = mask::plane(TILE);
        let shape = GraphShape::new(3, TILE as usize, TILE as usize);

        // One pair of buffers for the whole run rather than one per face: 3 MB each at 512 square, held for one face
        // at a time rather than for the selection.
        let mut aligned = vec![0.0_f32; shape.len()];
        let mut restored = vec![0.0_f32; shape.len()];

        // Counted rather than accumulated, so the last step lands on exactly 1.0 instead of on whatever a run of
        // additions of `1 / 2n` drifted to — which is what the reference needs its progress clamp for.
        let steps = self.faces.len() * STEPS_PER_FACE;
        let mut done = 0;

        for (index, face) in self.faces.iter().enumerate() {
            if cancelled() {
                return Err(InferenceError::Cancelled);
            }

            // The forward alignment, used as it stands by the composite and inverted inside the warp, which is why both
            // take it in the same form.
            let transform = transform::alignment(&face.landmarks(), TILE);

            // `expect` rather than a folded error, as `run_tiled` does with its own conversion and for the same
            // reason: the buffer is allocated here at exactly the shape the graph is run at, so a disagreement is
            // this function contradicting itself rather than anything a caller could have caused or acted on.
            warp::align(&mut aligned, &sampler, (width, height), transform, TILE, RANGE)
                .expect("the scratch is allocated at the square the graph accepts");

            // The family's one branch, and the reference's `if fidelity >= 0` at the same point in the loop.
            //
            // Matched **here** rather than resolved into a closure or a function pointer above the loop: it is a
            // branch on a value that does not change during a run, which costs nothing any branch predictor has not
            // already paid, where the alternative is an indirect call per face and a lifetime puzzle to name the
            // closure's type over `B`.
            let ran = match self.weight {
                Some(weight) => B::run_weighted(&sessions[0], &aligned, shape, weight, &mut restored, shape),
                None => B::run_graph(&sessions[0], &aligned, shape, &mut restored, shape),
            };

            ran.map_err(InferenceError::run(&self.graph.name, index))?;

            done += 1;
            report(done as f64 / steps as f64);

            if cancelled() {
                return Err(InferenceError::Cancelled);
            }

            composite::blend(&mut canvas, &restored, &mask, transform, face.bounding_box(), TILE, RANGE);

            done += 1;
            report(done as f64 / steps as f64);

            if cancelled() {
                return Err(InferenceError::Cancelled);
            }
        }

        Ok(canvas)
    }
}

/// The photograph's own pixels as an RGB buffer at the requested channel, with alpha composited against black on the
/// way in, which is what every other pipeline in this crate does with it.
fn carried<T: Channel>(sampler: &Sampler<'_>, width: u32, height: u32) -> ImageBuffer<Rgb<T>, Vec<T>>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // Read through `Sampler` rather than through `DynamicImage`'s own accessor, which is typed `Rgba<u8>` and would
    // discard the low byte of every channel of a 16-bit source.
    //
    // Where the source already carries this depth with no alpha to premultiply, the conversion below is the
    // identity performed ten times a pixel — so the buffer is copied wholesale instead. For the ordinary case of
    // an eight-bit photograph carried to an eight-bit result that is one memcpy in place of twenty-four million
    // closure calls on a 24-megapixel source. The equality of the two paths is pinned by the test below.
    if let Some(buffer) = T::matching(sampler) {
        return buffer.clone();
    }

    ImageBuffer::from_fn(width, height, |x, y| {
        let [r, g, b] = sampler.rgb(x, y);

        // `to_unit` rather than the division written out three times: it is `Channel`'s own `[0, 1]` mapping, and
        // it is exactly inverse to the `from_unit` on the other side of each of these.
        Rgb([T::from_unit(r.to_unit()), T::from_unit(g.to_unit()), T::from_unit(b.to_unit())])
    })
}

#[cfg(test)]
mod tests {
    use crate::pipeline::test_support::stub_backend_runs;

    use super::*;

    /// `carried`'s converting path, with the depth-matched shortcut bypassed.
    fn carried_converting<T: Channel>(sampler: &Sampler<'_>, width: u32, height: u32) -> ImageBuffer<Rgb<T>, Vec<T>>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        ImageBuffer::from_fn(width, height, |x, y| {
            let [r, g, b] = sampler.rgb(x, y);

            Rgb([T::from_unit(r.to_unit()), T::from_unit(g.to_unit()), T::from_unit(b.to_unit())])
        })
    }

    #[test]
    fn the_depth_matched_copy_carries_exactly_what_the_conversion_would_have() {
        // Every value an eight-bit channel can hold, not a sample of them: the shortcut's whole claim is that the
        // round trip through `widen`, a divide by 65535 and a multiply by 255 is the identity, and the only honest
        // way to assert that is over the full domain.
        let eight: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(256, 1, |x, _| Rgb([x as u8, 255 - x as u8, (x as u8).wrapping_mul(7)]));
        let source = DynamicImage::ImageRgb8(eight);
        let sampler = Sampler::new(&source);

        let copied: ImageBuffer<Rgb<u8>, Vec<u8>> = carried(&sampler, 256, 1);
        let converted: ImageBuffer<Rgb<u8>, Vec<u8>> = carried_converting(&sampler, 256, 1);

        assert_eq!(copied.as_raw(), converted.as_raw(), "the eight-bit shortcut is not the conversion it replaces");

        // The sixteen-bit pairing, over a spread that includes both ends and the values either side of the
        // midpoint, where a reciprocal that was not exactly representable would show first.
        let wide: ImageBuffer<Rgb<u16>, Vec<u16>> = ImageBuffer::from_fn(512, 1, |x, _| {
            let value = (u32::from(x as u16) * 65535 / 511) as u16;
            Rgb([value, 65535 - value, value ^ 0x5555])
        });
        let source = DynamicImage::ImageRgb16(wide);
        let sampler = Sampler::new(&source);

        let copied: ImageBuffer<Rgb<u16>, Vec<u16>> = carried(&sampler, 512, 1);
        let converted: ImageBuffer<Rgb<u16>, Vec<u16>> = carried_converting(&sampler, 512, 1);

        assert_eq!(
            copied.as_raw(),
            converted.as_raw(),
            "the sixteen-bit shortcut is not the conversion it replaces"
        );
    }

    use std::sync::Mutex;

    use super::super::FaceRecovery;
    use crate::error::SessionError;
    use crate::models::Operation;
    use crate::models::face::tests::face_at;
    use crate::models::fidelity::Fidelity;
    use crate::models::precision::FloatPrecision;
    use crate::providers::ExecutionProvider;

    /// Which session call one face's restoration took, and the weight it carried where it took the weighted one.
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Call {
        // **Which call** rather than only what it was given, because the two mistakes this contract can make are
        // opposite and neither is visible in what comes back: a model with no second input sent down the weighted
        // call and a model with one sent down the single-input call both fail at a real runtime and both pass
        // silently against a fake that recorded the weight alone.
        /// The aligned face and a weight beside it.
        Weighted(f32),
        /// The aligned face alone.
        Graph,
    }

    /// What the fake graph was asked to do, so a test can say what reached it rather than what it returned.
    #[derive(Default)]
    struct Log {
        /// The call each run took, in order.
        calls: Vec<Call>,
        /// The two shapes each run declared, so a run at the wrong square is a failure rather than a silent resize.
        shapes: Vec<(GraphShape, GraphShape)>,
    }

    /// A session standing in for the restoration graph: it records the weight and answers with a known tensor.
    struct FakeSession {
        log: Arc<Mutex<Log>>,
        /// The value every element of the restoration carries, in the graph's own `[-1, 1]` range.
        restores: f32,
    }

    /// A backend with **no ONNX Runtime**, which is what makes the whole of this pipeline exercisable on every CI
    /// platform with no GPU and no model file.
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

        stub_backend_runs!("a restoration run takes a declared-shape seam"; run_tile);

        /// The single-input call, which is the one Santorini's graph takes. See [`Fake::run_weighted`].
        fn run_graph(
            handle: &SessionHandle<Self::Session>,
            input: &[f32],
            input_shape: GraphShape,
            output: &mut [f32],
            output_shape: GraphShape,
        ) -> Result<(), Self::Error> {
            handle.session().served(Call::Graph, input, input_shape, output, output_shape);

            Ok(())
        }

        stub_backend_runs!("a restoration run takes one output"; run_named_outputs);

        /// The weighted call, which is the one Athens' graph takes.
        ///
        /// Synthetic rather than invented inference: what both of these stand in for is the graph's *shape* — one
        /// image tensor, a weight or no weight, one tensor back — so that everything around the run is exercised.
        /// Nothing here is presented as a restoration.
        fn run_weighted(
            handle: &SessionHandle<Self::Session>,
            input: &[f32],
            input_shape: GraphShape,
            weight: f32,
            output: &mut [f32],
            output_shape: GraphShape,
        ) -> Result<(), Self::Error> {
            handle.session().served(Call::Weighted(weight), input, input_shape, output, output_shape);

            Ok(())
        }
    }

    impl FakeSession {
        /// Records `call` and fills `output` with one known value, for whichever of the two seams reached it.
        fn served(
            &self,
            call: Call,
            input: &[f32],
            input_shape: GraphShape,
            output: &mut [f32],
            output_shape: GraphShape,
        ) {
            {
                let mut log = self.log.lock().unwrap();
                log.calls.push(call);
                log.shapes.push((input_shape, output_shape));
            }

            // Refused exactly as the real seams do: a buffer the declared shape does not describe is the mistake
            // they exist to prevent.
            assert_eq!(input.len(), input_shape.len(), "the graph was fed a buffer {input_shape} does not describe");
            assert_eq!(output.len(), output_shape.len(), "the output buffer is not {output_shape}");

            output.fill(self.restores);
        }
    }

    /// A handle on a fake session restoring `restores`, and the log it records into.
    fn session(restores: f32) -> (SessionHandle<FakeSession>, Arc<Mutex<Log>>) {
        let log: Arc<Mutex<Log>> = Arc::default();
        let handle = SessionHandle::held(FakeSession { log: Arc::clone(&log), restores }, ExecutionProvider::Cpu);

        (handle, log)
    }

    /// The typed operation behind a constructor's carrier.
    fn typed(carried: Operation) -> FaceRecovery {
        match carried {
            Operation::FaceRecovery(operation) => operation,
            other => panic!("a face recovery constructor built {other:?}"),
        }
    }

    /// The Athens pipeline over `faces` at `fidelity`.
    ///
    /// A `fidelity` of `None` is a state no constructor offers, so it is reached the way it is actually reached: an
    /// Athens operation persisted before a fidelity was ever chosen, round-tripped through its serialized form.
    fn athens(faces: Faces, fidelity: Option<Fidelity>) -> Shared<Fake> {
        // Built through `FaceRecovery::pipeline` rather than by calling `pipeline` directly, so the weight resolution
        // on the variant row is **inside** what every test below exercises. That is the one thing the two variants
        // differ in, and a resolution that sent one down the other's call would still hand back a pipeline.
        let carried = fidelity.unwrap_or_else(max_fidelity);
        let operation = typed(FaceRecovery::athens(FloatPrecision::Fp32, faces, carried));

        let operation = if fidelity.is_some() { operation } else { stripped_of_its_fidelity(&operation) };

        operation.pipeline::<Fake>()
    }

    /// The Santorini pipeline over `faces`, built through the same seam.
    fn santorini(faces: Faces) -> Shared<Fake> {
        typed(FaceRecovery::santorini(FloatPrecision::Fp32, faces)).pipeline::<Fake>()
    }

    /// `operation` with the fidelity it carries removed, through the one door that can produce such a value.
    ///
    /// The field is `#[serde(default)]`, so an operation persisted before this parameter existed deserializes with
    /// nothing in it. Written as a round trip rather than by reaching into the struct, so what is built here is a
    /// value a user's disk can actually hold.
    fn stripped_of_its_fidelity(operation: &FaceRecovery) -> FaceRecovery {
        let json = serde_json::to_string(operation).expect("an operation serializes");
        let without = json.replace(r#","fidelity":1.0"#, "");

        assert_ne!(without, json, "the test did not remove the fidelity it meant to from {json}");

        serde_json::from_str(&without).expect("an operation carrying no fidelity deserializes")
    }

    /// A photograph with something in every channel, so a pixel carried across unchanged is distinguishable from one
    /// that was written.
    fn photograph(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([((x * 3 + y) % 256) as u8, ((x + y * 5) % 256) as u8, 128])
        }))
    }

    /// A maximum fidelity, which is the value the reference hard-codes for every Athens run.
    fn max_fidelity() -> Fidelity {
        Fidelity::MAXIMUM
    }

    #[test]
    fn an_empty_selection_returns_the_photograph_at_the_requested_depth_rather_than_untouched() {
        // The divergence from the reference this pipeline is built around. It hands its input straight back when
        // there is no face, so a caller asking for sixteen bits would get eight from a chain of one Athens over a
        // photograph with nobody in it — and would get whatever variant the decoder produced rather than the RGB
        // every other operation returns.
        let source = photograph(9, 7);
        let (handle, log) = session(0.0);
        let athens = athens(Faces::empty(), Some(max_fidelity()));

        for (depth, sixteen) in [(ChannelDepth::Eight, false), (ChannelDepth::Sixteen, true)] {
            let produced = athens
                .run(&source, std::slice::from_ref(&handle), depth, None, &|| false)
                .expect("an empty run");

            assert_eq!((produced.width(), produced.height()), (9, 7));
            assert_eq!(
                produced.as_rgb16().is_some(),
                sixteen,
                "{depth:?} did not produce the channel depth that was asked for"
            );

            // And it is the photograph's own pixels, not an empty buffer presented as one.
            let carried = Sampler::new(&produced);
            let original = Sampler::new(&source);
            for y in 0..7 {
                for x in 0..9 {
                    assert_eq!(carried.rgb(x, y), original.rgb(x, y), "({x}, {y}) is not the photograph's pixel");
                }
            }
        }

        assert!(log.lock().unwrap().calls.is_empty(), "an empty selection ran the graph");
        assert_eq!(athens.stages(), 0, "an empty selection reported a model run");
    }

    #[test]
    fn a_run_over_two_faces_runs_the_graph_twice_and_composites_both() {
        // The pipeline end to end with no ONNX Runtime, no GPU and no model file. What the fake supplies is the
        // graph's *shape* — one image tensor, one weight, one tensor back — so everything around the run is real.
        let source = photograph(300, 200);
        let (handle, log) = session(1.0);

        // Two faces far enough apart that their windows do not overlap, so each one's composite is checkable on its
        // own. The restoration is white in the graph's signed range, against a photograph that is not.
        let faces = Faces::new([face_at(30.0, 40.0, 110.0, 130.0), face_at(180.0, 40.0, 260.0, 130.0)]);
        let athens = athens(faces.clone(), Some(max_fidelity()));

        assert_eq!(athens.stages(), 2, "the selection's face count is not what the step record reports");

        let produced = athens
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("a run over two faces");

        {
            let log = log.lock().unwrap();
            assert_eq!(log.calls.len(), 2, "the graph did not run once per face");

            // At the square the weights are exported for, on both sides.
            let shape = GraphShape::new(3, TILE as usize, TILE as usize);
            assert_eq!(log.shapes, vec![(shape, shape); 2], "a face was run at a square the graph does not accept");
        }

        // Both faces landed: each one's own centre took the restoration, and a point well away from either did not.
        let restored = Sampler::new(&produced);
        let original = Sampler::new(&source);

        for face in faces.iter() {
            let centre_x = ((face.bounding_box().min.x + face.bounding_box().max.x) / 2.0) as u32;
            let centre_y = ((face.bounding_box().min.y + face.bounding_box().max.y) / 2.0) as u32;

            assert_ne!(
                restored.rgb(centre_x, centre_y),
                original.rgb(centre_x, centre_y),
                "the face at {:?} was not composited",
                face.bounding_box()
            );
        }

        assert_eq!(restored.rgb(2, 195), original.rgb(2, 195), "a corner far from either face was written");
    }

    #[test]
    fn a_deselected_face_is_left_exactly_as_the_photograph_had_it() {
        // The other half of "a run restores the faces it is given": a user who toggled a face off gets that face's
        // own pixels back, not a restoration of it. Written as the pair of runs rather than as one, because the
        // assertion that matters — the deselected face's box is untouched — would pass on a pipeline that composited
        // nothing at all. The first run is what makes it mean something.
        let source = photograph(300, 200);
        let deselected = face_at(30.0, 40.0, 110.0, 130.0);
        let kept = face_at(180.0, 40.0, 260.0, 130.0);

        let centre = |face: &crate::models::face::Face| {
            let box_of = face.bounding_box();

            (((box_of.min.x + box_of.max.x) / 2.0) as u32, ((box_of.min.y + box_of.max.y) / 2.0) as u32)
        };

        // Both selected: the face about to be deselected is one this pipeline does write over.
        let (handle, _) = session(1.0);
        let both = athens(Faces::new([deselected, kept]), Some(max_fidelity()))
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("a run over both faces");

        let original = Sampler::new(&source);
        let (x, y) = centre(&deselected);
        assert_ne!(
            Sampler::new(&both).rgb(x, y),
            original.rgb(x, y),
            "the face this test deselects is not one a run composites over, so deselecting it proves nothing"
        );

        // Only the second: one graph run, and the first face's box carried across untouched.
        let (handle, log) = session(1.0);
        let athens = athens(Faces::new([kept]), Some(max_fidelity()));

        assert_eq!(athens.stages(), 1, "a deselected face is still counted as a model run");

        let produced = athens
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("a run over one of the two faces");

        assert_eq!(log.lock().unwrap().calls.len(), 1, "the graph ran for a face the operation does not carry");

        // Every pixel of the deselected face's own box, rather than its centre alone: a composite in the wrong place
        // writes an ellipse, and an ellipse sampled at one point is a coin toss.
        let restored = Sampler::new(&produced);
        let box_of = deselected.bounding_box();

        for y in box_of.min.y as u32..box_of.max.y as u32 {
            for x in box_of.min.x as u32..box_of.max.x as u32 {
                assert_eq!(restored.rgb(x, y), original.rgb(x, y), "the deselected face was written at ({x}, {y})");
            }
        }

        // And the face that was kept still landed, so the run restored one of the two rather than neither.
        let (x, y) = centre(&kept);
        assert_ne!(restored.rgb(x, y), original.rgb(x, y), "the selected face was not composited");
    }

    #[test]
    fn the_weight_the_graph_receives_is_the_operations_own_fidelity() {
        // The whole of what this model's fourth session seam adds, and the one thing about it a wrong answer would
        // not report: a graph fed the wrong weight restores a face, just not the face the user asked for.
        let source = photograph(200, 200);
        let faces = Faces::new([face_at(40.0, 40.0, 140.0, 160.0)]);

        for value in [Fidelity::MIN, 0.25, 0.5, Fidelity::MAX] {
            let fidelity = Fidelity::new(value).expect("the test supplied a fidelity in range");
            let (handle, log) = session(0.0);

            athens(faces.clone(), Some(fidelity))
                .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .expect("a run at a stated fidelity");

            assert_eq!(
                log.lock().unwrap().calls,
                vec![Call::Weighted(value as f32)],
                "the graph was not fed the operation's own fidelity through the weighted call"
            );
        }
    }

    #[test]
    fn an_operation_carrying_no_fidelity_restores_at_the_maximum() {
        // `FaceRecoveryParams::fidelity` is an `Option` because the field is `#[serde(default)]`, so a deserialized
        // operation can reach a pipeline with nothing in it. `Fidelity::MAX` is the value the reference hard-codes
        // for every Athens run, so an operation that names none runs exactly as the reference does — where refusing
        // it would be a new public refusal for a state a caller cannot see the cause of, and binding zero would be
        // the *most* aggressive restoration as the default for a value that went missing.
        let source = photograph(200, 200);
        let faces = Faces::new([face_at(40.0, 40.0, 140.0, 160.0)]);
        let (handle, log) = session(0.0);

        athens(faces, None)
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("a run carrying no fidelity");

        assert_eq!(
            log.lock().unwrap().calls,
            vec![Call::Weighted(Fidelity::MAX as f32)],
            "an Athens operation carrying no fidelity did not reach the weighted call at the maximum"
        );
    }

    #[test]
    fn progress_never_decreases_reaches_one_once_and_reports_two_steps_per_face() {
        // Two steps because the composite is the slower of the pair on a large photograph and folding them together
        // would make that invisible in the report, and no reserved head because the detection the reference reserves
        // one for is a separate operation that has already finished.
        let source = photograph(300, 200);
        let (handle, _) = session(0.0);
        let faces = Faces::new([
            face_at(30.0, 40.0, 110.0, 130.0),
            face_at(150.0, 40.0, 230.0, 130.0),
            face_at(40.0, 120.0, 120.0, 190.0),
        ]);

        let watched: Arc<Mutex<Vec<f64>>> = Arc::default();
        let record = |fraction: f64| watched.lock().unwrap().push(fraction);

        athens(faces, Some(max_fidelity()))
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, Some(&record), &|| false)
            .expect("a run over three faces");

        let reported = watched.lock().unwrap().clone();

        assert_eq!(reported.len(), 6, "three faces did not report two steps each: {reported:?}");
        assert!(reported.windows(2).all(|pair| pair[1] >= pair[0]), "progress went backwards: {reported:?}");
        assert_eq!(reported.iter().filter(|fraction| **fraction == 1.0).count(), 1, "1.0 was not reported once");
        assert_eq!(reported.last(), Some(&1.0), "the run did not finish at the end of its range");
        // No fifth of the bar reserved for a detection this operation does not run: the first step is a sixth.
        assert!((reported[0] - 1.0 / 6.0).abs() < 1e-12, "the first step was not a face's share: {reported:?}");
    }

    #[test]
    fn an_empty_selection_reports_the_end_of_its_range_and_nothing_else() {
        // Rather than sitting partway through the range waiting for work that will not happen, which is what
        // keeping the reference's reserved head would produce.
        let source = photograph(16, 16);
        let (handle, _) = session(0.0);

        let watched: Arc<Mutex<Vec<f64>>> = Arc::default();
        let record = |fraction: f64| watched.lock().unwrap().push(fraction);

        athens(Faces::empty(), Some(max_fidelity()))
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, Some(&record), &|| false)
            .expect("an empty run");

        assert_eq!(*watched.lock().unwrap(), vec![1.0]);
    }

    #[test]
    fn a_cancellation_at_any_of_the_three_points_produces_no_image_at_all() {
        // Not the faces already composited, and not the buffer a cancellation landed in: a caller handed a partly
        // restored photograph has no way to tell it from a finished one. Three faces and six checks, so each of the
        // three points in the loop is reached at every face.
        let source = photograph(300, 200);
        let faces = Faces::new([
            face_at(30.0, 40.0, 110.0, 130.0),
            face_at(150.0, 40.0, 230.0, 130.0),
            face_at(40.0, 120.0, 120.0, 190.0),
        ]);

        // Nine checks over three faces, so every one of them is the first `true` in some run — including the last,
        // which is after the final composite and after 1.0 has been reported.
        for stop_at in 0..9 {
            let (handle, _) = session(0.0);
            let seen = std::cell::Cell::new(0_usize);
            let cancelled = || {
                let index = seen.get();
                seen.set(index + 1);

                index == stop_at
            };

            let outcome = athens(faces.clone(), Some(max_fidelity())).run(
                &source,
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                None,
                &cancelled,
            );

            assert!(
                matches!(outcome, Err(InferenceError::Cancelled)),
                "a cancellation at check {stop_at} produced {outcome:?}"
            );
        }

        // And every check is reachable: a run that is never cancelled asks nine times, which is three per face.
        let (handle, _) = session(0.0);
        let asked = std::cell::Cell::new(0_usize);
        let counting = || {
            asked.set(asked.get() + 1);

            false
        };

        athens(faces, Some(max_fidelity()))
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &counting)
            .expect("an uncancelled run");

        assert_eq!(asked.get(), 9, "the loop does not check for cancellation three times per face");
    }

    #[test]
    fn a_run_at_sixteen_bits_returns_a_sixteen_bit_image() {
        // The depth is the caller's and is resolved once for the whole chain, so a face-recovery step in the middle
        // of one must not narrow what the steps around it preserve.
        let source = photograph(200, 200);
        let (handle, _) = session(1.0);
        let faces = Faces::new([face_at(40.0, 40.0, 140.0, 160.0)]);

        let produced = athens(faces, Some(max_fidelity()))
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Sixteen, None, &|| false)
            .expect("a 16-bit run");

        assert!(produced.as_rgb16().is_some(), "a 16-bit run produced {produced:?}");

        // And the restoration reached it at 16-bit precision rather than through an 8-bit round trip: the fake
        // restores the graph's maximum, which decodes to the channel's maximum.
        let restored = Sampler::new(&produced);
        assert_eq!(restored.rgb(90, 100), [u16::MAX; 3], "the restored centre was not written at full precision");
    }

    #[test]
    fn neither_variant_can_reach_the_other_variants_session_call() {
        // The two mistakes the widening can make are opposite, and each is a run-time failure against a real graph
        // and a silent pass against a fake that recorded only what it was given. Both directions in one test,
        // because what each of them is wrong *against* is the other.
        let source = photograph(200, 200);
        let faces = Faces::new([face_at(40.0, 40.0, 140.0, 160.0)]);
        let half = Fidelity::new(0.5).expect("0.5 is in range");

        // Athens carrying a fidelity: the weighted call, with that value and no other.
        let (handle, log) = session(0.0);
        athens(faces.clone(), Some(half))
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("an Athens run at a stated fidelity");
        assert_eq!(
            log.lock().unwrap().calls,
            vec![Call::Weighted(0.5)],
            "Athens did not bind the fidelity it carries"
        );

        // Athens carrying none: still the weighted call, at the maximum. **Not** the single-input one — that is the
        // regression this whole decision exists to prevent, and it is a stored selection that used to run failing
        // after an unrelated change, for a reason the runtime's error does not name.
        let (handle, log) = session(0.0);
        athens(faces.clone(), None)
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("an Athens run carrying no fidelity");
        assert_eq!(
            log.lock().unwrap().calls,
            vec![Call::Weighted(Fidelity::MAX as f32)],
            "a deserialized Athens operation was sent down the single-input path, which its graph refuses"
        );

        // Santorini: the single-input call, and no weight of any value — not a default standing in for the one it
        // does not take.
        let (handle, log) = session(0.0);
        santorini(faces)
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("a Santorini run");
        assert_eq!(
            log.lock().unwrap().calls,
            vec![Call::Graph],
            "Santorini was sent down the weighted path, which its graph has no second input for"
        );
    }

    #[test]
    fn both_variants_frame_and_feather_one_selection_into_the_same_pixels() {
        // The family-wide requirement, stated over the one thing that can be asserted without the weights: given the
        // same photograph, the same selection and the same restoration coming back, the two models composite
        // identically. What a caller chooses between them is how the pixels inside a face are rebuilt — not where
        // the face is framed, how much of the surrounding picture is swept into the aligned square, or how visible
        // the seam is.
        //
        // It holds by construction now that the alignment, the mask and the composite are one shared file — and the
        // point of pinning it is that a later variant reaching for its own template or its own square would break
        // it, which is exactly the change this test is here to fail.
        let source = photograph(300, 200);
        let faces = Faces::new([face_at(30.0, 40.0, 110.0, 130.0), face_at(180.0, 40.0, 260.0, 130.0)]);

        let (weighted_handle, _) = session(0.6);
        let by_athens = athens(faces.clone(), Some(max_fidelity()))
            .run(&source, std::slice::from_ref(&weighted_handle), ChannelDepth::Sixteen, None, &|| false)
            .expect("an Athens run");

        let (plain_handle, _) = session(0.6);
        let by_santorini = santorini(faces)
            .run(&source, std::slice::from_ref(&plain_handle), ChannelDepth::Sixteen, None, &|| false)
            .expect("a Santorini run");

        assert_eq!(
            (by_athens.width(), by_athens.height()),
            (by_santorini.width(), by_santorini.height()),
            "the two models produced different frames"
        );

        let one = Sampler::new(&by_athens);
        let other = Sampler::new(&by_santorini);

        for y in 0..200 {
            for x in 0..300 {
                assert_eq!(
                    one.rgb(x, y),
                    other.rgb(x, y),
                    "({x}, {y}) differs between the two models over one restoration"
                );
            }
        }

        // And the two results are not simply the photograph handed back, which is what would make the comparison
        // above pass on a pipeline that composited nothing at all.
        let original = Sampler::new(&source);
        assert_ne!(one.rgb(70, 85), original.rgb(70, 85), "neither model composited the first face");
    }

    #[test]
    fn a_santorini_run_over_two_faces_performs_two_unweighted_runs_and_composites_both() {
        // The model this slice adds, end to end with no ONNX Runtime, no GPU and no model file — the same traversal
        // the Athens test above makes, through the other of the contract's two calls.
        let source = photograph(300, 200);
        let (handle, log) = session(1.0);

        let faces = Faces::new([face_at(30.0, 40.0, 110.0, 130.0), face_at(180.0, 40.0, 260.0, 130.0)]);
        let restorer = santorini(faces.clone());

        assert_eq!(restorer.stages(), 2, "the selection's face count is not what the step record reports");

        let produced = restorer
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
            .expect("a Santorini run over two faces");

        {
            let log = log.lock().unwrap();

            // **No weight of any value**, rather than a default in place of the one this graph does not take: its
            // second input does not exist, and a weight sent anyway is refused by the runtime rather than ignored.
            assert_eq!(log.calls, vec![Call::Graph; 2], "Santorini did not take the single-input call once per face");

            // At the square the weights are exported for, on both sides — the same square the other model runs at,
            // which is what makes the tile a family-tier constant.
            let shape = GraphShape::new(3, TILE as usize, TILE as usize);
            assert_eq!(log.shapes, vec![(shape, shape); 2], "a face was run at a square the graph does not accept");
        }

        let restored = Sampler::new(&produced);
        let original = Sampler::new(&source);

        for face in faces.iter() {
            let centre_x = ((face.bounding_box().min.x + face.bounding_box().max.x) / 2.0) as u32;
            let centre_y = ((face.bounding_box().min.y + face.bounding_box().max.y) / 2.0) as u32;

            assert_ne!(
                restored.rgb(centre_x, centre_y),
                original.rgb(centre_x, centre_y),
                "the face at {:?} was not composited",
                face.bounding_box()
            );
        }

        assert_eq!(restored.rgb(2, 195), original.rgb(2, 195), "a corner far from either face was written");
    }

    #[test]
    fn an_empty_santorini_selection_returns_the_photograph_at_the_requested_depth_and_runs_no_graph() {
        // The same divergence from the reference the other model states, and it is the contract's rather than
        // either model's: the photograph is carried across at the depth the run was asked for whether or not there
        // is a face in it.
        let source = photograph(9, 7);
        let (handle, log) = session(0.0);
        let restorer = santorini(Faces::empty());

        let watched: Arc<Mutex<Vec<f64>>> = Arc::default();
        let record = |fraction: f64| watched.lock().unwrap().push(fraction);

        for (depth, sixteen) in [(ChannelDepth::Eight, false), (ChannelDepth::Sixteen, true)] {
            let produced = restorer
                .run(&source, std::slice::from_ref(&handle), depth, Some(&record), &|| false)
                .expect("an empty Santorini run");

            assert_eq!((produced.width(), produced.height()), (9, 7));
            assert_eq!(
                produced.as_rgb16().is_some(),
                sixteen,
                "{depth:?} did not produce the channel depth that was asked for"
            );

            let carried = Sampler::new(&produced);
            let original = Sampler::new(&source);
            for y in 0..7 {
                for x in 0..9 {
                    assert_eq!(carried.rgb(x, y), original.rgb(x, y), "({x}, {y}) is not the photograph's pixel");
                }
            }
        }

        assert!(log.lock().unwrap().calls.is_empty(), "an empty selection ran the graph");
        assert_eq!(restorer.stages(), 0, "an empty selection reported a model run");
        assert_eq!(*watched.lock().unwrap(), vec![1.0, 1.0], "an empty run did not report the end of its range");
    }

    #[test]
    fn a_zero_area_photograph_is_refused_for_santorini_too() {
        // An empty selection over an image that exists is a request; an image that does not is not — and the
        // refusal is the contract's rather than either model's, so it is checked through the model that did not
        // write it.
        let (handle, log) = session(0.0);
        let restorer = santorini(Faces::new([face_at(0.0, 0.0, 10.0, 10.0)]));

        for (width, height) in [(0, 8), (8, 0), (0, 0)] {
            let source = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));

            let error = restorer
                .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .expect_err("a photograph with no area was restored");

            assert!(
                matches!(error, InferenceError::Untileable { width: w, height: h } if (w, h) == (width, height)),
                "a {width}x{height} photograph was refused with {error:?}"
            );
        }

        assert!(log.lock().unwrap().calls.is_empty(), "a photograph with no area reached the graph");
    }

    #[test]
    fn a_santorini_run_reports_two_steps_per_face_and_never_decreases() {
        // The progress schedule is the contract's, so it holds for the model that binds no weight exactly as it
        // does for the one that does — two steps per face over the whole range, with no fifth reserved for a
        // detection this operation does not run.
        let source = photograph(300, 200);
        let (handle, _) = session(0.0);
        let faces = Faces::new([
            face_at(30.0, 40.0, 110.0, 130.0),
            face_at(150.0, 40.0, 230.0, 130.0),
            face_at(40.0, 120.0, 120.0, 190.0),
        ]);

        let watched: Arc<Mutex<Vec<f64>>> = Arc::default();
        let record = |fraction: f64| watched.lock().unwrap().push(fraction);

        santorini(faces)
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, Some(&record), &|| false)
            .expect("a Santorini run over three faces");

        let reported = watched.lock().unwrap().clone();

        assert_eq!(reported.len(), 6, "three faces did not report two steps each: {reported:?}");
        assert!(reported.windows(2).all(|pair| pair[1] >= pair[0]), "progress went backwards: {reported:?}");
        assert_eq!(reported.iter().filter(|fraction| **fraction == 1.0).count(), 1, "1.0 was not reported once");
        assert_eq!(reported.last(), Some(&1.0), "the run did not finish at the end of its range");
        assert!((reported[0] - 1.0 / 6.0).abs() < 1e-12, "the first step was not a face's share: {reported:?}");
    }

    #[test]
    fn a_cancelled_santorini_run_produces_no_image_at_any_of_the_three_points() {
        // Not the faces already composited, and not the buffer a cancellation landed in. Three faces and nine
        // checks, so each of the three points in the loop is reached at every face.
        let source = photograph(300, 200);
        let faces = Faces::new([
            face_at(30.0, 40.0, 110.0, 130.0),
            face_at(150.0, 40.0, 230.0, 130.0),
            face_at(40.0, 120.0, 120.0, 190.0),
        ]);

        for stop_at in 0..9 {
            let (handle, _) = session(0.0);
            let seen = std::cell::Cell::new(0_usize);
            let cancelled = || {
                let index = seen.get();
                seen.set(index + 1);

                index == stop_at
            };

            let outcome = santorini(faces.clone()).run(
                &source,
                std::slice::from_ref(&handle),
                ChannelDepth::Eight,
                None,
                &cancelled,
            );

            assert!(
                matches!(outcome, Err(InferenceError::Cancelled)),
                "a cancellation at check {stop_at} produced {outcome:?}"
            );
        }
    }

    #[test]
    fn a_santorini_run_at_sixteen_bits_returns_a_sixteen_bit_image() {
        // The depth is the caller's and is resolved once for the whole chain, so neither of the family's models may
        // narrow what the steps around it preserve.
        let source = photograph(200, 200);
        let (handle, _) = session(1.0);
        let faces = Faces::new([face_at(40.0, 40.0, 140.0, 160.0)]);

        let produced = santorini(faces)
            .run(&source, std::slice::from_ref(&handle), ChannelDepth::Sixteen, None, &|| false)
            .expect("a 16-bit Santorini run");

        assert!(produced.as_rgb16().is_some(), "a 16-bit run produced {produced:?}");

        // And the restoration reached it at 16-bit precision rather than through an 8-bit round trip.
        let restored = Sampler::new(&produced);
        assert_eq!(restored.rgb(90, 100), [u16::MAX; 3], "the restored centre was not written at full precision");
    }

    #[test]
    fn a_zero_area_photograph_is_refused_before_anything_is_allocated() {
        // An empty selection over an image that exists is a request; an image that does not is not. The same
        // refusal the enhancement path makes of an image it cannot partition.
        let (handle, _) = session(0.0);
        let athens = athens(Faces::new([face_at(0.0, 0.0, 10.0, 10.0)]), Some(max_fidelity()));

        for (width, height) in [(0, 8), (8, 0), (0, 0)] {
            let source = DynamicImage::ImageRgb8(ImageBuffer::new(width, height));

            let error = athens
                .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .expect_err("a photograph with no area was restored");

            assert!(
                matches!(error, InferenceError::Untileable { width: w, height: h } if (w, h) == (width, height)),
                "a {width}x{height} photograph was refused with {error:?}"
            );
        }
    }
}
