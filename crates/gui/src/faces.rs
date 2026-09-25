//! Finding the faces in an open photograph, so a face recovery can be run over them.
//!
//! One command, [`detect_faces`], and the seam beneath it. It is deliberately **not** folded into `enhance`:
//! the window has to hold the faces whatever else is true — it reports how many a photograph has, and a later
//! slice draws a box per face and lets a user turn one off — and none of that can be done with a set that only
//! ever existed inside one `enhance` call. See design.md D1.
//!
//! # Why this is its own module and not part of `enhance`
//!
//! It runs no chain, holds no result, and touches neither [`Runs`](crate::enhance::Runs) nor the `opai://`
//! scheme. What it shares with `enhance/run.rs` is the order it does things in, and that is shared as code —
//! [`load_framed`](load_framed) — rather than restated.
//!
//! # What the framing does to a face
//!
//! The crop is applied before the detector sees the picture, exactly as it is before a chain, so every
//! coordinate answered is in the framed photograph's own pixels: what the window draws on, and what the
//! recovery model is applied to. A framing returned to is one identity returned to, so `opai`'s run store
//! serves it without transferring or opening anything — which is what makes turning a photograph to 90°, then
//! 50°, then back to 90° cost two detections rather than three. See design.md D3.
//!
//! # There is no stopping a detection
//!
//! No `cancel_detect`, and no slot. A window that has moved on discards the answer by run name, as it already
//! discards a progress report about a run it abandoned. An abandoned detection costs one bounded run — the
//! detector sees one fixed square whatever the photograph's size — and its result is written to the run store,
//! where the next request for that framing is served by it. See design.md D11.
//!
//! # No record names its run
//!
//! The run is not even part of [`Request`]: every record here is emitted inside the `detect_faces` command's span,
//! which carries it, and the file writes it onto each of them from there. See `crate::command`.

use std::future::Future;

use opai::{Analysis, Detection, ExecuteOptions, Executed, Face, Faces, FloatPrecision, InferenceError, Opai, Picture};
use serde::Serialize;
use tauri::State;

use crate::command::{Answer, CommandError, Ended, Traceparent, command_span, traced};
use crate::enhance::{Processor, Reporting, reporting};
use crate::images::{Crop, Opened, load_framed};
use crate::setup::Setup;

/// The precision the detector is always run at, whatever the recovery model's tier.
///
/// **A constant rather than a function of the recovery model**, which is where this diverges from the
/// reference. `face-detection` already requires that the two builds find the same faces in the same order,
/// differing at most sub-pixel and comparing equal below a hundredth of a pixel — and a [`Face`] is quantized
/// to a hundredth of a pixel on acceptance, so in the overwhelming majority of cases the two are the *same
/// value*. The tier therefore buys nothing and costs a second detector build on disk plus a second run-store
/// entry per photograph per framing, where one would serve both. See design.md D2.
const DETECTION_PRECISION: FloatPrecision = FloatPrecision::Fp32;

/// Why the faces in a photograph could not be found.
///
/// The same three refusals a chain has before it runs, plus the run's own failure. There is no stopped arm:
/// a detection cannot be stopped — see this module's header and design.md D11.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum DetectError {
    /// The window asked before the application had finished starting itself.
    #[error("the application is still starting and cannot detect faces yet")]
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

    /// The detection itself failed; `message` is [`InferenceError`]'s own sentence.
    #[error("{message}")]
    Detect {
        /// The library's own sentence, untranslated and in full.
        message: String,
    },
}

impl CommandError for DetectError {
    fn ended(&self) -> Ended<'_> {
        match self {
            // Refusals this crate makes before `opai` is asked anything.
            Self::NotReady | Self::UnknownSource { .. } => Ended::Failed { error: self, recorded: false },
            // `opai` records the failed decode and the failed detection, where each happened.
            Self::UnreadableSource { .. } | Self::Detect { .. } => Ended::Failed { error: self, recorded: true },
        }
    }
}

impl Answer for Vec<Face> {}

