//! Asking for an open photograph to be analysed for Autopilot, and withdrawing the question.
//!
//! Two commands, [`suggest`] and [`cancel_suggest`], the seam beneath the first, and [`Analyses`], the [`Stops`]
//! table the second reaches into. The analysis itself — narrowing, ordering, the face signal, cancellation — is
//! `opai`'s [`Opai::suggest`]; what this module decides is which open photograph is analysed, at which framing, and
//! under what name the window can stop it.
//!
//! # Why this is its own module and not part of `enhance` or `faces`
//!
//! It holds no result, touches neither [`Runs`](crate::enhance::Runs) nor the `opai://` scheme, and runs no chain.
//! It is not a second arm on `detect_faces` either: the two answer different questions and fail differently — a
//! detection cannot be stopped, and an analysis must be. What it shares with both is the order it does things in,
//! and that is shared as code through [`load_framed`]. See design.md D1.
//!
//! # What the framing does to an analysis
//!
//! The crop is applied before the analysis sees the picture, exactly as it is before a detection. So the face
//! signal's detection is stored under the identity `detect_faces` asks for at that framing, and the faces the
//! window sizes after an analysis are served from the run store rather than detected again. On a photograph nobody
//! has cropped, the framed picture is the photograph. See design.md D2.
//!
//! # No progress is reported
//!
//! The one slow step an analysis can take is a first-use transfer of the face detector, and reporting it on
//! `enhance:progress` would drive the canvas chip, which names an enhancement being run. See design.md D5.
//!
//! # No record names its run
//!
//! Every record here is emitted inside the `suggest` or `cancel_suggest` command's span, which carries the run, and the
//! file writes it onto each of them from there. See `crate::command`.

use std::future::Future;

use opai::{ExecuteOptions, Family, InferenceError, Opai, Picture, Scale, Suggestion, Suggestions};
use serde::Serialize;
use tauri::State;

use crate::command::{Answer, CommandError, Ended, Traceparent, command_span, traced, traced_sync};
use crate::enhance::Processor;
use crate::images::{Crop, Opened, load_framed};
use crate::setup::Setup;
use crate::stops::Stops;

/// One enhancement an analysis concluded a photograph calls for, as it crosses to the window.
///
/// **`gui`'s own rather than a `Serialize` on [`Suggestion`]**, which would fix a wire shape in the library for one
/// front end. Tagged by `family` in snake_case, which is how an operation spells its family on the way in:
///
/// ```text
/// [{ "family": "colorization" }, { "family": "upscale", "scale": 4.0 }]
/// ```
///
/// Serialize-only: the window never sends a suggestion back, it turns one into an operation. See design.md D4.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub(crate) enum SuggestionWire {
    /// See [`Suggestion::Denoise`].
    Denoise,
    /// See [`Suggestion::FaceRecovery`].
    FaceRecovery,
    /// See [`Suggestion::Colorization`].
    Colorization,
    /// See [`Suggestion::LightAdjustment`].
    LightAdjustment,
    /// See [`Suggestion::ColorBalance`].
    ColorBalance,
    /// See [`Suggestion::Sharpen`].
    Sharpen,
    /// See [`Suggestion::Upscale`].
    Upscale {
        /// How much to enlarge by, as a number.
        scale: Scale,
    },
}

impl From<Suggestion> for SuggestionWire {
    fn from(suggestion: Suggestion) -> Self {
        // One arm per arm and no wildcard, so a suggestion `opai` gains later fails to compile here rather than being
        // dropped on the way to the window.
        match suggestion {
            Suggestion::Denoise => Self::Denoise,
            Suggestion::FaceRecovery => Self::FaceRecovery,
            Suggestion::Colorization => Self::Colorization,
            Suggestion::LightAdjustment => Self::LightAdjustment,
            Suggestion::ColorBalance => Self::ColorBalance,
            Suggestion::Sharpen => Self::Sharpen,
            Suggestion::Upscale { scale } => Self::Upscale { scale },
        }
    }
}

