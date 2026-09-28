//! The measurement loop, and what one model's measurement can come to.
//!
//! The order per model is **release every resident session → warm-up runs (untimed) → release again → cold start
//! (timed) → timed runs → statistics**, and the two releases are different jobs. The first is the boundary between
//! models: it stops one model's sessions from holding memory while the next is measured, and it is what makes a
//! downgrade attributable to the model that hit it. The second is what makes the cold start a cold start — it drops
//! the session the warm-up built, so the next call pays graph optimization plus the provider's own compilation.
//!
//! # Why sequential is a correctness property here
//!
//! `process::run` accumulates every session handle it acquires for the length of the call, so a session a run is using
//! cannot be reclaimed underneath it; those handles drop when `process` returns. Because the sweep is sequential,
//! nothing is outstanding at either release point — the cache's own reference is the last one, and the native
//! session is dropped inside `release_sessions` before it returns. A concurrent sweep would have another model's
//! handles outstanding there, so a "cold" start could find a session still resident. It would also be measuring
//! contention rather than the model.
//!
//! # What this loop refuses to know
//!
//! Which models the library will run. There is no list here, no flag, and no vocabulary for it: a model is attempted
//! and what comes back is read. A refusal costs one `process` call, which installs nothing and opens nothing because
//! the whole chain is checked before any transfer — and when every family has a pipeline, there is nothing here to
//! remove.

use std::time::{Duration, Instant};

use opai::{
    CancellationToken, Detection, Enhanced, ExecuteOptions, ExecutionProvider, Faces, InferenceError, OnInference,
    Opai, Picture, ProcessOptions, ProviderReport,
};

use crate::cli::Options;
use crate::input::Input;
use crate::select::Selected;
use crate::stats::{self, Stats};

/// What the auxiliary detection run found, or why there is nothing to hand the models that needed it.
///
/// One model in the catalogue cannot do any work without a result only another model can produce: a face-recovery
/// run restores the faces it is given and finds none of its own. Without them the measurement is of nothing — the
/// run composites no face and returns almost immediately, so a sweep would report a sub-millisecond time for a model
/// that takes seconds on a real photograph, **and would report it as a success**.
///
/// The two answers that are not a set of faces are kept apart from each other in the reporting and are the same
/// thing to a model: a failed detection and a photograph with nobody in it both mean this row has nothing to
/// measure.
#[derive(Debug, Clone, PartialEq)]
pub enum Detected {
    /// The pass ran and found faces.
    Found(Faces),

    /// The pass ran and found none.
    ///
    /// Failing the rows is the required outcome rather than measuring the empty case: a sweep of a photograph with
    /// no face in it has nothing to say about a face-recovery model.
    Empty,

    /// The pass itself failed, and this is the reason the library gave.
    ///
    /// Kept rather than discarded, so the rows that depended on it say **why** rather than saying that something
    /// went wrong earlier.
    Failed {
        /// The reason, as the library phrased it.
        reason: String,
    },
}

impl Detected {
    /// The faces to hand the models that need them, or the reason there are none.
    ///
    /// # Errors
    ///
    /// The reason a row that needed this result cannot be measured, phrased for that row's own report.
    pub fn usable(&self) -> Result<&Faces, String> {
        match self {
            Self::Found(faces) => Ok(faces),
            Self::Empty => Err("no face was detected in this image, so there is nothing to restore".to_string()),
            Self::Failed { reason } => Err(format!("the faces to restore could not be detected: {reason}")),
        }
    }

    /// How many faces were found, for the report's conditions.
    pub fn count(&self) -> usize {
        match self {
            Self::Found(faces) => faces.len(),
            Self::Empty | Self::Failed { .. } => 0,
        }
    }
}

