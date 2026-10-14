//! What a run reports while it works, and the three weightings behind the one number it reports.

use std::sync::{Arc, Mutex};

use crate::models::{ArtifactId, Subject};
use crate::progress::{OnProgress, Progress};
use crate::task::lock;

// Carried from the reference implementation, including the detail that makes it work: the split is decided by whether
// an install happened rather than by whether one was possible, so a model already on disk does not give up a fifth of
// its range to a phase that never ran.
/// How much of one operation's range its install owns, **where an install actually happened**.
pub(crate) const INSTALL_SHARE: f64 = 0.2;

/// Which part of carrying out one operation a report is about.
///
/// [`Installing`](Self::Installing) **nests** the install's own [`Progress`] rather than flattening it, so a front
/// end showing "Downloading Kyoto 4x — 41 MB of 128 MB" gets the byte counts unchanged.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Stage {
    // Nested rather than flattened also keeps the install layer at one representation of itself.
    /// The operation's model is being put on disk, and this is the install's own report.
    Installing(Progress),
    /// The model is on disk and is being run over the image.
    Running,
    // A variant of its own rather than `Running` reported at `1.0`, so that a front end can say *"Upscale — from an
    // earlier run"* instead of showing a bar jump a fifth of its width with no explanation. The three take different
    // amounts of time for different reasons, and a bar that cannot tell them apart cannot explain either a long wait
    // or an instant one.
    /// The model was not run at all: this operation's result was already known and was served as it stood.
    Cached,
}

// A type of its own rather than a widening of `Progress`, which is install-shaped: a tile loop moves no bytes and
// extracts nothing, so widening would mean two fields that are meaningless on most reports and a `dependency` that
// had to become optional.
/// One report from a run.
#[derive(Debug, Clone, PartialEq)]
pub struct InferenceProgress {
    // Behind an `Arc` because a report is emitted per tile, per region and per pass, and the subject is the same value
    // every time. A `Subject` is not cheap to copy: `Operation::FaceRecovery` owns a `Faces`, so copying it per report
    // would copy the whole detected set of a crowded photograph each time, for a field every consumer reads through
    // `display_name`.
    /// What this report is about, which is what a front end names in its label.
    ///
    /// `Arc` derefs, so `report.subject.display_name()` works; a `match` on it needs a `&*`.
    pub subject: Arc<Subject>,
    /// Whether the model is being put on disk or being run.
    pub stage: Stage,
    /// How far this operation has got, in `0.0..=1.0`, over the install and the run together.
    pub operation_fraction: f64,
    /// How far the whole run has got, in `0.0..=1.0`.
    ///
    /// Never decreases over one run, and reaches exactly `1.0` when it finishes. In particular it does not return to
    /// the start when one operation hands over to the next, when an install hands over to a run, or when one pass of
    /// a multi-pass upscale hands over to the next.
    pub chain_fraction: f64,
}

// `Arc<dyn Fn>` rather than a generic, mirroring `OnProgress` and for the reason that one is: it crosses a
// `spawn_blocking` boundary, so it has to be `Send + Sync + 'static` and owned.
/// The callback a caller registers to receive [`InferenceProgress`].
pub type OnInference = Arc<dyn Fn(&InferenceProgress) + Send + Sync>;

// Equal shares because what an operation will cost is not knowable before it runs — a denoise and an 8x upscale each
// take half the bar and the second takes twenty times as long — and the reference divides equally for the same reason.
// What the division does fix is the defect that matters: without it the bar refills from zero once per operation.
/// One run's reporting, divided across the operations in its chain.
///
/// **Equal shares per operation.**
pub(crate) struct ChainProgress {
    /// Where reports go, or `None` for a caller that asked for none — which is what makes reporting free for one.
    on_progress: Option<OnInference>,
    /// How many operations the chain has, which is what each one's share is derived from.
    operations: usize,
}

impl ChainProgress {
    /// The reporting of a chain of `operations` operations, going to `on_progress`.
    pub(crate) fn new(on_progress: Option<OnInference>, operations: usize) -> Self {
        Self { on_progress, operations }
    }

