//! One bar over a chain's face detection and the chain that follows it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::inference::progress::{InferenceProgress, OnInference, Stage};
use crate::models::Operation;

// A chain carrying a face recovery is two runs underneath — the detection `process` makes and the chain
// itself — and each reports its own `0..1` and lands on exactly 1. Drawn as they arrive that is two sweeps: the bar
// fills, empties and fills again, which reads as the enhancement having been applied twice. So the two are reported as
// one run in which every enhancement owns an equal slot, and the detection is part of a slot rather than one of its own:
// a face recovery and an upscale are two enhancements, `0..0.5` and `0.5..1`, however the face recovery found its faces.
//
// The figure is the reference's own `progressAfterDetect`, which reserved exactly this fifth inside its face recovery
// because its package acquired the detector itself.
/// How much of the first enhancement's slot a detection owns, before that enhancement starts.
const DETECTION_SHARE: f64 = 0.2;

/// A chain's face detection and the chain itself, reported as one run whose bar moves forward once.
///
/// [`detection`](Self::detection) reports the detection [`Opai::process`](crate::Opai::process) makes for a
/// [`FaceSelection::Detect`](crate::FaceSelection::Detect), and [`chain`](Self::chain) the chain it runs afterwards.
/// Both report into the one callback this was built over.
///
/// **Every enhancement owns `1 / N` of the bar, and the detection is not one of them.** A chain of `N` operations is
/// `N` equal slots, as [`Opai::process`](crate::Opai::process) reports it on its own. A detection that does work is
/// mapped onto the first fifth of the **first slot**, and the first operation's own reports onto the rest of it; every
/// later slot is forwarded as it is. A face recovery applied first — the usual chain — therefore owns its whole slot,
/// detection included: with an upscale after it, `0..0.5` and `0.5..1`. A face recovery applied after another
/// operation (a denoise) cannot own its own detection's progress, because the detection runs before the whole chain and
/// the bar only moves forward; there it is the first operation's slot that holds it.
///
/// **A detection the store served owns nothing.** Nearly every chain after the first at a framing finds its faces in
/// the run store, and reporting that would jump every re-run part of the way in for nothing. So the detection's own
/// reports are forwarded only once one says it is fetching or running, and the chain is forwarded untouched unless one
/// was.
pub(crate) struct Handover {
    /// Where every report goes in the end.
    inner: OnInference,
    /// Whether the detection reported any work, which decides the share the chain is mapped onto.
    detected: Arc<AtomicBool>,
    /// How much of the bar one operation owns: `1 / N`.
    slot: f64,
}

impl Handover {
    /// Splits the reports `on_progress` receives over a detection and `operations`, the chain it is made for.
    ///
    /// `operations` is the chain in any order: a face selection changes no family and the apply order changes no
    /// length, so the chain before its faces are filled in is the same chain for this purpose.
    pub(crate) fn new(on_progress: &OnInference, operations: &[Operation]) -> Self {
        // An empty chain has no slot to share; `1` keeps the arithmetic finite for a detection nothing asked for.
        #[allow(clippy::cast_precision_loss, reason = "a chain is a handful of operations")]
        let slot = 1.0 / operations.len().max(1) as f64;

        Self { inner: Arc::clone(on_progress), detected: Arc::default(), slot }
    }

    /// The callback for the detection: its reports mapped onto the head of the first slot, and a detection served
    /// from the store not reported at all.
    pub(crate) fn detection(&self) -> OnInference {
        let inner = Arc::clone(&self.inner);
        let detected = Arc::clone(&self.detected);
        let head = self.slot * DETECTION_SHARE;

        Arc::new(move |report: &InferenceProgress| {
            if matches!(report.stage, Stage::Cached) {
                return;
            }

            detected.store(true, Ordering::Relaxed);
            inner(&InferenceProgress { chain_fraction: report.chain_fraction * head, ..report.clone() });
        })
    }

    /// The callback for the chain: the first slot's reports mapped onto what a detection that reported left of it, and
    /// every other report as it is; or every report as it is, where the detection reported nothing.
    ///
    /// **Ask for it once the detection has ended**, which is when whether it reported anything is known. The
    /// detection's future completing orders every store its callbacks made before this load.
    pub(crate) fn chain(&self) -> OnInference {
        if !self.detected.load(Ordering::Relaxed) {
            return Arc::clone(&self.inner);
        }

        let inner = Arc::clone(&self.inner);
        let slot = self.slot;

        Arc::new(move |report: &InferenceProgress| {
            // Continuous at the slot's end, where both arms give `slot`, so the bar does not jump into the second one.
            let chain_fraction = if report.chain_fraction < slot {
                slot * DETECTION_SHARE + report.chain_fraction * (1.0 - DETECTION_SHARE)
            } else {
                report.chain_fraction
            };

            inner(&InferenceProgress { chain_fraction, ..report.clone() });
        })
    }
}

