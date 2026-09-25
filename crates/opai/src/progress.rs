//! What initialization reports while it works, and the reporter that composes it.

use std::sync::{Arc, Mutex};

use crate::models::ArtifactId;

/// How much of a dependency's report its transfer is worth **where an expansion follows it**, leaving the rest to that
/// expansion.
///
/// Extraction is not a pause. A couple of gigabytes of LZMA2 is a minute or more of single-threaded work with nothing
/// arriving over the network, and without this split the last throttled tick of the download would put the bar on 100%
/// and leave it there for exactly as long as the slowest step takes. Giving the expansion the last fifth is what makes
/// that minute legible as work rather than as a hang.
const DOWNLOAD_SHARE: f64 = 0.8;

/// Whether an install has an expansion phase after its transfer, which is what its report is spread over.
///
/// Carried rather than inferred from whether [`Reporter::extracting`] is ever called, because the split has to be
/// decided *before* the first report: a transfer capped at [`DOWNLOAD_SHARE`] that turns out to have nothing to expand
/// would sit at 80% and then jump, which is the dishonest bar the split exists to avoid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expansion {
    /// An archive is expanded after the transfer, so the transfer owns [`DOWNLOAD_SHARE`] of the report.
    Follows,
    /// Nothing follows. A bare file — a model graph — is already what it will be on disk, so the transfer owns the
    /// whole `0.0..=1.0` range and a finished transfer is a finished install.
    None,
}

/// Which dependency a report is about.
///
/// Carried in the report rather than supplied by the caller: the Go app hardcoded the name at each call site, once per
/// dependency it installed, and later slices add three more without either front end having to learn their names.
///
/// `Clone` rather than `Copy`, because [`Dependency::Model`] names *which* model — there is one variant per archive
/// dependency and one variant for all the models, since which model is being installed is decided at run time. The
/// requirement is that a report name the model, and a unit variant could not.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Dependency {
    /// The ONNX Runtime for this platform.
    Runtime,
    /// The CUDA runtime libraries, installed only on a machine with an NVIDIA adapter.
    Cuda,
    /// cuDNN, installed alongside CUDA.
    Cudnn,
    /// TensorRT, installed only on a machine with an RTX-branded NVIDIA adapter.
    TensorRt,
    /// One AI model's files, installed the first time an operation needing them is run.
    Model(ArtifactId),
}

impl Dependency {
    /// The dependency's name, for a front end with nothing better to show.
    ///
    /// Borrowed rather than `&'static str`, because a model's name is its artifact id and that is composed at run
    /// time. A front end showing an operation's own name instead is what slice 5 reports; this is what an install
    /// on its own can say about itself.
    pub fn as_str(&self) -> &str {
        // The names the Go GUI hardcoded at each of its four emit sites, kept verbatim: they are what a user has
        // already seen in the download dialog, and the point of carrying them here is that neither front end has to
        // learn them again.
        match self {
            Dependency::Runtime => "ONNX Runtime",
            Dependency::Cuda => "NVIDIA CUDA",
            Dependency::Cudnn => "NVIDIA cuDNN",
            Dependency::TensorRt => "NVIDIA TensorRT",
            // The published name, which is the only name an install knows: the variant's user-facing label lives in
            // `models/` and belongs to the operation, not to the transfer.
            Dependency::Model(id) => id.as_str(),
        }
    }
}

/// One dependency an initialization is about to account for, as it is reported before any of it starts.
///
/// A row of the plan rather than of the install: it names a dependency this machine needs, whether or not that
/// dependency turns out to have any work to do. A front end drawing a component list needs the ones already on disk
/// as much as the ones about to be transferred — a list of only the missing ones has no denominator for "1 of 3
/// components installed", and would be a different length on the second launch than on the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedDependency {
    /// The dependency this row names, the same value its [`Progress`] reports carry.
    pub dependency: Dependency,
    /// The total published size, in bytes, of the files backing it.
    ///
    /// A `u64` rather than an `Option<u64>`: a pinned release archive always declares a size, which is why this is
    /// reportable before a request has been made at all. The `Option` on [`Progress::total`] exists for a response
    /// that declared none, which cannot arise here.
    pub size: u64,
}

