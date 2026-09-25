//! What a live view is composed from, and the two threads that write it.
//!
//! Nothing here draws. This module is the state between the sweep and whatever is rendering it, and it exists in
//! that shape for one reason: **the measurement must not be charged for the display**. So it is written from two
//! places with two different costs.
//!
//! - The **listener** side — [`Shared::started`] and [`Shared::stage`] — is called from the sweep's own task at the
//!   boundaries between runs, never inside one. It takes a mutex, and that mutex is never held across terminal I/O:
//!   a renderer copies the state out under the lock and composes its frame from the copy, so a listener call can
//!   never wait on a terminal write.
//!
//! - The **run** side — the callback [`Shared::on_progress`] hands to `ProcessOptions` — is called by the library
//!   once per tile, from inside the section being timed, on the blocking thread the inference runs on. What it does
//!   there is two relaxed atomic stores and a return: no I/O, no allocation, no lock, and nothing drawn. A renderer
//!   polls those atomics on its own timer.
//!
//! The one exception is [`Shared::installing`], which costs a lock and sometimes an allocation; its own doc comment
//! says why neither is charged to a measurement.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use opai::{InferenceProgress, OnInference, Phase, Progress, Stage as RunStage};

use crate::select::Selected;
use crate::stats;
use crate::sweep::{ModelResult, Stage};

/// Nothing has been reported for the run in flight yet.
const IDLE: u8 = 0;
/// The model is on disk and is being run over the image.
const RUNNING: u8 = 1;
/// The result was already known and was served as it stood.
const CACHED: u8 = 2;
/// The model's files are being transferred.
const DOWNLOADING: u8 = 3;
/// The transfer is being expanded onto disk.
const EXTRACTING: u8 = 4;
/// A part of an install this binary has no name for, which is how a third [`Phase`] reads rather than a build error.
const INSTALLING: u8 = 5;

/// The code for `phase`, which is what an [`Activity`] is stored as.
///
/// `Phase` is `#[non_exhaustive]`, so a third part maps onto [`INSTALLING`] rather than failing to compile this
/// binary — nothing about a benchmark depends on which part of an install is in force.
pub fn code(phase: Phase) -> u8 {
    match phase {
        Phase::Downloading => DOWNLOADING,
        Phase::Extracting => EXTRACTING,
        _ => INSTALLING,
    }
}

/// How an install code reads on a line.
///
/// Beside the codes rather than beside either of the two places a line is composed: both the live view and the plain
/// startup renderer name these parts, and two transcriptions of one `#[non_exhaustive]` enum are two things to
/// remember to extend.
pub fn label(code: u8) -> &'static str {
    match code {
        DOWNLOADING => "downloading",
        EXTRACTING => "extracting",
        _ => "installing",
    }
}

/// The model being measured, and everything about it that a frame is composed from.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// The variant's codename, which is what labels the line and what names it on the command line.
    pub codename: &'static str,

    /// The operation's own display name — `Kyoto 4x (FP32)` — which names a transfer that has not yet said which
    /// of the model's artifacts it is moving.
    ///
    /// An [`Arc<str>`] rather than a `String` because a snapshot of this is taken on every frame: the name is written
    /// once per model and read fifteen times a second, so cloning it should not be an allocation.
    pub display_name: Arc<str>,

    /// Its position in the sweep, 1-based.
    pub position: usize,

    /// How many models the sweep covers.
    pub total: usize,

    /// Where the measurement loop has got to, or `None` in the instant between a model starting and its first run.
    ///
    /// Never [`Stage::Run`]: a finished run is not a place the loop sits, so it is folded into the two fields below
    /// instead of being displayed as a stage.
    pub stage: Option<Stage>,

    /// The last timed run that completed, or `None` before the first one does.
    pub last: Option<Duration>,

    /// The median of every timed run so far, or `None` before the first one completes — what the estimate is derived
    /// from.
    ///
    /// Held rather than derived on demand: this is read once per frame and changes once per completed run, and
    /// `stats::compute` sorts a copy of the samples to answer. Computed through that same function, so the figure
    /// watched during a sweep and the one in the table afterwards cannot disagree.
    ///
    /// The cold start is deliberately not among the samples. It is several times a steady-state run by construction,
    /// which is the whole reason it is measured separately, and an estimate that included it would overstate every
    /// figure.
    pub median: Option<Duration>,
}

/// What the run in flight is doing, as the library last reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activity {
    /// Nothing has been reported yet.
    Idle,

    /// The model is being run over the image.
    Running,

    /// The model was not run at all: this operation's result was already known.
    ///
    /// Shown as itself rather than as a very fast run, because a sweep whose runs executed nothing is a finding —
    /// the report calls it `NothingExecuted` — and this is that finding made visible a minute earlier.
    Cached,

    /// The model's files are being put on disk.
    Installing(Install),
}

