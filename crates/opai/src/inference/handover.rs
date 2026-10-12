//! One bar over a chain's face detection and the chain that follows it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::inference::process::in_apply_order;
use crate::inference::progress::{InferenceProgress, OnInference, Stage};
use crate::models::{Family, Operation};

// A chain carrying a face recovery is two runs underneath — the detection `Opai::find_chain_faces` makes and the chain
// itself — and each reports its own `0..1` and lands on exactly 1. Drawn as they arrive that is two sweeps: the bar
// fills, empties and fills again, which reads as the enhancement having been applied twice. So the detection is mapped
// onto the head of the range and the chain onto what it leaves.
//
// The figure is the reference's own `progressAfterDetect`, which reserved exactly this fifth inside its face recovery
// because its package acquired the detector itself.
/// How much of the bar a detection owns, before the chain that asked for it starts.
const DETECTION_SHARE: f64 = 0.2;

/// A chain's face detection and the chain itself, reported as one run whose bar moves forward once.
///
/// Wire [`detection`](Self::detection) into the [`ExecuteOptions`](crate::ExecuteOptions) handed to
/// [`Opai::find_chain_faces`](crate::Opai::find_chain_faces), and [`chain`](Self::chain) into the
/// [`ProcessOptions`](crate::ProcessOptions) handed to [`Opai::process`](crate::Opai::process) afterwards. Both report
/// into the one callback this was built over.
///
/// **A detection the store served owns nothing.** Nearly every chain after the first at a framing finds its faces in
/// the run store, and reporting that as a fifth of the bar would jump every re-run a fifth of the way in for nothing. So
/// the detection's own reports are forwarded only once one says it is fetching or running, and the chain takes the
/// whole bar unless one was.
///
/// **The detection counts as the face recovery's step.** Its reports carry the
/// [`operation_index`](InferenceProgress::operation_index) of the first face recovery in the chain as it is applied,
/// and the chain's [`operation_count`](InferenceProgress::operation_count), rather than its own `0` of `1`. Its
/// [`subject`](InferenceProgress::subject) and [`stage`](InferenceProgress::stage) are left alone.
pub struct Handover {
    /// Where every report goes in the end.
    inner: OnInference,
    /// Whether the detection reported any work, which decides the share the chain is mapped onto.
    detected: Arc<AtomicBool>,
    /// The position and count a detection report is rewritten to: the first face recovery's place in the applied
    /// chain, and the chain's length.
    attributed: (usize, usize),
}

impl Handover {
    /// Splits the reports `on_progress` receives over a detection and `operations`, the chain it is made for.
    ///
    /// `operations` must be the chain later handed to [`Opai::process`](crate::Opai::process) — in any order, since
    /// it is put into the applied order here as it is there. A face selection changes no family, so the chain before
    /// [`Opai::find_chain_faces`](crate::Opai::find_chain_faces) fills it in is the same chain for this purpose.
    pub fn new(on_progress: &OnInference, operations: &[Operation]) -> Self {
        let applied = in_apply_order(operations);
        // A chain with no face recovery makes no detection, so its `0` is never read.
        let position = applied.iter().position(|operation| operation.family() == Family::FaceRecovery).unwrap_or(0);

        Self { inner: Arc::clone(on_progress), detected: Arc::default(), attributed: (position, applied.len()) }
    }

    /// The callback for the detection: its reports mapped onto the head of the bar and attributed to the face
    /// recovery, and a detection served from the store not reported at all.
    pub fn detection(&self) -> OnInference {
        let inner = Arc::clone(&self.inner);
        let detected = Arc::clone(&self.detected);
        let (operation_index, operation_count) = self.attributed;

        Arc::new(move |report: &InferenceProgress| {
            if matches!(report.stage, Stage::Cached) {
                return;
            }

            detected.store(true, Ordering::Relaxed);
            inner(&InferenceProgress {
                chain_fraction: report.chain_fraction * DETECTION_SHARE,
                operation_index,
                operation_count,
                ..report.clone()
            });
        })
    }

    /// The callback for the chain: what a detection that reported left of the bar, or the whole of it.
    ///
    /// **Ask for it once the detection has ended**, which is when whether it reported anything is known. The
    /// detection's future completing orders every store its callbacks made before this load.
    pub fn chain(&self) -> OnInference {
        if !self.detected.load(Ordering::Relaxed) {
            return Arc::clone(&self.inner);
        }

        let inner = Arc::clone(&self.inner);

        Arc::new(move |report: &InferenceProgress| {
            let chain_fraction = DETECTION_SHARE + report.chain_fraction * (1.0 - DETECTION_SHARE);

            inner(&InferenceProgress { chain_fraction, ..report.clone() });
        })
    }
}

impl std::fmt::Debug for Handover {
    /// Written out rather than derived: a callback has nothing to print.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handover")
            .field("detected", &self.detected.load(Ordering::Relaxed))
            .field("attributed", &self.attributed)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::models::{
        Denoise, Detection, FaceRecovery, Faces, FloatPrecision, Scale, Strength, Subject, Upscale, UpscaleVariant,
    };