/// Why a photograph's suggestions could not be answered.
///
/// The same three refusals a detection has before it runs, the analysis's own failure, and one a detection does
/// not have: [`Stopped`](Self::Stopped), so the window can stay silent for a photograph it closed and report one
/// that genuinely failed.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum SuggestError {
    /// The window asked before the application had finished starting itself.
    #[error("the application is still starting and cannot analyse a photograph yet")]
    NotReady,

    /// The source names an identity this application never admitted — a bug in the interface.
    #[error("no image with identity `{identity}` has been opened")]
    UnknownSource {
        /// What was asked for.
        identity: String,
    },

    /// The source was admitted and could not be read: a photograph on a drive that has been ejected.
    #[error("`{identity}` was opened and can no longer be read: {message}")]
    UnreadableSource {
        /// Which admitted file.
        identity: String,
        /// The library's own sentence, untranslated, so it can be copied into a bug report.
        message: String,
    },

    /// The window withdrew the question, or the application is shutting down. No suggestions are answered, not
    /// even ones already determined.
    #[error("the analysis was stopped")]
    Stopped,

    /// The analysis itself failed; `message` is [`InferenceError`]'s own sentence.
    #[error("{message}")]
    Analyse {
        /// The library's own sentence, untranslated and in full.
        message: String,
    },
}

impl CommandError for SuggestError {
    fn ended(&self) -> Ended<'_> {
        match self {
            Self::Stopped => Ended::Stopped,
            // Refusals this crate makes before `opai` is asked anything.
            Self::NotReady | Self::UnknownSource { .. } => Ended::Failed { error: self, recorded: false },
            // `opai` records the failed decode and the failed analysis, where each happened.
            Self::UnreadableSource { .. } | Self::Analyse { .. } => Ended::Failed { error: self, recorded: true },
        }
    }
}

impl Answer for Vec<SuggestionWire> {}

// ── The cancellation table ────────────────────────────────────────────────────────────────────────────────────────

/// The kind of job [`Analyses`] holds. Never constructed; it only keeps this table a type of its own.
pub(crate) enum Analysis {}

/// The analyses in flight, by the window's name for each, and the stops that overtook their own analysis.
///
/// A [`Stops`] of its own rather than one shared with export, so `cancel_suggest` can never stop an export.
pub(crate) type Analyses = Stops<Analysis>;

// ── The command ───────────────────────────────────────────────────────────────────────────────────────────────────

/// What this module asks of the library, so the rules above can be tested without a real [`Opai`].
///
/// The same shape `faces`' [`Detector`](crate::faces::Detector) seam takes, for the same reason: what this module
/// decides is the order it does things in, and that is checkable without a detector on disk.
pub(crate) trait Analyser {
    /// Analyses `source`, narrowed to `families`, as [`Opai::suggest`] does.
    fn suggest(
        &self,
        source: &Picture,
        families: &[Family],
        options: ExecuteOptions,
    ) -> impl Future<Output = Result<Suggestions, InferenceError>> + Send;
}

impl Analyser for Opai {
    fn suggest(
        &self,
        source: &Picture,
        families: &[Family],
        options: ExecuteOptions,
    ) -> impl Future<Output = Result<Suggestions, InferenceError>> + Send {
        // Always `Some`: the window always names a set, because the settings store always has one, and an empty one
        // is a real answer ("check nothing") rather than "every family". See design.md D4.
        Opai::suggest(self, source, Some(families), Some(options))
    }
}

/// What one request to analyse carries, gathered so the seam below takes one argument rather than five.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// The window's name for this analysis, minted before the call — what a stop names.
    pub(crate) run: String,
    /// The identity of the image to analyse, as the window already addresses its pixels by.
    pub(crate) source: String,
    /// What to run on.
    pub(crate) processor: Processor,
    /// How the image is framed, or `None` to analyse the whole photograph.
    pub(crate) crop: Option<Crop>,
    /// The families the analysis may suggest. Empty checks nothing.
    pub(crate) families: Vec<Family>,
}