/// An install in progress, as the line reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    /// Which part of the install this is: `downloading`, `extracting`, or `installing` where it is neither.
    pub phase: &'static str,

    /// The dependency being obtained, or `None` before the first report naming one arrives.
    pub dependency: Option<String>,

    /// Bytes moved so far in this phase.
    pub bytes: u64,

    /// The phase's total, where one is known.
    pub total: Option<u64>,
}

/// One frame's worth of state, copied out so that nothing is drawn while a lock is held.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The model being measured, or `None` before the first one starts.
    pub model: Option<Model>,

    /// How far the run in flight has got, in `0.0..=1.0`.
    pub fraction: f64,

    /// What that run is doing.
    pub activity: Activity,
}

/// The state a renderer reads and the sweep writes.
///
/// Held behind an [`Arc`] by both: the listener is borrowed by the sweep future for its whole life, so the draw arm
/// cannot reach it through the listener and holds a handle of its own.
#[derive(Debug, Default)]
pub struct Shared {
    /// Written at the boundaries between runs, from the sweep's task.
    model: Mutex<Option<Model>>,

    /// Every timed run the model in flight has taken, in run order.
    ///
    /// Beside [`Model`] rather than in it: a snapshot is taken once per frame and these are needed only to recompute
    /// the median, once per completed run. Kept here, a frame copies the answer instead of the samples.
    runs: Mutex<Vec<Duration>>,

    /// `chain_fraction` as bits, written per tile from the blocking thread the inference runs on.
    ///
    /// One `AtomicU64` rather than a lock because there is nothing to synchronise: the bar is a number that is
    /// allowed to be one frame stale, and a mutex here would put the inference thread at risk of waiting on a
    /// renderer that is mid-draw.
    fraction: AtomicU64,

    /// Which of the six things above the last report was, written in the same store as the fraction.
    activity: AtomicU8,

    /// An install's bytes so far, and its total with `0` standing for "not declared".
    bytes: AtomicU64,
    total: AtomicU64,

    /// The dependency an install is obtaining. Touched only by the install arm — see the module note.
    dependency: Mutex<Option<String>>,

    /// Finished models waiting to be written into the terminal's own scrollback.
    ///
    /// A queue rather than a direct write, because the terminal is owned by whatever is drawing and the listener is
    /// borrowed by the sweep future for its whole life — the two cannot both hold it. What that costs is that a
    /// finished model reaches the screen on the next frame rather than within the call, which is at most one tick.
    scrollback: Mutex<Vec<String>>,
}

impl Shared {
    /// A fresh state, before any model has started.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The callback a run reports its progress to.
    ///
    /// Registered on **every** run — warm-up, cold start and timed runs alike. What that charges a measurement is
    /// two relaxed stores per tile, against a tile of convolutional inference; what it buys is the only report a
    /// model's transfer has ever had, and a bar that moves during the minute a CPU run takes.
    pub fn on_progress(self: &Arc<Self>) -> OnInference {
        let shared = Arc::clone(self);

        Arc::new(move |progress: &InferenceProgress| shared.report(progress))
    }

    /// What one report from a run does, and the whole of it.
    fn report(&self, progress: &InferenceProgress) {
        self.fraction.store(progress.chain_fraction.to_bits(), Ordering::Relaxed);

        match &progress.stage {
            RunStage::Running => self.activity.store(RUNNING, Ordering::Relaxed),
            RunStage::Cached => self.activity.store(CACHED, Ordering::Relaxed),
            RunStage::Installing(install) => self.installing(install),
            // `Stage` is `#[non_exhaustive]`: a fourth part of a run should read as an ordinary run rather than fail
            // to compile this binary, since nothing about a benchmark depends on which one is in force.
            _ => self.activity.store(RUNNING, Ordering::Relaxed),
        }
    }

    /// An install's report, which is the one arm that does more than store a number.
    ///
    /// It costs a lock, and one allocation on the report where the name changes. Neither is charged to a
    /// measurement: a model is put on disk by the **warm-up**, so this arm cannot be reached from inside a timed
    /// section — and where a sweep asked for no warm-up at all, the cold start it would be reached from is exactly
    /// the figure the report already warns may cover a transfer.
    fn installing(&self, install: &Progress) {
        self.activity.store(code(install.phase), Ordering::Relaxed);
        self.bytes.store(install.bytes, Ordering::Relaxed);
        // `0` for an undeclared total: neither the pinned size nor the response nor the archive header declared one,
        // and a transfer of zero bytes is not a thing that is reported.
        self.total.store(install.total.unwrap_or(0), Ordering::Relaxed);

        let name = install.dependency.as_str();
        let mut dependency = lock(&self.dependency);

        // Only where it changed, so a transfer reporting itself several times a second allocates once.
        if dependency.as_deref() != Some(name) {
            *dependency = Some(name.to_string());
        }
    }

    /// A model's measurement is beginning.
    ///
    /// Everything the previous model left is dropped here rather than at its end, so that a state written between
    /// two models cannot show one model's fraction under the next one's name.
    pub fn started(&self, index: usize, total: usize, selected: &Selected) {
        *lock(&self.model) = Some(Model {
            codename: selected.codename,
            display_name: selected.operation.display_name().into(),
            position: index + 1,
            total,
            stage: None,
            last: None,
            median: None,
        });

        lock(&self.runs).clear();
        self.reset_run();
    }