/// Detects the faces every face-recovery row of `selection` is to be measured over, **once**.
///
/// Made after the picture is loaded and before the first model is measured, and only where a row needs it — see
/// [`select::needs_faces`](crate::select::needs_faces), which is what decides whether this is called at all.
///
/// # Once, not per model
///
/// Every model boundary in the sweep drains the session cache, so a detection made per model would pay a full
/// detection session build inside each face-recovery row's measurement: untimed but real wall clock, and on TensorRT
/// larger than everything else in the sweep.
///
/// # Its cost is charged to nothing
///
/// This happens outside every timed section, before the first `started` is reported, so no model's cold start and no
/// model's runs contain any of it.
///
/// The run cache is **off** for it, as it is for the sweep's own runs by default: a served analysis builds no model,
/// and a pass that was served from an earlier invocation would hand the sweep faces without having demonstrated that
/// the detector works on this machine.
///
/// # The application's detection, not the sweep's precision
///
/// [`Detection::for_face_recovery`], whatever precision the rows were selected at: that is the detection the
/// application feeds every face recovery with, so the faces a row restores are the faces the application would have
/// handed it. Following the sweep's precision would benchmark a detector build the application never runs.
pub async fn detect(opai: &Opai, input: &Input, provider: ExecutionProvider, cancel: &CancellationToken) -> Detected {
    let options = ExecuteOptions { provider, cancel: cancel.clone(), cache: false, ..Default::default() };

    match opai.execute(&input.picture, &Detection::for_face_recovery(), Some(options)).await {
        Ok(executed) if executed.value.is_empty() => Detected::Empty,
        Ok(executed) => Detected::Found(executed.value),
        Err(error) => Detected::Failed { reason: error.to_string() },
    }
}

/// What a sweep came to.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepResult {
    /// Every model the report covers, in the order they were measured.
    ///
    /// A model the library declined is here only where the caller **named** it, which is what makes the refusal and
    /// its reason visible to whoever asked about that model specifically.
    pub results: Vec<ModelResult>,

    /// How many models the library declined and this sweep dropped, because the caller named none.
    ///
    /// A refusal is not a measurement and is not a failure, and today it would be twenty rows of it. Reported as a
    /// count with a line saying that naming one shows why, which is the whole of what the report says about them.
    pub skipped: usize,
}

impl SweepResult {
    /// How many models failed to be measured.
    ///
    /// Interruptions are not counted: a Ctrl-C is not a fault in a model, and counting one would make a cancelled
    /// run indistinguishable from a broken one.
    pub fn failures(&self) -> usize {
        self.results.iter().filter(|result| matches!(result.outcome, Outcome::Failed { .. })).count()
    }

    /// Whether the sweep was stopped before it finished.
    pub fn interrupted(&self) -> bool {
        self.results.iter().any(|result| matches!(result.outcome, Outcome::Interrupted))
    }

    /// The process's exit status: failure where a model failed or the sweep was interrupted, success otherwise.
    ///
    /// A model the library has no pipeline for does **not** count. Nothing about the sweep went wrong, and a status
    /// that said otherwise could never be used as a check.
    pub fn exit_code(&self) -> i32 {
        i32::from(self.failures() > 0 || self.interrupted())
    }
}

/// One model's row in the report.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelResult {
    /// The variant's codename.
    pub codename: &'static str,
    /// Its family, for the report's type column.
    pub family: opai::Family,
    /// How it is described, which is the operation's own display name: `Kyoto 4x (FP16)`.
    pub display_name: String,
    /// What became of it.
    pub outcome: Outcome,
}

/// The four things a model's measurement can come to.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// It was measured.
    Completed(Box<Measured>),

    /// The library has no pipeline for it.
    ///
    /// Told from a failure by the library's own [`InferenceError::Unsupported`] and by nothing this crate decides,
    /// which is what makes a model that gains a pipeline measured by the same sweep with nothing here changed.
    Declined {
        /// The reason the library gave.
        reason: String,
    },

    /// Something went wrong measuring it: its weights could not be obtained, no session could be built, or a run
    /// failed.
    Failed {
        /// The reason the library gave, with the stage that hit it.
        reason: String,
    },

    /// The operator stopped the sweep during this model's measurement.
    Interrupted,
}

impl Outcome {
    /// How this outcome reads at the end of a one-line report of the model, with `completed` saying how a
    /// measurement itself is spelled.
    ///
    /// The three endings that are not a measurement are written here once. Both renderers — the plain lines and the
    /// live view's scrollback — need all four, and they differ only in how much of a measurement they have room for;
    /// written twice, a reworded decline reached one of them and not the other.
    pub fn ending(&self, completed: impl FnOnce(&Measured) -> String) -> String {
        match self {
            Self::Completed(measured) => completed(measured),
            Self::Declined { reason } => format!("declined: {reason}"),
            Self::Failed { reason } => format!("failed: {reason}"),
            Self::Interrupted => "interrupted".to_string(),
        }
    }
}

