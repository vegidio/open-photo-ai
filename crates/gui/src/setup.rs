//! What the application does between launch and being ready to use.
//!
//! Three things live here: the state that makes an initialization happen at most once and keeps its handle, the wire
//! types that carry `opai`'s plan and progress reports to the window, and the two commands the setup dialog calls.

// None of `opai`'s reporting types derive `serde`, and `opai::Dependency` and `opai::Phase` are both
// `#[non_exhaustive]`, so the boundary needs a shape this crate controls. That shape is deliberately thin: a
// dependency is identified by the name the library gives it and nothing here matches on the enum, for the reason
// `name_of` gives.

use std::future::Future;
use std::sync::Arc;

use opai::{Dependency, InitError, InitOptions, Opai, Phase, PlannedDependency, Progress, SupportedProviders};
use serde::Serialize;
use tauri::async_runtime::Mutex;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::command::{Answer, CommandError, Ended, Traceparent, command_span, traced, traced_sync};

/// One initialization at a time, one initialization per process, and the handle it produced.
///
/// One field answers all three of the questions this application has about initialization:
///
/// - the **lock**: a second request waits rather than starting a second install.
/// - the **memo**, because a stored value is a success to return rather than an install to repeat — and because it
///   stores *only* success, a failed attempt stays retryable.
/// - the **handle**, which is what the enhancement and detection commands reach the ready application through.
#[derive(Debug)]
pub(crate) struct Setup<T> {
    // The requests genuinely overlap: React's StrictMode double-mounts the start-up effect in development, a reloaded
    // webview asks again, and the failure dialog's retry is a third call site.
    //
    // `tauri::async_runtime::Mutex` rather than `std::sync::Mutex`: the guard is held across an `.await`, which a
    // `std` guard cannot be, not being `Send`. It is tokio's, re-exported by Tauri, so this costs no dependency.
    //
    // Generic over what it holds so the rule above can be driven by a test with a fake initializer — no network, no
    // runtime and no configuration directory. That is the same seam `opai`'s own `pipeline::Backend` is,
    // one layer down, and for the same reason: this is the one piece of this module that is a rule rather than a
    // wiring.
    ready: Mutex<Option<T>>,
}

// Written out rather than derived, because `#[derive(Default)]` would demand `T: Default` — which `Opai`, being a
// handle to something that had to be installed, is not.
impl<T> Default for Setup<T> {
    fn default() -> Self {
        Self { ready: Mutex::new(None) }
    }
}

impl<T> Setup<T> {
    /// Runs `initialize` unless this process already has a value, and answers with `read` of the one it keeps.
    ///
    /// `read` runs on the stored value on **every** call, the memoized one included, which is what makes a repeat
    /// request answer with the same report having installed nothing.
    ///
    /// # Errors
    ///
    /// Whatever `initialize` failed with, unchanged and unremembered.
    pub(crate) async fn ready<F, Fut, E, R>(&self, initialize: F, read: impl FnOnce(&T) -> R) -> Result<R, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        // Projects rather than hands over, which is the whole shape of this signature: the caller says what to read
        // off the stored value and gets that, so no handle leaves. Handing a clone back to a command that would drop
        // it would be a second route to a value this type is the keeper of.
        //
        // The lock is taken for the whole of the work rather than only around the check: a second request that
        // arrived during the first would otherwise pass the check and start a second install of the same archive.
        let mut ready = self.ready.lock().await;

        if let Some(value) = ready.as_ref() {
            return Ok(read(value));
        }

        // `?` before the store, which is what leaves a failure retryable: only a success is written.
        Ok(read(ready.insert(initialize().await?)))
    }

    /// `read` of the stored value, or `None` where this process has not initialized one yet.
    ///
    /// [`ready`](Self::ready)'s counterpart for a caller that **needs** the application rather than one that is
    /// asking for it to exist. The lock is released the moment `read` returns.
    pub(crate) async fn peek<R>(&self, read: impl FnOnce(&T) -> R) -> Option<R> {
        // The enhancement commands run on a window that has already been through the setup dialog, and one that
        // started an initialization of its own would turn an enhancement asked for too early into a multi-gigabyte
        // download with no dialog in front of it.
        //
        // Projects rather than hands over, exactly as `ready` does and for the same reason. What a caller reads off
        // is a clone of the handle, which `Opai` documents as a refcount bump — and the lock is released the moment
        // that clone is taken, so a run lasting minutes does not hold the mutex a retry of the setup dialog would
        // need.
        self.ready.lock().await.as_ref().map(read)
    }
}

