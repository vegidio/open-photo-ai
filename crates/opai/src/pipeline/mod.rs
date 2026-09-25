//! The contract every model is written against: what a planned operation runs, as one shape a driver can hold without
//! knowing which model it is.
//!
//! The seam that keeps the two drivers, the chain and the data operation, model-blind. Everything a driver does with an
//! operation between deciding to run it and having its result — what must be on disk, which sessions to open and
//! under which settings, how many model runs the step is, and running it — is asked of the traits here. No driver
//! names a model, a pipeline module or a contract; a model reaches one through
//! [`Operation::pipeline`](crate::models::Operation::pipeline) or
//! [`DataModel::pipeline`](crate::models::operation::sealed::DataModel::pipeline) and is never named again.
//!
//! Beside the traits: [`Backend`], where a run's sessions come from, and [`session`], the only code that feeds a
//! tensor to ONNX Runtime.

// A module of `opai` rather than part of the `imaging` crate, because it is not a leaf: it names sessions, execution
// providers, `InferenceError` and `ArtifactId`, and `session` names `ort`. Models depend on this and on `imaging`, and
// never on `inference`.
//
// `B` is a parameter of the traits rather than of `run`, which is what lets a driver hold a `dyn ImagePipeline<B>` at
// all. Putting it on the method would make the trait non-object-safe and force the caller back to an enum over the
// contracts.
//
// The backend stays generic rather than being erased to a `dyn` object, because erasing it would make
// `Backend::Session` a `Box<dyn Any>` and discard the typed `SessionHandle` that makes the session cache's reference
// count a compile-time guarantee. It also costs nothing: the fake-backend suite instantiates the whole of this on a
// counting fake, so every property stays checked on a runner with no ONNX Runtime, no GPU, no model file and no
// network.

mod backend;
pub(crate) mod session;
#[cfg(test)]
pub(crate) mod test_support;

use std::sync::Arc;

use image::{DynamicImage, ImageBuffer, Rgb};
use imaging::ChannelDepth;
use imaging::present::{Plan, plan, presented};
use imaging::tensor::{Channel, Normalisation};

pub(crate) use backend::Backend;
use session::GraphShape;

use crate::error::InferenceError;
use crate::models::ArtifactId;
use crate::providers::profile::EpProfile;
use crate::sessions::SessionHandle;

/// What both drivers ask of a planned operation's model code, whatever its result is.
///
/// The two questions a driver asks before an operation runs, and nothing else: what must be on disk, and which
/// sessions to open under which settings. Object-safe.
pub(crate) trait Model<B: Backend>: Send + Sync {
    // Object-safe, which is what lets `acquire` be written once over a `&dyn Model<B>` rather than monomorphised per
    // contract. `Send + Sync` because a driver moves a handle to the pipeline onto a blocking thread for the duration
    // of its run and keeps the plan itself on the runtime. Every implementation is plain data, so none pays anything
    // for it.

    /// The distinct artifacts this operation needs on disk, in the order it first needs them.
    ///
    /// What the head of the operation's progress range is divided between, which is why it is the *distinct* ones: an
    /// 8x Kyoto run installs the 4x weights and the 2x weights, and a repeated pass is one artifact served twice
    /// rather than two downloads.
    fn required(&self) -> &[ArtifactId];

    // The one place the two contracts are answered alike, and the reason `acquire` needs no branch. A convolutional
    // pass sequence is several runs of artifacts that share **one** measured profile — Kyoto at 8x is two artifacts
    // from a single measured declaration — while a diffusion graph set is one run of artifacts that each carry **their
    // own**, the transformer's differing from the two VAE halves' in the one per-graph override this project has.
    //
    // A graph opened under another graph's configuration is not an error anything downstream can detect: it produces
    // a working session and a wrong or slower result. So the two are answered alike — as pairs of an artifact and the
    // settings measured *for that artifact* — rather than as a list of artifacts beside one profile, which is the
    // shape that would make the mistake writable.

    /// Every session this operation needs before any of it runs, each paired with the settings it is opened under.
    fn sessions(&self) -> Vec<(&ArtifactId, &EpProfile)>;
}

/// One planned operation whose result is a picture, as the chain driver sees it.
pub(crate) trait ImagePipeline<B: Backend>: Model<B> {
    // `stages` is on this trait rather than on `Model`, because it is read by exactly one thing — `process::run`'s
    // per-step `debug` record, where it distinguishes a two-pass Kyoto run from a three-graph Osaka one — and a data
    // pipeline is always one run and writes its own record. A method on the base that one of the two contracts answers
    // with a constant is a method that only looks shared.

