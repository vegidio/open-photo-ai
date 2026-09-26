//! Running a chain over an open image, and stopping one.
//!
//! [`enhance_with`] is the whole of what the `enhance` command does once the library handle has been taken
//! off managed state, and the order of what it does is its contract.
//!
//! Its records name no run: they are emitted inside the `enhance` command's span, which carries it, and the file
//! writes it onto each of them from there. See `crate::command`.

use std::future::Future;
use std::path::PathBuf;

use opai::{
    Analysis, CancellationToken, Enhanced, ExecuteOptions, Executed, ExecutionProvider, Faces, InferenceError, Opai,
    OutputDepth, Picture, ProcessOptions,
};
use serde::{Deserialize, Serialize};

use super::operation::{Requested, UnknownOperation};
use super::progress::{Handover, Reporting};
use super::slot::Runs;
use crate::command::{Answer, CommandError, Ended};
use crate::faces::{ChainFaces, DetectedFace, for_chain};
use crate::images::{Crop, Opened, load_framed};

/// The processor the window chose for this run: an [`ExecutionProvider`], read from the settings store's spelling.
///
/// The store spells each provider as `keyof SupportedProviders` plus "auto" — `auto`, `cpu`, `coreml`, `cuda`,
/// `tensorrt` — which is the library's own spelling of the same five, lowercased. The library's parse is
/// case-insensitive, so the store's words are read **through it** rather than through a mirror of five arms kept here:
/// a provider the library adds is readable here the moment it is published, and one it renames cannot be matched by a
/// stale arm. The settings store persists these strings, so they do not change; see `frontend/stores/settings.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Processor(pub(crate) ExecutionProvider);

impl TryFrom<String> for Processor {
    type Error = opai::InitError;

    /// The provider `text` names, or the library's own refusal of a name no build published.
    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse().map(Self)
    }
}

impl From<Processor> for ExecutionProvider {
    fn from(processor: Processor) -> Self {
        processor.0
    }
}

/// How a run ended.
///
/// A stop is **not** a failure: a user who changed their mind hasn't had their enhancement break. The window
/// tells the two apart by this tag, not by inspecting a rejection.
///
/// The enhanced arm carries only a description, not the pixels — those are served over the `opai://` scheme,
/// addressed by `identity`. See design.md D4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub(crate) enum Enhancement {
    /// The chain ran and its result is being held.
    Enhanced {
        /// What the result's pixels are addressed by, composed from the source's identity and every operation
        /// applied.
        identity: String,
        /// The result's width in pixels.
        width: u32,
        /// The result's height in pixels.
        height: u32,
        /// Every face found in the framed photograph, for a chain carrying a face recovery — what the window counts
        /// and offers the picker over. Absent for a chain that looked for none, and for one whose detection failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        faces: Option<Vec<DetectedFace>>,
        /// Why the faces could not be found, where they could not: `opai`'s own sentence, untranslated. The recovery
        /// then restored none and the rest of the chain ran — see [`crate::faces::for_chain`].
        #[serde(skip_serializing_if = "Option::is_none")]
        faces_error: Option<String>,
    },
    /// The run was stopped, by the window or by a later run displacing it. No image at all.
    Stopped,
}

impl Answer for Enhancement {
    fn ended(&self) -> Ended<'_> {
        match self {
            Self::Enhanced { .. } => Ended::Finished,
            Self::Stopped => Ended::Stopped,
        }
    }
}

/// Why a run could not be asked for, or could not finish. A stop is deliberately not among these — see
/// [`Enhancement`].
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum EnhanceError {
    /// The window asked for a run before the application had finished starting itself.
    ///
    /// Refused rather than starting initialization here: the setup dialog is what a multi-gigabyte install
    /// happens behind.
    #[error("the application is still starting and cannot run an enhancement yet")]
    NotReady,

    /// The source names an identity this application never admitted — a bug in the interface.
    #[error("no image with identity `{identity}` has been opened")]
    UnknownSource {
        /// What was asked for.
        identity: String,
    },

    /// An operation in the chain names something this application cannot run.
    ///
    /// Carries **where** in the chain as well as what, since the whole chain is judged before any of it runs.
    #[error("operation {index} cannot be run: {reason}")]
    UnknownOperation {
        /// Which position in the chain, counted from zero.
        index: usize,
        /// What could not be served.
        reason: UnknownOperation,
    },

    /// The source was admitted and could not be read: a photograph on a drive that has been ejected.
    #[error("`{identity}` was opened and can no longer be read: {message}")]
    UnreadableSource {
        /// Which admitted file.
        identity: String,
        /// The library's own sentence, untranslated, so it can be copied into a bug report.
        message: String,
    },

    /// The run itself failed; `message` is [`InferenceError`]'s own sentence.
    #[error("{message}")]
    Enhance {
        /// The library's own sentence, untranslated and in full.
        message: String,
    },
}

impl CommandError for EnhanceError {
    fn ended(&self) -> Ended<'_> {
        match self {
            // Refusals this crate makes before `opai` is asked anything.
            Self::NotReady | Self::UnknownSource { .. } | Self::UnknownOperation { .. } => {
                Ended::Failed { error: self, recorded: false }
            }
            // `opai` records the failed decode and the failed run, where each happened.
            Self::UnreadableSource { .. } | Self::Enhance { .. } => Ended::Failed { error: self, recorded: true },
        }
    }
}

