//! Finding the faces in an open photograph, so a face recovery can be run over them.
//!
//! Two ways in. [`for_chain`] is how a chain carrying a face recovery gets its faces: `enhance` and `export` detect
//! inside the run they were asked for, with one progress stream, and hand the recovery the faces the user's
//! [`FaceChoice`] keeps — so the window never has to detect first and enhance second. [`detect_faces`] is the
//! picker's: the window holds the faces too — it reports how many a photograph has and draws a box per face for a
//! user to turn off — and it asks for them there when it has none yet. Both run the same detection, so one run-store
//! entry serves both. See design.md D1.
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
//! # There is no stopping the picker's detection
//!
//! A chain's detection stops with the chain: it runs under the chain's own cancellation. The picker's has no
//! `cancel_detect`, and no slot. A window that has moved on discards the answer by run name, as it already
//! discards a progress report about a run it abandoned. An abandoned detection costs one bounded run — the
//! detector sees one fixed square whatever the photograph's size — and its result is written to the run store,
//! where the next request for that framing is served by it. See design.md D11.
//!
//! # No record names its run
//!
//! The run is not even part of [`Request`]: every record here is emitted inside the `detect_faces` command's span,
//! which carries it, and the file writes it onto each of them from there. See `crate::command`.

use std::future::Future;

use opai::{Analysis, Detection, ExecuteOptions, Executed, Face, Faces, InferenceError, Opai, Picture};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::command::{Answer, CommandError, Ended, Traceparent, command_span, traced};
use crate::enhance::{Enhancer, Processor, Reporting, Requested, reporting};
use crate::images::{Crop, Opened, load_framed};
use crate::setup::Setup;

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

/// One face found, as the window is told it: `opai`'s own [`Face`], its [`key`](face_key), and whether a recovery
/// can restore it.
///
/// **The face is flattened in unchanged**, so its fields are `opai`'s own spelling. See `Face` in
/// `frontend/ipc/faces.ts`, which explains why that spelling is kept.
///
/// `key` is what the window records a choice by, and what [`FaceChoice`] names a face with on the way back — written
/// here, once, so the two ends cannot spell one face two ways.
///
/// `restorable` is [`Face::restorable`], published rather than restated: the window counts the faces a recovery will
/// restore by the same rule [`FaceChoice::keeps`] applies and the autopilot suggests face recovery by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DetectedFace {
    /// The face, as the detector reported it.
    #[serde(flatten)]
    pub(crate) face: Face,
    /// The face's identity for a choice: see [`face_key`].
    pub(crate) key: String,
    /// Whether it is small enough for a recovery to give it back more detail than it holds.
    pub(crate) restorable: bool,
}

impl From<Face> for DetectedFace {
    fn from(face: Face) -> Self {
        Self { face, key: face_key(&face), restorable: face.restorable() }
    }
}

/// A face's identity, as far as a choice among faces is concerned: its four bounding-box coordinates,
/// `min.x,min.y,max.x,max.y`.
///
/// **The same four numbers `Faces::write_cache_signature` folds**, for the reasons `crates/opai/src/models/face.rs`
/// gives. *Bounding box only*, because the box uniquely identifies a deterministically detected face and the landmarks
/// and the confidence move with it. *Nothing digested*, because a key read back out of a set does not need to be short
/// and a legible one is legible in a debugger and a log.
///
/// **Stable across a re-detection**: a coordinate is quantized to a hundredth of a pixel as [`Face::new`] accepts it,
/// so two detections of one photograph at one framing produce the same numbers. The coordinates are in the **framed**
/// photograph's pixels, so a framing nobody has been at matches no key recorded at another, and one returned to
/// matches every key recorded there.
///
/// Each number is `f32`'s shortest spelling, the one JavaScript also prints for it, with a negative zero written as
/// `0` — so a key a debugger shows on either side reads the same.
pub(crate) fn face_key(face: &Face) -> String {
    let rect = face.bounding_box();
    // `+ 0.0` turns a negative zero into a positive one and changes nothing else.
    let number = |value: f32| (value + 0.0).to_string();

    format!("{},{},{},{}", number(rect.min.x), number(rect.min.y), number(rect.max.x), number(rect.max.y))
}

