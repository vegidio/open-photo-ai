//! Finding the faces a chain's face recoveries restore, as `process` does it before the chain.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::inference::faces::FaceSelection;
use crate::models::{FaceRecovery, Faces};

fn santorini() -> Operation {
    FaceRecovery::santorini(FloatPrecision::Fp32, Faces::empty())
}

/// [`process_with`] over `operations` with `faces`, through a detection answering `answer`: the outcome, how many times
/// the detection was asked, and the options it was asked with.
async fn processed(
    backend: &Fake,
    operations: &[Operation],
    options: ProcessOptions,
    answer: Result<Faces, InferenceError>,
) -> (Result<Enhanced, InferenceError>, usize, Option<ExecuteOptions>) {
    let calls = AtomicUsize::new(0);
    let handed = Mutex::new(None);
    let detect = |options: Option<ExecuteOptions>| {
        calls.fetch_add(1, Ordering::SeqCst);
        *handed.lock().unwrap() = options;

        std::future::ready(answer)
    };

    let outcome = process_with(backend, None, &picture(120, 80), operations, Some(options), detect).await;

    (outcome, calls.load(Ordering::SeqCst), handed.into_inner().unwrap())
}

#[tokio::test]
async fn faces_as_given_detect_nothing() {
    let (_, calls, _) =
        processed(&Fake::new(), &[santorini(), kyoto(2.0)], ProcessOptions::default(), Ok(Faces::empty())).await;

    assert_eq!(calls, 0, "a run with its faces as given ran a detection");
}

#[tokio::test]
async fn a_chain_without_a_face_recovery_detects_nothing_and_was_not_asked() {
    let options = ProcessOptions { faces: FaceSelection::restorable(), ..Default::default() };

    let (outcome, calls, _) = processed(&Fake::new(), &[kyoto(2.0)], options, Ok(Faces::empty())).await;

    assert_eq!(calls, 0, "a chain carrying no face recovery ran a detection");
    let enhanced = outcome.expect("the chain ran");
    assert!(matches!(enhanced.faces, ChainFaces::NotAsked), "{:?}", enhanced.faces);
}

#[tokio::test]
async fn the_detection_runs_once_under_the_runs_provider_stop_and_cache() {
    let cancel = CancellationToken::new();
    let options = ProcessOptions {
        provider: ExecutionProvider::Cpu,
        cancel: cancel.clone(),
        cache: false,
        faces: FaceSelection::restorable(),
        ..Default::default()
    };

    let (_, calls, handed) =
        processed(&Fake::new(), &[santorini(), kyoto(2.0), santorini()], options, Ok(Faces::empty())).await;

    assert_eq!(calls, 1, "two face recoveries did not share one detection");
    let handed = handed.expect("the detection was handed no options");
    assert_eq!(handed.provider, ExecutionProvider::Cpu);
    assert!(!handed.cache, "the detection kept a store the run turned off");
    cancel.cancel();
    assert!(handed.cancel.is_cancelled(), "the detection runs under a token other than the run's");
}

#[tokio::test]
async fn a_stop_during_the_detection_runs_nothing() {
    let backend = Fake::new();
    let options = ProcessOptions { faces: FaceSelection::restorable(), ..Default::default() };

    let (outcome, _, _) =
        processed(&backend, &[santorini(), kyoto(2.0)], options, Err(InferenceError::Cancelled)).await;

    assert!(matches!(outcome, Err(InferenceError::Cancelled)), "{outcome:?}");
    assert!(backend.log().ran.is_empty(), "the chain ran after the detection was stopped");
}

#[tokio::test]
async fn a_detection_with_no_progress_wanted_reports_nothing() {
    let options = ProcessOptions { faces: FaceSelection::restorable(), ..Default::default() };

    let (_, _, handed) = processed(&Fake::new(), &[santorini()], options, Ok(Faces::empty())).await;

    assert!(
        handed.expect("asked").on_progress.is_none(),
        "a caller that asked for no reports was handed a callback"
    );
}