/// What this module asks of the library, so the rules above can be tested without a real [`Opai`].
pub(crate) trait Enhancer {
    /// Runs `operations` over `source`, as [`Opai::process`] does.
    fn process(
        &self,
        source: &Picture,
        operations: &[opai::Operation],
        options: ProcessOptions,
    ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send;

    /// Runs `analysis` over `source`, as [`Opai::execute`] does: the detection a chain carrying a face recovery makes
    /// for its faces — see [`crate::faces::for_chain`].
    ///
    /// **The default finds nobody**, which is what every fake that is not about faces wants: a chain carrying no face
    /// recovery never asks, and one that does restores an empty selection. [`Opai`] runs the real detection.
    fn detect(
        &self,
        _source: &Picture,
        _analysis: &Analysis,
        _options: ExecuteOptions,
    ) -> impl Future<Output = Result<Executed<Faces>, InferenceError>> + Send {
        std::future::ready(Ok(Executed {
            value: Faces::empty(),
            providers: opai::ProviderReport { requested: ExecutionProvider::Auto, actual: Vec::new() },
        }))
    }
}

impl Enhancer for Opai {
    fn process(
        &self,
        source: &Picture,
        operations: &[opai::Operation],
        options: ProcessOptions,
    ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
        Opai::process(self, source, operations, Some(options))
    }

    fn detect(
        &self,
        source: &Picture,
        analysis: &Analysis,
        options: ExecuteOptions,
    ) -> impl Future<Output = Result<Executed<Faces>, InferenceError>> + Send {
        Opai::execute(self, source, analysis, Some(options))
    }
}

/// What one request to enhance carries, gathered so the seam below takes one argument rather than five.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// The window's name for this run, minted before the call — see design.md D3.
    pub(crate) run: String,
    /// The identity of the image to enhance, as the window already addresses its pixels by.
    pub(crate) source: String,
    /// The chain to run, in order, each operation over the result of the one before it.
    pub(crate) operations: Vec<Requested>,
    /// What to run on.
    pub(crate) processor: Processor,
    /// How the image is framed, or `None` to run over the whole photograph.
    pub(crate) crop: Option<Crop>,
}

/// A chain judged whole and the source it runs over found: what [`prepare_chain`] goes on from.
pub(crate) struct Judged<'a> {
    /// The identity the source was asked for by.
    source: &'a str,
    /// The chain as the window asked for it, which a face recovery's choice of faces is read from.
    requested: &'a [Requested],
    /// `requested`, resolved.
    operations: Vec<opai::Operation>,
    /// Where the source is on disk.
    path: PathBuf,
}