    /// How many model runs this step is — passes for a convolutional operation, graphs for a diffusion one.
    ///
    /// It is not a progress weighting: within an operation the weighting is the pipeline's own business and is by
    /// input pixel area rather than by count.
    fn stages(&self) -> usize;

    // The sessions are borrowed rather than owned, which is what keeps the chain's ledger in the chain: a pipeline
    // cannot retain a handle, drop one, or reorder them, so "a session a run holds cannot be reclaimed" stays a
    // property of the driver's own reference counts. Acquisition stays on the chain side too, which is what reports
    // an install once per operation rather than once per pass.
    //
    // The error fold happens inside `run` rather than at the call site, which is what makes a failed Osaka run read in
    // a log exactly like a failed Kyoto one.

    /// Runs `input` and hands back the picture, or the reason there is none.
    ///
    /// `sessions` are the handles [`sessions`](Model::sessions) asked for, acquired in that order and borrowed for the
    /// duration of the call. A pipeline never acquires a session itself.
    ///
    /// `progress` is reported over the whole operation, `0.0..=1.0`, and is `None` where nobody is listening.
    /// `cancelled` is checked on whatever schedule the pipeline's own unit of work gives it — per tile for a pass
    /// sequence, per region for a diffusion run.
    ///
    /// # Errors
    ///
    /// [`InferenceError`], and in every case **no image at all** — not the passes or regions that had already
    /// succeeded, and not the buffer a cancellation landed in. A pipeline folds its own per-step error type through
    /// [`InferenceError::from_tiling`], which is why each implementation carries the operation's display name.
    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        depth: ChannelDepth,
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DynamicImage, InferenceError>;
}

/// One planned operation whose result is **not** a picture, as the execute driver sees it.
///
/// The output type is the **pipeline's**, not the caller's: an operation names it through
/// [`DataOperation`](crate::DataOperation), so a detection run cannot be asked for anything but faces and the
/// mismatch is not a refusal but something that does not compile.
pub(crate) trait DataPipeline<B: Backend>: Model<B> {
    // A trait of its own rather than `ImagePipeline` generic over the output, because there is no `depth` parameter
    // here. Bits per channel is a property of pixels a caller receives, and a run that hands back coordinates has no
    // answer to give — a single trait would have to carry the parameter anyway and document ignoring it, which is a
    // field a caller can set that does nothing.

    /// What one run of this produces.
    type Output;

    /// Runs `input` and hands back what it found, or the reason there is nothing.
    ///
    /// `sessions` are the handles [`sessions`](Model::sessions) asked for, acquired in that order and borrowed for
    /// the duration of the call, exactly as [`ImagePipeline::run`]'s are and with the same guarantee behind the borrow.
    ///
    /// `progress` is reported over the whole operation, `0.0..=1.0`, and is `None` where nobody is listening.
    /// `cancelled` is checked on whatever schedule the pipeline's own unit of work gives it.
    ///
    /// # Errors
    ///
    /// [`InferenceError`], and in every case **no result at all** — not a partial one, and not an empty one standing
    /// in for the answer. A caller handed an empty set with no error has no way to tell it from a genuine finding.
    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Output, InferenceError>;
}

// An `Arc` rather than a `Box` for one mechanical reason: the run is handed to a blocking thread, which takes a
// `'static` closure, while the plan itself stays on the runtime and is indexed by a loop that can walk backwards over
// a prefix it had skipped. A clone per *operation* is the same idiom the progress reporter beside it already uses,
// and is unmeasurable against a session build costing seconds.
/// A picture-producing pipeline as the chain holds it: one per planned operation, shared with the blocking thread
/// that runs it.
pub(crate) type Shared<B> = Arc<dyn ImagePipeline<B>>;

// An `Arc` for the reason `Shared` is one: the run travels into a `'static` blocking closure while the driver keeps
// reporting on the runtime.
/// A data pipeline as the execute driver holds it.
pub(crate) type SharedData<B, T> = Arc<dyn DataPipeline<B, Output = T>>;