    fn kyoto() -> Operation {
        Operation::Upscale(Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp32), Scale::new(2.0).unwrap()))
    }

    fn santorini() -> Operation {
        FaceRecovery::santorini(FloatPrecision::Fp32, Faces::empty())
    }

    fn denoise() -> Operation {
        Denoise::stockholm(FloatPrecision::Fp32, Strength::clamped(1.0))
    }

    /// One report, as the library builds one, about operation `index` of `count`.
    fn reported(subject: &Arc<Subject>, stage: Stage, chain: f64, (index, count): (usize, usize)) -> InferenceProgress {
        InferenceProgress {
            subject: Arc::clone(subject),
            stage,
            operation_fraction: chain,
            operation_index: index,
            operation_count: count,
            chain_fraction: chain,
        }
    }

    /// Everything a [`Handover`] over `operations` forwarded: the detection reporting at each of `detection`'s
    /// stages, then the chain's upscale, operation `1` of `2`, reporting `0..=steps` of its own range.
    fn handed_over(operations: &[Operation], detection: &[Stage], steps: usize) -> Vec<InferenceProgress> {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let collector = Arc::clone(&sent);
        let inner: OnInference = Arc::new(move |report: &InferenceProgress| {
            collector.lock().unwrap().push(report.clone());
        });
        let handover = Handover::new(&inner, operations);

        let detecting = handover.detection();
        let subject = Arc::new(Subject::Analysis(Detection::for_face_recovery()));
        for stage in detection {
            // The detection's own single-operation run, as `execute` reports it.
            detecting(&reported(&subject, stage.clone(), 1.0, (0, 1)));
        }

        let chaining = handover.chain();
        let upscale = Arc::new(Subject::Enhancement(kyoto()));
        for step in 0..=steps {
            chaining(&reported(&upscale, Stage::Running, step as f64 / steps as f64, (1, 2)));
        }

        sent.lock().unwrap().clone()
    }

    /// Which reports came from the detection.
    fn detections(reports: &[InferenceProgress]) -> Vec<&InferenceProgress> {
        reports.iter().filter(|report| matches!(*report.subject, Subject::Analysis(_))).collect()
    }

    #[test]
    fn a_detection_that_worked_owns_the_head_of_the_bar_and_the_chain_the_rest() {
        let reports = handed_over(&[santorini(), kyoto()], &[Stage::Running], 2);
        let percent = reports.iter().map(|report| (report.chain_fraction * 100.0).round() as u8).collect::<Vec<_>>();

        assert_eq!(percent, [20, 20, 60, 100]);
        // And lands on exactly 1, which is what a front end reads as done.
        assert_eq!(reports.last().map(|report| report.chain_fraction), Some(1.0));
    }

    #[test]
    fn a_detection_the_store_served_reports_nothing_and_the_chain_owns_the_whole_bar() {
        let reports = handed_over(&[santorini(), kyoto()], &[Stage::Cached], 2);
        let fractions = reports.iter().map(|report| report.chain_fraction).collect::<Vec<_>>();

        assert_eq!(fractions, [0.0, 0.5, 1.0]);
    }

    #[test]
    fn a_detection_is_counted_as_the_face_recoverys_step() {
        let reports = handed_over(&[santorini(), kyoto()], &[Stage::Running, Stage::Running], 2);

        let detected = detections(&reports);
        assert_eq!(detected.len(), 2);
        assert!(
            detected.iter().all(|report| (report.operation_index, report.operation_count) == (0, 2)),
            "a detection report kept its own single-operation position"
        );
        // Only the counter is rewritten: a front end still sees what the report is about and what it is doing.
        assert!(detected.iter().all(|report| matches!(report.stage, Stage::Running)));
    }

    #[test]
    fn a_detection_is_counted_at_the_face_recoverys_place_in_the_applied_chain() {
        // Listed with the face recovery second, applied with the denoise in front of it.
        let reports = handed_over(&[kyoto(), santorini(), denoise()], &[Stage::Running], 1);

        let detected = detections(&reports);
        assert_eq!((detected[0].operation_index, detected[0].operation_count), (1, 3));
    }

    #[test]
    fn the_chains_reports_keep_their_own_position() {
        let reports = handed_over(&[santorini(), kyoto()], &[Stage::Running], 2);

        let chained: Vec<_> =
            reports.iter().filter(|report| matches!(*report.subject, Subject::Enhancement(_))).collect();
        assert_eq!(chained.len(), 3);
        assert!(chained.iter().all(|report| (report.operation_index, report.operation_count) == (1, 2)));
    }

    #[test]
    fn the_bar_never_decreases_across_the_handover() {
        let reports = handed_over(&[santorini(), kyoto()], &[Stage::Running, Stage::Running, Stage::Running], 10);

        for pair in reports.windows(2) {
            assert!(
                pair[1].chain_fraction >= pair[0].chain_fraction,
                "the bar went backwards: {} then {}",
                pair[0].chain_fraction,
                pair[1].chain_fraction
            );
        }
    }
}