/// Which of the faces found a face recovery restores, as the window sends it: the user's own exceptions to the
/// default, by [`face_key`].
///
/// **The default is [`Face::restorable`]**: a face small enough for a recovery to add detail is restored, and a larger
/// one — which a recovery can only soften — is left alone. `skipped` holds the faces the user turned off although the
/// default would restore them, and `restored` the ones turned on although it would not. A face in neither follows the
/// default, which is why a framing nobody has been at — every face at new coordinates — is decided by size, and one
/// returned to finds the user's choices where they were left.
///
/// **Keys, not faces.** The window does not have to know the faces before it asks for a run: the run finds them, and
/// this says which to keep.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FaceChoice {
    /// Faces the default would restore and the user turned off.
    #[serde(default)]
    pub(crate) skipped: Vec<String>,
    /// Faces the default would leave alone and the user turned on.
    #[serde(default)]
    pub(crate) restored: Vec<String>,
}

impl FaceChoice {
    /// Whether a face recovery restores `face` under this choice.
    pub(crate) fn keeps(&self, face: &Face) -> bool {
        let key = face_key(face);

        !self.skipped.contains(&key) && (self.restored.contains(&key) || face.restorable())
    }
}

/// What finding the faces for a chain came to, for the window to record beside the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChainFaces {
    /// The chain carries no face recovery, so nothing was looked for.
    NotAsked,
    /// Every face found in the framed photograph, in the order the detector found them — chosen or not, since the
    /// window counts and draws them all.
    Found(Vec<DetectedFace>),
    /// The detection failed, and the recovery restored none. `opai`'s own sentence, untranslated.
    Failed(String),
}

/// `operations` with every face recovery handed the faces `requested` keeps among those found in `picture`, and what
/// was found.
///
/// **Inside the run, not before it.** The window used to detect, record, and only then ask for the chain; now the
/// chain it asks for detects for itself, so a face recovery is one request with one progress stream — `on_progress`,
/// which [`Handover`](crate::enhance::Handover) maps onto the head of the bar — and one stop. The detection is
/// [`Detection::for_face_recovery`], the one the picker and the autopilot run too, so it is served from the run store
/// whenever either already asked: a re-run at a framing already visited detects nothing.
///
/// `operations` is the chain `requested` resolved to, in the same order, with every face recovery built over an empty
/// selection.
///
/// **A detection that fails does not fail the chain.** The recovery restores nothing and the rest of the chain runs:
/// refusing it would make a failed detection also cancel an upscale asked for in the same list, a second failure
/// caused by the first. What failed comes back as [`ChainFaces::Failed`], for the window to say so. See design.md D10.
///
/// # Errors
///
/// [`InferenceError::Cancelled`] where the run was stopped during the detection — the one outcome the chain cannot go
/// on from.
pub(crate) async fn for_chain<E: Enhancer>(
    enhancer: &E,
    picture: &Picture,
    requested: &[Requested],
    operations: Vec<opai::Operation>,
    options: ExecuteOptions,
) -> Result<(Vec<opai::Operation>, ChainFaces), InferenceError> {
    if !operations.iter().any(|operation| matches!(operation, opai::Operation::FaceRecovery(_))) {
        return Ok((operations, ChainFaces::NotAsked));
    }

    let found = match enhancer.detect(picture, &Detection::for_face_recovery(), options).await {
        Ok(Executed { value, .. }) => value,
        Err(InferenceError::Cancelled) => return Err(InferenceError::Cancelled),
        // Recorded by `opai`, where it happened; this only notes what the chain does about it.
        Err(error) => {
            tracing::debug!(%error, "the faces for a face recovery could not be found; it restores none");

            return Ok((operations, ChainFaces::Failed(error.to_string())));
        }
    };

    let operations = requested
        .iter()
        .zip(operations)
        .map(|(requested, operation)| match operation {
            opai::Operation::FaceRecovery(recovery) => {
                let choice = requested.faces.clone().unwrap_or_default();

                // In the order the detector found them: `Faces` folds an order-sensitive signature into the cache tag,
                // so the same choice asks for the same stored result every time.
                let kept = Faces::new(found.iter().copied().filter(|face| choice.keeps(face)));

                opai::Operation::FaceRecovery(recovery.with_faces(kept))
            }
            other => other,
        })
        .collect();

    tracing::debug!(found = found.len(), "the faces for a face recovery were found");

    Ok((operations, ChainFaces::Found(found.iter().copied().map(DetectedFace::from).collect())))
}