/// A pipeline's own progress callback, with the `Option` unwrapped once.
pub(crate) fn reporter(progress: Option<&dyn Fn(f64)>) -> impl Fn(f64) + '_ {
    // Every pipeline takes an `Option<&dyn Fn(f64)>` and wants a plain `Fn(f64)` to call from the handful of points it
    // reports at. Written inline, that is an `if let` per pipeline whose failure mode is silent — a pipeline that
    // forgot it would drop its progress and still compile, run and pass every test that does not watch the reports. A
    // tiled pipeline mostly leaves the reporting to `run_tiled`, but not always: a multi-pass run marks its own start,
    // because `run_tiled`'s `0.0` is per-pass and must not send the bar back to the beginning at each one. The
    // exception is the loops that accumulate, where the sum advances inside the guard.
    move |fraction| {
        if let Some(progress) = progress {
            progress(fraction);
        }
    }
}

/// Reports `fraction`, then refuses to go on if the run was cancelled: one step boundary of a pipeline that counts its
/// own steps.
///
/// A cancelled run returns no image at all — not the photograph, and not a partly corrected buffer, which a caller has
/// no way to tell from a finished one.
pub(crate) fn checkpoint(
    report: &dyn Fn(f64),
    cancelled: &dyn Fn() -> bool,
    fraction: f64,
) -> Result<(), InferenceError> {
    report(fraction);

    if cancelled() {
        return Err(InferenceError::Cancelled);
    }

    Ok(())
}

// The identity half of every pipeline that runs one graph once, and nothing of its contract: what it does with the
// graph's output stays in the family's own pipeline, which embeds this and answers `OnOneGraph`.
/// A pipeline served by one graph: the artifact, the settings it is opened under, and the name a failure is reported
/// in.
pub(crate) struct SingleGraph {
    /// The single artifact, held as a one-element slice for [`Model::required`].
    artifact: [ArtifactId; 1],
    /// The measured tuning the session is opened under.
    profile: EpProfile,
    /// The operation's display name, for naming it in a failure.
    pub(crate) name: String,
}

impl SingleGraph {
    pub(crate) fn new(name: String, artifact: ArtifactId, profile: EpProfile) -> Self {
        Self { artifact: [artifact], profile, name }
    }
}

/// A pipeline whose [`Model`] answers are its one graph's.
pub(crate) trait OnOneGraph: Send + Sync {
    fn graph(&self) -> &SingleGraph;
}

impl<B: Backend, P: OnOneGraph> Model<B> for P {
    fn required(&self) -> &[ArtifactId] {
        &self.graph().artifact
    }

    /// One artifact against its own profile. The single-graph shape of every family but upscale.
    fn sessions(&self) -> Vec<(&ArtifactId, &EpProfile)> {
        let graph = self.graph();

        vec![(&graph.artifact[0], &graph.profile)]
    }
}

// The depth match every whole-image pipeline opened its `run` with, written once. What each of them says about *why*
// it hoists the branch — a full-resolution loop compiled per channel type rather than branching per pixel over twenty-
// four million of them — is the reason this exists rather than something any one of them decided.
/// A picture-producing pipeline whose body is written once over the channel type and monomorphised for `u8` and
/// `u16`, rather than branching on the depth per pixel.
///
/// Implementing this and [`Model`] is implementing [`ImagePipeline`]: the blanket impl below matches on the requested
/// [`ChannelDepth`] once and hands the result back as the matching [`DynamicImage`].
///
/// Generic over the backend per method rather than per trait: a trait parameter a downstream type could fill is what
/// would stop the blanket impl from coexisting with the pipelines that implement [`ImagePipeline`] directly.
pub(crate) trait DepthGeneric: Send + Sync {
    /// How many model runs this step is; see [`ImagePipeline::stages`].
    fn stages(&self) -> usize;

    /// [`ImagePipeline::run`]'s body, once, at whichever channel the caller asked for.
    ///
    /// # Errors
    ///
    /// As [`ImagePipeline::run`].
    fn run_at<T: Channel, B: Backend>(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImageBuffer<Rgb<T>, Vec<T>>, InferenceError>
    where
        Rgb<T>: image::Pixel<Subpixel = T>;
}

impl<B: Backend, P: DepthGeneric + Model<B>> ImagePipeline<B> for P {
    fn stages(&self) -> usize {
        DepthGeneric::stages(self)
    }