/// What this module asks of the library, so the rules above can be tested without a real [`Opai`].
///
/// The same shape `enhance/run.rs`'s [`Enhancer`](crate::enhance::enhance_with) seam takes, for the same
/// reason: what this module decides is the order it does things in, and that is checkable without a detector
/// on disk.
pub(crate) trait Detector {
    /// Runs `analysis` over `source`, as [`Opai::execute`] does.
    fn execute(
        &self,
        source: &Picture,
        analysis: &Analysis,
        options: ExecuteOptions,
    ) -> impl Future<Output = Result<Executed<Faces>, InferenceError>> + Send;
}

impl Detector for Opai {
    fn execute(
        &self,
        source: &Picture,
        analysis: &Analysis,
        options: ExecuteOptions,
    ) -> impl Future<Output = Result<Executed<Faces>, InferenceError>> + Send {
        Opai::execute(self, source, analysis, Some(options))
    }
}

/// What one request to detect carries, gathered so the seam below takes one argument rather than three.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// The identity of the image to look in, as the window already addresses its pixels by.
    pub(crate) source: String,
    /// What to run on.
    pub(crate) processor: Processor,
    /// How the image is framed, or `None` to look at the whole photograph.
    pub(crate) crop: Option<Crop>,
}

/// Finds the faces in an open photograph as it is framed.
///
/// The whole of what the [`detect_faces`] command does once the library handle has been taken off managed
/// state.
///
/// # The order of what happens is the contract
///
/// ```text
///   1. resolve the source           --> a refusal here decodes nothing and fetches nothing
///   2. decode the source
///   3. frame it                     --> what is detected on, and what the run store keys on
///   4. run
/// ```
///
/// Nothing is displaced by any of it: unlike a chain, a detection holds no slot, so a refused request costs
/// the window nothing it was watching.
///
/// # Errors
///
/// [`DetectError`]. An empty answer is not one of them — a photograph with nobody in it is a finding, and
/// `face-recovery` specifies an empty selection as a request rather than an error. See design.md D10.
pub(crate) async fn detect_with<D: Detector>(
    detector: &D,
    opened: &Opened,
    reporting: Option<Reporting>,
    request: Request,
) -> Result<Vec<Face>, DetectError> {
    let path = opened
        .resolve(&request.source)
        .ok_or_else(|| DetectError::UnknownSource { identity: request.source.clone() })?;

    // Not logged here, nor the detection's failure below: `opai` records each once, where it happened.
    let picture = load_framed(path, request.crop).await.map_err(|error| DetectError::UnreadableSource {
        identity: request.source.clone(),
        message: error.to_string(),
    })?;

    // `..Default::default()` keeps this source-compatible as `ExecuteOptions` grows. `cache` stays on, which is
    // what makes a framing returned to free, and `cancel` is left at the token nobody holds — D11.
    let options = ExecuteOptions {
        provider: request.processor.into(),
        on_progress: reporting.as_ref().map(|reporting| std::sync::Arc::clone(&reporting.on_progress)),
        ..Default::default()
    };

    let outcome = detector.execute(&picture, &Detection::newyork(DETECTION_PRECISION), options).await;

    // Released whatever the run did, so the window's indicator lands where the detection actually got to.
    if let Some(reporting) = &reporting {
        reporting.release();
    }

    match outcome {
        Ok(Executed { value: faces, .. }) => {
            tracing::debug!(found = faces.len(), "the faces in a photograph were detected");

            Ok(faces.iter().copied().collect())
        }
        Err(error) => Err(DetectError::Detect { message: error.to_string() }),
    }
}