/// Analyses an open photograph as it is framed, and answers what it calls for.
///
/// The whole of what the [`suggest`] command does once the library handle has been taken off managed state.
///
/// # The order of what happens is the contract
///
/// ```text
///   1. register the run             --> a stop that arrived first answers `stopped`, nothing read
///   2. resolve the source           --> a refusal here decodes nothing and fetches nothing
///   3. decode and frame it          --> what is analysed, and what the run store keys on
///   4. stopped during the decode?   --> `stopped`
///   5. analyse
///   6. stopped while analysing?     --> `stopped`, even if an answer came back
///   7. deregister (on drop)
/// ```
///
/// Nothing is displaced by any of it: an analysis holds no slot, so it cannot disturb a run in flight or another
/// analysis. See design.md D6.
///
/// # Errors
///
/// [`SuggestError`]. An empty answer is not one of them, and neither is an incomplete analysis: its reason is
/// logged and what was read is answered.
pub(crate) async fn suggest_with<A: Analyser>(
    analyser: &A,
    opened: &Opened,
    analyses: &Analyses,
    request: Request,
) -> Result<Vec<SuggestionWire>, SuggestError> {
    let Some(registration) = analyses.register(&request.run) else {
        tracing::debug!("an analysis was stopped before it started");

        return Err(SuggestError::Stopped);
    };

    let path = opened
        .resolve(&request.source)
        .ok_or_else(|| SuggestError::UnknownSource { identity: request.source.clone() })?;

    // Not logged here, nor the analysis's failure or incompleteness below: `opai` records each once, where it
    // happened.
    let picture = load_framed(opened, &request.source, path, request.crop).await.map_err(|error| {
        SuggestError::UnreadableSource { identity: request.source.clone(), message: error.to_string() }
    })?;

    // A decode is not cancellable, so a stop that landed during it is noticed here rather than after an analysis
    // nobody will read.
    if registration.cancel().is_cancelled() {
        tracing::debug!("an analysis was stopped while its photograph was decoded");

        return Err(SuggestError::Stopped);
    }

    // `..Default::default()` keeps this source-compatible as `ExecuteOptions` grows. `cache` stays on, which is what
    // lets `detect_faces` be served the face signal's detection, and `on_progress` stays `None` — D5.
    let options = ExecuteOptions {
        provider: request.processor.into(),
        cancel: registration.cancel().clone(),
        ..Default::default()
    };

    let outcome = analyser.suggest(&picture, &request.families, options).await;

    // Cancellation is cooperative, so an analysis whose every signal was already known can answer `Ok` without
    // noticing a stop. The window withdrew the question either way, and an answer to it must not be applied.
    if registration.cancel().is_cancelled() {
        tracing::debug!("an analysis was stopped while it ran");

        return Err(SuggestError::Stopped);
    }

    match outcome {
        // The reason an analysis was incomplete stays out of the answer — the window applies what came back — and out of
        // this log: `opai`'s conclusion names the unread signal and why. See design.md D4.
        Ok(Suggestions { suggested, .. }) => {
            tracing::debug!(suggested = suggested.len(), "a photograph was analysed");

            Ok(suggested.into_iter().map(SuggestionWire::from).collect())
        }
        Err(InferenceError::Cancelled | InferenceError::Shutdown) => {
            tracing::debug!("an analysis was stopped while it ran");

            Err(SuggestError::Stopped)
        }
        Err(error) => Err(SuggestError::Analyse { message: error.to_string() }),
    }
}

// The command's name is written once more, in `frontend/ipc/autopilot.ts`.
/// Analyse an open image, as the user has framed it, for the enhancements it calls for.
///
/// `run` is the window's own name for this analysis, minted before the call — a stop and its analysis cross the
/// boundary independently and either may arrive first.
///
/// `families` is the set the analysis may suggest from. Empty checks nothing and answers nothing, which is not a
/// failure.
///
/// `crop` is how the user has framed that image, or absent to analyse the whole photograph.
///
/// Answers the suggestions in the order the application applies enhancements in.
///
/// # Errors
///
/// [`SuggestError`]. A stopped analysis is [`SuggestError::Stopped`].
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its wire shape plus its managed state"
)]
#[tauri::command]
pub(crate) async fn suggest(
    run: String,
    source: String,
    processor: Processor,
    families: Vec<Family>,
    crop: Option<Crop>,
    setup: State<'_, Setup<Opai>>,
    opened: State<'_, Opened>,
    analyses: State<'_, Analyses>,
    traceparent: Traceparent,
) -> Result<Vec<SuggestionWire>, SuggestError> {
    traced(command_span!("suggest", traceparent, run = run), async move {
        // A clone of the handle rather than the handle, and the lock released before the analysis starts — see
        // `Setup::peek`.
        let opai = setup.peek(Opai::clone).await.ok_or(SuggestError::NotReady)?;

        suggest_with(&opai, &opened, &analyses, Request { run, source, processor, crop, families }).await
    })
    .await
}