    fn run(
        &self,
        input: &DynamicImage,
        sessions: &[SessionHandle<B::Session>],
        depth: ChannelDepth,
        progress: Option<&dyn Fn(f64)>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DynamicImage, InferenceError> {
        match depth {
            ChannelDepth::Eight => self.run_at::<u8, B>(input, sessions, progress, cancelled).map(u8::into_dynamic),
            ChannelDepth::Sixteen => self.run_at::<u16, B>(input, sessions, progress, cancelled).map(u16::into_dynamic),
        }
    }
}

/// The fixed square a whole-image pipeline runs its one graph at, and the schedule the run reports on.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Square {
    /// The side of the square the graph was exported at, and the only one it accepts.
    pub(crate) side: u32,
    /// How many planes the graph returns at that side.
    pub(crate) planes: usize,
    /// How many progress steps the whole run is. The presentation ends the first and the graph run the second.
    pub(crate) steps: usize,
}

/// What [`SingleGraph::present_and_run`] leaves behind: the tensor the graph was shown, what it returned, and whatever
/// the presentation handed over beside the tensor.
pub(crate) struct Ran<S> {
    /// The three planes the graph was run over.
    pub(crate) tensor: Vec<f32>,
    /// The [`Square::planes`] planes it returned.
    pub(crate) output: Vec<f32>,
    /// What the presentation produced besides the tensor.
    pub(crate) presented: S,
}

/// What [`reflected`] hands over beside the tensor: the geometry it used, and the photograph as the graph saw it.
pub(crate) struct Shown {
    /// The size the photograph was resampled to, and the extension that fills the rest of the square.
    pub(crate) planned: Plan,
    /// The photograph resampled to the planned size, before it was padded.
    pub(crate) resampled: DynamicImage,
}

impl SingleGraph {
    // The first two steps of every pipeline that runs its graph once over a fixed square, whatever it then does with
    // the answer. The order is the one each of them wrote out by hand: nothing is allocated for an empty photograph,
    // a run cancelled before it started does no work, and each of the two steps is a boundary the run reports and
    // checks cancellation at.
    /// Refuses an empty photograph, presents `input` to the graph through `present`, and runs the graph once over the
    /// result, reporting and checking cancellation after each of the two.
    ///
    /// A cancelled run returns no image at all — not the photograph, and not a partly corrected buffer, which a
    /// caller has no way to tell from a finished one.
    ///
    /// # Errors
    ///
    /// [`InferenceError::Untileable`] for a photograph with no area, [`InferenceError::Cancelled`] at any step
    /// boundary, and the graph's own failure folded through [`InferenceError::run`].
    pub(crate) fn present_and_run<B: Backend, S>(
        &self,
        session: &SessionHandle<B::Session>,
        input: &DynamicImage,
        square: Square,
        report: &dyn Fn(f64),
        cancelled: &dyn Fn() -> bool,
        present: impl FnOnce(&DynamicImage) -> (Vec<f32>, S),
    ) -> Result<Ran<S>, InferenceError> {
        let (width, height) = (input.width(), input.height());

        // Before anything is allocated. There is no scaling of an empty photograph onto the square, and a graph run
        // over a square holding nothing but a reflection of nothing is not a correction of anything.
        if width == 0 || height == 0 {
            return Err(InferenceError::Untileable { width, height });
        }

        if cancelled() {
            return Err(InferenceError::Cancelled);
        }

        let side = square.side as usize;
        let (tensor, presented) = present(input);

        checkpoint(report, cancelled, 1.0 / square.steps as f64)?;

        let produced = GraphShape::new(square.planes, side, side);
        let mut output = vec![0.0_f32; produced.len()];
        B::run_graph(session, &tensor, GraphShape::new(3, side, side), &mut output, produced)
            .map_err(InferenceError::run(&self.name, 0))?;

        checkpoint(report, cancelled, 2.0 / square.steps as f64)?;

        Ok(Ran { tensor, output, presented })
    }
}

/// The presentation light adjustment and colour balance share: the photograph's longer side fitted onto the `canvas`
/// square, and the rest of it filled by reflection, as planar CHW in `range`.
///
/// For [`SingleGraph::present_and_run`], and only for a non-empty photograph, which that refuses first.
pub(crate) fn reflected(canvas: u32, range: Normalisation) -> impl FnOnce(&DynamicImage) -> (Vec<f32>, Shown) {
    move |input| {
        let planned = plan(input.width(), input.height(), canvas);
        let mut tensor = vec![0.0_f32; GraphShape::new(3, canvas as usize, canvas as usize).len()];

        // `expect` rather than a folded error, as `run_tiled` does with its own conversion and for the same reason:
        // the scratch is allocated here at exactly the shape the graph is run at, so a disagreement is this function
        // contradicting itself rather than anything a caller could have caused or acted on.
        let resampled = presented(input, planned, range, &mut tensor)
            .expect("the scratch is allocated at the square the graph accepts");

        (tensor, Shown { planned, resampled })
    }
}