impl Answer for Vec<DetectedFace> {}

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
) -> Result<Vec<DetectedFace>, DetectError> {
    let path = opened
        .resolve(&request.source)
        .ok_or_else(|| DetectError::UnknownSource { identity: request.source.clone() })?;

    // Not logged here, nor the detection's failure below: `opai` records each once, where it happened.
    let picture = load_framed(opened, &request.source, path, request.crop).await.map_err(|error| {
        DetectError::UnreadableSource { identity: request.source.clone(), message: error.to_string() }
    })?;

    // `..Default::default()` keeps this source-compatible as `ExecuteOptions` grows. `cache` stays on, which is
    // what makes a framing returned to free, and `cancel` is left at the token nobody holds — D11.
    let options = ExecuteOptions {
        provider: request.processor.into(),
        on_progress: reporting.as_ref().map(|reporting| std::sync::Arc::clone(&reporting.on_progress)),
        ..Default::default()
    };

    // The library's one detection for feeding a face recovery, so the faces the picker offers are the ones autopilot
    // found and the ones a restore is handed, from one run-store entry. Its precision is a constant rather than the
    // recovery model's tier — see `Detection::for_face_recovery` and design.md D2.
    let outcome = detector.execute(&picture, &Detection::for_face_recovery(), options).await;

    // Released whatever the run did, so the window's indicator lands where the detection actually got to.
    if let Some(reporting) = &reporting {
        reporting.release();
    }

    match outcome {
        Ok(Executed { value: faces, .. }) => {
            tracing::debug!(found = faces.len(), "the faces in a photograph were detected");

            Ok(faces.iter().copied().map(DetectedFace::from).collect())
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
) -> Result<Vec<DetectedFace>, DetectError> {
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
    use crate::test_support::{admitted, boxed, executed};

    use opai::{Confidence, Point, Rect};

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
        Request { source: source.to_string(), processor: Processor(opai::ExecutionProvider::CoreMl), crop: None }
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
            .map(|answered| answered.into_iter().map(|detected| detected.face).collect())
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
    fn each_face_answered_says_whether_a_recovery_can_restore_it_and_is_otherwise_the_librarys_own_shape() {
        let (_dir, opened, identity) = fixture();
        let large = boxed(0.0, 0.0, 600.0, 600.0);
        let detector = Recording { answers: vec![face(0.0), large], ..Recording::default() };

        let answered = tauri::async_runtime::block_on(detect_with(&detector, &opened, None, request(&identity)))
            .expect("detectable");

        assert_eq!(answered.iter().map(|detected| detected.restorable).collect::<Vec<_>>(), [true, false]);

        // The wire shape: `opai`'s own fields, flattened in, plus the key and the flag — and a face read back from it
        // is the face.
        let json = serde_json::to_value(&answered[0]).expect("a detected face serializes");
        let mut plain = serde_json::to_value(face(0.0)).expect("a face serializes");
        let fields = plain.as_object_mut().expect("a face is an object");
        fields.insert("key".into(), "0,4,3,7".into());
        fields.insert("restorable".into(), true.into());
        assert_eq!(json, plain);
        assert_eq!(serde_json::from_value::<Face>(json).expect("the library reads it back"), face(0.0));
    }

    // ── A key, a choice, and the faces a chain finds for itself ──────────────────────────────────────────────────────

    #[test]
    fn a_key_is_the_four_coordinates_as_both_sides_print_them() {
        assert_eq!(face_key(&boxed(10.0, 20.0, 110.0, 140.0)), "10,20,110,140");
        assert_eq!(face_key(&boxed(10.25, 4.5, 13.0, 7.01)), "10.25,4.5,13,7.01");
        // Quantized on acceptance, so a re-detection a sub-pixel apart is one key.
        assert_eq!(face_key(&boxed(10.001, 4.0, 13.0, 7.0)), face_key(&boxed(10.0, 4.0, 13.0, 7.0)));
        // A face reaching past the top-left edge, including the zero a rounding leaves negative.
        assert_eq!(face_key(&boxed(-0.001, -12.5, 3.0, 7.0)), "0,-12.5,3,7");
    }

    #[test]
    fn a_choice_keeps_what_the_default_keeps_except_where_the_user_said_otherwise() {
        let small = boxed(0.0, 0.0, 100.0, 100.0);
        let large = boxed(0.0, 0.0, 600.0, 600.0);
        let key = |face: &Face| face_key(face);

        let none = FaceChoice::default();
        assert!(none.keeps(&small), "a restorable face is restored by default");
        assert!(!none.keeps(&large), "a face too large to restore is left alone by default");

        let turned = FaceChoice { skipped: vec![key(&small)], restored: vec![key(&large)] };
        assert!(!turned.keeps(&small), "a face the user skipped was restored");
        assert!(turned.keeps(&large), "a large face the user chose was left alone");
    }

    /// An enhancer whose detection finds `found`, or fails.
    struct Finding {
        found: Result<Vec<Face>, InferenceError>,
    }

    impl Enhancer for Finding {
        async fn process(
            &self,
            _source: &Picture,
            _operations: &[opai::Operation],
            _options: opai::ProcessOptions,
        ) -> Result<opai::Enhanced, InferenceError> {
            unreachable!("finding the faces runs no chain")
        }

        async fn detect(
            &self,
            _source: &Picture,
            analysis: &Analysis,
            _options: ExecuteOptions,
        ) -> Result<Executed<Faces>, InferenceError> {
            assert_eq!(*analysis, Detection::for_face_recovery(), "a chain detected with another detector");

            self.found.clone().map(|found| executed(Faces::new(found)))
        }
    }

    /// `requested` resolved and handed its faces by [`for_chain`] over a small picture.
    fn chain_over(
        enhancer: &Finding,
        requested: &[Requested],
    ) -> Result<(Vec<opai::Operation>, ChainFaces), InferenceError> {
        let operations = requested.iter().map(|requested| requested.resolve().expect("published")).collect();
        let picture = Picture::new("/p.png", image::DynamicImage::new_rgb8(8, 8), "0123456789abcdef");

        tauri::async_runtime::block_on(for_chain(enhancer, &picture, requested, operations, ExecuteOptions::default()))
    }

    fn athens(choice: Option<FaceChoice>) -> Requested {
        Requested {
            faces: choice,
            ..Requested::named(opai::Family::FaceRecovery, "athens", opai::Precision::Fp32, &[("fidelity", 1.0)])
        }
    }

    fn upscale() -> Requested {
        Requested::named(opai::Family::Upscale, "kyoto", opai::Precision::Fp32, &[("scale", 2.0)])
    }

    #[test]
    fn a_chain_without_a_face_recovery_looks_for_nobody() {
        let failing = Finding { found: Err(InferenceError::Cancelled) };

        let (operations, faces) = chain_over(&failing, &[upscale()]).expect("nothing was looked for");

        assert_eq!(faces, ChainFaces::NotAsked);
        assert_eq!(operations, [upscale().resolve().expect("published")]);
    }

    #[test]
    fn a_face_recovery_is_handed_the_faces_its_choice_keeps_in_the_order_they_were_found() {
        let small = boxed(0.0, 0.0, 100.0, 100.0);
        let large = boxed(200.0, 0.0, 800.0, 600.0);
        let skipped = boxed(900.0, 0.0, 1000.0, 100.0);
        let enhancer = Finding { found: Ok(vec![small, large, skipped]) };
        let choice = FaceChoice { skipped: vec![face_key(&skipped)], restored: Vec::new() };

        let (operations, faces) = chain_over(&enhancer, &[athens(Some(choice)), upscale()]).expect("found");

        assert_eq!(
            operations,
            [
                opai::FaceRecovery::athens(opai::FloatPrecision::Fp32, Faces::new([small]), opai::Fidelity::MAXIMUM),
                upscale().resolve().expect("published"),
            ]
        );
        // Every face found comes back, chosen or not: the window counts and draws them all.
        assert_eq!(faces, ChainFaces::Found([small, large, skipped].map(DetectedFace::from).to_vec()));
    }

    #[test]
    fn a_detection_that_fails_leaves_the_recovery_restoring_nobody_and_says_why() {
        let enhancer = Finding { found: Err(InferenceError::Untileable { width: 0, height: 0 }) };

        let (operations, faces) = chain_over(&enhancer, &[athens(None)]).expect("a failure does not fail the chain");

        assert_eq!(operations, [athens(None).resolve().expect("published")]);
        assert_eq!(faces, ChainFaces::Failed(InferenceError::Untileable { width: 0, height: 0 }.to_string()));
    }

    #[test]
    fn a_stop_during_the_detection_stops_the_chain() {
        let enhancer = Finding { found: Err(InferenceError::Cancelled) };

        assert!(matches!(chain_over(&enhancer, &[athens(None)]), Err(InferenceError::Cancelled)));
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