// The command's name is written once more, in `frontend/ipc/autopilot.ts`.
/// Stop an analysis the window asked for.
///
/// **Names the analysis**, and touches no other one and no enhancement run. A stop that arrives **before** its own
/// analysis still lands on it — see [`Analyses`].
///
/// Answers nothing and fails at nothing; whether anything was stopped is only in the log.
#[tauri::command]
pub(crate) fn cancel_suggest(run: String, analyses: State<'_, Analyses>, traceparent: Traceparent) {
    traced_sync(command_span!("cancel_suggest", traceparent, run = run), || {
        if analyses.stop(&run) {
            tracing::debug!("an analysis in flight was stopped from the window");
        } else {
            tracing::debug!("a stop named an analysis that is not in flight");
        }
    });
}

/// The scale an upscale added by hand arrives at, for a photograph framed to `width` x `height`: `opai`'s ladder, with
/// the neutral 1x where the photograph is already big enough.
///
/// **The ladder is the library's**, [`opai::suggested_scale`], and the same one an analysis suggests an upscale by —
/// so the add menu and Autopilot cannot disagree about what the same photograph calls for, and the window restates
/// neither threshold. The window asks with the **framed** dimensions, the size that will actually be upscaled.
///
/// 1x rather than "nothing" above the ladder, because the question here is not *whether* to upscale — the user has
/// already added one — but what it should start at, and a factor that changes nothing is the conservative start.
fn default_scale(width: u32, height: u32) -> f64 {
    opai::suggested_scale(width, height).map_or(Scale::MIN, Scale::get)
}