/// A completed measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    /// Session construction plus one inference, measured after the warm-up so it never covers a transfer.
    pub cold: Duration,
    /// Every timed run, in run order — which is what `--verbose` prints and what a thermal ramp shows up in.
    pub runs: Vec<Duration>,
    /// The distribution over those runs.
    pub stats: Stats,
    /// The dimensions of the image the last run produced.
    pub output: (u32, u32),
    /// What this model's runs were actually executed on.
    pub providers: Verdict,
}

/// What a model's runs were executed on, folded across every run it took.
///
/// The library's own reading of a [`ProviderReport`], under this crate's name for it. Classifying one report is
/// `opai`'s job — the three ways to misread one are documented there, and a front end saying *"CUDA could not open
/// this model, so it ran on the CPU"* needs the same answer this does.
///
/// What stays here is the *fold*: this is the union across every run a model took rather than the cold start's report
/// alone, although the cold start is where a session is built and so where a downgrade is decided. The timed runs use
/// the cold start's sessions, so the two agree in every case anyone has described, and a union cannot be the thing
/// that misses one.
pub use opai::ProviderVerdict as Verdict;

/// Where in one model's measurement the loop has got to.
///
/// The loop's own vocabulary, and deliberately not the library's: an **install** is not here, because it arrives on
/// the run's progress callback, from the library, on another thread, carrying the dependency and its byte counts.
/// Routing that through the listener would mean this loop learning about installs, which it has no other reason to
/// know about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// An untimed warm-up run, 1-based; also what puts a model on disk that is not there already.
    Warmup {
        /// Which warm-up run this is.
        run: u32,
        /// How many there are.
        of: u32,
    },

    /// The timed cold start: session construction plus one inference, after the warm-up and after a release.
    Cold,

    /// A timed run, 1-based.
    Timed {
        /// Which timed run this is.
        run: u32,
        /// How many there are.
        of: u32,
    },

    /// A timed run has finished, and this is what it took.
    ///
    /// Not a place the loop sits but a thing that just happened, which is why a renderer folds it into what it knows
    /// rather than displaying it as a stage. The cold start is **not** reported this way: it is measured before the
    /// first timed run and is several times a steady-state run by construction, so an estimate derived from the runs
    /// so far must not see it.
    Run {
        /// What the run took.
        elapsed: Duration,
    },
}

/// Told as each model starts and finishes and as the measurement moves through it, so a renderer can follow the
/// sweep without the sweep knowing about it.
///
/// The seam the live view attaches to: silent for `--json`, one line per model for a sweep with no terminal, and the
/// live view otherwise — and the sweep is the same in all three.
pub trait Listener {
    /// A model's measurement is beginning.
    fn started(&mut self, index: usize, total: usize, selected: &Selected);

    /// The measurement of the model in flight has reached `stage`.
    fn stage(&mut self, stage: Stage);

    /// Where this renderer wants one, the callback every run reports its progress to.
    ///
    /// Asked once per sweep and registered on **every** run — warm-up, cold start and timed runs alike — which is
    /// the only way the transfer that puts a model on disk is reported at all: an install reports itself through a
    /// *run's* callback, and the one the startup path registers covers the runtime and the GPU libraries and stops
    /// there.
    ///
    /// # Note [Nothing is drawn from inside a timed section]
    ///
    /// The models invoke this per tile, from inside the section whose duration is the whole point of the binary, so
    /// what a renderer returns here may store a number and nothing more. A store is not a write, and anything that
    /// **writes to a terminal** from in here is inside the measurement.
    fn on_progress(&self) -> Option<OnInference>;

    /// A model's measurement has ended, however it ended.
    ///
    /// No position, unlike [`started`](Self::started): a renderer that wants one already has it from the `started`
    /// that opened this model, and every implementor of this trait discarded the pair.
    fn finished(&mut self, result: &ModelResult);
}

/// A listener that says nothing, for `--json`, where stdout is the document and stderr is for progress that a
/// machine did not ask for.
pub struct Silent;

impl Listener for Silent {
    fn started(&mut self, _index: usize, _total: usize, _selected: &Selected) {}
    fn stage(&mut self, _stage: Stage) {}

    /// None, so that a `--json` sweep is charged nothing at all for reporting it is not making.
    fn on_progress(&self) -> Option<OnInference> {
        None
    }

    fn finished(&mut self, _result: &ModelResult) {}
}

/// One line per model to **stderr**, so that `perftest > results.txt` still shows a sweep making progress and the
/// report alone reaches the file.
pub struct Lines;