    /// A model's measurement has ended, and `line` is what the scrollback should keep of it.
    pub fn finished(&self, line: String) {
        lock(&self.scrollback).push(line);

        *lock(&self.model) = None;
        self.reset_run();
    }

    /// Everything finished since this was last called, in the order the models were measured.
    pub fn scrollback(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.scrollback))
    }

    /// The measurement of the model in flight has reached `stage`.
    pub fn stage(&self, stage: Stage) {
        {
            let mut held = lock(&self.model);

            if let Some(model) = held.as_mut() {
                match stage {
                    // Not a place the loop sits but a thing that just happened, so it is folded into what is known
                    // about the model rather than displayed as a stage of its own.
                    Stage::Run { elapsed } => {
                        let mut runs = lock(&self.runs);
                        runs.push(elapsed);

                        model.last = Some(elapsed);
                        model.median = Some(stats::compute(&runs).median);
                    }
                    entered => model.stage = Some(entered),
                }
            }
        }

        // A run starts from nothing rather than from the last one's final report, so a bar never opens full.
        self.reset_run();
    }

    /// One frame's worth of state.
    ///
    /// Copied out rather than borrowed, which is what lets the caller compose and draw with no lock held.
    pub fn snapshot(&self) -> Snapshot {
        let model = lock(&self.model).clone();
        let fraction = f64::from_bits(self.fraction.load(Ordering::Relaxed));

        let activity = match self.activity.load(Ordering::Relaxed) {
            RUNNING => Activity::Running,
            CACHED => Activity::Cached,
            IDLE => Activity::Idle,
            phase => Activity::Installing(Install {
                phase: label(phase),
                dependency: lock(&self.dependency).clone(),
                bytes: self.bytes.load(Ordering::Relaxed),
                total: match self.total.load(Ordering::Relaxed) {
                    0 => None,
                    declared => Some(declared),
                },
            }),
        };

        Snapshot { model, fraction, activity }
    }

    /// Forgets everything the run that just ended reported.
    fn reset_run(&self) {
        self.fraction.store(0.0_f64.to_bits(), Ordering::Relaxed);
        self.activity.store(IDLE, Ordering::Relaxed);
        self.bytes.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        *lock(&self.dependency) = None;
    }
}

/// The spinner's frames, indexed by the tick counter.
///
/// An array and a modulo rather than a dependency: `throbber-widgets-tui` is maintained and is a `ratatui` widget,
/// and its whole content is the line below.
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The codename's column, wide enough for the longest the catalogue publishes.
const CODENAME_WIDTH: usize = 12;

/// The position column, wide enough for `(20/20)`.
const POSITION_WIDTH: usize = 7;

/// The stage column, wide enough for `downloading` and `warm-up 1/1`.
const STAGE_WIDTH: usize = 13;

/// The bar's cells.
const BAR_WIDTH: usize = 20;

/// The active line, composed to `width`.
///
/// A pure function of the state and a width, returning a `String`, which is what lets every property below be
/// asserted with no terminal: that each stage is named, that a narrow width drops the right things in the right
/// order, and that the result never exceeds the width it was given.
///
/// Nothing is ever wrapped. A one-line viewport that wraps is a two-line viewport, and `insert_before` will corrupt
/// it — so what does not fit is dropped, worst first, and what is left is truncated from the right.
pub fn line(snapshot: &Snapshot, tick: usize, width: usize) -> String {
    let Some(model) = snapshot.model.as_ref() else {
        return String::new();
    };

    let head = format!(
        "  {} {:<CODENAME_WIDTH$} {:<POSITION_WIDTH$} {:<STAGE_WIDTH$}",
        FRAMES[tick % FRAMES.len()],
        fit(model.codename, CODENAME_WIDTH),
        format!("({}/{})", model.position, model.total),
        stage_label(model, &snapshot.activity),
    );

    // Left to right, and dropped right to left: the estimate is the first thing a narrow terminal loses and the
    // model's own name is the last.
    let mut segments = vec![bar(snapshot.fraction)];
    segments.extend(tail(model, snapshot));

    while !segments.is_empty() {
        let composed = format!("{head}  {}", segments.join("  "));

        if composed.chars().count() <= width {
            return composed;
        }

        segments.pop();
    }

    fit(head.trim_end(), width)
}

/// How far the run in flight has got, as cells.
fn bar(fraction: f64) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * BAR_WIDTH as f64).round() as usize;

    format!("{}{}", "█".repeat(filled), "░".repeat(BAR_WIDTH - filled))
}