impl std::fmt::Debug for Handover {
    /// Written out rather than derived: a callback has nothing to print.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handover")
            .field("detected", &self.detected.load(Ordering::Relaxed))
            .field("slot", &self.slot)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::inference::process::in_apply_order;
    use crate::models::{
        Denoise, Detection, FaceRecovery, Faces, Family, FloatPrecision, Scale, Strength, Subject, Upscale,
        UpscaleVariant,
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

    /// One report, as the library builds one.
    fn reported(subject: &Arc<Subject>, stage: Stage, chain: f64) -> InferenceProgress {
        InferenceProgress { subject: Arc::clone(subject), stage, operation_fraction: chain, chain_fraction: chain }
    }

    /// Everything a [`Handover`] over `operations` forwarded: the detection reporting at each of `detection`'s
    /// stages, then the chain as `process` reports it — each operation, in the applied order, reporting `0..=steps`
    /// of its own equal slot.
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
            detecting(&reported(&subject, stage.clone(), 1.0));
        }

        let chaining = handover.chain();
        let applied = in_apply_order(operations);
        let count = applied.len();
        for (index, operation) in applied.iter().enumerate() {
            let subject = Arc::new(Subject::Enhancement(operation.clone()));
            for step in 0..=steps {
                let chain = (index as f64 + step as f64 / steps as f64) / count as f64;
                chaining(&reported(&subject, Stage::Running, chain));
            }
        }

        sent.lock().unwrap().clone()
    }

    /// Which reports came from the detection.
    fn detections(reports: &[InferenceProgress]) -> Vec<&InferenceProgress> {
        reports.iter().filter(|report| matches!(*report.subject, Subject::Analysis(_))).collect()
    }

    /// The reports' chain fractions as whole percentages.
    fn percents(reports: &[InferenceProgress]) -> Vec<u8> {
        reports.iter().map(|report| (report.chain_fraction * 100.0).round() as u8).collect()
    }

    /// The span of the bar the reports about `family` covered, the detection counting as the face recovery.
    fn span(reports: &[InferenceProgress], family: Family) -> (f64, f64) {
        let fractions = reports
            .iter()
            .filter(|report| {
                let reported = report.subject.family();
                reported == family || (family == Family::FaceRecovery && reported == Family::Detection)
            })
            .map(|report| report.chain_fraction);

        fractions.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), fraction| {
            (low.min(fraction), high.max(fraction))
        })
    }

    #[test]
    fn a_face_recovery_and_its_detection_own_the_first_half_and_an_upscale_the_second() {
        let reports = handed_over(&[kyoto(), santorini()], &[Stage::Running, Stage::Running], 4);

        assert_eq!(span(&reports, Family::FaceRecovery), (0.1, 0.5));
        assert_eq!(span(&reports, Family::Upscale), (0.5, 1.0));
        // The detection holds the first fifth of the face recovery's half; the recovery runs over the rest of it.
        assert_eq!(percents(&detections(&reports).into_iter().cloned().collect::<Vec<_>>()), [10, 10]);
        // And lands on exactly 1, which is what a front end reads as done.
        assert_eq!(reports.last().map(|report| report.chain_fraction), Some(1.0));
    }

    #[test]
    fn a_detection_the_store_served_reports_nothing_and_the_chain_is_forwarded_as_it_is() {
        let reports = handed_over(&[santorini(), kyoto()], &[Stage::Cached], 2);

        assert!(detections(&reports).is_empty());
        let fractions = reports.iter().map(|report| report.chain_fraction).collect::<Vec<_>>();
        assert_eq!(fractions, [0.0, 0.25, 0.5, 0.5, 0.75, 1.0]);
    }

    #[test]
    fn every_operation_owns_an_equal_slot_with_or_without_a_detection() {
        for detection in [&[Stage::Running][..], &[Stage::Cached][..]] {
            let reports = handed_over(&[kyoto(), santorini(), denoise()], detection, 3);

            // Applied as denoise, face recovery, upscale: thirds, whatever the detection did.
            let (_, denoise_end) = span(&reports, Family::Denoise);
            let recovery = reports
                .iter()
                .filter(|report| report.subject.family() == Family::FaceRecovery)
                .map(|report| report.chain_fraction)
                .collect::<Vec<_>>();
            let (upscale_start, upscale_end) = span(&reports, Family::Upscale);

            assert!((denoise_end - 1.0 / 3.0).abs() < 1e-12, "{denoise_end}");
            assert!(
                (recovery[0] - 1.0 / 3.0).abs() < 1e-12 && (recovery[3] - 2.0 / 3.0).abs() < 1e-12,
                "{recovery:?}"
            );
            assert!((upscale_start - 2.0 / 3.0).abs() < 1e-12 && upscale_end == 1.0);
        }
    }

    #[test]
    fn a_face_recovery_applied_second_leaves_its_detection_in_the_first_slot() {
        // The detection runs before the whole chain, so with a denoise applied first it can only sit at the head of
        // the denoise's slot if the bar is to move forward only.
        let reports = handed_over(&[santorini(), denoise()], &[Stage::Running], 2);

        assert_eq!(percents(&reports), [10, 10, 30, 50, 50, 75, 100]);
    }

    #[test]
    fn the_bar_never_decreases_across_the_handover() {
        for operations in [vec![santorini(), kyoto()], vec![santorini(), denoise(), kyoto()], vec![santorini()]] {
            let reports = handed_over(&operations, &[Stage::Running, Stage::Running, Stage::Running], 10);

            for pair in reports.windows(2) {
                assert!(
                    pair[1].chain_fraction >= pair[0].chain_fraction,
                    "the bar went backwards: {} then {}",
                    pair[0].chain_fraction,
                    pair[1].chain_fraction
                );
            }
            assert_eq!(reports.last().map(|report| report.chain_fraction), Some(1.0));
        }
    }
}