impl Listener for Lines {
    fn started(&mut self, index: usize, total: usize, selected: &Selected) {
        eprintln!("perftest: [{}/{total}] measuring {}", index + 1, selected.operation.display_name());
    }

    /// Ignored: this renderer's whole shape is one line as a model starts and one as it ends, and a line per stage
    /// on a redirected stderr is the megabyte of output the two-line form exists to avoid.
    fn stage(&mut self, _stage: Stage) {}

    /// None. This renderer has nothing per tile to show, so a sweep with no terminal is charged nothing for it.
    fn on_progress(&self) -> Option<OnInference> {
        None
    }

    fn finished(&mut self, result: &ModelResult) {
        // Through `crate::view::figure`, as the live view is, so that all three spell one measurement one way.
        // `Duration`'s own `Debug` is the notation to stay away from — see `report::DURATION_WIDTH`.
        let ending = result
            .outcome
            .ending(|measured| format!("median {}", crate::view::figure(measured.stats.median)));

        eprintln!("perftest:       {} — {ending}", result.codename);
    }
}

/// Measures every model in `selection`, one at a time.
///
/// `named` is whether the caller named the models: a sweep that named none drops the ones the library declines and
/// counts them, and one that named them reports each refusal with its reason. That is the only thing the two differ
/// in, and it is a presentation decision rather than a measurement one — the same call is made either way.
///
/// Nothing here calls [`std::process::exit`]: a Ctrl-C breaks the loop and returns what was measured, so the summary
/// is never the thing that gets skipped.
pub async fn sweep(
    opai: &Opai,
    input: &Input,
    selection: &[Selected],
    options: &Options,
    named: bool,
    cancel: &CancellationToken,
    listener: &mut dyn Listener,
) -> SweepResult {
    let mut results = Vec::with_capacity(selection.len());
    let mut skipped = 0;

    // Once per sweep rather than once per run: what is cloned per run is an `Arc`, and asking a renderer for its
    // callback inside the loop would be the sweep consulting it about a measurement.
    let on_progress = listener.on_progress();

    for (index, selected) in selection.iter().enumerate() {
        // Between models rather than only before a cold start: an interruption during one model's runs must not be
        // followed by an attempt at the next.
        if cancel.is_cancelled() {
            break;
        }

        listener.started(index, selection.len(), selected);

        // A row whose input the auxiliary pass could not supply is failed **with that reason** and never measured:
        // not released, not warmed up and not timed, so nothing about it reaches the statistics. Reported rather
        // than dropped, so whoever selected it is told why it has no number.
        let outcome = match &selected.blocked {
            Some(reason) => Outcome::Failed { reason: reason.clone() },
            None => measure(opai, input, selected, options, cancel, listener, on_progress.as_ref()).await,
        };

        let result = ModelResult {
            codename: selected.codename,
            family: selected.family,
            display_name: selected.operation.display_name(),
            outcome,
        };

        // A refusal the caller did not ask about is dropped here and counted, which is the one place the two shapes
        // of sweep differ.
        if !named && matches!(result.outcome, Outcome::Declined { .. }) {
            skipped += 1;
            continue;
        }

        listener.finished(&result);
        results.push(result);
    }

    SweepResult { results, skipped }
}