/// Judges the whole of `requested` and resolves `source`: the refusals a canvas run and an export make before anything
/// is displaced, registered or read.
///
/// The whole chain comes first, so a bad second operation doesn't first fetch a model for the first.
///
/// # Errors
///
/// [`EnhanceError::UnknownOperation`], naming the first operation that cannot be served, then
/// [`EnhanceError::UnknownSource`].
pub(crate) fn judge_chain<'a>(
    opened: &Opened,
    source: &'a str,
    requested: &'a [Requested],
) -> Result<Judged<'a>, EnhanceError> {
    let operations = requested
        .iter()
        .enumerate()
        .map(|(index, requested)| {
            requested.resolve().map_err(|reason| EnhanceError::UnknownOperation { index, reason })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let path = opened
        .resolve(source)
        .ok_or_else(|| EnhanceError::UnknownSource { identity: source.to_string() })?;

    Ok(Judged { source, requested, operations, path })
}

/// What a judged chain runs over, and with, once [`prepare_chain`] has framed its source and found its faces.
pub(crate) struct Prepared {
    /// The framed source: the chain's input, and what every cache key is folded from.
    pub(crate) picture: Picture,
    /// The chain, with every face recovery handed the faces it keeps.
    pub(crate) operations: Vec<opai::Operation>,
    /// What finding the faces came to — see [`for_chain`].
    pub(crate) faces: ChainFaces,
    /// The bar the detection reported onto the head of, whose [`Handover::chain`] the chain reports through.
    pub(crate) handover: Option<Handover>,
}

/// Decodes and frames a judged chain's source, then finds the faces its face recoveries restore — steps shared by a
/// canvas run and an export, taken once the caller has claimed whatever a stop reaches the run through.
///
/// The detection runs on `processor` under `cancel`, so it is stopped with the chain and runs where it would, and
/// reports onto the head of `on_progress`'s bar — see [`Handover`]. `None` is a run stopped during the detection,
/// before the chain began.
///
/// # Errors
///
/// [`EnhanceError::UnreadableSource`]. Not logged here: `opai` records the failed decode, where it happened.
pub(crate) async fn prepare_chain<E: Enhancer>(
    enhancer: &E,
    opened: &Opened,
    judged: Judged<'_>,
    crop: Option<Crop>,
    processor: Processor,
    cancel: CancellationToken,
    on_progress: Option<&opai::OnInference>,
) -> Result<Option<Prepared>, EnhanceError> {
    let Judged { source, requested, operations, path } = judged;

    let picture = load_framed(opened, source, path, crop)
        .await
        .map_err(|error| EnhanceError::UnreadableSource { identity: source.to_string(), message: error.to_string() })?;

    let handover = on_progress.map(Handover::new);
    let detecting = ExecuteOptions {
        provider: processor.into(),
        on_progress: handover.as_ref().map(Handover::detection),
        cancel,
        ..Default::default()
    };

    let Ok((operations, faces)) = for_chain(enhancer, &picture, requested, operations, detecting).await else {
        return Ok(None);
    };

    Ok(Some(Prepared { picture, operations, faces, handover }))
}

/// Runs a chain over an open image, and answers what it produced without its pixels.
///
/// The whole of what the [`enhance`](super::enhance) command does once the library handle has been taken
/// off managed state.
///
/// # The order of what happens is the contract
///
/// ```text
///   1. judge the whole chain        --> a refusal here fetches nothing, for any operation
///   2. resolve the source           --> a refusal here displaces nothing
///   3. write the slot               --> the run in flight stops; the held result is dropped
///   4. decode the source
///   5. frame it                     --> the chain's input, and what every cache key is folded from
///   6. find the faces               --> only for a chain carrying a face recovery; stoppable like the run
///   7. run
/// ```
///
/// # A face recovery finds its own faces
///
/// Step 6 is [`for_chain`]: the chain detects inside this request rather than the window detecting first and asking
/// second, so the whole of it is one stop and one progress stream, the detection on the head of the bar. The faces
/// found come back beside the result for the window to count and offer, and a detection that failed comes back as the
/// reason, with the chain run anyway over no faces.
///
/// A refused request must not displace the run the user is watching (1, 2 before 3). The slot is written
/// before decoding (3 before 4) so a displaced run stops immediately rather than after a slow decode, and so
/// dropping the held result happens the moment the next run is *asked for* rather than when it completes —
/// bounding resident cost to one. See design.md D2, D4.
///
/// # The framing is applied here, and `opai` never learns about it
///
/// The crop is applied to the loaded picture before [`Enhancer::process`] sees it, which is what makes every
/// downstream consequence follow for free: the chain runs over the framed pixels, each step's cache key is
/// folded from the framed picture's identity, and the result's identity is composed from it. A `crop` field
/// on `ProcessOptions` would instead put a second way of composing an identity inside the function that
/// already composes one, and `process`'s input would stop being the picture its caller handed it.
///
/// It comes *after* the slot is written for the same reason the decode does: a full-resolution warp of a
/// sixty-megapixel photograph is slow work, and a run displaced during it must already have stopped. See
/// design.md D7.
///
/// # Errors
///
/// [`EnhanceError`]. A stop is not one of them: it is [`Enhancement::Stopped`].
pub(crate) async fn enhance_with<E: Enhancer>(
    enhancer: &E,
    opened: &Opened,
    runs: &Runs,
    reporting: Option<Reporting>,
    request: Request,
) -> Result<Enhancement, EnhanceError> {
    let judged = judge_chain(opened, &request.source, &request.operations)?;

    // Past every refusal: this is the point the previous run stops and its result goes.
    let cancel = runs.start(&request.run, &request.source);

    // A stop that overtook its own run — answered before anything is decoded or fetched.
    if cancel.is_cancelled() {
        tracing::debug!("a run was stopped before it started");

        return Ok(Enhancement::Stopped);
    }

    let on_progress = reporting.as_ref().map(|reporting| &reporting.on_progress);
    let Some(Prepared { picture, operations, faces, handover }) =
        prepare_chain(enhancer, opened, judged, request.crop, request.processor, cancel.clone(), on_progress).await?
    else {
        // Stopped during the detection, before the chain began.
        if let Some(reporting) = &reporting {
            reporting.release();
        }
        tracing::debug!("a run was stopped while its faces were being found");

        return Ok(Enhancement::Stopped);
    };

    // `..Default::default()` keeps this source-compatible as `ProcessOptions` grows; `cache` stays on by
    // default so a re-run doesn't pay twice.
    //
    // `OutputDepth::Eight` diverges from that type's own advice to default to `Source`, deliberately: this
    // result is drawn in a webview, which can't show more than eight bits per channel, so sixteen would only
    // double resident cost for nothing drawable. Export asks for `Source` instead.
    let options = ProcessOptions {
        provider: request.processor.into(),
        depth: OutputDepth::Eight,
        on_progress: handover.as_ref().map(Handover::chain),
        cancel,
        ..Default::default()
    };

    let outcome = enhancer.process(&picture, &operations, options).await;

    // Released whatever the run did, so the window's indicator lands where the run actually got to.
    if let Some(reporting) = &reporting {
        reporting.release();
    }

    match outcome {
        Ok(Enhanced { picture, .. }) => {
            let (width, height) = picture.dimensions();
            let identity = picture.identity().to_string();

            // Cancellation is cooperative and checked between tiles, so a run displaced mid-flight can still
            // answer `Ok` without noticing (e.g. a fully-cached chain). Its pixels were already dropped by the
            // run that displaced it, so report it as stopped rather than hand back a `GONE` address.
            if !runs.finish(&request.run, picture) {
                tracing::debug!("a run was displaced before it finished; it reports as stopped");

                return Ok(Enhancement::Stopped);
            }

            let (faces, faces_error) = match faces {
                ChainFaces::NotAsked => (None, None),
                ChainFaces::Found(found) => (Some(found), None),
                ChainFaces::Failed(message) => (None, Some(message)),
            };

            Ok(Enhancement::Enhanced { identity, width, height, faces, faces_error })
        }
        // Not a failure — a user who changed their mind and a run displaced on their behalf both arrive here.
        Err(InferenceError::Cancelled) => {
            tracing::debug!("a run was stopped");

            Ok(Enhancement::Stopped)
        }
        Err(error) => Err(EnhanceError::Enhance { message: error.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use super::*;
    use crate::enhance::slot::Resident;
    use crate::enhance::test_support::{enhanced, fixture, framed_request, kyoto, request, run_now};
    use crate::test_support::{boxed, executed};

    use opai::{Family, Precision};

    /// An enhancer that answers with a picture of its own and records what it was asked to do — the chain, the
    /// provider and the depth, the half of the seam a fake can check without a model on disk.
    #[derive(Default)]
    struct Recording {
        seen: Mutex<Vec<(Vec<opai::Operation>, ExecutionProvider, OutputDepth)>>,
    }

    impl Enhancer for Recording {
        fn process(
            &self,
            source: &Picture,
            operations: &[opai::Operation],
            options: ProcessOptions,
        ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
            self.seen
                .lock()
                .expect("unpoisoned")
                .push((operations.to_vec(), options.provider, options.depth));

            // An identity of the enhanced result's shape, without reimplementing `opai`'s own composition.
            let identity = if operations.is_empty() {
                source.identity().to_string()
            } else {
                format!("{:0>16x}", operations.len())
            };
            let result = enhanced(source, 8, 4, &identity);

            async move { Ok(result) }
        }
    }

    /// An enhancer that works until the run is stopped, then reports `Cancelled` — what `opai` itself does.
    struct Interruptible;

    impl Enhancer for Interruptible {
        async fn process(
            &self,
            _source: &Picture,
            _operations: &[opai::Operation],
            options: ProcessOptions,
        ) -> Result<Enhanced, InferenceError> {
            options.cancel.cancelled().await;

            Err(InferenceError::Cancelled)
        }
    }

    /// An enhancer that is displaced while it works and answers without ever having noticed — reachable
    /// because cancellation is checked only between tiles, and a fully cached chain checks nothing at all.
    struct Displaced(Arc<Runs>);

    impl Enhancer for Displaced {
        fn process(
            &self,
            source: &Picture,
            _operations: &[opai::Operation],
            _options: ProcessOptions,
        ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
            self.0.start("run-2", source.identity());

            let result = enhanced(source, 4, 2, "cafebabecafebabe");

            async move { Ok(result) }
        }
    }

    /// An enhancer whose run fails for a reason that is not a cancellation.
    struct Failing;

    impl Enhancer for Failing {
        async fn process(
            &self,
            _source: &Picture,
            _operations: &[opai::Operation],
            _options: ProcessOptions,
        ) -> Result<Enhanced, InferenceError> {
            Err(InferenceError::Untileable { width: 0, height: 0 })
        }
    }

    /// An enhancer that answers a picture twice the size of the one it was given, named after it — the shape
    /// `opai`'s own `identity_after` composes a result's identity in.
    ///
    /// The `Recording` above cannot serve these tests: it answers a fixed size at an identity composed from
    /// the chain's *length*, so what it was handed is invisible in what it answers. What a framing has to be
    /// visible in is exactly those two things.
    #[derive(Default)]
    struct Doubling {
        /// The dimensions, identity and transparency of every picture this was handed, in order.
        seen: Mutex<Vec<(u32, u32, String, bool)>>,
    }

    impl Enhancer for Doubling {
        fn process(
            &self,
            source: &Picture,
            _operations: &[opai::Operation],
            _options: ProcessOptions,
        ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
            let (width, height) = source.dimensions();

            self.seen.lock().expect("unpoisoned").push((
                width,
                height,
                source.identity().to_string(),
                source.pixels().color().has_alpha(),
            ));

            let result = enhanced(source, width * 2, height * 2, &format!("{}-enhanced", source.identity()));

            async move { Ok(result) }
        }
    }

    #[test]
    fn a_chain_over_an_admitted_source_runs_and_answers_what_it_produced() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Recording::default();

        let outcome = run_now(&enhancer, &opened, &runs, request("run-1", &identity, vec![kyoto(), kyoto()]))
            .expect("an admitted source and a published chain should run");

        let Enhancement::Enhanced { identity: produced, width, height, .. } = outcome else {
            panic!("a completed run was not reported as enhanced");
        };

        assert_ne!(
            produced, identity,
            "the result carries the source's own identity, so a chain is invisible in it"
        );
        assert_eq!((width, height), (8, 4), "the dimensions answered are not the result's own");

        // What reached the library: the chain in order, the window's processor, and the drawable depth (D7).
        let seen = enhancer.seen.lock().expect("unpoisoned");
        assert_eq!(seen.len(), 1, "the chain was run more or less than once");
        assert_eq!(seen[0].0.len(), 2, "the chain did not arrive whole");
        assert_eq!(seen[0].1, ExecutionProvider::CoreMl, "the window's processor was not the one used");
        assert_eq!(seen[0].2, OutputDepth::Eight);

        // The pixels are held, addressed by the identity that was answered.
        assert!(matches!(runs.resolve(&produced), Some(Resident::Picture(_))));
    }

    #[test]
    fn a_source_that_was_never_opened_is_refused_and_nothing_is_run() {
        let opened = Opened::default();
        let runs = Runs::default();
        let enhancer = Recording::default();

        let refused = run_now(&enhancer, &opened, &runs, request("run-1", "0123456789abcdef", vec![kyoto()]))
            .expect_err("an unadmitted identity is not something this application may read");

        assert_eq!(refused, EnhanceError::UnknownSource { identity: "0123456789abcdef".to_string() });
        assert!(
            enhancer.seen.lock().expect("unpoisoned").is_empty(),
            "a refused request still reached the library"
        );
    }

    #[test]
    fn a_refused_request_does_not_displace_the_run_the_user_is_watching() {
        // Every refusal happens before the slot is written, so a malformed request doesn't stop the run in
        // flight as a side effect.
        let opened = Opened::default();
        let runs = Runs::default();
        let in_flight = runs.start("run-1", "cafebabecafebabe");

        let refused =
            run_now(&Recording::default(), &opened, &runs, request("run-2", "0123456789abcdef", vec![kyoto()]))
                .expect_err("an unadmitted identity is refused");

        assert!(matches!(refused, EnhanceError::UnknownSource { .. }));
        assert!(!in_flight.is_cancelled(), "a refused request stopped the run that was actually running");
    }

    #[test]
    fn an_operation_the_application_cannot_run_is_refused_by_position() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Recording::default();

        // The second operation is the one that cannot be served; nothing is fetched for the first, since the
        // whole chain is judged before any of it begins.
        let chain =
            vec![kyoto(), Requested::named(opai::Family::Upscale, "berlin", Precision::Fp32, &[("scale", 2.0)])];

        let refused = run_now(&enhancer, &opened, &runs, request("run-1", &identity, chain))
            .expect_err("a chain naming an unpublished model is refused");

        assert_eq!(
            refused,
            EnhanceError::UnknownOperation {
                index: 1,
                reason: UnknownOperation::Model { family: Family::Upscale, codename: "berlin".to_string() },
            },
            "the refusal did not say which operation could not be served"
        );
        assert!(enhancer.seen.lock().expect("unpoisoned").is_empty(), "a model was fetched for a refused chain");
    }

    #[test]
    fn an_empty_chain_is_accepted_and_denotes_the_source() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Recording::default();

        // A user who toggled every enhancement off is asking a meaningful question, not making a mistake.
        let outcome = run_now(&enhancer, &opened, &runs, request("run-1", &identity, Vec::new()))
            .expect("an empty chain is a request rather than a mistake");

        let Enhancement::Enhanced { identity: produced, .. } = outcome else {
            panic!("an empty chain was not reported as enhanced");
        };

        // Applying nothing derives nothing — `opai`'s own rule for an empty chain.
        assert_eq!(produced, identity, "an empty chain answered an identity other than the source's");
        assert_eq!(enhancer.seen.lock().expect("unpoisoned")[0].0.len(), 0);
    }

    #[test]
    fn a_run_displaced_while_it_worked_reports_as_stopped_rather_than_as_an_enhancement() {
        let (_dir, opened, runs, identity) = fixture();
        let runs = Arc::new(runs);

        // Answers `Ok` having never noticed it was displaced; its pixels went when the successor took the
        // slot, so it must not be reported as an enhancement pointing at a `GONE` address.
        let outcome =
            run_now(&Displaced(Arc::clone(&runs)), &opened, &runs, request("run-1", &identity, vec![kyoto()]))
                .expect("the run itself did not fail");

        assert_eq!(
            outcome,
            Enhancement::Stopped,
            "a displaced run reported an enhancement whose pixels nobody kept"
        );
        assert!(
            matches!(runs.resolve("cafebabecafebabe"), Some(Resident::Displaced)),
            "the displaced run's result was written back into the slot the successor holds"
        );
    }

    #[test]
    fn a_run_in_flight_is_stopped_by_name_and_reported_as_stopped() {
        let (_dir, opened, runs, identity) = fixture();
        let (opened, runs) = (Arc::new(opened), Arc::new(runs));

        let outcome = tauri::async_runtime::block_on(async {
            let running = spawn_run(Interruptible, &opened, &runs, request("run-1", &identity, vec![kyoto()]));

            // Waited for rather than slept past, so the stop lands while the run is actually in flight.
            settled(&runs, "run-1").await;

            // Names the run, so it can't land on a successor started meanwhile.
            assert!(runs.stop("run-1"), "the run in flight was not the one the stop found");

            running.await.expect("the run should not panic")
        })
        .expect("a stop is not a failure");

        assert_eq!(outcome, Enhancement::Stopped, "a stopped run was not reported as stopped");
    }

    /// Drives `enhance_with` on a task of its own, so the caller can act on the slot while it works.
    fn spawn_run<E: Enhancer + Send + Sync + 'static>(
        enhancer: E,
        opened: &Arc<Opened>,
        runs: &Arc<Runs>,
        request: Request,
    ) -> tauri::async_runtime::JoinHandle<Result<Enhancement, EnhanceError>> {
        let opened = Arc::clone(opened);
        let runs = Arc::clone(runs);

        tauri::async_runtime::spawn(async move { enhance_with(&enhancer, &opened, &runs, None, request).await })
    }

    /// Waits until `run` is the one the slot holds. A blocking sleep loop, for the same reason `setup.rs`'s
    /// own concurrency tests use one: no timer crate, and the loop is bounded by the actual condition.
    async fn settled(runs: &Runs, run: &str) {
        while !runs.holds(run) {
            tauri::async_runtime::spawn_blocking(|| std::thread::sleep(std::time::Duration::from_millis(1)))
                .await
                .expect("the sleep should run");
        }
    }

    #[test]
    fn a_stop_that_overtakes_its_own_run_stops_it_before_anything_is_decoded() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Recording::default();

        // The stop arrives first — the two cross the boundary independently. D3.
        runs.stop("run-1");

        let outcome = run_now(&enhancer, &opened, &runs, request("run-1", &identity, vec![kyoto()]))
            .expect("a stop is not a failure");

        assert_eq!(outcome, Enhancement::Stopped);
        assert!(
            enhancer.seen.lock().expect("unpoisoned").is_empty(),
            "a run nobody was waiting for went on to reach the library anyway"
        );
    }

    #[test]
    fn a_stop_naming_a_displaced_run_leaves_the_one_in_flight_alone() {
        let (_dir, opened, runs, identity) = fixture();

        // A cleanup for a run already displaced, arriving after its successor started. This is what
        // `cancel_enhance` does; the command is two lines over it and a log.
        runs.start("run-1", &identity);
        runs.stop("run-1");

        let outcome = run_now(&Recording::default(), &opened, &runs, request("run-2", &identity, vec![kyoto()]))
            .expect("the successor should run");

        assert!(
            matches!(outcome, Enhancement::Enhanced { .. }),
            "a stop for a displaced run stopped its successor"
        );
    }

    #[test]
    fn a_stopped_run_and_a_failed_run_are_reported_distinguishably() {
        let (_dir, opened, _runs, identity) = fixture();

        // A run that broke is an error, carrying the library's own sentence.
        let failed = run_now(&Failing, &opened, &Runs::default(), request("run-1", &identity, vec![kyoto()]))
            .expect_err("a run that broke is a failure");

        let EnhanceError::Enhance { message } = &failed else {
            panic!("a failed run was reported as something other than a failure: {failed:?}");
        };
        assert_eq!(message, &InferenceError::Untileable { width: 0, height: 0 }.to_string());

        // A stop is not an error at all — a user who changed their mind hasn't been told their enhancement
        // broke.
        let runs = Runs::default();
        runs.stop("run-2");
        let stopped = run_now(&Failing, &opened, &runs, request("run-2", &identity, vec![kyoto()]))
            .expect("a stop is not a failure");

        assert_eq!(stopped, Enhancement::Stopped);

        // Pinned on the wire as well, since the window branches on the serialized shape.
        let stopped = serde_json::to_value(&stopped).expect("an outcome serializes");
        let failed = serde_json::to_value(&failed).expect("a failure serializes");
        assert_eq!(stopped, serde_json::json!({ "outcome": "stopped" }));
        assert_eq!(failed["kind"], "enhance", "a failure and a stop are not told apart by their tags");
    }

    #[test]
    fn a_second_run_stops_the_first_without_the_window_asking() {
        let (_dir, opened, runs, identity) = fixture();
        let (opened, runs) = (Arc::new(opened), Arc::new(runs));

        let (first, second) = tauri::async_runtime::block_on(async {
            let first = spawn_run(Interruptible, &opened, &runs, request("run-1", &identity, vec![kyoto()]));

            settled(&runs, "run-1").await;

            // Nothing asked for the first run to stop — it stops because a second was asked for. D2.
            let second =
                enhance_with(&Recording::default(), &opened, &runs, None, request("run-2", &identity, vec![kyoto()]))
                    .await;

            (first.await.expect("the displaced run should not panic"), second)
        });

        assert_eq!(first.expect("a displaced run is not a failure"), Enhancement::Stopped);
        assert!(matches!(second.expect("the successor should run"), Enhancement::Enhanced { .. }));
    }

    #[test]
    fn the_processors_the_window_names_are_the_ones_the_library_runs() {
        for (named, provider) in [
            ("auto", ExecutionProvider::Auto),
            ("cpu", ExecutionProvider::Cpu),
            ("coreml", ExecutionProvider::CoreMl),
            ("cuda", ExecutionProvider::Cuda),
            ("tensorrt", ExecutionProvider::TensorRt),
        ] {
            let parsed: Processor = serde_json::from_value(serde_json::json!(named))
                .unwrap_or_else(|error| panic!("`{named}` is how the settings store spells a processor: {error}"));

            assert_eq!(ExecutionProvider::from(parsed), provider);
        }

        // A name no build ever published is refused, as the library refuses it, rather than run on something else.
        assert!(serde_json::from_value::<Processor>(serde_json::json!("openvino")).is_err());
    }

    // ── The framing a run is given ────────────────────────────────────────────────────────────────────────

    /// A framing of the 8x6 fixture, injected: nothing in this application can set one until slice 3.
    fn framing(left: u32, top: u32, width: u32, height: u32) -> Crop {
        Crop::new(left, top, width, height, 0, false, false).expect("a rectangle with area is a legal crop")
    }

    #[test]
    fn a_run_over_a_framed_image_enhances_the_framing() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Doubling::default();

        let outcome = run_now(
            &enhancer,
            &opened,
            &runs,
            framed_request("run-1", &identity, vec![kyoto()], framing(2, 1, 4, 3)),
        )
        .expect("an admitted source and a published chain should run");

        let Enhancement::Enhanced { width, height, .. } = outcome else {
            panic!("a completed run was not reported as enhanced");
        };

        // The fixture is 8x6 and the doubling enhancer would have answered 16x12 for the whole of it. The
        // chain ran over the framing, so what came back is twice the rectangle.
        assert_eq!((width, height), (8, 6), "the chain ran over the whole photograph rather than the framing");

        let seen = enhancer.seen.lock().expect("unpoisoned");
        assert_eq!((seen[0].0, seen[0].1), (4, 3), "the library was handed the photograph rather than the framing");
        assert_ne!(seen[0].2, identity, "the framed pixels were handed over under the whole photograph's name");
    }

    #[test]
    fn two_framings_of_one_photograph_answer_two_results() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Doubling::default();

        let left = run_now(
            &enhancer,
            &opened,
            &runs,
            framed_request("run-1", &identity, vec![kyoto()], framing(0, 0, 4, 6)),
        );
        let right = run_now(
            &enhancer,
            &opened,
            &runs,
            framed_request("run-2", &identity, vec![kyoto()], framing(4, 0, 4, 6)),
        );

        let (Ok(Enhancement::Enhanced { identity: left, .. }), Ok(Enhancement::Enhanced { identity: right, .. })) =
            (left, right)
        else {
            panic!("both runs should have enhanced something");
        };

        // Neither can be served where the other was asked for, which is what stops work done for one framing
        // being reused for another — the per-operation cache keys off the identity the chain was given.
        assert_ne!(left, right, "two framings of one photograph answered one result");
    }

    #[test]
    fn the_same_chain_over_the_same_framing_answers_the_same_result() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Doubling::default();
        let crop = framing(1, 1, 5, 4);

        let first = run_now(&enhancer, &opened, &runs, framed_request("run-1", &identity, vec![kyoto()], crop));
        let second = run_now(&enhancer, &opened, &runs, framed_request("run-2", &identity, vec![kyoto()], crop));

        let (Ok(Enhancement::Enhanced { identity: first, .. }), Ok(Enhancement::Enhanced { identity: second, .. })) =
            (first, second)
        else {
            panic!("both runs should have enhanced something");
        };

        // Stable, not merely unique: a re-run has to find what the first run cached, which it can only do if
        // the framed input is named the same way both times.
        assert_eq!(first, second, "one framing of one photograph answered two results");
    }

    #[test]
    fn a_request_carrying_no_crop_runs_over_the_whole_photograph() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Doubling::default();

        let outcome = run_now(&enhancer, &opened, &runs, request("run-1", &identity, vec![kyoto()]))
            .expect("an admitted source and a published chain should run");

        let Enhancement::Enhanced { identity: produced, width, height, .. } = outcome else {
            panic!("a completed run was not reported as enhanced");
        };

        assert_eq!((width, height), (16, 12), "a run with no framing did not run over the whole photograph");

        {
            // Scoped, because the enhancer below takes this same lock: the run after it would otherwise
            // wait on a guard this test is still holding.
            let seen = enhancer.seen.lock().expect("unpoisoned");
            assert_eq!((seen[0].0, seen[0].1), (8, 6), "the library was handed something other than the file's pixels");
            assert_eq!(seen[0].2, identity, "the photograph was renamed by a framing that was never asked for");
        }

        assert_eq!(produced, format!("{identity}-enhanced"), "the result was not named after the photograph");
    }

    #[test]
    fn a_framing_that_changes_nothing_answers_the_run_already_computed() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Doubling::default();

        let first = run_now(&enhancer, &opened, &runs, request("run-1", &identity, vec![kyoto()]))
            .expect("an admitted source and a published chain should run");

        // No flip, no turn, and a rectangle covering the whole of the 8x6 fixture. The Crop/Rotate dialog can
        // be opened and dismissed having changed nothing, and that must not discard everything already
        // computed for the photograph — so the framed picture has to be the source itself, identity included,
        // which is what lets the library's per-operation cache answer the second run from the first.
        let second = run_now(
            &enhancer,
            &opened,
            &runs,
            framed_request("run-2", &identity, vec![kyoto()], framing(0, 0, 8, 6)),
        )
        .expect("a framing that changes nothing is a run like any other");

        assert_eq!(second, first, "a framing that changed nothing minted a second result for one photograph");

        // The other half of it, and the half a `gui` test can actually see: what the library was handed the
        // second time is the photograph under its own name, not a copy of it renamed by the framing.
        let seen = enhancer.seen.lock().expect("unpoisoned");
        assert_eq!(
            (seen[1].0, seen[1].1, &seen[1].2),
            (8, 6, &identity),
            "a framing that changed nothing handed the library a picture of its own"
        );
    }

    #[test]
    fn a_turned_framing_is_run_rather_than_refused() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = Doubling::default();

        // A turn exposes corners no source pixel covers, so the chain is handed `Rgba8` where the file was
        // `Rgb8`. The pipelines have always accepted an alpha-bearing picture — a PNG with transparency can
        // reach them straight off disk — and this asserts it rather than assuming it.
        let turned = Crop::new(0, 0, 10_000, 10_000, 45_000, false, false).expect("legal");

        let outcome = run_now(&enhancer, &opened, &runs, framed_request("run-1", &identity, vec![kyoto()], turned))
            .expect("a turned framing is a picture like any other");

        assert!(matches!(outcome, Enhancement::Enhanced { .. }), "a turned framing was not run");
        assert!(
            enhancer.seen.lock().expect("unpoisoned")[0].3,
            "the transparency a turn produced was lost on the way"
        );
    }

    #[test]
    fn each_ending_is_recorded_by_whoever_saw_it() {
        let s = String::new;
        let reason = UnknownOperation::Model { family: opai::Family::Upscale, codename: s() };
        let cells = [
            (EnhanceError::NotReady, "recorded by the wrapper"),
            (EnhanceError::UnknownSource { identity: s() }, "recorded by the wrapper"),
            (EnhanceError::UnknownOperation { index: 0, reason }, "recorded by the wrapper"),
            (EnhanceError::UnreadableSource { identity: s(), message: s() }, "recorded by opai"),
            (EnhanceError::Enhance { message: s() }, "recorded by opai"),
        ];
        for (error, cell) in cells {
            assert_eq!(CommandError::ended(&error).cell(), cell, "{error:?}");
        }

        let answer = Enhancement::Enhanced { identity: s(), width: 1, height: 1, faces: None, faces_error: None };
        assert_eq!(Answer::ended(&answer).cell(), "finished");
        assert_eq!(Answer::ended(&Enhancement::Stopped).cell(), "stopped");
    }

    #[test]
    fn an_answer_that_found_no_faces_keeps_the_wire_shape_it_always_had() {
        // Pinned before the faces were added beside it, so a chain carrying no face recovery answers exactly as before.
        let answer = Enhancement::Enhanced {
            identity: "cafebabecafebabe".to_string(),
            width: 600,
            height: 400,
            faces: None,
            faces_error: None,
        };

        assert_eq!(
            serde_json::to_value(answer).expect("an answer serializes"),
            serde_json::json!({ "outcome": "enhanced", "identity": "cafebabecafebabe", "width": 600, "height": 400 })
        );
        assert_eq!(
            serde_json::to_value(Enhancement::Stopped).expect("an answer serializes"),
            serde_json::json!({ "outcome": "stopped" })
        );
    }

    /// An enhancer whose detection finds one small face, and which records the chain it was asked to run.
    #[derive(Default)]
    struct FindingOne {
        seen: Mutex<Vec<Vec<opai::Operation>>>,
    }

    impl FindingOne {
        fn face() -> opai::Face {
            boxed(1.0, 1.0, 5.0, 5.0)
        }
    }

    impl Enhancer for FindingOne {
        async fn process(
            &self,
            source: &Picture,
            operations: &[opai::Operation],
            _options: ProcessOptions,
        ) -> Result<Enhanced, InferenceError> {
            self.seen.lock().expect("unpoisoned").push(operations.to_vec());

            Ok(enhanced(source, 8, 6, "cafebabecafebabe"))
        }

        async fn detect(
            &self,
            _source: &Picture,
            _analysis: &Analysis,
            _options: ExecuteOptions,
        ) -> Result<Executed<Faces>, InferenceError> {
            Ok(executed(Faces::new([Self::face()])))
        }
    }

    #[test]
    fn a_face_recovery_finds_its_faces_inside_the_run_and_answers_them() {
        let (_dir, opened, runs, identity) = fixture();
        let enhancer = FindingOne::default();
        let recovery = Requested::named(Family::FaceRecovery, "santorini", Precision::Fp32, &[]);

        let answer =
            run_now(&enhancer, &opened, &runs, request("run-1", &identity, vec![recovery])).expect("the chain runs");

        let Enhancement::Enhanced { faces: Some(faces), faces_error: None, .. } = answer else {
            panic!("the faces found were not answered: {answer:?}");
        };
        assert_eq!(faces, [DetectedFace::from(FindingOne::face())]);

        // The chain ran over the face the detection found, which is restorable and not skipped.
        let seen = enhancer.seen.lock().expect("unpoisoned");
        assert_eq!(
            seen.as_slice(),
            [vec![opai::FaceRecovery::santorini(
                opai::FloatPrecision::Fp32,
                Faces::new([FindingOne::face()])
            )]]
        );
    }

    #[test]
    fn a_chain_carrying_no_face_recovery_answers_no_faces() {
        let (_dir, opened, runs, identity) = fixture();

        let answer = run_now(&FindingOne::default(), &opened, &runs, request("run-1", &identity, vec![kyoto()]))
            .expect("the chain runs");

        assert!(matches!(answer, Enhancement::Enhanced { faces: None, faces_error: None, .. }), "{answer:?}");
    }

    #[test]
    fn the_librarys_units_are_beneath_the_commands_span_and_carry_its_run() {
        let (_dir, opened, runs, identity) = fixture();

        let (recorded, answer) = crate::test_support::recorded(|| {
            tauri::async_runtime::block_on(crate::command::traced(
                crate::command::command_span!("enhance", crate::command::Traceparent::default(), run = "run-1"),
                enhance_with(&Recording::default(), &opened, &runs, None, request("run-1", &identity, vec![kyoto()])),
            ))
        });
        assert!(matches!(answer, Ok(Enhancement::Enhanced { .. })), "{answer:?}");

        let command = recorded.only("enhance");
        assert_eq!(command.field("run"), Some("run-1"));
        assert_eq!(command.field("outcome"), Some("finished"));

        // The photograph's decode is `opai`'s unit, opened on a blocking thread of the library's own.
        let load = recorded.only("image_load");
        assert_eq!(load.target, "opai::unit");
        assert!(
            recorded.descends_from(load.index, command.index),
            "the unit rooted a trace of its own: {recorded:#?}"
        );
    }
}