/// What the measurement is doing, as the line names it.
///
/// An install takes precedence over the stage the loop is in, and deliberately: an operator told a model is being
/// measured, who then waits several minutes for what the README documents as a sub-second run, has been told the
/// wrong thing.
fn stage_label(model: &Model, activity: &Activity) -> String {
    if let Activity::Installing(install) = activity {
        return install.phase.to_string();
    }

    match model.stage {
        None => "starting".to_string(),
        Some(Stage::Warmup { run, of }) => format!("warm-up {run}/{of}"),
        Some(Stage::Cold) => "cold start".to_string(),
        Some(Stage::Timed { run, of }) => format!("run {run}/{of}"),
        // Never stored — a finished run is folded into the model rather than entered as a stage — but a label is
        // owed for every value the type can hold.
        Some(Stage::Run { .. }) => "run".to_string(),
    }
}

/// Whatever the stage in flight has to say for itself beyond its bar.
fn tail(model: &Model, snapshot: &Snapshot) -> Vec<String> {
    match &snapshot.activity {
        // The dependency being obtained rather than the model, which the line has already named on the left: a
        // multi-graph variant transfers several artifacts, and which one is in flight is the part that is not
        // already on screen.
        Activity::Installing(install) => {
            let name = install.dependency.as_deref().unwrap_or(&model.display_name);

            vec![match install.total {
                Some(total) => format!("{name} — {} of {}", bytes(install.bytes), bytes(total)),
                None => format!("{name} — {}", bytes(install.bytes)),
            }]
        }

        // Said rather than shown as a very fast run, which is the report's own `NothingExecuted` finding made
        // visible a minute before the table is rendered.
        Activity::Cached => vec!["from an earlier run".to_string()],

        Activity::Running | Activity::Idle => {
            let mut tail = Vec::new();

            if let Some(last) = model.last {
                tail.push(format!("last {}", figure(last)));
            }

            if let Some(remaining) = estimate(model, snapshot) {
                tail.push(format!("~{}", figure(remaining)));
            }

            tail
        }
    }
}

/// What is left of this model's timed runs, or `None` where there is nothing to derive it from.
///
/// `(remaining runs + what is left of the one in flight) × the median of the runs already taken`, over **this model
/// alone**. It does not span models: what Kyoto at 4x costs says close to nothing about what the next family costs
/// when the next family has a pipeline — the published figures spread over an order of magnitude — and a number
/// carrying that error would be presented with an authority it has not got. The model counter already answers how
/// far through the sweep the run is; this answers how long until the row appears, which is the question with an
/// answer.
///
/// The cold start is excluded because it is not among the samples [`Model::median`] is derived from, which is where
/// that exclusion is made.
pub fn estimate(model: &Model, snapshot: &Snapshot) -> Option<Duration> {
    // An install is not a run, and nothing measured so far says anything about how long one takes.
    if matches!(snapshot.activity, Activity::Installing(_)) {
        return None;
    }

    // The warm-up and the cold start are not what the median is over, so nothing is offered during either.
    let Some(Stage::Timed { run, of }) = model.stage else {
        return None;
    };

    // Nothing is offered before the first timed run has completed: there is nothing to derive it from.
    let median = model.median?;

    let in_flight = 1.0 - snapshot.fraction.clamp(0.0, 1.0);
    let remaining = f64::from(of.saturating_sub(run)) + in_flight;

    Some(Duration::from_secs_f64(median.as_secs_f64() * remaining))
}

/// One duration, spelled as the summary table spells it.
///
/// Through [`crate::report::duration`] rather than beside it, so a figure read off a line while a sweep runs and the
/// same figure read out of the table afterwards cannot be spelled two ways. Its padding is a column's business and is
/// dropped here, where there are no columns.
pub fn figure(value: Duration) -> String {
    crate::report::duration(value).trim().to_string()
}

/// What the scrollback keeps of a model that has finished.
///
/// Written into the terminal's own scrollback rather than into the view, which is the whole reason the view is
/// inline: a line put there is the terminal's, so it survives the models measured after it, it survives a Ctrl-C,
/// and it is still on screen after the process has exited.
///
/// The consequence is that it is **final** — a model cannot be amended once it is there — which is why it carries
/// only what is known at that moment. Everything over the sweep as a whole, the throughput figures and the downgrade
/// warnings included, is still the summary table's, rendered at the end from the sweep's own result.
pub fn finished(result: &ModelResult) -> String {
    let ending = result
        .outcome
        .ending(|measured| format!("cold {}  median {}", figure(measured.cold), figure(measured.stats.median)));

    // Indented past the spinner, so a finished model's codename sits in the same column as the active line's.
    format!("    {:<CODENAME_WIDTH$}  {ending}", fit(result.codename, CODENAME_WIDTH))
}

/// A byte count, as a transfer reports itself.
fn bytes(value: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;

    let value = value as f64;

    if value < KB {
        format!("{value:.0} B")
    } else if value < MB {
        format!("{:.0} KB", value / KB)
    } else if value < GB {
        format!("{:.0} MB", value / MB)
    } else {
        format!("{:.2} GB", value / GB)
    }
}