/// Measures one model.
///
/// Never ends the process: everything that can go wrong is returned as an [`Outcome`] so the sweep carries on with
/// the models that remain. A sweep is a long job, and ending it on the first failure would throw away every
/// measurement taken before it.
async fn measure(
    opai: &Opai,
    input: &Input,
    selected: &Selected,
    options: &Options,
    cancel: &CancellationToken,
    listener: &mut dyn Listener,
    on_progress: Option<&OnInference>,
) -> Outcome {
    // The boundary between models. The previous model's `Enhanced` has already been dropped — only its dimensions
    // and its provider report were kept — so nothing is outstanding and the native sessions are freed before this
    // returns.
    opai.release_sessions();

    // Unique to this invocation and this model, so no run of this measurement can be served from the result of an
    // earlier one. The pixels are shared and identical: every run does identical work, and only the key differs.
    // Done unconditionally rather than only under `--cache`, which costs an `Arc::clone` and a `format!` per run and
    // means the cached and uncached paths measure the same thing.
    let base = format!(
        "{}-{}-{}",
        input.picture.identity(),
        selected.codename,
        std::time::SystemTime::UNIX_EPOCH.elapsed().map(|since| since.as_nanos()).unwrap_or_default()
    );

    // Folded across every run this model takes, cold start included.
    let mut built: Vec<ExecutionProvider> = Vec::new();

    // The warm-up, which is also what puts the model on disk where it is not there already — and is exactly why the
    // cold start below is measured *after* it rather than on the very first run.
    for index in 0..options.warmup {
        listener.stage(Stage::Warmup { run: index + 1, of: options.warmup });

        match run(opai, input, selected, options, cancel, on_progress, &format!("{base}-w{index}")).await {
            Ok(enhanced) => fold(&mut built, &enhanced.providers),
            Err(error) => return classify(&error, &format!("warm-up run {}", index + 1)),
        }
    }

    // What makes the cold start a cold start: the session the warm-up built is dropped, so the next call pays the
    // full construction cost.
    opai.release_sessions();

    listener.stage(Stage::Cold);

    let started = Instant::now();
    let cold = match run(opai, input, selected, options, cancel, on_progress, &format!("{base}-cold")).await {
        Ok(enhanced) => {
            let elapsed = started.elapsed();
            fold(&mut built, &enhanced.providers);
            elapsed
        }
        Err(error) => return classify(&error, "cold-start run"),
    };

    let mut runs = Vec::with_capacity(options.runs as usize);
    let mut output = (0, 0);

    for index in 0..options.runs {
        listener.stage(Stage::Timed { run: index + 1, of: options.runs });

        let started = Instant::now();
        let enhanced = match run(opai, input, selected, options, cancel, on_progress, &format!("{base}-{index}")).await
        {
            Ok(enhanced) => enhanced,
            Err(error) => return classify(&error, &format!("run {}", index + 1)),
        };
        let elapsed = started.elapsed();

        // After the clock is read, so nothing a listener does is inside the section this loop is measuring.
        listener.stage(Stage::Run { elapsed });

        fold(&mut built, &enhanced.providers);
        output = enhanced.picture.dimensions();
        runs.push(elapsed);

        // And the result goes here, before the next run and long before the next model: it holds an `Arc` over a
        // 2560x2560 image at 4x, and keeping the sweep's outputs would change what the later models are measured
        // under.
        drop(enhanced);
    }

    let stats = stats::compute(&runs);

    Outcome::Completed(Box::new(Measured {
        cold,
        runs,
        stats,
        output,
        providers: ProviderReport { requested: options.provider, actual: built }.verdict(),
    }))
}

/// One run: the whole of what a timed section contains.
///
/// `on_progress` is the renderer's own, registered on every run alike so that a model's transfer — which happens
/// inside the warm-up and is the longest unattended wait this binary has — is reported rather than silent. What it
/// may do from in here is bounded; see Note [Nothing is drawn from inside a timed section] on
/// [`Listener::on_progress`].
async fn run(
    opai: &Opai,
    input: &Input,
    selected: &Selected,
    options: &Options,
    cancel: &CancellationToken,
    on_progress: Option<&OnInference>,
    identity: &str,
) -> Result<Enhanced, InferenceError> {
    // The same pixels behind an `Arc`, under an identity of this run's own. There is nothing to restore and nothing
    // to leak, which is what the reference needs a deferred restore and a comment about early returns for.
    let picture = Picture::new(input.picture.path(), input.picture.shared_pixels(), identity);

    let process = ProcessOptions {
        provider: options.provider,
        on_progress: on_progress.cloned(),
        cancel: cancel.clone(),
        // Off by default, inverting the library's: with the cache on, `process` encodes the result and writes it
        // inside the call being timed, which for a 4x Kyoto is a 2560x2560 PNG per run.
        cache: options.cache,
        ..Default::default()
    };

    opai.process(&picture, std::slice::from_ref(&selected.operation), Some(process)).await
}

/// Which of the four outcomes an error is.
///
/// The decline is the library's own [`InferenceError::Unsupported`] and nothing this crate decides. The
/// interruption is [`InferenceError::Cancelled`], which is what the token the sweep holds produces — reported as an
/// interruption rather than a failure, because sending somebody to debug a model that is working is the one thing
/// this distinction exists to prevent.
fn classify(error: &InferenceError, stage: &str) -> Outcome {
    match error {
        InferenceError::Unsupported { .. } => Outcome::Declined { reason: error.to_string() },
        InferenceError::Cancelled => Outcome::Interrupted,
        other => Outcome::Failed { reason: format!("{stage}: {other}") },
    }
}