/// What the window is told while the application starts itself.
///
/// One channel carries both, so the plan and the progress reports arrive in the order they were produced.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum SetupEvent {
    // One channel rather than two global events under two names: a progress report drawn before the plan that
    // dimensions it is a row with no size and no denominator.
    /// Every component this machine needs, in the order they will be handled. Sent once, before anything moves.
    Plan { rows: Vec<PlanRow> },
    /// One component's position, as [`opai`] reported it.
    Progress(ProgressRow),
}

/// One row of the plan: a component this machine needs, whether or not it turns out to have work to do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanRow {
    /// The component's name, which is both the row's key and the label the dialog shows.
    name: String,
    /// The total published size, in bytes, of the files backing it.
    size: u64,
}

impl From<&PlannedDependency> for PlanRow {
    fn from(planned: &PlannedDependency) -> Self {
        Self { name: name_of(&planned.dependency), size: planned.size }
    }
}

/// One report about one component.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProgressRow {
    /// Which component this is about, matching the [`PlanRow::name`] of its row.
    name: String,
    /// What that component is doing.
    state: RowState,
    /// How far through the whole install of this component the report is, in `0.0..=1.0`.
    fraction: f64,
}

impl From<&Progress> for ProgressRow {
    fn from(progress: &Progress) -> Self {
        Self {
            name: name_of(&progress.dependency),
            state: RowState::of(progress.phase),
            fraction: progress.fraction,
        }
    }
}

/// What a row of the dialog says about its component.
///
/// The fourth state the dialog draws — queued — is not here, because it is not something a component *reports*: it is
/// the state of a row the plan named and no report has reached yet, which the window derives from its own rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RowState {
    /// It was already on disk, current and complete. Nothing was transferred and nothing expanded.
    Installed,
    /// Bytes are arriving over the network.
    Downloading,
    // `opai` gives that minute the last fifth of the bar precisely so it reads as work; a row still labelled
    // "Downloading" through it would take half of that back.
    /// The archive is being expanded, which for a couple of gigabytes of LZMA2 is a minute of work with nothing
    /// arriving.
    Extracting,
}

impl RowState {
    /// The state for `phase`, and [`RowState::Downloading`] — the dialog's plain "this is working" label — for a phase
    /// this crate does not know.
    fn of(phase: Phase) -> Self {
        match phase {
            Phase::AlreadyInstalled => RowState::Installed,
            Phase::Downloading => RowState::Downloading,
            Phase::Extracting => RowState::Extracting,
            // Mandatory — `Phase` is `#[non_exhaustive]` and gains a variant whenever a new kind of install work does.
            // `Downloading` is the safe direction: a new phase is a new kind of *work*, so treating it as work shows a
            // row in progress rather than claiming something finished that did not.
            _ => RowState::Downloading,
        }
    }
}

/// The name a dependency is known by on the wire, which is the name `opai` gives it.
///
/// [`Dependency::as_str`] is unique per variant and, for a model, is the artifact id. It is **not** translated.
fn name_of(dependency: &Dependency) -> String {
    // Nothing here matches on `Dependency`, and that is the point: the enum is `#[non_exhaustive]` and gains a variant
    // every time a dependency family does, so a translation table in this crate would compile unchanged and quietly
    // file the new dependency under a `_` arm.
    //
    // "ONNX Runtime", "NVIDIA CUDA", "NVIDIA cuDNN" and "NVIDIA TensorRT" are product names, the Wails GUI hardcoded
    // exactly these four strings at its four emit sites, and the design draws them in the monospace face because they
    // are identifiers — so none of them is translated.
    dependency.as_str().to_string()
}