/// The scale a newly added upscale starts at, for a photograph framed to `width` x `height`.
///
/// Synchronous and infallible: two integers in, one factor out. The command's name is written once more, in
/// `frontend/ipc/autopilot.ts`.
#[tauri::command]
pub(crate) fn suggested_scale(width: u32, height: u32, traceparent: Traceparent) -> f64 {
    traced_sync(command_span!("suggested_scale", traceparent), || default_scale(width, height))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_hand_added_upscale_starts_on_the_librarys_ladder_and_at_1x_above_it() {
        assert_eq!(default_scale(1024, 1024), 4.0);
        assert_eq!(default_scale(2048, 2048), 2.0);
        assert_eq!(default_scale(6000, 4000), 1.0);
        // The framed size is what the window asks with, so a cut of a large photograph gets the small one's answer.
        assert_eq!(default_scale(800, 600), 4.0);
    }

    use std::sync::Mutex;

    use super::*;
    use crate::images::cropped;
    use crate::test_support::{Captured, admitted};

    /// What a recording analyser was handed on one call.
    #[derive(Debug, Clone)]
    struct Seen {
        picture: Picture,
        families: Vec<Family>,
        provider: opai::ExecutionProvider,
    }

    /// An analyser that answers a fixed result and records every call.
    struct Recording {
        seen: Mutex<Vec<Seen>>,
        answer: fn() -> Result<Suggestions, InferenceError>,
    }

    impl Recording {
        /// One that answers `answer`.
        fn answering(answer: fn() -> Result<Suggestions, InferenceError>) -> Self {
            Self { seen: Mutex::new(Vec::new()), answer }
        }

        /// What it was handed, one entry per call.
        fn seen(&self) -> Vec<Seen> {
            self.seen.lock().expect("unpoisoned").clone()
        }
    }

    impl Default for Recording {
        fn default() -> Self {
            Self::answering(|| Ok(Suggestions { suggested: Vec::new(), incomplete: None }))
        }
    }

    impl Analyser for Recording {
        fn suggest(
            &self,
            source: &Picture,
            families: &[Family],
            options: ExecuteOptions,
        ) -> impl Future<Output = Result<Suggestions, InferenceError>> + Send {
            self.seen.lock().expect("unpoisoned").push(Seen {
                picture: source.clone(),
                families: families.to_vec(),
                provider: options.provider,
            });

            let answer = (self.answer)();

            async move { answer }
        }
    }

    /// Colorization and a fourfold upscale, in the library's order.
    fn colorize_and_upscale() -> Result<Suggestions, InferenceError> {
        let scale = Scale::new(4.0).expect("4x is in range");

        Ok(Suggestions { suggested: vec![Suggestion::Colorization, Suggestion::Upscale { scale }], incomplete: None })
    }

    /// A request for `source`, naming the run `run`, over the whole photograph, allowing upscale and colorization.
    fn request(run: &str, source: &str) -> Request {
        Request {
            run: run.to_string(),
            source: source.to_string(),
            processor: Processor(opai::ExecutionProvider::CoreMl),
            crop: None,
            families: vec![Family::Upscale, Family::Colorization],
        }
    }

    /// A temporary directory, an empty registry and table, and one admitted image's identity.
    fn fixture() -> (tempfile::TempDir, Opened, Analyses, String) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let opened = Opened::default();
        let identity = admitted(&dir, &opened);

        (dir, opened, Analyses::default(), identity)
    }

    /// [`suggest_with`] driven to completion.
    fn suggest_now<A: Analyser>(
        analyser: &A,
        opened: &Opened,
        analyses: &Analyses,
        request: Request,
    ) -> Result<Vec<SuggestionWire>, SuggestError> {
        tauri::async_runtime::block_on(suggest_with(analyser, opened, analyses, request))
    }

    // ── The wire ──────────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn every_suggestion_crosses_under_its_familys_own_spelling() {
        let scale = Scale::new(2.0).expect("2x is in range");
        let every = [
            Suggestion::Denoise,
            Suggestion::FaceRecovery,
            Suggestion::Colorization,
            Suggestion::LightAdjustment,
            Suggestion::ColorBalance,
            Suggestion::Sharpen,
            Suggestion::Upscale { scale },
        ];

        for suggestion in every {
            let wire = serde_json::to_value(SuggestionWire::from(suggestion)).expect("serializable");

            // Against `Family`'s own serialization rather than a transcription of it: the window matches these
            // against the `Family` strings the catalogue already sends.
            let family = serde_json::to_value(suggestion.family()).expect("serializable");

            assert_eq!(wire["family"], family, "{suggestion:?} crossed under a family other than its own");
        }
    }

    #[test]
    fn a_suggested_upscale_carries_its_scale_as_a_number_and_nothing_else_carries_a_value() {
        let scale = Scale::new(4.0).expect("4x is in range");

        assert_eq!(
            serde_json::to_value(SuggestionWire::from(Suggestion::Upscale { scale })).expect("serializable"),
            serde_json::json!({ "family": "upscale", "scale": 4.0 })
        );
        assert_eq!(
            serde_json::to_value(SuggestionWire::from(Suggestion::FaceRecovery)).expect("serializable"),
            serde_json::json!({ "family": "face_recovery" }),
            "a unit suggestion carried a value, or a model or precision, the window has no use for"
        );
    }

    #[test]
    fn every_refusal_crosses_under_the_kind_the_window_matches_on() {
        let cases = [
            (SuggestError::NotReady, "notReady"),
            (SuggestError::UnknownSource { identity: "a".to_string() }, "unknownSource"),
            (
                SuggestError::UnreadableSource { identity: "a".to_string(), message: "b".to_string() },
                "unreadableSource",
            ),
            (SuggestError::Stopped, "stopped"),
            (SuggestError::Analyse { message: "b".to_string() }, "analyse"),
        ];

        for (error, kind) in cases {
            let wire = serde_json::to_value(&error).expect("serializable");

            assert_eq!(wire["kind"], kind, "{error:?} crossed under the wrong kind");
        }
    }

    // ── The command ───────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn an_identity_this_application_never_admitted_is_refused_before_anything_runs() {
        let (_dir, opened, analyses, _identity) = fixture();
        let analyser = Recording::default();

        let refused = suggest_now(&analyser, &opened, &analyses, request("run-1", "0123456789abcdef"))
            .expect_err("an unopened identity is not analysable");

        assert_eq!(refused, SuggestError::UnknownSource { identity: "0123456789abcdef".to_string() });
        assert!(analyser.seen().is_empty(), "a refused request still reached the analysis");
        assert!(analyses.is_idle(), "a refused request left its token in the table");
    }

    #[test]
    fn an_admitted_file_that_can_no_longer_be_read_is_refused_as_unreadable() {
        let (dir, opened, analyses, identity) = fixture();
        let analyser = Recording::default();

        std::fs::remove_file(dir.path().join("holiday.png")).expect("the fixture was written a moment ago");

        let refused = suggest_now(&analyser, &opened, &analyses, request("run-1", &identity))
            .expect_err("a deleted file cannot be analysed");

        assert!(
            matches!(&refused, SuggestError::UnreadableSource { identity: refused, .. } if *refused == identity),
            "the refusal did not name the photograph that could not be read: {refused}"
        );
        assert!(analyser.seen().is_empty(), "a refused request still reached the analysis");
    }

    #[test]
    fn a_stop_that_arrives_first_answers_stopped_and_analyses_nothing() {
        let (_dir, opened, analyses, identity) = fixture();
        let analyser = Recording::default();

        analyses.stop("run-1");

        let refused =
            suggest_now(&analyser, &opened, &analyses, request("run-1", &identity)).expect_err("it was stopped");

        assert_eq!(refused, SuggestError::Stopped);
        assert!(analyser.seen().is_empty(), "an analysis a stop overtook still ran");
    }

    #[test]
    fn the_picture_handed_to_the_analysis_is_the_framed_one() {
        // D2: the framing is applied before the analysis sees the picture, which is what keys its face detection on
        // the identity `detect_faces` asks for at that framing.
        let (_dir, opened, analyses, identity) = fixture();
        let analyser = Recording::default();
        let crop = Crop::new(1, 1, 4, 3, 0, false, false).expect("a rectangle with area");

        suggest_now(&analyser, &opened, &analyses, Request { crop: Some(crop), ..request("run-1", &identity) })
            .expect("the fixture is analysable");

        let seen = analyser.seen();
        let handed = &seen.first().expect("the analysis was not run at all").picture;

        assert_eq!(handed.dimensions(), (4, 3), "the analysis was handed the whole photograph rather than the cut");

        let source = tauri::async_runtime::block_on(opai::image::load(
            opened.resolve(&identity).expect("the fixture is admitted"),
        ))
        .expect("the fixture is readable");

        assert_eq!(handed.identity(), cropped(&source, crop).identity(), "the framed picture was named differently");
    }

    #[test]
    fn an_unframed_photograph_is_analysed_as_it_was_opened() {
        let (_dir, opened, analyses, identity) = fixture();
        let analyser = Recording::default();

        suggest_now(&analyser, &opened, &analyses, request("run-1", &identity)).expect("the fixture is analysable");

        let seen = analyser.seen();
        let handed = &seen.first().expect("the analysis was not run at all").picture;

        assert_eq!(handed.dimensions(), (8, 6), "an unframed photograph was altered before analysis");
        assert_eq!(handed.identity(), identity, "an unframed photograph was analysed under another identity");
    }

    #[test]
    fn the_families_and_the_processor_reach_the_analysis_unchanged() {
        let (_dir, opened, analyses, identity) = fixture();
        let analyser = Recording::default();

        let named = vec![Family::Upscale, Family::FaceRecovery, Family::Denoise];
        suggest_now(
            &analyser,
            &opened,
            &analyses,
            Request {
                families: named.clone(),
                processor: Processor(opai::ExecutionProvider::Cpu),
                ..request("run-1", &identity)
            },
        )
        .expect("the fixture is analysable");

        // An empty set is "check nothing", passed through rather than widened to every family.
        suggest_now(&analyser, &opened, &analyses, Request { families: Vec::new(), ..request("run-2", &identity) })
            .expect("an empty set is not a failure");

        let seen = analyser.seen();

        assert_eq!(seen[0].families, named, "the families named were reordered or narrowed on the way");
        assert_eq!(
            seen[0].provider,
            Processor(opai::ExecutionProvider::Cpu).into(),
            "the analysis ran on a processor nobody named"
        );
        assert!(seen[1].families.is_empty(), "an empty set of families was widened on the way");
        assert_eq!(seen[1].provider, Processor(opai::ExecutionProvider::CoreMl).into());
    }

    #[test]
    fn the_suggestions_are_answered_in_the_order_the_library_gave_them() {
        let (_dir, opened, analyses, identity) = fixture();
        let analyser = Recording::answering(colorize_and_upscale);

        let answered = suggest_now(&analyser, &opened, &analyses, request("run-1", &identity)).expect("analysable");

        let scale = Scale::new(4.0).expect("4x is in range");
        assert_eq!(answered, vec![SuggestionWire::Colorization, SuggestionWire::Upscale { scale }]);
        assert!(analyses.is_idle(), "a finished analysis left its token in the table");
    }

    #[test]
    fn an_incomplete_analysis_answers_what_it_read_rather_than_failing() {
        let (_dir, opened, analyses, identity) = fixture();
        let analyser = Recording::answering(|| {
            let scale = Scale::new(4.0).expect("4x is in range");

            Ok(Suggestions {
                suggested: vec![Suggestion::Upscale { scale }],
                incomplete: Some(InferenceError::Untileable { width: 0, height: 0 }),
            })
        });

        let log = Captured::default();
        let subscriber = tracing_subscriber::fmt().with_writer(log.clone()).finish();

        // `block_on` drives the future on this thread, so the thread-local subscriber sees the command's own record.
        let answered = tracing::subscriber::with_default(subscriber, || {
            suggest_now(&analyser, &opened, &analyses, request("run-1", &identity))
        })
        .expect("an incomplete analysis is not a failure");

        let scale = Scale::new(4.0).expect("4x is in range");
        assert_eq!(answered, vec![SuggestionWire::Upscale { scale }]);

        // Nothing is logged here about it: `opai`'s own conclusion names the unread signal and why, and a second
        // record in this crate would be a second account of one failure.
        let log = log.text();
        assert!(
            !log.contains("WARN") && !log.contains("ERROR"),
            "an incomplete analysis was logged again: {log}"
        );
    }

    #[test]
    fn an_analysis_the_library_reports_cancelled_or_shut_down_answers_stopped() {
        let (_dir, opened, analyses, identity) = fixture();

        let withdrawn: [fn() -> Result<Suggestions, InferenceError>; 2] =
            [|| Err(InferenceError::Cancelled), || Err(InferenceError::Shutdown)];

        for answer in withdrawn {
            let refused = suggest_now(&Recording::answering(answer), &opened, &analyses, request("run-1", &identity))
                .expect_err("the analysis was stopped");

            assert_eq!(refused, SuggestError::Stopped);
        }
    }

    #[test]
    fn an_analysis_that_fails_carries_the_librarys_own_sentence() {
        let (_dir, opened, analyses, identity) = fixture();
        let analyser = Recording::answering(|| Err(InferenceError::Untileable { width: 0, height: 0 }));

        let refused =
            suggest_now(&analyser, &opened, &analyses, request("run-1", &identity)).expect_err("this analysis fails");

        assert_eq!(
            refused,
            SuggestError::Analyse { message: InferenceError::Untileable { width: 0, height: 0 }.to_string() }
        );
    }

    /// An analyser that stops its own run while working, then answers suggestions anyway — the fully-cached analysis
    /// that never looked at its token.
    struct StoppedMidway<'a> {
        analyses: &'a Analyses,
        run: &'static str,
    }

    impl Analyser for StoppedMidway<'_> {
        fn suggest(
            &self,
            _source: &Picture,
            _families: &[Family],
            _options: ExecuteOptions,
        ) -> impl Future<Output = Result<Suggestions, InferenceError>> + Send {
            self.analyses.stop(self.run);

            async { colorize_and_upscale() }
        }
    }

    #[test]
    fn a_stop_that_lands_while_the_analysis_runs_answers_no_suggestions_even_ones_determined() {
        let (_dir, opened, analyses, identity) = fixture();
        let analyser = StoppedMidway { analyses: &analyses, run: "run-1" };

        let refused =
            suggest_now(&analyser, &opened, &analyses, request("run-1", &identity)).expect_err("it was stopped");

        assert_eq!(refused, SuggestError::Stopped);
    }

    // ── Analysing does not disturb enhancing ──────────────────────────────────────────────────────────────────────

    #[test]
    fn analysing_and_stopping_an_analysis_leave_the_enhancement_run_alone() {
        use crate::enhance::{Resident, Runs};
        use tauri::Manager;

        let (_dir, opened, _analyses, identity) = fixture();
        let app = tauri::test::mock_builder()
            .manage(Runs::default())
            .manage(Analyses::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build");
        let runs = app.state::<Runs>();
        let analyses = app.state::<Analyses>();

        // A run in flight over one photograph...
        let token = runs.start("enhance-1", "5ec0ffee5ec0ffee");

        // ...while another is analysed, and a third analysis is stopped through the command, before and after it.
        suggest_now(&Recording::answering(colorize_and_upscale), &opened, &analyses, request("suggest-1", &identity))
            .expect("the fixture is analysable");
        cancel_suggest("suggest-2".to_string(), app.state::<Analyses>(), Traceparent::default());
        cancel_suggest("enhance-1".to_string(), app.state::<Analyses>(), Traceparent::default());

        assert!(!token.is_cancelled(), "an analysis, or a stop for one, stopped the enhancement run");
        assert!(
            runs.finish(
                "enhance-1",
                Picture::new("/pictures/holiday.jpg", image::DynamicImage::new_rgb8(2, 2), "aaaaaaaaaaaaaaaa")
            ),
            "an analysis displaced or released the enhancement run"
        );
        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Picture(_))),
            "the enhancement run's result is not held"
        );
    }

    // ── Against the real models ───────────────────────────────────────────────────────────────────────────────────

    #[test]
    #[ignore = "installs the runtime and the face detector into the real configuration directory; run by hand with \
                the GUI closed"]
    fn a_detection_after_an_analysis_is_served_from_the_run_store() {
        use std::sync::Arc;

        use crate::enhance::Reporting;
        use crate::faces;

        tauri::async_runtime::block_on(async {
            let opai = Opai::initialize(opai::APP_NAME, None).await.expect("the application must start");

            let dir = tempfile::tempdir().expect("a temporary directory");
            let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/test.dat");
            let bytes = std::fs::read(&fixture).expect("the committed photograph is readable");
            let opened = Opened::default();

            for processor in [Processor(opai::ExecutionProvider::Cpu), Processor(opai::ExecutionProvider::CoreMl)] {
                // The run store is on disk and outlives the process, and it keys on the picture rather than the
                // provider. So the committed photograph as it stands would be served from an earlier run — or the
                // other provider's pass — and the check would pass having run nothing. A nonce after the JPEG's end
                // marker makes a file the store has never seen, whose pixels are the fixture's.
                let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("after 1970");
                let path = dir.path().join(format!("test-{processor:?}.jpg"));
                std::fs::write(&path, [bytes.as_slice(), format!("{processor:?}-{nonce:?}").as_bytes()].concat())
                    .expect("writable");

                crate::images::files::describe_and_admit(vec![path.clone()], &opened).await.expect("admissible");
                let identity = opai::image::identity(&path).await.expect("a written file has an identity");

                let (width, height) = opai::image::load(&path).await.expect("decodable").dimensions();
                let crop = Crop::new(0, 0, width, height, 0, false, false).expect("the whole photograph has area");

                let analyses = Analyses::default();
                let request = Request {
                    run: format!("suggest-{processor:?}"),
                    source: identity.clone(),
                    processor,
                    crop: Some(crop),
                    families: vec![Family::FaceRecovery],
                };

                let suggested = suggest_with(&opai, &opened, &analyses, request).await.expect("analysable");
                assert!(
                    suggested.contains(&SuggestionWire::FaceRecovery),
                    "the committed photograph has faces, and none were found: {suggested:?}"
                );

                let reports = Arc::new(Mutex::new(Vec::new()));
                let collector = Arc::clone(&reports);
                let reporting = Reporting::new("detect", move |report| {
                    collector.lock().expect("unpoisoned").push(report);
                });

                let faces = faces::detect_with(
                    &opai,
                    &opened,
                    Some(reporting),
                    faces::Request { source: identity.clone(), processor, crop: Some(crop) },
                )
                .await
                .expect("detectable");

                let reports = reports.lock().expect("unpoisoned");

                assert!(!faces.is_empty(), "the detection after an analysis found no faces");
                assert!(!reports.is_empty(), "the detection reported nothing at all");
                assert!(
                    reports.iter().all(|report| report.stage.is_none()),
                    "the detection after an analysis ran or fetched a model rather than being served: {:?}",
                    reports.iter().map(|report| report.stage).collect::<Vec<_>>()
                );
            }
        });
    }

    #[test]
    fn each_ending_is_recorded_by_whoever_saw_it() {
        let s = String::new;
        let cells = [
            (SuggestError::Stopped, "stopped"),
            (SuggestError::NotReady, "recorded by the wrapper"),
            (SuggestError::UnknownSource { identity: s() }, "recorded by the wrapper"),
            (SuggestError::UnreadableSource { identity: s(), message: s() }, "recorded by opai"),
            (SuggestError::Analyse { message: s() }, "recorded by opai"),
        ];
        for (error, cell) in cells {
            assert_eq!(CommandError::ended(&error).cell(), cell, "{error:?}");
        }
    }
}
