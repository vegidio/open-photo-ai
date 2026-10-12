//! Finding the faces a chain's face recoveries restore: one detection over the chain's input, handed out per recovery
//! by a caller's selection.
//!
//! A face recovery restores the faces it is given and finds none of its own, so a front end running one has to detect
//! first. This is that step, once, for every front end: [`fill_chain_faces`] holds every decision and
//! [`Opai::find_chain_faces`](crate::Opai::find_chain_faces) adds only the detection.
//!
//! ```text
//!   chain has no face recovery?  --> chain unchanged, NotAsked, nothing detected
//!   Detection::for_face_recovery()
//!      Ok(found)        --> each face recovery := its faces the selection keeps, in detector order
//!                           Found(every face, kept or not)
//!      Err(Cancelled)   --> stopped: no chain
//!      Err(other)       --> chain unchanged (recoveries keep their empty selection), Failed(error)
//! ```
//!
//! **A failed detection does not fail the chain.** Refusing it would make the failure also cancel an upscale asked for
//! in the same chain, a second failure caused by the first. A stop is different: the user stopped the run, so the
//! chain does not go on.

use std::future::Future;

use crate::error::InferenceError;
use crate::inference::execute::ExecuteOptions;
use crate::models::{Face, Faces, Operation};

/// What finding the faces for a chain came to, for a front end to show beside the result.
#[derive(Debug, Clone)]
pub enum ChainFaces {
    /// The chain carries no face recovery, so nothing was looked for.
    NotAsked,
    /// Every face found in the chain's input, in the order the detector found them, kept or not: a front end that
    /// lets a user choose among them needs them all.
    Found(Faces),
    /// The detection failed, and every face recovery restores nobody. The rest of the chain is unaffected.
    Failed(InferenceError),
}

/// A chain ready to run: every face recovery handed its faces, and what finding them came to.
#[derive(Debug, Clone)]
pub struct FacedChain {
    /// The chain as it was passed in, in the same order, with every face recovery carrying the faces its selection
    /// kept. Every other operation is unchanged.
    pub operations: Vec<Operation>,
    /// What the detection found, or why it found nothing.
    pub faces: ChainFaces,
}

/// The selection a caller with no per-face choices passes: keep exactly the faces [`Face::restorable`] keeps.
///
/// The rule the GUI applies to a face nobody has chosen about and that Autopilot suggests face recovery by, so a front
/// end with no way to choose restores the faces the GUI restores before a user changes anything. The index is ignored.
pub fn default_face_selection(_index: usize, face: &Face) -> bool {
    face.restorable()
}

/// `operations` with every face recovery handed the faces `select` keeps among those `detected` found.
///
/// The pure half of [`Opai::find_chain_faces`](crate::Opai::find_chain_faces), public so a front end can answer its
/// own test seams by the same rules without a detector on disk.
///
/// `select(index, face)` decides per recovery, `index` being the recovery's position in `operations` as passed in,
/// before [`Opai::process`](crate::Opai::process) puts the chain into apply order. Kept faces stay in detector order:
/// [`Faces`] folds an order-sensitive signature into the cache tag, so one choice asks for one stored result.
///
/// A chain with no face recovery is [`ChainFaces::NotAsked`] whatever `detected` holds. A failed detection is
/// [`ChainFaces::Failed`], with the chain answered unchanged.
///
/// # Errors
///
/// [`InferenceError::Cancelled`], where `detected` is a stop: the one outcome the chain does not go on from.
pub fn fill_chain_faces(
    operations: Vec<Operation>,
    detected: Result<Faces, InferenceError>,
    select: impl Fn(usize, &Face) -> bool,
) -> Result<FacedChain, InferenceError> {
    if !asks_for_faces(&operations) {
        return Ok(FacedChain { operations, faces: ChainFaces::NotAsked });
    }

    let found = match detected {
        Ok(found) => found,
        Err(InferenceError::Cancelled) => return Err(InferenceError::Cancelled),
        // Recorded by `execute`, where it happened; this only notes what the chain does about it.
        Err(error) => {
            tracing::debug!(%error, "the faces for a face recovery could not be found; it restores none");

            return Ok(FacedChain { operations, faces: ChainFaces::Failed(error) });
        }
    };

    let operations = operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| match operation {
            Operation::FaceRecovery(recovery) => {
                let kept = Faces::new(found.iter().copied().filter(|face| select(index, face)));

                Operation::FaceRecovery(recovery.with_faces(kept))
            }
            other => other,
        })
        .collect();

    tracing::debug!(found = found.len(), "the faces for a face recovery were found");

    Ok(FacedChain { operations, faces: ChainFaces::Found(found) })
}