/// Which of the failure dialog's three shapes a failed initialization gets.
///
/// What each one decides is whether the known cause is named and whether Try again is offered:
///
/// | Kind                        | throttling note | Try again |
/// |-----------------------------|-----------------|-----------|
/// | [`Failure::Transfer`]       | shown           | offered   |
/// | [`Failure::Unrecoverable`]  | hidden          | withheld  |
/// | [`Failure::Other`]          | hidden          | offered   |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Failure {
    // Three kinds because the dialog has exactly three renderings, and this crate does not serialize an enum with no
    // reader.
    //
    // The match lives here rather than in `opai` because what it produces is a decision about a dialog: which
    // grouping gets a paragraph of advice, and which gets a button. `InitError`'s own doc says its named variants
    // exist because "a front end reports each differently", so reading that taxonomy is not duplicating it. A `cli`
    // reporting the same failure prints the message and exits, with no button to withhold.
    /// Getting the files did not work: a transfer that did not complete, or files that arrived and did not match
    /// what was published.
    Transfer,
    /// A second attempt produces the same answer.
    Unrecoverable,
    /// Everything else, and every variant added after this was written.
    Other,
}

impl Failure {
    /// How `error` is drawn. A variant this crate does not name is [`Failure::Other`].
    fn of(error: &InitError) -> Self {
        match error {
            // `HashMismatch` is a *transfer* failure rather than a verification one: an archive that arrived and hashed
            // wrong is most often a truncated response from the host that throttles them, which is the same story and
            // the same advice.
            InitError::Download { .. } | InitError::HashMismatch { .. } => Failure::Transfer,

            // `RuntimeUnavailable` because its doc says so in as many words: it "failed to load earlier in this process
            // and cannot be retried".
            InitError::InvalidName { .. }
            | InitError::InvalidApp { .. }
            | InitError::UnsupportedPlatform { .. }
            | InitError::UnknownProvider { .. }
            | InitError::RuntimeUnavailable { .. } => Failure::Unrecoverable,

            // Mandatory — `InitError` is `#[non_exhaustive]` — and `Other` is the safe direction on both readers: an
            // unknown failure gets no advice that might be false, and gets the button, because offering a retry that
            // fails again costs a second and refusing one that would have worked costs the launch.
            //
            // `AlreadyRunning` lands here deliberately — the library's own message for it ends "close it and try
            // again", so withholding the button would contradict the sentence beside it.
            _ => Failure::Other,
        }
    }
}

/// Why an initialization could not finish, as the window is told.
///
/// The wire shape is `{"kind":"initialize","failure":"transfer","message":"..."}`.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum SetupError {
    // `opai::InitError` is neither `Serialize` nor `Clone`, so something has to cross the boundary in its place: the
    // rendered message, which is the whole of the reason the dialog shows, and the classification, which is what
    // decides the shape it shows it in.
    /// The library refused or could not finish the initialization. `message` is [`InitError`]'s own sentence,
    /// which already names what it was working on.
    #[error("{message}")]
    Initialize {
        // Named `failure` rather than `kind` because `kind` is already the `#[serde(tag)]` discriminating this enum.
        /// Which of the dialog's three shapes this failure gets.
        failure: Failure,
        /// The library's own sentence, untranslated, shown in full so it can be copied into a bug report.
        message: String,
    },
}

impl CommandError for SetupError {
    fn ended(&self) -> Ended<'_> {
        match self {
            // `opai` records a failed initialization itself, with how long it ran.
            Self::Initialize { .. } => Ended::Failed { error: self, recorded: true },
        }
    }
}

impl Answer for SupportedProviders {}