impl PlannedDependency {
    /// The plan row for `descriptor`, reading both facts off it.
    ///
    /// Beside [`Reporter::for_dependency`] and reading the same two fields, so the row a front end draws and the bar
    /// that later fills it cannot disagree about which dependency it is or how large it is.
    pub(crate) fn for_dependency(descriptor: &crate::deps::release::Dependency) -> Self {
        Self { dependency: descriptor.progress.clone(), size: descriptor.published_size() }
    }
}

/// The callback a caller registers to receive the plan.
///
/// Handed a slice rather than one row at a time, because the plan is one value: a recipient handed a stream of rows
/// would have to reconstruct the list and guess when it was complete.
///
/// `Arc<dyn Fn>` for the reason [`OnProgress`] is: it is stored beside a callback that already has to be one, and
/// a generic would not survive being handed across the same boundaries.
pub type OnPlan = Arc<dyn Fn(&[PlannedDependency]) + Send + Sync>;

/// Which part of an install a report is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Phase {
    /// Bytes are arriving over the network.
    Downloading,
    /// The downloaded archive is being expanded into the install directory.
    Extracting,
    /// Nothing happened: the dependency was already on disk, current and complete, so no bytes were transferred and
    /// nothing was expanded.
    ///
    /// A phase that is not a phase, and the tension is deliberate. A recipient has to be able to tell this from a
    /// transfer that just finished — both carry a `fraction` of `1.0` — and only the phase distinguishes "this is on
    /// disk and always was" from "this arrived a moment ago". That is why it is not called `Skipped` or `Done`: it
    /// says *which*, at the one place a front end reads.
    ///
    /// Its report carries [`Progress::bytes`] of 0, because the field means bytes moved in the current phase and
    /// nothing moved. The dependency's size is on [`Progress::total`] and on
    /// [`PlannedDependency::size`].
    AlreadyInstalled,
}

/// One progress report.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// The dependency being installed.
    pub dependency: Dependency,
    /// The phase the bytes below belong to.
    pub phase: Phase,
    /// Bytes moved so far **in the current phase**.
    pub bytes: u64,
    /// The current phase's total, where one is known. `None` when neither the pinned size nor the response nor the
    /// archive header declared one.
    pub total: Option<u64>,
    /// How far through the whole install of this dependency the report is, in `0.0..=1.0`. Never decreases across
    /// reports for one dependency, and reaches `1.0` only once the expansion has finished.
    pub fraction: f64,
}

/// The callback a caller registers to receive [`Progress`].
///
/// `Arc<dyn Fn>` rather than a generic: the same callback is handed to `rust-sak`'s extraction hook, which requires
/// `Fn + Send + Sync + 'static`, and it crosses a `spawn_blocking` boundary to get there.
pub type OnProgress = Arc<dyn Fn(&Progress) + Send + Sync>;

/// Composes one dependency's download and expansion into a single 0-100% report.
///
/// Throttling is inherited rather than repeated: `rust-sak` already coalesces both the download and the extraction
/// hooks to roughly 256 KiB or 100 ms. What this adds is the phase split, the monotonic clamp, and the terminal report
/// that lands on exactly `1.0` — so a throttled tick can never be the last thing a front end sees.
pub(crate) struct Reporter {
    dependency: Dependency,
    on_progress: Option<OnProgress>,
    state: Mutex<State>,
}

impl State {
    /// The share of the report the transfer owns, which is all of it where nothing is expanded afterwards.
    fn download_share(&self) -> f64 {
        match self.expansion {
            Expansion::Follows => DOWNLOAD_SHARE,
            Expansion::None => 1.0,
        }
    }
}