/// Adds one run's report to the set of providers this model's sessions were built on.
fn fold(built: &mut Vec<ExecutionProvider>, report: &ProviderReport) {
    for provider in &report.actual {
        if !built.contains(provider) {
            built.push(*provider);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opai::Family;
    use opai::UnsupportedReason;

    /// A report as `process` would return one.
    fn report(requested: ExecutionProvider, actual: &[ExecutionProvider]) -> ProviderReport {
        ProviderReport { requested, actual: actual.to_vec() }
    }

    /// A result carrying one outcome, which is all the exit-status and counting rules read.
    fn result(codename: &'static str, outcome: Outcome) -> ModelResult {
        ModelResult { codename, family: Family::Upscale, display_name: codename.to_string(), outcome }
    }

    fn completed() -> Outcome {
        Outcome::Completed(Box::new(Measured {
            cold: Duration::from_secs(2),
            runs: vec![Duration::from_millis(400)],
            stats: stats::compute(&[Duration::from_millis(400)]),
            output: (2560, 2560),
            providers: Verdict::AsRequested(ExecutionProvider::Cpu),
        }))
    }

    #[test]
    fn a_refusal_is_told_from_a_failure_by_the_librarys_own_error_and_not_by_anything_here() {
        let refusal = InferenceError::Unsupported {
            operation: "Osaka 4x (FP16)".to_string(),
            reason: UnsupportedReason::IncompleteGraphSet { missing: "decoder" },
        };

        let outcome = classify(&refusal, "cold-start run");

        // The library's reason, carried through — which is the thing that makes naming a model worth doing.
        assert_eq!(
            outcome,
            Outcome::Declined {
                reason: "no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph".to_string()
            }
        );
    }

    #[test]
    fn a_cancellation_is_an_interruption_rather_than_a_failure() {
        assert_eq!(classify(&InferenceError::Cancelled, "run 3"), Outcome::Interrupted);
    }

    #[test]
    fn anything_else_is_a_failure_naming_the_stage_that_hit_it() {
        let untileable = InferenceError::Untileable { width: 3, height: 3 };

        let Outcome::Failed { reason } = classify(&untileable, "cold-start run") else {
            panic!("an untileable image is a failure");
        };

        assert!(reason.starts_with("cold-start run: "), "{reason}");
        assert!(reason.contains("a 3x3 image has no pixels to run"), "{reason}");
    }

    #[test]
    fn a_models_provider_report_is_folded_across_every_run_it_took() {
        let mut built = Vec::new();

        // Five runs on CoreML plus one that fell back: a set, not a list, so CoreML is named once — and the CPU is
        // named even though only one run hit it, which is the whole reason this is a union.
        for _ in 0..5 {
            fold(&mut built, &report(ExecutionProvider::CoreMl, &[ExecutionProvider::CoreMl]));
        }
        fold(&mut built, &report(ExecutionProvider::CoreMl, &[ExecutionProvider::Cpu]));

        assert_eq!(built, vec![ExecutionProvider::CoreMl, ExecutionProvider::Cpu]);
    }

    #[test]
    fn a_sweep_in_which_everything_completed_exits_reporting_success() {
        let sweep = SweepResult { results: vec![result("kyoto", completed())], skipped: 0 };

        assert_eq!(sweep.failures(), 0);
        assert!(!sweep.interrupted());
        assert_eq!(sweep.exit_code(), 0);
    }

    #[test]
    fn declines_alone_exit_reporting_success() {
        // Nothing about the sweep went wrong. A status that said otherwise could never be used as a check.
        let sweep = SweepResult {
            results: vec![result("osaka", Outcome::Declined { reason: "no pipeline".to_string() })],
            skipped: 0,
        };

        assert_eq!(sweep.failures(), 0);
        assert_eq!(sweep.exit_code(), 0);

        // And so does a sweep whose declines were dropped because the caller named nothing.
        assert_eq!(SweepResult { results: Vec::new(), skipped: 20 }.exit_code(), 0);
    }

    #[test]
    fn a_failure_exits_reporting_failure_and_is_counted() {
        let sweep = SweepResult {
            results: vec![
                result("kyoto", completed()),
                result("osaka", Outcome::Failed { reason: "cold-start run: out of memory".to_string() }),
            ],
            skipped: 0,
        };

        assert_eq!(sweep.failures(), 1);
        assert_eq!(sweep.exit_code(), 1);
    }

    #[test]
    fn an_interruption_exits_reporting_failure_without_being_counted_as_one() {
        let sweep = SweepResult {
            results: vec![result("kyoto", completed()), result("tokyo", Outcome::Interrupted)],
            skipped: 0,
        };

        // Not a model failure — reporting it as one would send somebody to debug a model that is fine — but not a
        // clean run either, so the status says so.
        assert_eq!(sweep.failures(), 0);
        assert!(sweep.interrupted());
        assert_eq!(sweep.exit_code(), 1);
    }

    /// A listener that records everything it is told, in the order it was told.
    #[derive(Default)]
    struct Recording {
        events: Vec<String>,
    }

    impl Listener for Recording {
        fn started(&mut self, index: usize, total: usize, selected: &Selected) {
            self.events.push(format!("started {}/{total} {}", index + 1, selected.codename));
        }

        fn stage(&mut self, stage: Stage) {
            self.events.push(format!("{stage:?}"));
        }

        fn on_progress(&self) -> Option<OnInference> {
            None
        }

        fn finished(&mut self, result: &ModelResult) {
            self.events.push(format!("finished {}", result.codename));
        }
    }

    #[test]
    fn the_four_stages_are_reported_in_loop_order_with_their_counters() {
        // Over the source, because the loop cannot be run without the runtime, a model file and a machine to execute
        // on — and what is being asserted is the *order of the call sites*, which is a property of the loop rather
        // than of any run of it. The same device the exit assertion below uses, and for the same reason.
        let source = include_str!("sweep.rs");
        let sweep = source.split("#[cfg(test)]").next().expect("the file has a first part");

        let calls: Vec<&str> =
            sweep.lines().map(str::trim).filter(|line| line.starts_with("listener.stage(")).collect();

        assert_eq!(
            calls,
            vec![
                "listener.stage(Stage::Warmup { run: index + 1, of: options.warmup });",
                "listener.stage(Stage::Cold);",
                "listener.stage(Stage::Timed { run: index + 1, of: options.runs });",
                "listener.stage(Stage::Run { elapsed });",
            ],
            "the stages are reported somewhere other than the four points the loop already had"
        );

        // And the finished run is reported *after* the clock is read, so nothing a listener does can land inside the
        // section being measured.
        let numbered: Vec<(usize, &str)> = sweep.lines().map(str::trim).enumerate().collect();
        let read = numbered.iter().rposition(|(_, line)| *line == "let elapsed = started.elapsed();");
        let reported = numbered.iter().rposition(|(_, line)| line.starts_with("listener.stage(Stage::Run"));

        assert!(read < reported, "the finished run is reported before its own clock is read");
    }

    #[test]
    fn the_two_renderers_that_predate_the_view_are_unmoved_by_a_stage() {
        // `Silent` says nothing whatever it is told, and `Lines` keeps its shape of one line as a model starts and
        // one as it ends: a line per stage on a redirected stderr is what that shape exists to avoid.
        Silent.stage(Stage::Cold);
        Lines.stage(Stage::Timed { run: 3, of: 5 });
        Lines.stage(Stage::Run { elapsed: Duration::from_millis(412) });
    }

    #[test]
    fn a_listener_is_told_the_stages_between_a_models_start_and_its_end() {
        let mut recording = Recording::default();

        recording.stage(Stage::Warmup { run: 1, of: 1 });
        recording.stage(Stage::Cold);
        recording.stage(Stage::Timed { run: 1, of: 2 });
        recording.stage(Stage::Run { elapsed: Duration::from_millis(400) });
        recording.finished(&result("kyoto", completed()));

        assert_eq!(
            recording.events,
            vec![
                "Warmup { run: 1, of: 1 }".to_string(),
                "Cold".to_string(),
                "Timed { run: 1, of: 2 }".to_string(),
                "Run { elapsed: 400ms }".to_string(),
                "finished kyoto".to_string(),
            ]
        );
    }

    #[test]
    fn a_pass_that_supplied_nothing_leaves_the_rows_it_blocked_unusable_and_says_why() {
        // Two answers that are not a set of faces and one reason each, phrased for the row's own report — so a
        // failed row says what went wrong rather than that something did. A detection that succeeded and found
        // nobody is the same thing to a model as one that failed: this row has nothing to measure.
        let empty = Detected::Empty.usable().expect_err("an empty pass supplied faces");
        assert!(empty.contains("no face was detected"), "{empty}");

        let failed = Detected::Failed { reason: "no session could be built".to_string() }
            .usable()
            .expect_err("a failed pass supplied faces");
        assert!(failed.contains("could not be detected"), "{failed}");
        assert!(failed.contains("no session could be built"), "the row lost the library's own reason: {failed}");

        assert_eq!(Detected::Empty.count(), 0);
        assert_eq!(Detected::Failed { reason: String::new() }.count(), 0);
    }

    #[test]
    fn a_blocked_row_is_reported_as_failed_and_never_measured() {
        // Failed rather than declined, because nothing about the library refused it, and failed rather than dropped,
        // because whoever selected it is owed the reason. It contributes to the failure count and so to the exit
        // status, which is correct: the row it names has no number.
        let blocked = SweepResult {
            results: vec![
                result("kyoto", completed()),
                result("athens", Outcome::Failed { reason: "no face was detected in this image".to_string() }),
            ],
            skipped: 0,
        };

        assert_eq!(blocked.failures(), 1);
        assert_eq!(blocked.exit_code(), 1);
        assert!(!blocked.interrupted(), "a row with no faces to restore is not an interruption");

        // And it is never measured: the loop answers a blocked row before `measure`, so it pays no release, no
        // warm-up and no timed run, and nothing about it reaches the statistics. Over the source, because what is
        // being asserted is a call that does not happen.
        let sweep = include_str!("sweep.rs");
        let body = sweep.split("#[cfg(test)]").next().expect("the file has a first part");
        let loop_body = body.split("for (index, selected) in selection.iter().enumerate()").nth(1).expect("the loop");

        let short_circuit = loop_body.find("Some(reason) => Outcome::Failed").expect("the loop answers a blocked row");
        let measured = loop_body.find("None => measure(").expect("the loop measures an unblocked row");

        assert!(short_circuit < measured, "a blocked row reaches the measurement before it is answered");
    }

    #[test]
    fn the_auxiliary_run_is_made_once_and_outside_every_measurement() {
        // The two properties the requirement asks for, and neither is visible in a value: the pass has to happen
        // **before** the first model starts and **not once per model**, so what is asserted is that this file's
        // measurement loop does not contain it and that its one caller makes it before the sweep. Asserted over the
        // source, because a call that is never made is otherwise only visible by its absence.
        let sweep = include_str!("sweep.rs");
        let body = sweep.split("#[cfg(test)]").next().expect("the file has a first part");

        // The measurement loop and everything under it: `measure`, `run`, and the release points around them.
        let measured = body.split("/// Measures one model.").nth(1).expect("the file measures a model");
        assert!(
            !measured.contains("detect("),
            "the auxiliary detection run is made from inside a model's measurement"
        );

        // And in its one caller it happens before the sweep is even assembled, so nothing it costs is inside a
        // `started`/`finished` bracket.
        let main = include_str!("main.rs");
        let run = main.split("async fn run(options: &Options)").nth(1).expect("main runs a sweep");

        let detected = run.find("sweep::detect(").expect("the sweep makes the auxiliary run");
        let swept = run.find("let sweep = Sweep {").expect("main assembles the sweep");
        assert!(detected < swept, "the auxiliary run is made after the sweep is assembled");
        assert_eq!(run.matches("sweep::detect(").count(), 1, "the auxiliary run is made more than once");

        // Behind the condition that any row needs it, so a sweep of upscalers does not make it at all.
        let guard = run.find("select::needs_faces(").expect("the pass is guarded by whether a row needs it");
        assert!(guard < detected, "the auxiliary run is made before it is known whether any row needs one");
    }

    #[test]
    fn nothing_in_the_sweep_ends_the_process_itself() {
        // The summary for the models that finished is printed by the caller, so an exit from in here is the one
        // thing that could skip it. Asserted over the source, because a call that is never made is otherwise only
        // visible by its absence.
        let source = include_str!("sweep.rs");
        // Everything above this module, so the assertion does not find its own needle.
        let sweep = source.split("#[cfg(test)]").next().expect("the file has a first part");

        let needle = concat!("process", "::exit");
        let calls: Vec<&str> = sweep
            .lines()
            .filter(|line| line.contains(needle) && !line.trim_start().starts_with("//"))
            .collect();

        assert!(calls.is_empty(), "the sweep ends the process: {calls:?}");
    }
}