// The command's name is written once on the TypeScript side too, in `frontend/ipc/setup.ts`; nothing in either
// toolchain notices when one of the two is renamed alone.
/// Start the application: install what this machine needs, load the runtime and keep the handle.
///
/// This is the GUI's whole application startup, and it is asked for without the user asking: the window calls it on
/// mount, because nothing in the application can be used before it has finished.
///
/// `channel` is created by the caller and its handler attached before the invoke, so no report can be emitted before
/// there is somewhere to put it. A send that fails is logged and discarded — the install must not fail because a
/// window went away.
///
/// Answers with [`opai::SupportedProviders`]: what this machine turned out to be able to be asked to run on, which
/// the settings pane's processor list is built from.
///
/// A repeat call is answered from [`Setup`]'s memo with the same report, nothing reinstalled.
///
/// # Errors
///
/// [`SetupError`], carrying [`opai::InitError`]'s message and its [`Failure`] classification. A failure is not
/// remembered, so a later call — the failure dialog's Try again — tries again.
#[tauri::command]
pub(crate) async fn initialize(
    channel: Channel<SetupEvent>,
    setup: State<'_, Setup<Opai>>,
    traceparent: Traceparent,
) -> Result<SupportedProviders, SetupError> {
    // The providers ride on this answer rather than arriving through a `providers` command of their own because this
    // call is the only moment they are decided — a separate command asked before setup finished would have to either
    // block on the same mutex, report an empty machine or raise an error the window has to tell apart from a real
    // one, and none of those three states exists here.
    traced(command_span!("initialize", traceparent), async move {
        setup
            .ready(
                || async {
                    let plan = channel.clone();
                    let on_plan: opai::OnPlan = Arc::new(move |rows: &[PlannedDependency]| {
                        send(&plan, SetupEvent::Plan { rows: rows.iter().map(PlanRow::from).collect() });
                    });

                    let progress = channel.clone();
                    let on_progress: opai::OnProgress = Arc::new(move |report: &Progress| {
                        send(&progress, SetupEvent::Progress(ProgressRow::from(report)));
                    });

                    // `..Default::default()` rather than every field named, which is the idiom `InitOptions` documents: a
                    // field added later is source-compatible here.
                    let options = InitOptions {
                        app: Some(opai::GUI.to_string()),
                        on_plan: Some(on_plan),
                        on_progress: Some(on_progress),
                        ..Default::default()
                    };

                    Opai::initialize(opai::APP_NAME, Some(options)).await
                },
                Opai::providers,
            )
            .await
            // Not logged here: `opai` records the failure itself, once, with how long initialization ran.
            .map_err(|error| SetupError::Initialize { failure: Failure::of(&error), message: error.to_string() })
    })
    .await
}

/// Sends one event, logging and discarding a failure.
fn send(channel: &Channel<SetupEvent>, event: SetupEvent) {
    // A send fails when the channel's window has gone — a reload, a close — and an install in flight must not be
    // brought down by that. There is nobody left to propagate to either: this runs inside a callback the library
    // calls for effect.
    if let Err(error) = channel.send(event) {
        tracing::warn!(%error, "could not report setup progress to the window");
    }
}