    /// The reporting of operation `index` of the chain, which is `subject`.
    pub(crate) fn operation(&self, index: usize, subject: impl Into<Subject>) -> OperationProgress {
        // The subject is wrapped once here rather than per report. A chain also builds one of these per **cache hit**,
        // to emit a single `cached()` and drop it.
        OperationProgress {
            on_progress: self.on_progress.clone(),
            subject: Arc::new(subject.into()),
            index,
            operations: self.operations,
            state: Mutex::new(OperationState { installed: false, high: 0.0 }),
        }
    }

    /// Whether anything is listening, so a caller can skip composing what nobody reads.
    pub(crate) fn wanted(&self) -> bool {
        self.on_progress.is_some()
    }
}

/// What one operation has reported so far.
#[derive(Debug)]
struct OperationState {
    /// Whether an install actually happened, which is what decides the split — see [`INSTALL_SHARE`]. Recorded rather
    /// than assumed, because a model already on disk reports nothing.
    installed: bool,
    /// The highest operation fraction reported so far. What is actually emitted, so a report can never go backwards
    /// however the phases interleave.
    high: f64,
}

// The install and the run cannot each report `0.0..=1.0` over the same range — the caller sees one bar, and a transfer
// handing over to a run would send it back to the start.
/// The reporting of one operation of a chain: the install and the run folded into the one share it owns.
///
/// **The install takes the head of the range and the run takes the rest.**
pub(crate) struct OperationProgress {
    on_progress: Option<OnInference>,
    subject: Arc<Subject>,
    index: usize,
    operations: usize,
    // Behind a mutex because the two phases report from different places: the install from the asynchronous task the
    // transfer runs on, the run from the blocking thread the tile loop runs on.
    state: Mutex<OperationState>,
}

impl OperationProgress {
    /// The install callback to hand to the session acquisition, covering `artifacts` artifacts, between which the
    /// head of the range is divided.
    ///
    /// `None` where nobody is listening, which is what the install layer itself checks for.
    pub(crate) fn installing(self: &Arc<Self>, artifacts: &[ArtifactId]) -> Option<OnProgress> {
        self.on_progress.as_ref()?;

        let reporter = Arc::clone(self);
        // Divided because an operation can install more than one model — an 8x Kyoto run installs the 4x weights and
        // the 2x weights — and each install reports its own `0.0..=1.0`. Undivided, the second transfer would start
        // again from where the first began.
        let share = INSTALL_SHARE / artifacts.len().max(1) as f64;
        // The artifacts this operation needs, in the order it needs them — which the operation itself answers, so an
        // artifact's position here is a lookup rather than an observation. Watching the reports instead would work
        // only while installs are strictly serial and while the pass sequence is the whole of what an operation
        // requires, and neither is a property of this layer to rely on.
        //
        // Keyed on the dependency's own name, which every variant answers and which is the artifact id for a model.
        let order: Vec<String> = artifacts.iter().map(|id| id.as_str().to_string()).collect();

        Some(Arc::new(move |progress: &Progress| {
            let name = progress.dependency.as_str();
            // A report naming something this operation does not require takes the head of the range rather than
            // widening it: the share is already divided, and there is no position for it to occupy.
            let position = order.iter().position(|known| known == name).unwrap_or(0);

            let fraction = (position as f64 + progress.fraction) * share;
            reporter.emit(Stage::Installing(progress.clone()), fraction, true);
        }))
    }

    /// Reports the run's own `0.0..=1.0` position, mapped onto whatever the install left of this operation's range.
    pub(crate) fn running(&self, fraction: f64) {
        // Read per report rather than fixed when the run starts, so the split is decided by what actually happened
        // rather than by what the code guessed would.
        let installed = lock(&self.state).installed;
        let (base, span) = if installed { (INSTALL_SHARE, 1.0 - INSTALL_SHARE) } else { (0.0, 1.0) };

        self.emit(Stage::Running, base + fraction * span, false);
    }