/// The reporter's mutable position. Behind a mutex because the two phases report from different places: the download
/// from the async task, the expansion from the blocking thread `fs::extract` runs on.
#[derive(Debug)]
struct State {
    /// Whether an expansion follows the transfer, fixed when the reporter is built.
    expansion: Expansion,
    downloaded: u64,
    download_total: Option<u64>,
    extracting: bool,
    extracted: u64,
    extract_total: Option<u64>,
    /// The highest fraction reported so far, which is what is actually emitted. A resumed transfer that corrects its
    /// own byte count downward, or an expansion whose header total turns out to be optimistic, must not walk a
    /// progress bar backwards.
    high: f64,
}

impl Reporter {
    /// Starts a report for `dependency`, with the download's expected size where the pin declares one and whether an
    /// expansion follows the transfer.
    pub(crate) fn new(
        dependency: Dependency,
        on_progress: Option<OnProgress>,
        download_total: Option<u64>,
        expansion: Expansion,
    ) -> Self {
        let state = State {
            expansion,
            download_total,
            downloaded: 0,
            extracting: false,
            extracted: 0,
            extract_total: None,
            high: 0.0,
        };

        Self { dependency, on_progress, state: Mutex::new(state) }
    }

    /// Starts a report for `descriptor`, reading every fact but the callback off it.
    ///
    /// The three arguments below are each a function of the descriptor — the summed source sizes, its own progress
    /// name, and whether an expansion follows — so a caller that assembles them by hand is a caller that can get one
    /// wrong. [`Dependency::expansion`](crate::deps::release::Dependency::expansion) exists so the reporter a caller
    /// builds and the branch the pipeline takes cannot disagree, and that only holds while every caller calls it;
    /// this is what makes it hold without their having to.
    ///
    /// [`new`](Self::new) stays for the tests, which need a reporter over a shape no descriptor produces.
    pub(crate) fn for_dependency(
        descriptor: &crate::deps::release::Dependency,
        on_progress: Option<OnProgress>,
    ) -> Self {
        Self::new(
            descriptor.progress.clone(),
            on_progress,
            Some(descriptor.published_size()),
            descriptor.expansion(),
        )
    }

    /// Records the transfer's absolute position, adopting a total the response discovered.
    ///
    /// Absolute rather than incremental because that is what makes a resume and a restart both correct without the
    /// reporter knowing which happened: a transfer that continues a partial starts at ninety percent and one that had
    /// to start over goes back to zero, and the monotonic clamp decides what a front end actually sees.
    pub(crate) fn downloading(&self, bytes: u64, total: Option<u64>) {
        let mut state = self.lock();
        state.downloaded = bytes;
        // Only ever fills a gap the pin left. A resumed response's `Content-Length` describes the remaining tail, and
        // adopting that as the whole would put the bar past 100% for every resumed download.
        if state.download_total.is_none() {
            state.download_total = total;
        }
        self.emit(state);
    }

    /// Moves the report into the expansion phase and positions it there.
    pub(crate) fn extracting(&self, bytes: u64, total: Option<u64>) {
        let mut state = self.lock();
        state.extracting = true;
        state.extracted = bytes;
        state.extract_total = total;
        self.emit(state);
    }

    /// Lands the report on exactly `1.0`, rather than wherever the last throttled tick happened to fall.
    pub(crate) fn finish(&self) {
        let mut state = self.lock();
        state.high = 1.0;
        self.emit(state);
    }

    /// Reports that there was nothing to do: this dependency was already on disk, current and complete.
    ///
    /// **Exactly one report**, and it is terminal — no download phase, no expansion phase and no intermediate
    /// fraction, because none of that happened. It is not [`finish`](Self::finish) with a different phase: `finish`
    /// lands a bar that has been moving, and this is the whole of what such a dependency ever reports.
    ///
    /// `bytes` of 0 against the published `total`, which is what says the size without claiming it was transferred.
    pub(crate) fn already_installed(&self) {
        let Some(on_progress) = self.on_progress.as_ref() else {
            return;
        };

        // Read off the state rather than taken as an argument, so the size a skipped dependency reports is the same
        // pinned sum the bar would have been spread over had it had anything to do.
        let total = {
            let mut state = self.lock();
            state.high = 1.0;
            state.download_total
        };

        let progress = Progress {
            dependency: self.dependency.clone(),
            phase: Phase::AlreadyInstalled,
            bytes: 0,
            total,
            fraction: 1.0,
        };

        on_progress(&progress);
    }