/// `text` cut to `width` **characters**, because a codename is counted in cells and a `█` is three bytes.
fn fit(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

/// A lock taken through a poisoned mutex.
///
/// A poisoned lock means a thread panicked while holding it. Nothing guarded here is an invariant a run depends on —
/// it is what a line says — and losing a sweep over a display is the wrong trade.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opai::{Dependency, Family, FloatPrecision, Scale, Upscale};

    /// The model every test measures against, built from the catalogue so its display name is the library's own.
    fn selected() -> Selected {
        Selected {
            codename: "kyoto",
            family: Family::Upscale,
            operation: Upscale::kyoto(FloatPrecision::Fp32, Scale::new(4.0).expect("4x is in range")),
            blocked: None,
        }
    }

    /// A report as a run would make one.
    fn running(fraction: f64) -> InferenceProgress {
        InferenceProgress {
            subject: Arc::new(selected().operation.clone().into()),
            stage: RunStage::Running,
            operation_fraction: fraction,
            chain_fraction: fraction,
        }
    }

    /// A report as an install inside a run would make one.
    fn installing(phase: Phase, bytes: u64, total: Option<u64>) -> InferenceProgress {
        InferenceProgress {
            subject: Arc::new(selected().operation.clone().into()),
            stage: RunStage::Installing(Progress {
                dependency: Dependency::Runtime,
                phase,
                bytes,
                total,
                fraction: 0.5,
            }),
            operation_fraction: 0.1,
            chain_fraction: 0.1,
        }
    }

    #[test]
    fn a_runs_report_stores_its_fraction_and_what_it_was_doing_and_nothing_else() {
        let shared = Shared::new();
        shared.started(0, 1, &selected());

        let before = shared.snapshot();
        assert_eq!(before.fraction, 0.0);
        assert_eq!(before.activity, Activity::Idle);

        shared.on_progress()(&running(0.25));

        let after = shared.snapshot();
        assert_eq!(after.fraction, 0.25);
        assert_eq!(after.activity, Activity::Running);
        // The listener's half of the state is untouched: a run reports what a run knows, which is a number.
        assert_eq!(after.model, before.model);
    }

    #[test]
    fn a_served_result_is_reported_as_itself_rather_than_as_a_very_fast_run() {
        let shared = Shared::new();
        shared.started(0, 1, &selected());

        shared.on_progress()(&InferenceProgress { stage: RunStage::Cached, chain_fraction: 1.0, ..running(1.0) });

        // Which is the report's own `NothingExecuted` finding, a minute earlier.
        assert_eq!(shared.snapshot().activity, Activity::Cached);
    }

    #[test]
    fn an_install_stores_the_dependency_its_phase_and_its_byte_counts() {
        let shared = Shared::new();
        shared.started(0, 1, &selected());
        let on_progress = shared.on_progress();

        on_progress(&installing(Phase::Downloading, 41_000_000, Some(128_000_000)));

        let Activity::Installing(install) = shared.snapshot().activity else {
            panic!("an install reports itself as one");
        };
        assert_eq!(install.phase, "downloading");
        assert_eq!(install.dependency.as_deref(), Some(Dependency::Runtime.as_str()));
        assert_eq!(install.bytes, 41_000_000);
        assert_eq!(install.total, Some(128_000_000));

        // And the expansion that follows the transfer is told from it.
        on_progress(&installing(Phase::Extracting, 1_000, None));

        let Activity::Installing(expanding) = shared.snapshot().activity else {
            panic!("an expansion reports itself as one");
        };
        assert_eq!(expanding.phase, "extracting");
        // An undeclared total is absent rather than zero: nothing declared one, which is not the same as a transfer
        // of nothing.
        assert_eq!(expanding.total, None);
    }

    #[test]
    fn a_run_that_follows_an_install_stops_reporting_the_install() {
        let shared = Shared::new();
        shared.started(0, 1, &selected());
        let on_progress = shared.on_progress();

        on_progress(&installing(Phase::Downloading, 41_000_000, Some(128_000_000)));
        on_progress(&running(0.4));

        assert_eq!(shared.snapshot().activity, Activity::Running);
    }

    #[test]
    fn what_a_run_is_charged_for_reporting_itself_is_stores_and_a_return() {
        // The load-bearing claim of this slice, and the one that cannot be asserted from a value: the callback is
        // registered inside sections whose duration is the whole point of the binary, so what matters is what its
        // body *does*. Read here rather than assumed, over the run's own arm alone — the install arm is named in its
        // own doc comment as the exception, and it cannot be reached from inside a timed section.
        let source = include_str!("view.rs");
        let module = source.split("#[cfg(test)]").next().expect("the file has a first part");

        let body = module
            .split("fn report(&self, progress: &InferenceProgress) {")
            .nth(1)
            .expect("the callback's body")
            .split("\n    }")
            .next()
            .expect("the callback's body ends");

        for forbidden in ["print", "write", "format!", "to_string", "to_owned", "clone", "lock(", "Vec::", "String::"] {
            assert!(!body.contains(forbidden), "the run's own report does `{forbidden}`:\n{body}");
        }

        // What it does instead: it stores.
        assert_eq!(body.matches("store(").count(), 4, "{body}");
        assert!(body.contains("Ordering::Relaxed"), "{body}");
    }

    #[test]
    fn a_models_start_leaves_nothing_of_the_model_before_it() {
        let shared = Shared::new();

        shared.started(0, 2, &selected());
        shared.stage(Stage::Timed { run: 1, of: 5 });
        shared.stage(Stage::Run { elapsed: Duration::from_millis(400) });
        shared.on_progress()(&installing(Phase::Downloading, 41_000_000, Some(128_000_000)));

        let osaka = Selected { codename: "osaka", ..selected() };
        shared.started(1, 2, &osaka);

        let snapshot = shared.snapshot();
        let model = snapshot.model.expect("a model is being measured");

        assert_eq!(model.codename, "osaka");
        assert_eq!(model.position, 2);
        assert_eq!(model.total, 2);
        // None of the previous model's runs, its last figure, its stage, its fraction or its transfer.
        assert_eq!(model.median, None);
        assert_eq!(model.last, None);
        assert_eq!(model.stage, None);
        assert_eq!(snapshot.fraction, 0.0);
        assert_eq!(snapshot.activity, Activity::Idle);
    }

    #[test]
    fn a_sequence_of_listener_calls_leaves_the_state_a_frame_is_composed_from() {
        let shared = Shared::new();

        shared.started(0, 3, &selected());
        shared.stage(Stage::Warmup { run: 1, of: 1 });
        shared.stage(Stage::Cold);
        shared.stage(Stage::Timed { run: 1, of: 2 });
        shared.stage(Stage::Run { elapsed: Duration::from_millis(412) });
        shared.stage(Stage::Timed { run: 2, of: 2 });
        shared.on_progress()(&running(0.6));
        shared.stage(Stage::Run { elapsed: Duration::from_millis(408) });

        let snapshot = shared.snapshot();
        let model = snapshot.model.expect("a model is being measured");

        assert_eq!(model.codename, "kyoto");
        assert_eq!(&*model.display_name, selected().operation.display_name());
        assert_eq!(model.position, 1);
        assert_eq!(model.total, 3);
        // The last stage the loop *entered*: a finished run is folded into the two fields below it instead.
        assert_eq!(model.stage, Some(Stage::Timed { run: 2, of: 2 }));
        assert_eq!(model.last, Some(Duration::from_millis(408)));
        // Both runs are behind the median, which is what the estimate reads rather than the samples themselves.
        assert_eq!(model.median, Some(Duration::from_millis(410)));
        // And the run that just ended is not still filling the next one's bar.
        assert_eq!(snapshot.fraction, 0.0);
    }

    #[test]
    fn the_cold_start_is_not_among_the_runs_an_estimate_is_derived_from() {
        let shared = Shared::new();

        shared.started(0, 1, &selected());
        shared.stage(Stage::Cold);
        shared.stage(Stage::Timed { run: 1, of: 1 });
        shared.stage(Stage::Run { elapsed: Duration::from_millis(400) });

        let model = shared.snapshot().model.expect("a model is being measured");

        // One run, not two: the median is the timed run's alone. Were the cold start among the samples this would
        // be the midpoint of the two, and the cold start is several times a steady-state run by construction.
        assert_eq!(model.median, Some(Duration::from_millis(400)));
    }

    #[test]
    fn a_stage_reported_between_two_models_is_dropped_rather_than_attributed_to_one() {
        let shared = Shared::new();

        shared.started(0, 1, &selected());
        shared.finished("kyoto — done".to_string());
        shared.stage(Stage::Cold);

        assert_eq!(shared.snapshot().model, None);
    }

    /// A state as the listener and the callback between them would leave it.
    fn snapshot(stage: Option<Stage>, runs: &[Duration], fraction: f64, activity: Activity) -> Snapshot {
        Snapshot {
            model: Some(Model {
                codename: "kyoto",
                display_name: "Kyoto 4x (FP32)".into(),
                position: 1,
                total: 1,
                stage,
                last: runs.last().copied(),
                median: (!runs.is_empty()).then(|| stats::compute(runs).median),
            }),
            fraction,
            activity,
        }
    }

    /// The width of a terminal nothing has to be dropped for.
    const WIDE: usize = 200;

    #[test]
    fn every_stage_a_measurement_passes_through_is_named() {
        let named = |stage, activity| line(&snapshot(stage, &[], 0.3, activity), 0, WIDE).trim_end().to_string();

        assert!(named(Some(Stage::Warmup { run: 1, of: 2 }), Activity::Running).contains("warm-up 1/2"));
        assert!(named(Some(Stage::Cold), Activity::Running).contains("cold start"));
        assert!(named(Some(Stage::Timed { run: 3, of: 5 }), Activity::Running).contains("run 3/5"));
        assert!(named(None, Activity::Idle).contains("starting"));

        // And the install, which takes precedence over the stage the loop is in because it is the thing the
        // operator is actually waiting for.
        let transfer = Activity::Installing(Install {
            phase: "downloading",
            dependency: Some("up_kyoto_4x_fp32".to_string()),
            bytes: 41_000_000,
            total: Some(128_000_000),
        });
        let installing = named(Some(Stage::Warmup { run: 1, of: 1 }), transfer);

        assert!(installing.contains("downloading"), "{installing}");
        assert!(!installing.contains("warm-up"), "{installing}");
        // Naming what is being obtained, and how far it has got.
        assert!(installing.contains("up_kyoto_4x_fp32"), "{installing}");
        assert!(installing.contains("41 MB of 128 MB"), "{installing}");

        // A served result is said rather than shown as a very fast run.
        let cached = named(Some(Stage::Timed { run: 1, of: 5 }), Activity::Cached);
        assert!(cached.contains("from an earlier run"), "{cached}");
    }

    #[test]
    fn the_model_and_its_position_in_the_sweep_are_always_on_the_line() {
        let mut state = snapshot(Some(Stage::Timed { run: 2, of: 5 }), &[], 0.0, Activity::Running);
        let model = state.model.as_mut().expect("a model");
        model.position = 7;
        model.total = 20;

        let rendered = line(&state, 0, WIDE);

        assert!(rendered.contains("kyoto"), "{rendered}");
        assert!(rendered.contains("(7/20)"), "{rendered}");
    }

    #[test]
    fn a_watched_figure_and_a_table_figure_are_spelled_identically() {
        let runs = [Duration::from_millis(412), Duration::from_millis(408)];
        let state = snapshot(Some(Stage::Timed { run: 3, of: 5 }), &runs, 0.0, Activity::Running);

        let rendered = line(&state, 0, WIDE);
        let last = crate::report::duration(Duration::from_millis(408));

        // The table's own spelling, without the column padding it carries for the table's sake.
        assert!(rendered.contains(&format!("last {}", last.trim())), "{rendered} lacks {last:?}");

        let remaining = estimate(state.model.as_ref().expect("a model"), &state).expect("an estimate");
        assert!(rendered.contains(&format!("~{}", crate::report::duration(remaining).trim())), "{rendered}");
    }

    #[test]
    fn the_spinner_turns_with_the_tick_and_comes_back_round() {
        let state = snapshot(Some(Stage::Cold), &[], 0.0, Activity::Running);

        let frames: Vec<String> =
            (0..FRAMES.len()).map(|tick| line(&state, tick, WIDE).chars().take(4).collect()).collect();

        // Every frame distinct, and the eleventh is the first again.
        for window in frames.windows(2) {
            assert_ne!(window[0], window[1]);
        }
        assert_eq!(line(&state, FRAMES.len(), WIDE), line(&state, 0, WIDE));
    }

    #[test]
    fn a_narrow_terminal_loses_the_estimate_first_then_the_last_run_then_the_bar() {
        let runs = [Duration::from_millis(412)];
        let state = snapshot(Some(Stage::Timed { run: 2, of: 5 }), &runs, 0.5, Activity::Running);

        let full = line(&state, 0, WIDE);
        assert!(full.contains('~') && full.contains("last") && full.contains('█'));

        // One character narrower than it needs at each step, so each width is the first that cannot hold the piece
        // above it.
        let without_estimate = line(&state, 0, full.chars().count() - 1);
        assert!(!without_estimate.contains('~'), "{without_estimate}");
        assert!(without_estimate.contains("last") && without_estimate.contains('█'), "{without_estimate}");

        let without_last = line(&state, 0, without_estimate.chars().count() - 1);
        assert!(!without_last.contains("last"), "{without_last}");
        assert!(without_last.contains('█'), "{without_last}");

        let without_bar = line(&state, 0, without_last.chars().count() - 1);
        assert!(!without_bar.contains('█') && !without_bar.contains('░'), "{without_bar}");
        // And what survives to the last is the model's own name and where the sweep has got to.
        assert!(without_bar.contains("kyoto") && without_bar.contains("run 2/5"), "{without_bar}");
    }

    #[test]
    fn no_line_at_any_width_exceeds_it_or_wraps() {
        let transfer = Activity::Installing(Install {
            phase: "downloading",
            dependency: Some("up_osaka_vae_encoder_fp16".to_string()),
            bytes: 41_000_000,
            total: Some(2_400_000_000),
        });

        let states = [
            snapshot(None, &[], 0.0, Activity::Idle),
            snapshot(Some(Stage::Warmup { run: 1, of: 1 }), &[], 0.0, transfer),
            snapshot(Some(Stage::Cold), &[], 0.9, Activity::Running),
            snapshot(Some(Stage::Timed { run: 4, of: 5 }), &[Duration::from_secs(90)], 0.5, Activity::Running),
            snapshot(Some(Stage::Timed { run: 1, of: 5 }), &[], 1.0, Activity::Cached),
        ];

        for state in &states {
            for width in 0..120 {
                let rendered = line(state, width % FRAMES.len(), width);

                assert!(rendered.chars().count() <= width, "{width}: {rendered:?}");
                assert!(!rendered.contains('\n'), "{width}: {rendered:?}");
            }
        }
    }

    #[test]
    fn there_is_no_line_before_the_first_model_starts() {
        let nothing = Snapshot { model: None, fraction: 0.0, activity: Activity::Idle };

        assert_eq!(line(&nothing, 0, WIDE), "");
    }

    #[test]
    fn nothing_is_estimated_before_a_models_first_timed_run_has_completed() {
        // The run is in flight and half done, and there is still nothing to derive a figure from.
        let state = snapshot(Some(Stage::Timed { run: 1, of: 5 }), &[], 0.5, Activity::Running);

        assert_eq!(estimate(state.model.as_ref().expect("a model"), &state), None);
        assert!(!line(&state, 0, WIDE).contains('~'));
    }

    #[test]
    fn nothing_is_estimated_during_the_warm_up_the_cold_start_or_an_install() {
        let runs = [Duration::from_millis(400)];

        for stage in [Some(Stage::Warmup { run: 1, of: 1 }), Some(Stage::Cold), None] {
            let state = snapshot(stage, &runs, 0.5, Activity::Running);
            assert_eq!(estimate(state.model.as_ref().expect("a model"), &state), None, "{stage:?}");
        }

        // And not during a transfer, however far through the timed runs the loop says it is: nothing measured so far
        // says anything about how long obtaining a model takes.
        let installing =
            Activity::Installing(Install { phase: "downloading", dependency: None, bytes: 1, total: Some(2) });
        let state = snapshot(Some(Stage::Timed { run: 2, of: 5 }), &runs, 0.5, installing);

        assert_eq!(estimate(state.model.as_ref().expect("a model"), &state), None);
    }

    #[test]
    fn the_estimate_falls_as_a_models_runs_complete() {
        let run = Duration::from_millis(400);

        let at = |completed: usize, current: u32, fraction: f64| {
            let runs = vec![run; completed];
            let state = snapshot(Some(Stage::Timed { run: current, of: 5 }), &runs, fraction, Activity::Running);

            estimate(state.model.as_ref().expect("a model"), &state).expect("an estimate")
        };

        // Run 2 of 5 just started: three runs after it, plus the whole of this one.
        assert_eq!(at(1, 2, 0.0), run * 4);
        // Halfway through it.
        assert_eq!(at(1, 2, 0.5), run.mul_f64(3.5));
        // And it falls at every step through to the last.
        let steps = [at(1, 2, 0.0), at(2, 3, 0.0), at(3, 4, 0.0), at(4, 5, 0.0), at(4, 5, 0.5)];
        for window in steps.windows(2) {
            assert!(window[1] < window[0], "{steps:?}");
        }
    }

    #[test]
    fn the_estimate_covers_the_model_in_flight_and_does_not_claim_to_cover_the_ones_after_it() {
        let runs = [Duration::from_millis(400)];

        let for_position = |position: usize, total: usize| {
            let mut state = snapshot(Some(Stage::Timed { run: 2, of: 5 }), &runs, 0.0, Activity::Running);
            let model = state.model.as_mut().expect("a model");
            model.position = position;
            model.total = total;

            estimate(state.model.as_ref().expect("a model"), &state).expect("an estimate")
        };

        // The first of twenty models and the only model of one produce the same figure: nineteen models still to be
        // measured add nothing to it, which is the whole of what this requirement asks.
        assert_eq!(for_position(1, 20), for_position(1, 1));
        assert_eq!(for_position(1, 20), Duration::from_millis(400) * 4);
    }

    #[test]
    fn the_median_the_estimate_uses_is_the_one_the_report_would_publish() {
        // Read through `stats::compute` rather than averaged here, so a watched figure and the table's median cannot
        // be two different statistics.
        let runs = [Duration::from_millis(300), Duration::from_millis(900), Duration::from_millis(400)];
        let state = snapshot(Some(Stage::Timed { run: 4, of: 5 }), &runs, 0.0, Activity::Running);

        let remaining = estimate(state.model.as_ref().expect("a model"), &state).expect("an estimate");

        assert_eq!(remaining, stats::compute(&runs).median * 2);
    }

    #[test]
    fn a_byte_count_is_spelled_at_the_scale_it_is_read_at() {
        assert_eq!(bytes(900), "900 B");
        assert_eq!(bytes(41_000), "41 KB");
        assert_eq!(bytes(41_000_000), "41 MB");
        assert_eq!(bytes(2_400_000_000), "2.40 GB");
    }

    #[test]
    fn the_bar_is_the_fraction_and_is_always_its_full_width() {
        assert_eq!(bar(0.0).chars().count(), BAR_WIDTH);
        assert_eq!(bar(1.0), "█".repeat(BAR_WIDTH));
        assert_eq!(bar(0.5), format!("{}{}", "█".repeat(10), "░".repeat(10)));
        // A fraction outside the range the library promises cannot make the line a different length.
        assert_eq!(bar(-1.0).chars().count(), BAR_WIDTH);
        assert_eq!(bar(9.0), "█".repeat(BAR_WIDTH));
    }
}