/// Find the faces in an open image, as the user has framed it.
///
/// `run` is the window's own name for this detection, minted before the call — the same arrangement an
/// enhancement run uses, and what lets one progress subscription tell this detection's reports from those of
/// a run the window has abandoned.
///
/// `crop` is how the user has framed that image, or absent to look at the whole photograph. **Every coordinate
/// answered is in the framed photograph's pixels**, which is what the window draws on and what a recovery run
/// is applied to.
///
/// Answers the faces in the order the detector found them. That order is load-bearing: [`Faces`] keeps it, and
/// it is folded into a face-recovery operation's run cache tag.
///
/// The command's name is written once more, in `frontend/ipc/faces.ts`.
///
/// # Errors
///
/// [`DetectError`]. Finding nobody is not one of them.
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its wire shape plus its managed state"
)]
#[tauri::command]
pub(crate) async fn detect_faces(
    app: tauri::AppHandle,
    run: String,
    source: String,
    processor: Processor,
    crop: Option<Crop>,
    setup: State<'_, Setup<Opai>>,
    opened: State<'_, Opened>,
    traceparent: Traceparent,
) -> Result<Vec<Face>, DetectError> {
    traced(command_span!("detect_faces", traceparent, run = run), async move {
        // A clone of the handle rather than the handle, and the lock released before the run starts — see
        // `Setup::peek`.
        let opai = setup.peek(Opai::clone).await.ok_or(DetectError::NotReady)?;
        let reporting = reporting(&app, &run);

        detect_with(&opai, &opened, Some(reporting), Request { source, processor, crop }).await
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::enhance::{RunProgress, Stage};
    use crate::images::cropped;
    use crate::test_support::admitted;

    use opai::{Confidence, Point, ProviderReport, Rect};

    /// A detector that answers a fixed set of faces and records every picture it was handed.
    #[derive(Default)]
    struct Recording {
        seen: Mutex<Vec<Picture>>,
        answers: Vec<Face>,
    }

    impl Detector for Recording {
        fn execute(
            &self,
            source: &Picture,
            _analysis: &Analysis,
            _options: ExecuteOptions,
        ) -> impl Future<Output = Result<Executed<Faces>, InferenceError>> + Send {
            self.seen.lock().expect("unpoisoned").push(source.clone());

            let found = Faces::new(self.answers.iter().copied());

            async move { Ok(executed(found)) }
        }
    }

    /// A detector whose run fails.
    struct Failing;

    impl Detector for Failing {
        async fn execute(
            &self,
            _source: &Picture,
            _analysis: &Analysis,
            _options: ExecuteOptions,
        ) -> Result<Executed<Faces>, InferenceError> {
            Err(InferenceError::Untileable { width: 0, height: 0 })
        }
    }

    /// A detector that reports its way through a transfer and a run before answering.
    ///
    /// The reports are `opai`'s own shape, so what the window receives is what the thinning made of what the
    /// library actually sends.
    struct Progressing;

    impl Detector for Progressing {
        async fn execute(
            &self,
            _source: &Picture,
            analysis: &Analysis,
            options: ExecuteOptions,
        ) -> Result<Executed<Faces>, InferenceError> {
            let subject = Arc::new(opai::Subject::Analysis(*analysis));

            if let Some(on_progress) = &options.on_progress {
                for step in 0..=10 {
                    let fraction = f64::from(step) / 10.0;

                    on_progress(&opai::InferenceProgress {
                        subject: Arc::clone(&subject),
                        stage: installing(&subject, fraction),
                        operation_fraction: fraction,
                        // A transfer occupies only the head of the run's own range, as it does for a chain.
                        chain_fraction: fraction / 10.0,
                    });
                }

                for step in 0..=10 {
                    let fraction = f64::from(step) / 10.0;

                    on_progress(&opai::InferenceProgress {
                        subject: Arc::clone(&subject),
                        stage: opai::Stage::Running,
                        operation_fraction: fraction,
                        chain_fraction: 0.1 + fraction * 0.9,
                    });
                }
            }

            Ok(executed(Faces::empty()))
        }
    }

    /// One install report at `fraction` of the detector's own transfer.
    fn installing(subject: &Arc<opai::Subject>, fraction: f64) -> opai::Stage {
        opai::Stage::Installing(opai::Progress {
            dependency: opai::Dependency::Model(
                subject.required_artifacts().first().expect("a detection needs a model").clone(),
            ),
            phase: opai::Phase::Downloading,
            bytes: (fraction * 1000.0) as u64,
            total: Some(1000),
            fraction,
        })
    }

    /// What a fake [`Detector`] hands back: the faces, and a provider report no test asserts about.
    fn executed(faces: Faces) -> Executed<Faces> {
        Executed {
            value: faces,
            providers: ProviderReport { requested: opai::ExecutionProvider::Auto, actual: Vec::new() },
        }
    }

    /// One face, as the detector would have reported it.
    fn face(left: f32) -> Face {
        Face::new(
            Rect::new(Point::new(left, 4.0), Point::new(left + 3.0, 7.0)),
            [Point::new(left + 1.0, 5.0); Face::LANDMARKS],
            Confidence::new(0.9).expect("0.9 is inside the permitted range"),
        )
    }

    /// A request for `source`, over the whole photograph.
    fn request(source: &str) -> Request {
        Request { source: source.to_string(), processor: Processor::Coreml, crop: None }
    }

    /// A temporary directory, an empty registry, and one admitted image's identity.
    fn fixture() -> (tempfile::TempDir, Opened, String) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let opened = Opened::default();
        let identity = admitted(&dir, &opened);

        (dir, opened, identity)
    }

    /// [`detect_with`] driven to completion, with no progress reporting.
    fn detect_now<D: Detector>(detector: &D, opened: &Opened, request: Request) -> Result<Vec<Face>, DetectError> {
        tauri::async_runtime::block_on(detect_with(detector, opened, None, request))
    }

    #[test]
    fn an_identity_this_application_never_admitted_is_refused_before_anything_runs() {
        let (_dir, opened, _identity) = fixture();
        let detector = Recording::default();

        let refused = detect_now(&detector, &opened, request("0123456789abcdef"))
            .expect_err("an unopened identity is not detectable");

        assert_eq!(refused, DetectError::UnknownSource { identity: "0123456789abcdef".to_string() });
        assert!(
            detector.seen.lock().expect("unpoisoned").is_empty(),
            "a refused request still reached the detector"
        );
    }

    #[test]
    fn an_admitted_file_that_can_no_longer_be_read_is_refused_before_anything_runs() {
        let (dir, opened, identity) = fixture();
        let detector = Recording::default();

        std::fs::remove_file(dir.path().join("holiday.png")).expect("the fixture was written a moment ago");

        let refused = detect_now(&detector, &opened, request(&identity)).expect_err("a deleted file has no faces");

        assert!(
            matches!(&refused, DetectError::UnreadableSource { identity: refused, .. } if *refused == identity),
            "the refusal did not name the photograph that could not be read: {refused}"
        );
        assert!(
            detector.seen.lock().expect("unpoisoned").is_empty(),
            "a refused request still reached the detector"
        );
    }

    #[test]
    fn the_picture_handed_to_the_detector_is_the_framed_one() {
        // D3: the framing is applied before the detector sees the picture, which is what puts every coordinate
        // answered in the framed photograph's own pixels and what keys the run store on the framing.
        let (_dir, opened, identity) = fixture();
        let detector = Recording::default();
        let crop = Crop::new(1, 1, 4, 3, 0, false, false).expect("a rectangle with area");

        detect_now(&detector, &opened, Request { crop: Some(crop), ..request(&identity) })
            .expect("the fixture is detectable");

        let seen = detector.seen.lock().expect("unpoisoned");
        let handed = seen.first().expect("the detector was not run at all");

        assert_eq!(handed.dimensions(), (4, 3), "the detector was handed the whole photograph rather than the cut");

        // Against `cropped` itself rather than a transcription of its identity rule: what this pins is that the
        // seam is handed the same picture a chain's input is framed to, not how that picture is named.
        let source = tauri::async_runtime::block_on(opai::image::load(
            opened.resolve(&identity).expect("the fixture is admitted"),
        ))
        .expect("the fixture is readable");

        assert_eq!(handed.identity(), cropped(&source, crop).identity(), "the framed picture was named differently");
    }

    #[test]
    fn an_unframed_photograph_is_detected_on_as_it_was_opened() {
        let (_dir, opened, identity) = fixture();
        let detector = Recording::default();

        detect_now(&detector, &opened, request(&identity)).expect("the fixture is detectable");

        let seen = detector.seen.lock().expect("unpoisoned");
        let handed = seen.first().expect("the detector was not run at all");

        assert_eq!(handed.dimensions(), (8, 6), "an unframed photograph was altered before detection");
        assert_eq!(handed.identity(), identity, "an unframed photograph was detected on under another identity");
    }

    #[test]
    fn the_faces_answered_are_the_ones_found_in_the_order_they_were_found() {
        let (_dir, opened, identity) = fixture();
        let found = vec![face(0.0), face(20.0), face(40.0)];
        let detector = Recording { answers: found.clone(), ..Recording::default() };

        let answered = detect_now(&detector, &opened, request(&identity)).expect("detectable");

        assert_eq!(answered, found, "the faces answered are not the ones found, or not in the order they were");
    }

    #[test]
    fn a_photograph_with_nobody_in_it_answers_no_faces_rather_than_failing() {
        let (_dir, opened, identity) = fixture();
        let detector = Recording::default();

        let answered =
            detect_now(&detector, &opened, request(&identity)).expect("finding nobody is a finding, not a failure");

        assert!(answered.is_empty());
    }

    #[test]
    fn a_detection_that_fails_carries_the_librarys_own_sentence() {
        let (_dir, opened, identity) = fixture();

        let refused = detect_now(&Failing, &opened, request(&identity)).expect_err("this detector fails");

        assert_eq!(
            refused,
            DetectError::Detect { message: InferenceError::Untileable { width: 0, height: 0 }.to_string() }
        );
    }

    // ── What the window is told while the detection happens ───────────────────────────────────────────────────

    /// A [`Reporting`] that collects what it sent, and the collection.
    fn collecting(run: &str) -> (Reporting, Arc<Mutex<Vec<RunProgress>>>) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let collector = Arc::clone(&sent);

        (Reporting::new(run, move |report| collector.lock().expect("unpoisoned").push(report)), sent)
    }

    /// The reports one detection under the run `run` put on the wire.
    fn reported(run: &str) -> Vec<RunProgress> {
        let (_dir, opened, identity) = fixture();
        let (reporting, sent) = collecting(run);
        let request = request(&identity);

        tauri::async_runtime::block_on(detect_with(&Progressing, &opened, Some(reporting), request))
            .expect("the fixture is detectable");

        let sent = sent.lock().expect("unpoisoned");
        sent.clone()
    }

    #[test]
    fn a_transfer_of_the_detector_is_reported_as_installing_with_its_own_fraction() {
        // D8, and the whole reason this command reports at all: the models are fetched on first use, so a window
        // that said nothing would sit still through a download.
        let reports = reported("run-1");

        let installing = reports.iter().filter(|report| report.stage == Some(Stage::Installing)).collect::<Vec<_>>();

        assert!(!installing.is_empty(), "the detector's transfer was not reported at all");
        assert!(
            installing.iter().all(|report| report.install_fraction.is_some()),
            "a transfer was reported without the figure that makes it distinguishable from a stall"
        );
    }

    #[test]
    fn every_report_names_the_run_the_command_was_given_and_the_family_it_belongs_to() {
        // Honest in Rust, named in TypeScript: the operation *is* a detection, and saying `face_recovery` here
        // would make the `operation` diagnostic disagree with its own family. The frontend maps it. See D8.
        let reports = reported("run-7");

        assert!(!reports.is_empty(), "a detection reported nothing at all");
        assert!(
            reports.iter().all(|report| report.run == "run-7"),
            "a report named a run other than the one the command was given"
        );
        assert!(
            reports.iter().all(|report| report.family == opai::Family::Detection),
            "a detection reported under a family other than its own"
        );
    }

    #[test]
    fn the_chain_fraction_reaches_one_exactly_once() {
        let reports = reported("run-1");

        let complete = reports.iter().filter(|report| report.chain_fraction >= 1.0).count();

        assert_eq!(complete, 1, "the indicator did not land on complete exactly once");
        assert_eq!(
            reports.last().map(|report| report.chain_fraction),
            Some(1.0),
            "the run's last report was not the one that completed it"
        );
    }

    #[test]
    fn each_ending_is_recorded_by_whoever_saw_it() {
        let s = String::new;
        let cells = [
            (DetectError::NotReady, "recorded by the wrapper"),
            (DetectError::UnknownSource { identity: s() }, "recorded by the wrapper"),
            (DetectError::UnreadableSource { identity: s(), message: s() }, "recorded by opai"),
            (DetectError::Detect { message: s() }, "recorded by opai"),
        ];
        for (error, cell) in cells {
            assert_eq!(CommandError::ended(&error).cell(), cell, "{error:?}");
        }
    }
}