    /// Takes the state lock, recovering from a poisoned mutex: a panicking callback must not turn every later report
    /// into a panic of its own, and the state it guards is a few counters with no invariant to break.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        crate::task::lock(&self.state)
    }

    /// Emits the current position, clamped so it never decreases. Takes the guard by value so the lock is released
    /// before the callback runs — a front end that re-enters would otherwise deadlock.
    fn emit(&self, mut state: std::sync::MutexGuard<'_, State>) {
        let Some(on_progress) = self.on_progress.as_ref() else {
            return;
        };

        state.high = state.high.max(state.fraction());

        // One test of the phase rather than three: the variant and the pair of counters it names have to agree, and
        // three separate ternaries are three chances for a later field to be added to only two of them.
        let (phase, bytes, total) = if state.extracting {
            (Phase::Extracting, state.extracted, state.extract_total)
        } else {
            (Phase::Downloading, state.downloaded, state.download_total)
        };

        let progress = Progress { dependency: self.dependency.clone(), phase, bytes, total, fraction: state.high };
        drop(state);

        on_progress(&progress);
    }
}

impl State {
    /// Where the bar sits: the transfer owns its share of it and the expansion owns the rest, which is nothing at all
    /// for an install with no expansion phase.
    fn fraction(&self) -> f64 {
        let share = self.download_share();
        let transferred = ratio(self.downloaded, self.download_total).unwrap_or(0.0) * share;
        if !self.extracting {
            return transferred;
        }

        // An expansion with no declared total still has to advance: reaching this phase at all means the transfer is
        // done, so the bar sits at the boundary and is carried to 1.0 by `finish`.
        let expanded = ratio(self.extracted, self.extract_total).unwrap_or(0.0);
        (share + expanded * (1.0 - share)).min(1.0)
    }
}

/// `done / total`, clamped to `0.0..=1.0`. `None` when the total is unknown; a total of zero counts as complete.
fn ratio(done: u64, total: Option<u64>) -> Option<f64> {
    total.map(|total| if total == 0 { 1.0 } else { (done as f64 / total as f64).min(1.0) })
}