/// End the application.
#[tauri::command]
pub(crate) fn quit(app: AppHandle, traceparent: Traceparent) {
    // The way out of a first launch that cannot finish: the setup dialog is modal, the shell behind it does nothing
    // yet, and the design puts Quit in the dialog's footer.
    //
    // A command of this application's own rather than `tauri-plugin-process`, which would bring a dependency, a
    // permission set and a second way to end the process for a call this crate makes in one line — the same judgement
    // `capabilities/default.json` already records about not granting `os:default`. Commands this application
    // registers itself need no capability entry at all.
    //
    // Exit code 0, because quitting from the setup dialog is a decision rather than a crash — the same code the
    // single-instance plugin's refused second launch uses. `AppHandle::exit` runs the exit path, so the managed
    // `Setup` is dropped with everything else; the claim it holds would be released by the process ending in any
    // case.
    traced_sync(command_span!("quit", traceparent), || app.exit(0));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use opai::{Detection, DetectionVariant, FloatPrecision};
    use tauri::async_runtime::{block_on, spawn, spawn_blocking};

    use super::*;

    /// A fake initializer: counts its runs and holds the lock long enough for a second request to reach it.
    ///
    /// The sleep is on a blocking thread rather than an async timer so this needs no timer crate, and it is what makes
    /// the concurrency test mean something: an implementation that checked the option and *then* took the lock would
    /// let every request through this window.
    async fn slow_success(runs: &AtomicUsize) -> Result<u32, String> {
        runs.fetch_add(1, Ordering::SeqCst);
        spawn_blocking(|| std::thread::sleep(Duration::from_millis(25)))
            .await
            .expect("the sleep should run");
        Ok(7)
    }

    #[test]
    fn concurrent_requests_produce_exactly_one_initialization() {
        let setup: Arc<Setup<u32>> = Arc::new(Setup::default());
        let runs = Arc::new(AtomicUsize::new(0));

        block_on(async {
            let requests: Vec<_> = (0..8)
                .map(|_| {
                    let setup = Arc::clone(&setup);
                    let runs = Arc::clone(&runs);
                    spawn(async move { setup.ready(|| slow_success(&runs), |stored| *stored).await })
                })
                .collect();

            for request in requests {
                request.await.expect("the request should not panic").expect("the fake initializer succeeds");
            }
        });

        assert_eq!(runs.load(Ordering::SeqCst), 1, "a second request started its own initialization");
    }

    #[test]
    fn a_request_after_success_does_no_work() {
        let setup: Setup<u32> = Setup::default();
        let runs = AtomicUsize::new(0);

        block_on(async {
            let first =
                setup.ready(|| slow_success(&runs), |stored| *stored).await.expect("the first request succeeds");
            let second = setup
                .ready(|| slow_success(&runs), |stored| *stored)
                .await
                .expect("the second request answers from the first");

            assert_eq!(first, second, "the memoized request read something other than the stored value");
        });

        assert_eq!(runs.load(Ordering::SeqCst), 1, "the stored success was not answered from");
    }

    #[test]
    fn a_request_after_a_failure_runs_again() {
        let setup: Setup<u32> = Setup::default();
        let runs = AtomicUsize::new(0);

        block_on(async {
            let failed = setup
                .ready(
                    || async {
                        runs.fetch_add(1, Ordering::SeqCst);
                        Err::<u32, String>("the network went away".to_string())
                    },
                    |stored| *stored,
                )
                .await;
            assert_eq!(failed, Err("the network went away".to_string()), "the error is returned unchanged");

            setup.ready(|| slow_success(&runs), |stored| *stored).await.expect("the retry succeeds");

            // And the retry's success is remembered, so the retryability does not cost the run-once rule.
            setup
                .ready(|| slow_success(&runs), |stored| *stored)
                .await
                .expect("the third request answers from the retry");
        });

        assert_eq!(runs.load(Ordering::SeqCst), 2, "only success is remembered");
    }

    /// What a second request is answered with, which is what `initialize` promises.
    ///
    /// Driven through [`Setup`] with a fake handle rather than through the command, because the command's projection
    /// is `Opai::providers` and an [`Opai`] is what an install *produces* — there is no way to one in a test without
    /// a network, a runtime and a configuration directory. What the promise actually rests on is here: a repeat
    /// request reads off the value already stored and starts nothing, so the report it answers with is the same
    /// report because it is read off the same handle.
    ///
    /// The second initializer would produce a *different* machine, which is what makes a stale answer visible: if the
    /// memo were bypassed, the two reports would disagree rather than merely being counted twice.
    #[test]
    fn a_repeat_request_answers_with_the_same_report_and_installs_nothing() {
        /// A stand-in for [`Opai`]: a handle with something to read off it, as the real one has `providers()`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Machine {
            cpu: bool,
            coreml: bool,
        }

        let setup: Setup<Machine> = Setup::default();
        let runs = AtomicUsize::new(0);

        block_on(async {
            let first = setup
                .ready(
                    || async {
                        runs.fetch_add(1, Ordering::SeqCst);
                        Ok::<Machine, String>(Machine { cpu: true, coreml: true })
                    },
                    |machine| *machine,
                )
                .await
                .expect("the first request succeeds");

            let second = setup
                .ready(
                    || async {
                        runs.fetch_add(1, Ordering::SeqCst);
                        Ok::<Machine, String>(Machine { cpu: true, coreml: false })
                    },
                    |machine| *machine,
                )
                .await
                .expect("the second request answers from the first");

            assert_eq!(first, Machine { cpu: true, coreml: true });
            assert_eq!(second, first, "the second request was answered from a second initialization");
        });

        assert_eq!(runs.load(Ordering::SeqCst), 1, "asking again re-ran the install");
    }

    /// Every [`Phase`] the library publishes, mapped.
    ///
    /// The `_` arm cannot be reached from here — that is what `#[non_exhaustive]` means from outside the crate — so
    /// what this pins is the three that exist and the direction the fourth would go: [`RowState`] has no state that
    /// claims something finished, so an unmapped phase can only land on work.
    #[test]
    fn every_phase_maps_to_what_the_dialog_draws() {
        assert_eq!(RowState::of(Phase::AlreadyInstalled), RowState::Installed);
        assert_eq!(RowState::of(Phase::Downloading), RowState::Downloading);
        assert_eq!(RowState::of(Phase::Extracting), RowState::Extracting);
    }

    #[test]
    fn a_plan_row_carries_the_librarys_own_name_and_size() {
        let planned = PlannedDependency { dependency: Dependency::Cudnn, size: 612_000_000 };
        let row = PlanRow::from(&planned);

        assert_eq!(row.name, "NVIDIA cuDNN");
        assert_eq!(row.size, 612_000_000);
    }

    #[test]
    fn a_progress_row_names_the_same_dependency_its_plan_row_did() {
        // A model, because its name is composed at run time rather than being one of the four product names — the
        // case a translation table in this crate would have had nothing to say about. Taken off a real operation
        // rather than composed here, because composing an artifact id is the library's business.
        let artifact = Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp32)).artifact();
        let dependency = Dependency::Model(artifact);
        let planned = PlannedDependency { dependency: dependency.clone(), size: 100 };
        let report = Progress { dependency, phase: Phase::Extracting, bytes: 40, total: Some(100), fraction: 0.9 };

        let row = ProgressRow::from(&report);
        assert_eq!(row.name, PlanRow::from(&planned).name, "the row key and the plan row disagree");
        assert_eq!(row.name, "dt_newyork_fp32");
        assert_eq!(row.state, RowState::Extracting);
        assert!((row.fraction - 0.9).abs() < f64::EPSILON);
    }

    /// What actually goes on the wire, which is the half of the contract `frontend/ipc/setup.ts` is written against.
    #[test]
    fn the_wire_shape_is_tagged_by_kind() {
        let plan = SetupEvent::Plan {
            rows: vec![PlanRow::from(&PlannedDependency { dependency: Dependency::Runtime, size: 184 })],
        };
        let progress = SetupEvent::Progress(ProgressRow::from(&Progress {
            dependency: Dependency::Runtime,
            phase: Phase::AlreadyInstalled,
            bytes: 0,
            total: Some(184),
            fraction: 1.0,
        }));

        assert_eq!(
            serde_json::to_string(&plan).expect("the plan event should serialize"),
            r#"{"kind":"plan","rows":[{"name":"ONNX Runtime","size":184}]}"#
        );
        assert_eq!(
            serde_json::to_string(&progress).expect("the progress event should serialize"),
            r#"{"kind":"progress","name":"ONNX Runtime","state":"installed","fraction":1.0}"#
        );
    }

    /// One [`InitError`] per arm, which is all `#[non_exhaustive]` permits from outside the crate — an exhaustive
    /// match here would not compile, and that is the coupling the `_` arm exists to refuse.
    #[test]
    fn each_failure_kind_has_an_init_error_that_reaches_it() {
        let transfer = InitError::HashMismatch {
            url: "https://example.invalid/runtime.7z".to_string(),
            expected: "abc".to_string(),
            actual: "def".to_string(),
        };
        assert_eq!(Failure::of(&transfer), Failure::Transfer);

        let unrecoverable = InitError::UnsupportedPlatform { dependency: "ONNX Runtime", os: "solaris", arch: "sparc" };
        assert_eq!(Failure::of(&unrecoverable), Failure::Unrecoverable);

        // `AlreadyRunning` is one of the variants the `_` arm answers, and the one that pins its direction: the
        // library's own message ends "close it and try again", so the arm has to keep the button — and it says
        // nothing about downloads, so the arm has to withhold the throttling advice. `Other` is both.
        let other = InitError::AlreadyRunning { holder: None };
        assert_eq!(Failure::of(&other), Failure::Other);
    }

    #[test]
    fn the_error_crossing_ipc_carries_the_librarys_own_sentence_and_its_kind() {
        let error = SetupError::Initialize {
            failure: Failure::Transfer,
            message: "downloading https://example.invalid/runtime.7z failed: connection reset".to_string(),
        };

        // `Display` is the library's sentence alone — the classification is for the dialog, not for the log line.
        assert_eq!(error.to_string(), "downloading https://example.invalid/runtime.7z failed: connection reset");
        assert_eq!(
            serde_json::to_string(&error).expect("the error should serialize"),
            r#"{"kind":"initialize","failure":"transfer","message":"downloading https://example.invalid/runtime.7z failed: connection reset"}"#
        );
    }

    #[test]
    fn a_failed_initialization_is_the_one_opai_recorded() {
        let failed = SetupError::Initialize { failure: Failure::Other, message: "a".to_string() };

        assert_eq!(CommandError::ended(&failed).cell(), "recorded by opai");
    }
}