    /// Reports that this operation was not run because its result was already known.
    ///
    /// Emitted at the operation's **full** share immediately, so that a chain of operations that were all already
    /// known fills the bar rather than leaving it stalled at the start.
    pub(crate) fn cached(&self) {
        // Through `emit` like every other report, which is what keeps the operation-to-chain mapping in one place and
        // is why a fully cached chain still lands on exactly 1.
        self.emit(Stage::Cached, 1.0, false);
    }

    /// Emits one report at `fraction` of this operation, clamped so it never decreases.
    fn emit(&self, stage: Stage, fraction: f64, installed: bool) {
        let Some(on_progress) = self.on_progress.as_ref() else {
            return;
        };

        let operation_fraction = {
            let mut state = lock(&self.state);
            state.installed |= installed;
            state.high = state.high.max(fraction).clamp(0.0, 1.0);
            state.high
        };

        // The chain position is composed from the index rather than accumulated, which is what makes the last report
        // of the last operation land on exactly 1: `(n - 1 + 1) / n` is 1 in floating point, where `(n - 1)/n + 1/n`
        // is not. A bar that stops at 99.98% is a bug report.
        let chain_fraction = (self.index as f64 + operation_fraction) / self.operations.max(1) as f64;

        on_progress(&InferenceProgress {
            subject: Arc::clone(&self.subject),
            stage,
            operation_fraction,
            chain_fraction,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::artifact::{ArtifactId, Family};
    use crate::models::upscale::passes::PassWeights;
    use crate::models::{FloatPrecision, Operation, Precision, Scale, Upscale, UpscaleVariant};
    use crate::progress::{Dependency, Phase};

    /// An upscale operation at `scale`, which is the only family with more than one pass to weight.
    fn kyoto(scale: f64) -> Operation {
        Operation::Upscale(Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp16), Scale::new(scale).unwrap()))
    }

    /// A chain whose reports are collected, returned alongside them.
    fn recording(operations: usize) -> (ChainProgress, Arc<Mutex<Vec<InferenceProgress>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let on_progress: OnInference = Arc::new(move |report: &InferenceProgress| {
            sink.lock().unwrap().push(report.clone());
        });

        (ChainProgress::new(Some(on_progress), operations), seen)
    }

    /// The artifact serving Kyoto's `native`x weights, which is what an install of this operation reports against.
    fn weight_id(native: u8) -> ArtifactId {
        ArtifactId::new(Family::Upscale, "kyoto", Some(native), Precision::Fp16)
    }

    /// The same, as the dependency an install names it by.
    fn weights(native: u8) -> Dependency {
        Dependency::Model(weight_id(native))
    }

    /// The artifacts an operation requires, in the order it needs them.
    fn requires(natives: &[u8]) -> Vec<ArtifactId> {
        natives.iter().copied().map(weight_id).collect()
    }

    /// One install report of `dependency`, at `fraction` of its own transfer.
    fn installing(dependency: Dependency, fraction: f64) -> Progress {
        Progress { dependency, phase: Phase::Downloading, bytes: 0, total: None, fraction }
    }

    /// The chain fractions reported so far, in order.
    fn chain(seen: &Arc<Mutex<Vec<InferenceProgress>>>) -> Vec<f64> {
        seen.lock().unwrap().iter().map(|report| report.chain_fraction).collect()
    }

    /// Asserts a sequence of reported fractions never goes backwards.
    fn assert_monotonic(fractions: &[f64]) {
        for pair in fractions.windows(2) {
            assert!(pair[1] >= pair[0], "the report went backwards: {} then {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn the_chain_fraction_never_decreases_across_a_multi_operation_multi_pass_run() {
        // Two operations, the second of them an 8x upscale served by a 4x pass and then a 2x pass over sixteen times
        // the pixels — which is every handover the weighting has to survive at once.
        let (progress, seen) = recording(2);

        let first = Arc::new(progress.operation(0, kyoto(2.0)));
        let install = first.installing(&requires(&[2])).unwrap();
        for step in 0..=4 {
            install(&installing(weights(2), f64::from(step) / 4.0));
        }
        for step in 0..=4 {
            first.running(f64::from(step) / 4.0);
        }

        let second = Arc::new(progress.operation(1, kyoto(8.0)));
        // Already on disk, so nothing installs and the run owns the whole of the operation's range.
        let weights = PassWeights::new(1000, 1000, &[4, 2]);
        for pass in 0..2 {
            let (base, span) = weights.share(pass);
            for step in 0..=4 {
                second.running(base + (f64::from(step) / 4.0) * span);
            }
        }

        let reported = chain(&seen);
        assert_monotonic(&reported);
        assert_eq!(reported.last().copied(), Some(1.0), "the run did not land on exactly 1");
    }

    #[test]
    fn an_install_handing_over_to_a_run_does_not_return_to_the_start() {
        let (progress, seen) = recording(1);
        let operation = Arc::new(progress.operation(0, kyoto(4.0)));

        let install = operation.installing(&requires(&[4])).unwrap();
        install(&installing(weights(4), 1.0));
        operation.running(0.0);

        let reported = chain(&seen);
        assert_monotonic(&reported);
        // The install owns the head of the range, so the run starts where it left off rather than at zero.
        assert!((reported[0] - INSTALL_SHARE).abs() < 1e-9, "the install did not own its share: {}", reported[0]);
        assert!((reported[1] - INSTALL_SHARE).abs() < 1e-9, "the run restarted the bar: {}", reported[1]);
    }

    #[test]
    fn a_model_already_on_disk_keeps_the_whole_of_its_operations_range() {
        let (progress, seen) = recording(1);
        let operation = Arc::new(progress.operation(0, kyoto(4.0)));

        operation.running(0.0);
        operation.running(1.0);

        assert_eq!(chain(&seen), vec![0.0, 1.0]);
    }

    #[test]
    fn two_installs_within_one_operation_divide_the_head_of_its_range() {
        // An 8x Kyoto run installs the 4x weights and the 2x weights, and each transfer reports its own 0..1.
        let (progress, seen) = recording(1);
        let operation = Arc::new(progress.operation(0, kyoto(8.0)));

        let install = operation.installing(&requires(&[4, 2])).unwrap();
        install(&installing(weights(4), 1.0));
        install(&installing(weights(2), 0.5));
        install(&installing(weights(2), 1.0));

        let reported = chain(&seen);
        assert_monotonic(&reported);
        assert!((reported[0] - INSTALL_SHARE / 2.0).abs() < 1e-9, "the first install took the whole share");
        assert!(
            (reported[2] - INSTALL_SHARE).abs() < 1e-9,
            "the two installs did not fill the head of the range"
        );
    }

    #[test]
    fn an_operation_handing_over_to_the_next_does_not_return_to_the_start() {
        let (progress, seen) = recording(3);

        for index in 0..3 {
            let operation = Arc::new(progress.operation(index, kyoto(2.0)));
            operation.running(0.0);
            operation.running(1.0);
        }

        let reported = chain(&seen);
        assert_monotonic(&reported);
        // Each operation owns an equal third, and the one starting is where the one before it finished.
        assert_eq!(reported.len(), 6);
        assert!((reported[1] - 1.0 / 3.0).abs() < 1e-9);
        assert!((reported[2] - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(reported.last().copied(), Some(1.0), "three thirds did not add up to exactly 1");
    }

    #[test]
    fn a_report_that_would_go_backwards_is_held_at_what_was_already_reported() {
        // The clamp is the safety net under the arithmetic above: whatever order the two phases happen to interleave
        // in, a front end never sees the bar move left.
        let (progress, seen) = recording(1);
        let operation = Arc::new(progress.operation(0, kyoto(2.0)));

        operation.running(0.75);
        operation.running(0.25);

        assert_eq!(chain(&seen), vec![0.75, 0.75]);
    }

    #[test]
    fn a_caller_that_asked_for_no_reports_is_charged_nothing() {
        let progress = ChainProgress::new(None, 2);
        let operation = Arc::new(progress.operation(0, kyoto(2.0)));

        assert!(!progress.wanted());
        // No callback to install against, so the install layer is handed nothing rather than a closure that discards.
        assert!(operation.installing(&requires(&[2])).is_none());
        operation.running(0.5);
    }
}