/// A callback that collects every report it is handed, returned with the collection.
///
/// One definition rather than one per test module: the three that wanted it differ only in what they wrap it in, and
/// the sink itself — the `Arc<Mutex<Vec<Progress>>>` and the closure pushing into it — was written out three times.
#[cfg(test)]
pub(crate) fn recording() -> (OnProgress, Arc<Mutex<Vec<Progress>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let on_progress: OnProgress = Arc::new(move |progress: &Progress| {
        sink.lock().unwrap().push(progress.clone());
    });

    (on_progress, seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reporter whose reports are collected, returned alongside the handle they land in.
    fn recording_reporter(total: Option<u64>) -> (Reporter, Arc<Mutex<Vec<Progress>>>) {
        let (on_progress, seen) = recording();
        (Reporter::new(Dependency::Runtime, Some(on_progress), total, Expansion::Follows), seen)
    }

    /// The fractions reported so far, in order.
    fn fractions(seen: &Arc<Mutex<Vec<Progress>>>) -> Vec<f64> {
        seen.lock().unwrap().iter().map(|p| p.fraction).collect()
    }

    /// Asserts a fraction matches, to within the rounding the share multiplication introduces — `0.9 * 0.8` is not
    /// exactly `0.72`. The one value worth comparing exactly is `1.0`, which `finish` assigns rather than computes.
    #[track_caller]
    fn assert_fractions(seen: &Arc<Mutex<Vec<Progress>>>, expected: &[f64]) {
        let actual = fractions(seen);
        assert_eq!(actual.len(), expected.len(), "got {actual:?}, expected {expected:?}");
        for (got, want) in actual.iter().zip(expected) {
            assert!((got - want).abs() < 1e-9, "got {actual:?}, expected {expected:?}");
        }
    }

    #[test]
    fn the_transfer_owns_four_fifths_of_the_bar_and_the_expansion_the_rest() {
        let (reporter, seen) = recording_reporter(Some(100));

        reporter.downloading(50, None);
        reporter.downloading(100, None);
        reporter.extracting(0, Some(10));
        reporter.extracting(5, Some(10));
        reporter.extracting(10, Some(10));

        assert_fractions(&seen, &[0.4, 0.8, 0.8, 0.9, 1.0]);
    }

    #[test]
    fn a_completed_transfer_does_not_reach_one_before_the_expansion_starts() {
        let (reporter, seen) = recording_reporter(Some(100));

        reporter.downloading(100, None);

        let last = seen.lock().unwrap().last().unwrap().clone();
        assert_eq!(last.phase, Phase::Downloading);
        assert_eq!(last.fraction, DOWNLOAD_SHARE);
        assert!(last.fraction < 1.0, "the expansion still has to be reported as work");
    }

    #[test]
    fn the_fraction_never_decreases() {
        let (reporter, seen) = recording_reporter(Some(100));

        // A transfer that resumed at ninety percent, then had to start over from zero.
        reporter.downloading(90, None);
        reporter.downloading(0, None);
        reporter.downloading(10, None);

        assert_fractions(&seen, &[0.72, 0.72, 0.72]);
    }

    #[test]
    fn finish_lands_on_exactly_one() {
        let (reporter, seen) = recording_reporter(Some(100));

        // A throttled tick left the bar short of the end, which is the normal case: the last chunk of an expansion is
        // rarely the one that gets reported.
        reporter.downloading(100, None);
        reporter.extracting(9, Some(10));
        reporter.finish();

        let last = seen.lock().unwrap().last().unwrap().clone();
        assert_eq!(last.fraction, 1.0);
        assert_eq!(last.phase, Phase::Extracting);
    }

    #[test]
    fn a_total_the_pin_declared_is_not_replaced_by_a_resumed_response() {
        // A resumed `206` reports only the remaining tail as its length; adopting it would put the bar past 100%.
        let (reporter, seen) = recording_reporter(Some(100));

        reporter.downloading(90, Some(10));

        let last = seen.lock().unwrap().last().unwrap().clone();
        assert_eq!(last.total, Some(100));
        assert!((last.fraction - 0.72).abs() < 1e-9, "got {}", last.fraction);
    }

    #[test]
    fn a_total_the_pin_did_not_declare_is_adopted_from_the_response() {
        let (reporter, seen) = recording_reporter(None);

        reporter.downloading(0, None);
        reporter.downloading(50, Some(100));

        let reports = seen.lock().unwrap().clone();
        assert_eq!(reports[0].total, None);
        assert_eq!(reports[0].fraction, 0.0);
        assert_eq!(reports[1].total, Some(100));
        assert!((reports[1].fraction - 0.4).abs() < 1e-9, "got {}", reports[1].fraction);
    }

    #[test]
    fn reports_carry_the_dependency_and_the_bytes_of_their_own_phase() {
        let (reporter, seen) = recording_reporter(Some(100));

        reporter.downloading(40, None);
        reporter.extracting(7, Some(70));

        let reports = seen.lock().unwrap().clone();
        assert_eq!(reports[0].dependency, Dependency::Runtime);
        assert_eq!((reports[0].bytes, reports[0].total), (40, Some(100)));
        assert_eq!((reports[1].bytes, reports[1].total), (7, Some(70)));
    }

    /// The artifact a model report names itself by.
    fn model() -> Dependency {
        Dependency::Model(ArtifactId::new(
            crate::models::Family::Upscale,
            "kyoto",
            Some(4),
            crate::models::Precision::Fp32,
        ))
    }

    #[test]
    fn an_install_with_no_expansion_spreads_the_whole_range_over_the_transfer() {
        // A model is a bare file: there is nothing after the transfer, so a transfer at 50% is an install at 50% and a
        // finished transfer is a finished install. Capping it at `DOWNLOAD_SHARE` would park the bar at 80% and then
        // jump, which is the dishonest bar the split exists to avoid.
        let (on_progress, seen) = recording();
        let reporter = Reporter::new(model(), Some(on_progress), Some(100), Expansion::None);

        reporter.downloading(50, None);
        reporter.downloading(100, None);
        reporter.finish();

        assert_fractions(&seen, &[0.5, 1.0, 1.0]);
        assert_eq!(seen.lock().unwrap().last().unwrap().fraction, 1.0);
    }

    #[test]
    fn a_transfer_that_reaches_its_total_with_no_expansion_to_follow_is_already_complete() {
        // The counterpart of `a_completed_transfer_does_not_reach_one_before_the_expansion_starts`: with no expansion
        // coming there is nothing left to report, so the transfer alone lands on 1.0 without waiting for `finish`.
        let (on_progress, seen) = recording();
        let reporter = Reporter::new(model(), Some(on_progress), Some(100), Expansion::None);

        reporter.downloading(100, None);

        let last = seen.lock().unwrap().last().unwrap().clone();
        assert_eq!(last.phase, Phase::Downloading);
        assert_eq!(last.fraction, 1.0);
    }

    #[test]
    fn a_model_reports_under_its_own_published_name() {
        let dependency = model();

        assert_eq!(dependency.as_str(), "up_kyoto_4x_fp32");

        let (on_progress, seen) = recording();
        Reporter::new(dependency.clone(), Some(on_progress), Some(10), Expansion::None).downloading(1, None);

        assert_eq!(seen.lock().unwrap()[0].dependency, dependency, "the report did not name the model");
    }

    #[test]
    fn a_model_install_with_no_callback_reports_nothing_and_does_not_panic() {
        let reporter = Reporter::new(model(), None, Some(100), Expansion::None);

        reporter.downloading(50, None);
        reporter.finish();
    }

    #[test]
    fn a_model_fraction_never_decreases_when_a_transfer_restarts() {
        // A resumed transfer that has to start over reports its absolute position, which goes backwards. What a front
        // end sees must not.
        let (on_progress, seen) = recording();
        let reporter = Reporter::new(model(), Some(on_progress), Some(100), Expansion::None);

        reporter.downloading(90, None);
        reporter.downloading(0, None);
        reporter.downloading(40, None);
        reporter.finish();

        let fractions: Vec<f64> = fractions(&seen);
        assert!(fractions.windows(2).all(|w| w[0] <= w[1]), "the fraction went backwards: {fractions:?}");
        assert_eq!(fractions.last(), Some(&1.0));
    }

    #[test]
    fn every_dependency_names_itself_the_way_the_go_app_did() {
        assert_eq!(Dependency::Runtime.as_str(), "ONNX Runtime");
        assert_eq!(Dependency::Cuda.as_str(), "NVIDIA CUDA");
        assert_eq!(Dependency::Cudnn.as_str(), "NVIDIA cuDNN");
        assert_eq!(Dependency::TensorRt.as_str(), "NVIDIA TensorRT");
    }

    #[test]
    fn an_install_with_no_callback_reports_nothing_and_does_not_panic() {
        let reporter = Reporter::new(Dependency::Runtime, None, Some(100), Expansion::Follows);

        reporter.downloading(50, None);
        reporter.extracting(1, Some(2));
        reporter.finish();
    }

    #[test]
    fn an_expansion_with_no_declared_total_still_sits_at_the_phase_boundary() {
        // TAR.XZ cannot report a total without decompressing twice, so the bar holds at the boundary rather than
        // jumping — and `finish` is what carries it home.
        let (reporter, seen) = recording_reporter(Some(100));

        reporter.downloading(100, None);
        reporter.extracting(512, None);
        reporter.finish();

        assert_fractions(&seen, &[0.8, 0.8, 1.0]);
    }
}