/// [`Opai::find_chain_faces`](crate::Opai::find_chain_faces) over any detection: `detect` is called at most once, with
/// `options`, and only for a chain carrying a face recovery.
pub(crate) async fn find_chain_faces<D, F>(
    operations: Vec<Operation>,
    select: impl Fn(usize, &Face) -> bool,
    options: Option<ExecuteOptions>,
    detect: D,
) -> Result<FacedChain, InferenceError>
where
    D: FnOnce(Option<ExecuteOptions>) -> F,
    F: Future<Output = Result<Faces, InferenceError>>,
{
    if !asks_for_faces(&operations) {
        return Ok(FacedChain { operations, faces: ChainFaces::NotAsked });
    }

    let detected = detect(options).await;

    fill_chain_faces(operations, detected, select)
}

/// Whether any operation in `operations` is a face recovery.
fn asks_for_faces(operations: &[Operation]) -> bool {
    operations.iter().any(|operation| matches!(operation, Operation::FaceRecovery(_)))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::models::{Confidence, FaceRecovery, Fidelity, FloatPrecision, Point, Rect, Scale, Upscale};
    use crate::task::lock;

    fn boxed(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Face {
        Face::new(
            Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y)),
            [Point::new(min_x, min_y); Face::LANDMARKS],
            Confidence::new(0.9).expect("0.9 is inside the permitted range"),
        )
    }

    fn athens(faces: Faces) -> Operation {
        FaceRecovery::athens(FloatPrecision::Fp32, faces, Fidelity::MAXIMUM)
    }

    fn santorini(faces: Faces) -> Operation {
        FaceRecovery::santorini(FloatPrecision::Fp32, faces)
    }

    fn upscale() -> Operation {
        Upscale::kyoto(FloatPrecision::Fp32, Scale::new(2.0).expect("2x is published"))
    }

    fn found(outcome: &Result<FacedChain, InferenceError>) -> &FacedChain {
        outcome.as_ref().expect("the chain was answered")
    }

    // ── The pure fill ────────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_chain_without_a_face_recovery_is_not_asked_whatever_the_detection_said() {
        for detected in [Ok(Faces::new([boxed(0.0, 0.0, 10.0, 10.0)])), Err(InferenceError::Cancelled)] {
            let outcome = fill_chain_faces(vec![upscale()], detected, default_face_selection);

            let chain = found(&outcome);
            assert!(matches!(chain.faces, ChainFaces::NotAsked), "{:?}", chain.faces);
            assert_eq!(chain.operations, [upscale()]);
        }
    }

    #[test]
    fn a_recovery_carries_the_faces_its_selection_keeps_in_detector_order_and_every_face_is_reported() {
        let (a, b, c) = (boxed(0.0, 0.0, 10.0, 10.0), boxed(20.0, 0.0, 30.0, 10.0), boxed(40.0, 0.0, 50.0, 10.0));
        let detected = Faces::new([a, b, c]);

        let outcome =
            fill_chain_faces(vec![upscale(), athens(Faces::empty())], Ok(detected.clone()), |_, face| *face != b);

        let chain = found(&outcome);
        // Input order kept, the upscale untouched.
        assert_eq!(chain.operations, [upscale(), athens(Faces::new([a, c]))]);
        assert!(matches!(&chain.faces, ChainFaces::Found(all) if *all == detected), "{:?}", chain.faces);
    }

    #[test]
    fn an_upscale_and_a_recovery_keeping_both_of_two_faces() {
        let (a, b) = (boxed(0.0, 0.0, 10.0, 10.0), boxed(20.0, 0.0, 30.0, 10.0));
        let detected = Faces::new([a, b]);

        let outcome = fill_chain_faces(vec![upscale(), athens(Faces::empty())], Ok(detected.clone()), |_, _| true);

        let chain = found(&outcome);
        assert_eq!(chain.operations, [upscale(), athens(Faces::new([a, b]))]);
        assert!(matches!(&chain.faces, ChainFaces::Found(all) if *all == detected), "{:?}", chain.faces);
    }

    #[test]
    fn two_recoveries_are_filled_independently_by_their_own_index() {
        let (a, b) = (boxed(0.0, 0.0, 10.0, 10.0), boxed(20.0, 0.0, 30.0, 10.0));

        let outcome = fill_chain_faces(
            vec![athens(Faces::empty()), upscale(), santorini(Faces::empty())],
            Ok(Faces::new([a, b])),
            |index, face| match index {
                0 => *face == a,
                2 => *face == b,
                other => panic!("asked about operation {other}, which is not a face recovery"),
            },
        );

        assert_eq!(found(&outcome).operations, [athens(Faces::new([a])), upscale(), santorini(Faces::new([b]))]);
    }

    #[test]
    fn nobody_in_the_photograph_is_found_with_no_faces_rather_than_failed() {
        let outcome = fill_chain_faces(vec![athens(Faces::empty())], Ok(Faces::empty()), default_face_selection);

        let chain = found(&outcome);
        assert!(matches!(&chain.faces, ChainFaces::Found(all) if all.is_empty()), "{:?}", chain.faces);
        assert_eq!(chain.operations, [athens(Faces::empty())]);
    }

    #[test]
    fn a_failed_detection_restores_nobody_runs_the_rest_and_says_why() {
        let error = InferenceError::Untileable { width: 0, height: 0 };

        let outcome =
            fill_chain_faces(vec![athens(Faces::empty()), upscale()], Err(error.clone()), default_face_selection);

        let chain = found(&outcome);
        assert_eq!(chain.operations, [athens(Faces::empty()), upscale()]);
        let ChainFaces::Failed(reported) = &chain.faces else {
            panic!("a failure was not reported: {:?}", chain.faces);
        };
        assert_eq!(reported.to_string(), error.to_string());
    }

    #[test]
    fn a_stop_during_the_detection_answers_no_chain() {
        let outcome = fill_chain_faces(vec![athens(Faces::empty())], Err(InferenceError::Cancelled), |_, _| true);

        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "{outcome:?}");
    }

    #[test]
    fn the_default_selection_keeps_only_the_faces_small_enough_to_restore() {
        let small = boxed(0.0, 0.0, 100.0, 100.0);
        let large = boxed(200.0, 0.0, 800.0, 600.0);

        let outcome =
            fill_chain_faces(vec![athens(Faces::empty())], Ok(Faces::new([small, large])), default_face_selection);

        let chain = found(&outcome);
        assert_eq!(chain.operations, [athens(Faces::new([small]))]);
        assert!(matches!(&chain.faces, ChainFaces::Found(all) if all.len() == 2), "{:?}", chain.faces);
    }

    // ── The step over a detection ────────────────────────────────────────────────────────────────────────────────────

    /// [`find_chain_faces`] over a detection that finds `answer`, and how many times it was asked.
    fn find_counting(operations: Vec<Operation>, answer: Faces) -> (Result<FacedChain, InferenceError>, usize) {
        let calls = AtomicUsize::new(0);
        let detect = |_options| {
            calls.fetch_add(1, Ordering::SeqCst);

            std::future::ready(Ok(answer))
        };

        let outcome = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime")
            .block_on(find_chain_faces(operations, default_face_selection, None, detect));

        (outcome, calls.load(Ordering::SeqCst))
    }

    #[test]
    fn a_chain_without_a_face_recovery_detects_nothing() {
        let (outcome, calls) = find_counting(vec![upscale()], Faces::empty());

        assert_eq!(calls, 0, "a chain carrying no face recovery ran a detection");
        assert!(matches!(found(&outcome).faces, ChainFaces::NotAsked));
    }

    #[test]
    fn two_recoveries_cost_exactly_one_detection() {
        let face = boxed(0.0, 0.0, 10.0, 10.0);

        let (outcome, calls) =
            find_counting(vec![athens(Faces::empty()), santorini(Faces::empty())], Faces::new([face]));

        assert_eq!(calls, 1, "the detection was not run exactly once");
        assert_eq!(found(&outcome).operations, [athens(Faces::new([face])), santorini(Faces::new([face]))]);
    }

    #[test]
    fn the_callers_options_reach_the_detection() {
        let cancel = crate::CancellationToken::new();
        let options = ExecuteOptions { cancel: cancel.clone(), ..Default::default() };
        let handed = std::sync::Mutex::new(None);

        let detect = |options: Option<ExecuteOptions>| {
            *lock(&handed) = options;

            std::future::ready(Ok(Faces::empty()))
        };

        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime")
            .block_on(find_chain_faces(vec![athens(Faces::empty())], default_face_selection, Some(options), detect))
            .expect("the chain was answered");

        let handed = lock(&handed).take().expect("the detection was handed no options");
        cancel.cancel();
        assert!(handed.cancel.is_cancelled(), "the detection runs under a token other than the caller's");
    }
}
