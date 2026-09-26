//! Core library for Open Photo AI.
//!
//! Everything both front ends need lives here: this crate knows nothing about any user interface.
//!
//! # Starting up
//!
//! [`prepare_library_path`] must run **first**, as the first statement of `main`: it puts the runtime's and
//! NVIDIA's library directories on the dynamic loader's search path (and restarts the process under it on
//! Linux). Skipping it means CPU-only inference.
//!
//! [`Opai::initialize`] then claims the configuration directory for this process (at most one per user),
//! installs the ONNX Runtime and GPU libraries this machine can use, and loads the runtime before returning.
//!
//! # Naming a model
//!
//! [`Upscale`], [`Denoise`], [`Sharpen`], [`LightAdjustment`], [`ColorBalance`], [`Colorization`], [`Detection`],
//! [`FaceRecovery`] and friends name a model, carry its parameters, and answer which files it needs — none of
//! them run anything. [`fn@catalogue`] is what a front end reads instead of hard-coding a variant, precision or
//! bound.
//!
//! A per-run parameter ([`Scale`], [`Strength`], [`Bias`], [`Fidelity`], [`Faces`]) is validated on acceptance
//! and is part of the operation's identity — two operations are equal exactly when they name the same model,
//! precision and parameters — and part of the cache tag, which instead answers whether two runs produce the
//! same image.
//!
//! What is loaded is independent of both: a resident session is filed under its artifact and provider, so
//! every operation needing one graph shares it.
//!
//! [`Detection`] is the one family whose result is not an image: it produces a [`DetectionOutput`] (a set of
//! [`Face`]s), which [`FaceRecovery`] takes as the faces to restore.
//!
//! # Putting a model on disk
//!
//! An artifact's files are installed on first use, into a directory of their own under `models/`, and every
//! file is verified against its published SHA-256 (resolved from the live listing, a cached copy, or a
//! compiled-in fallback, in that order). An artifact none of the three name is refused, nothing transferred.
//!
//! The listing is read lazily — never during [`Opai::initialize`] — so a process that never runs inference
//! never contacts the model host.
//!
//! [`InitOptions::models`] decides, once per process, what to do with files already on disk; the default
//! checks every one against its published hash.
//!
//! # Opening a model
//!
//! A named artifact becomes a loaded session on the best execution provider this machine supports, kept
//! resident and released 15 minutes after last use. Concurrent requests for the same not-yet-open model are
//! served by a single build.
//!
//! A session is never freed while something still holds it. A provider that cannot open a model is a
//! **downgrade**, not a failure — the model is rebuilt with nothing attached, and both what was asked for and
//! what it ran on are reported. The downgrade is not latched: the next request tries the requested provider
//! again.
//!
//! Compiled provider artifacts (TensorRT engines, CoreML MLPrograms) are cached under `engines/` per model,
//! discarded when the weights change or the ONNX Runtime version changes.
//!
//! Every downgrade is logged at `warn`; [`Opai::process`] also returns it as a [`ProviderReport`] for a front
//! end to show live.
//!
//! [`Opai::release_sessions`] releases every resident session at once.
//!
//! # Running an enhancement
//!
//! [`Opai::process`] takes a [`Picture`] and an ordered chain of [`Operation`]s, applies each to the previous
//! result, and returns an [`Enhanced`] (the picture plus a [`ProviderReport`]). See [`Opai::process`] for
//! which families run and which are refused.
//!
//! ## Upscale
//!
//! Kyoto, Tokyo and Saitama publish per-factor weights, reached by whole native passes plus one final resample
//! if the sequence overshoots the requested scale. Osaka is a diffusion model with no native factor: it
//! resamples to the target size first, then restores detail. Osaka is by far the heaviest — three resident
//! sessions plus float buffers proportional to the *output*, unbudgeted — and its noise field differs from the
//! Go application's, so results are not pixel-identical between the two.
//!
//! ## The bit depth of the result
//!
//! [`OutputDepth`] is 8 bits, 16, or the source's own, defaulting to 8, resolved once against the loaded image
//! before the chain runs.
//!
//! ## What a run reports
//!
//! [`InferenceProgress`] names the operation, its [`Stage`] (installing / running / already known), and
//! fractional progress for the operation and for the whole chain. Operations take equal shares; an install
//! only claims the head of its own share; an upscale's passes are weighted by input pixel area. No callback,
//! no cost.
//!
//! ## Stopping a run
//!
//! [`ProcessOptions::cancel`] is a [`CancellationToken`], checked cooperatively between tiles (the tile loop
//! is blocking and cannot be aborted by dropping the future). A cancelled run produces no image and reports
//! [`InferenceError::Cancelled`], distinct from [`InferenceError::Shutdown`].
//!
//! A stop also withdraws a run from a shared model transfer rather than aborting it outright — bytes already
//! moved stay on disk for the next run to resume onto.
//!
//! # The run cache
//!
//! Each step's result is kept, so repeating a step already seen is a read, not a run — no model installed,
//! no session acquired, and it still counts its full share of progress under [`Stage::Cached`].
//!
//! **The key is the identity of the pixels held**: source identity, operations applied so far, and resolved
//! depth. The execution provider is deliberately excluded — same enhancement, same key, whichever processor
//! made it — and a caller needing a real run per provider turns the cache off.
//!
//! An analysis is kept too, under its own encoding, and the two kinds of entry cannot be read as one another.
//!
//! An entry expires 24 hours after being stored; space is reclaimed by a sweep on store open. Nothing bounds
//! the store by size.
//!
//! **Losing the cache is never an error**: an unopenable disk store falls back to an in-memory one for the
//! process's life, or to no cache at all — [`Opai::cache_mode`] reports which. No cache failure reaches
//! [`InferenceError`]; each is logged where decided, with a declined write logged at a different level than a
//! broken one so a run silently caching nothing is still discoverable in `opai.log`.
//!
//! [`ProcessOptions::cache`] and [`ExecuteOptions::cache`] turn caching off per run, for one store.
//!
//! # Opening and writing images
//!
//! [`image`] reads a file into a [`Picture`] (camera RAW included), writes one out, and describes a file
//! without decoding it — each with an `async` and a `_blocking` form.
//!
//! # Limits
//!
//! - **Alpha does not survive inference.** A transparent source composites against black.
//! - **Memory is not budgeted.** An 8x 16-bit run of a large photograph can hold well over a gigabyte.
//! - **A downgrade is not attributed per operation** in [`Opai::process`]'s result — only the log carries the
//!   per-model answer, at `warn`.

// A session outliving the cache's hold on it is a property of ownership, replacing the Go app's manual
// locking discipline.
//
// The downgrade log line exists because a fallback run produces the right image several times slower with no
// other symptom.
//
// Cancellation is fine-grained because an upscale of a large photograph is the longest wait in this app, and
// a first model transfer can be longer still — hence a stop reaching it too.

mod app;
mod autopilot;
mod cache;
mod config;
mod deps;
mod error;
mod gpu;
mod hardware;
pub mod image;
mod inference;
mod instance;
mod libpath;
#[cfg(test)]
mod live_support;
pub mod logging;
mod models;
mod pipeline;
mod progress;
mod providers;
mod runtime;
mod sessions;
mod setup;
mod task;
pub mod telemetry;

pub use app::{CLI, GUI, Holder, PERF};
/// What an analysis concluded, and the one enhancement it calls for. The module stays private; only the
/// answer is public, not the signals behind it.
pub use autopilot::{Suggestion, Suggestions, suggested_scale};
pub use cache::CacheMode;
pub use deps::ModelTrust;
pub use error::{InferenceError, InitError, UnsupportedReason};
// Re-exported so a caller of this crate's image functions doesn't need `rust-sak` in its own manifest — it's
// pinned by git tag and unpublished.
pub use image::{EncodeOptions, ImageError, ImageFormat, ImageInfo, ImageIoError, Picture, RawFormat, RawImageInfo};
/// The inference vocabulary needed to ask for a run or read its report. The module stays private.
pub use inference::depth::OutputDepth;
pub use inference::execute::{ExecuteOptions, Executed};
pub use inference::process::{Enhanced, ProcessOptions, ProviderReport, ProviderVerdict};
pub use inference::progress::{InferenceProgress, OnInference, Stage};
pub use libpath::prepare_library_path;
pub use logging::LogError;
pub use models::{
    Analysis, ArtifactId, Bias, BuildError, ColorBalance, ColorBalanceParams, ColorBalanceVariant, Colorization,
    ColorizationVariant, Confidence, DataOperation, Denoise, DenoiseParams, DenoiseVariant, Detection, DetectionOutput,
    DetectionVariant, Face, FaceRecovery, FaceRecoveryParams, FaceRecoveryVariant, Faces, Family, FamilyEntry,
    Fidelity, FloatPrecision, GraphRole, GraphSet, LightAdjustment, LightAdjustmentParams, LightAdjustmentVariant,
    Operation, OsakaPrecision, ParameterEntry, ParameterKind, ParameterValues, Pass, Point, Precision, RangeError,
    Rect, Resolution, Scale, Sharpen, SharpenParams, SharpenVariant, Strength, Subject, UnknownRole, Upscale,
    UpscaleParams, UpscaleVariant, VariantEntry, catalogue,
};
pub use progress::{Dependency, OnPlan, OnProgress, Phase, PlannedDependency, Progress};
pub use providers::{ExecutionProvider, SupportedProviders};
// Re-exported so a caller only needs this crate to hold the cancellation token, not `tokio_util` too.
pub use tokio_util::sync::CancellationToken;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cache::Store;
use deps::artifact::ONNX_RUNTIME;
use sessions::{Sessions, caches};
use task::spawn_blocking;

// One name in one place: `gui` and `cli` must resolve the same config/model/cache/log directories, so this is
// shared rather than duplicated per front end.
//
// `initialize` still takes `name` as a parameter rather than reading this — the suite initializes under a
// temporary name, and this library can't assume it's always this application.
//
// Kept as the Go app's bundle identifier's *replacement* (not shared with it): the Go app derives its
// directories from the app name it passes in, not from the bundle id, so the two coexist safely as long as the
// names differ.
//
// Reused everywhere an app identity is needed: config dir, bundle identity, D-Bus name, Windows mutex, macOS
// socket path, and (on macOS) `~/Library/Application Support` layout.
/// The name this application is, as every front end passes it to [`Opai::initialize`] and [`logging::init`]: the
/// application's bundle identifier.
///
/// `io.vinicius.opai`, deliberately distinct from the Go app's `open-photo-ai` — no shared config, models,
/// cache, log, or claim. Cost: a duplicate ~175 MB runtime + model download for existing Go app users.
pub const APP_NAME: &str = "io.vinicius.opai";

/// Waits for the results the run cache is still writing in the background, or for `timeout` to pass, and returns
/// whether every one was written.
///
/// A run hands its result back before the result is stored, so a process that exits the moment its last run returns
/// can lose that entry. Dropping the last [`Opai`] handle waits for them already; a front end whose handle is never
/// dropped — one that exits from inside its event loop — calls this on its way out.
pub fn settle_cache_writes(timeout: std::time::Duration) -> bool {
    cache::settle(timeout)
}

/// Returns the application version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Everything about an initialization that has a sensible default.
///
/// Passed as an `Option` to [`Opai::initialize`]: only `name` has no default, so a caller wanting to "just
/// start" passes `None`.
///
/// Use `..Default::default()` to set one field, keeping later additions source-compatible:
///
/// ```
/// # use opai::InitOptions;
/// let options = InitOptions { app: Some(opai::GUI.to_string()), ..Default::default() };
/// ```
#[derive(Clone, Default)]
pub struct InitOptions {
    // Deliberately not `#[non_exhaustive]`: that would bar the `..Default::default()` construction above.

    // A `String`, not an enum: the set of embedders isn't closed. This project's own binaries use constants
    // ([`GUI`], [`CLI`], [`PERF`]) so a misspelling fails to compile.
    /// Who this process says it is. Defaults to none, meaning the `name` it initializes under.
    ///
    /// Named in every log record and in a single-instance refusal. Must survive being recorded as one word —
    /// see [`InitError::InvalidApp`].
    pub app: Option<String>,

    // Declared here (not per run) so a run's options can't be misread as controlling it.
    /// What this process does with a model whose files are already on disk. Defaults to
    /// [`ModelTrust::Published`], which checks every file against its published hash.
    ///
    /// [`ModelTrust::LocalFiles`] is for debugging an unpublished re-exported model — see that type for what
    /// it skips. Set only here, with no per-run override.
    pub models: ModelTrust,

    /// Where install progress goes. Defaults to none, at no cost.
    ///
    /// One [`Progress`] per phase of each dependency actually installed, each running 0 to `1.0`.
    pub on_progress: Option<OnProgress>,

    /// Where the install plan goes. Defaults to none, at no cost.
    ///
    /// Called **once**, listing every dependency this machine will account for with its published size — after
    /// the hardware is probed but **before anything is transferred, expanded or created**.
    ///
    /// Lists what this machine *needs*, not what's missing: an already-installed dependency is on the list too,
    /// reported moments later via [`InitOptions::on_progress`] with [`Phase::AlreadyInstalled`]. A refusal
    /// before the hardware is probed ([`InitError::AlreadyRunning`]) reports no plan at all.
    pub on_plan: Option<OnPlan>,
}

impl std::fmt::Debug for InitOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Written by hand, like `ProcessOptions`'s: a callback prints nothing useful except whether it's set.
        f.debug_struct("InitOptions")
            .field("app", &self.app)
            .field("models", &self.models)
            .field("on_progress", &self.on_progress.is_some())
            .field("on_plan", &self.on_plan.is_some())
            .finish()
    }
}

/// A ready application: its dependencies are on disk and its configuration directory is resolved.
///
/// Cheap to clone — state is behind an [`Arc`] — so a GUI can `app.manage()` one and keep another locally
/// without double-initializing.
#[derive(Debug, Clone)]
pub struct Opai {
    inner: Arc<Inner>,
}

/// The shared state behind an [`Opai`].
#[derive(Debug)]
struct Inner {
    // The ONNX environment is deliberately not a field: `ort` keeps it process-global, and sessions hold their
    // own `Arc` to it, so nothing here needs to track its lifetime.
    /// The application name every directory is resolved from.
    name: String,
    /// `<platform config dir>/<name>`, created.
    config_dir: PathBuf,
    /// This process's claim on `config_dir`: at most one Open Photo AI process per user is past initialization.
    ///
    /// Never read — released on drop, and since `Opai` is `Clone`-over-`Arc`, exactly when the last handle
    /// drops. No method releases it early.
    #[expect(dead_code, reason = "the claim is held rather than read; dropping it is what releases the lock")]
    claim: instance::Claim,
    /// What this machine turned out to be able to run, decided during initialization.
    providers: SupportedProviders,
    /// What this process does with a model whose files are already on disk, as this initialization declared.
    ///
    /// Held rather than read (the installer runs from its own copy), as [`claim`](Self::claim) is.
    #[expect(
        dead_code,
        reason = "the installer runs from the copy handed to Sessions; this is the record of it"
    )]
    models: ModelTrust,
    /// The sessions this application is holding in memory, and the one path that opens a model.
    ///
    /// Not a `static`: built against *this* initialization's provider report, since two `Opai`s in one process
    /// are two applications with two configuration directories.
    sessions: Sessions,
    /// The results of enhancements already carried out, so repeating one is a read rather than a run.
    ///
    /// Opening the same directory twice in one process yields one shared store rather than a lock failure.
    cache: Store,
}

/// How long dropping the last [`Opai`] handle waits for the run cache's queued writes: long enough for a large result
/// to be encoded and written, short enough that a process on its way out is not held there by a failing disk.
const SETTLE_ON_DROP: std::time::Duration = std::time::Duration::from_secs(10);

impl Drop for Inner {
    fn drop(&mut self) {
        // Before the claim is released with the other fields, so the next process to take the directory finds what
        // this one's last run produced. Writes queued by another `Opai` in the process are waited for too, which
        // costs at most the same bounded wait.
        if !cache::settle(SETTLE_ON_DROP) {
            tracing::warn!(timeout = ?SETTLE_ON_DROP, "results still being written to the run cache were abandoned");
        }
    }
}

/// This application's own sessions and the `ort` tile step, which is what a run is carried out against.
impl pipeline::Backend for Opai {
    // A trait seam so the chain, pass sequence, depth resolution, progress weighting, identity composition and
    // every refusal can be tested against a fake, on every CI platform; this impl is the half needing a runtime.
    type Session = ort::session::Session;
    type Error = ort::Error;

    async fn acquire(
        &self,
        artifact: &ArtifactId,
        profile: &providers::profile::EpProfile,
        requested: ExecutionProvider,
        interest: &sessions::Interest,
    ) -> Result<sessions::SessionHandle<Self::Session>, error::SessionError> {
        self.session(artifact, profile, requested, interest).await
    }

    fn run_tile(
        handle: &sessions::SessionHandle<Self::Session>,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(), Self::Error> {
        pipeline::session::run_tile(handle, input, output)
    }

    fn run_graph(
        handle: &sessions::SessionHandle<Self::Session>,
        input: &[f32],
        input_shape: pipeline::session::GraphShape,
        output: &mut [f32],
        output_shape: pipeline::session::GraphShape,
    ) -> Result<(), Self::Error> {
        pipeline::session::run_graph(handle, input, input_shape, output, output_shape)
    }

    fn run_named_outputs(
        handle: &sessions::SessionHandle<Self::Session>,
        input: &[f32],
        input_shape: pipeline::session::GraphShape,
        outputs: &mut [pipeline::session::NamedOutput<'_>],
    ) -> Result<(), Self::Error> {
        pipeline::session::run_named_outputs(handle, input, input_shape, outputs)
    }

    fn run_weighted(
        handle: &sessions::SessionHandle<Self::Session>,
        input: &[f32],
        input_shape: pipeline::session::GraphShape,
        weight: f32,
        output: &mut [f32],
        output_shape: pipeline::session::GraphShape,
    ) -> Result<(), Self::Error> {
        pipeline::session::run_weighted(handle, input, input_shape, weight, output, output_shape)
    }
}

impl Opai {
    /// Installs everything the application needs on disk and returns a handle to it.
    ///
    /// `name` determines the configuration directory — `~/.config/<name>` on Linux, `~/Library/Application
    /// Support/<name>` on macOS, `%APPDATA%\<name>` on Windows.
    ///
    /// `options` is [`InitOptions`], or `None` for its defaults.
    ///
    /// [`InitOptions::app`] is recorded beside the claim so a refused process names which application to
    /// close, and is what every log record of this process carries.
    ///
    /// # One process per user
    ///
    /// Claims the configuration directory for this process; a second process of the same user is refused with
    /// [`InitError::AlreadyRunning`]. The claim is taken right after the name is validated — before hardware is
    /// probed, before anything is transferred or created — so a refusal leaves nothing behind.
    ///
    /// Per user, not per machine, so two users or two application names never contend. Released when the last
    /// [`Opai`] handle is dropped, or automatically by the OS if the process dies uncleanly.
    ///
    /// [`version`] and [`fn@catalogue`] read nothing here and work regardless of the claim.
    ///
    /// What's installed depends on hardware: the ONNX Runtime always; on an NVIDIA machine, CUDA and cuDNN;
    /// additionally, on an RTX-branded adapter, TensorRT. Installed sequentially (not concurrently), in that
    /// order, since cuDNN links against the CUDA runtime.
    ///
    /// A GPU library not published for this platform is reported unsupported rather than attempted; the
    /// runtime itself is not — its absence fails the platform.
    ///
    /// A dependency already current and complete is skipped without a download or re-hash, reporting once as
    /// [`Phase::AlreadyInstalled`] at the point it would otherwise have installed — so observed order matches
    /// install order regardless of what had work to do.
    ///
    /// [`InitOptions::on_progress`] and [`InitOptions::on_plan`] behave as documented on those fields.
    ///
    /// The install directories only reach the loader's search path if [`prepare_library_path`] ran first, at
    /// the top of `main`.
    ///
    /// Once installed, the ONNX Runtime is loaded from `runtime` and its inference environment started (with
    /// telemetry off), before this returns. The environment is process-wide, started at most once, and lives
    /// until exit.
    ///
    /// Calling this more than once in one process is supported (a front end restarting its application layer
    /// after an error): the second call joins the existing claim rather than contending with it, re-checks
    /// installs, and reuses the already-loaded runtime library — even under a different `name`. The one
    /// exception is a repeat after a load that *failed*: see [`InitError::RuntimeUnavailable`].
    ///
    /// # Errors
    ///
    /// [`InitError::InvalidName`] for a `name` that can't be a directory; [`InitError::InvalidApp`] if the
    /// declared identity couldn't be recorded; [`InitError::AlreadyRunning`] if another process holds the
    /// claim; [`InitError::UnsupportedPlatform`] if no runtime is published for this platform;
    /// [`InitError::HashMismatch`] for an archive that isn't the published one; [`InitError::Download`],
    /// [`InitError::Fs`] or [`InitError::Io`] for transfer/expansion/bookkeeping failures, each naming what it
    /// was working on.
    ///
    /// [`InitError::Cancelled`] means the runtime shut down before the work ran — not a broken install.
    ///
    /// For the runtime load itself: [`InitError::RuntimeLoad`], [`InitError::RuntimeStart`], and
    /// [`InitError::RuntimeUnavailable`] (a prior load already failed in this process) — all naming the library
    /// path.
    pub async fn initialize(name: &str, options: Option<InitOptions>) -> Result<Self, InitError> {
        Self::recorded(name, Self::initialize_inner(name, options)).await
    }

    /// Runs `initialization` and records how it ended — once, whichever way that was — with how long it took.
    ///
    /// The shape of `deps::install`: the body returns through every `?` it likes, and this is the one place that sees
    /// the outcome. Separate from [`Self::initialize_inner`] so the suite can drive each arm against a directory of
    /// its own; `initialize` itself resolves the platform's directory and loads a real runtime.
    pub(crate) async fn recorded(
        name: &str,
        initialization: impl Future<Output = Result<Self, InitError>>,
    ) -> Result<Self, InitError> {
        use telemetry::unit::{self, Outcome, Unit, unit_span};
        use tracing::Instrument as _;

        let span = unit_span!("initialize", name);

        // Started before anything, so `duration` covers the claim and the name's validation as well as the install.
        let started = std::time::Instant::now();
        let outcome = initialization.instrument(span.clone()).await;
        let duration = started.elapsed();

        // Inside the span, so the closing record is in the initialization's trace.
        let _entered = span.enter();
        match &outcome {
            // The closing record, naming what the machine turned out to support — the answer to "why did this run on
            // the CPU".
            Ok(opai) => {
                tracing::info!(name, providers = ?opai.providers(), ?duration, "initialized");
                unit::ended(Unit::Initialize, &span, duration, Outcome::Finished);
            }
            // Below `warn`: this is what `cli` answers while the window is open, a correct refusal rather than a
            // fault. Its span still says the initialization did not happen, under a kind of its own.
            Err(error @ InitError::AlreadyRunning { holder }) => {
                tracing::info!(
                    name,
                    holder = holder.as_ref().map(tracing::field::display),
                    "initialization refused: another process holds the claim"
                );
                unit::ended(Unit::Initialize, &span, duration, Outcome::Failed { kind: error.kind(), error });
            }
            Err(error) => match error.stop_reason() {
                Some(reason) => {
                    tracing::info!(name, reason, ?duration, "initialization stopped");
                    unit::ended(Unit::Initialize, &span, duration, Outcome::Stopped);
                }
                None => {
                    tracing::warn!(name, ?duration, %error, "initialization failed");
                    unit::ended(Unit::Initialize, &span, duration, Outcome::Failed { kind: error.kind(), error });
                }
            },
        }

        outcome
    }

    /// The body of [`Opai::initialize`], which records none of its own outcomes: [`Self::recorded`] does.
    async fn initialize_inner(name: &str, options: Option<InitOptions>) -> Result<Self, InitError> {
        let InitOptions { app, models, on_progress, on_plan } = options.unwrap_or_default();

        // Resolved first: the claim below is recorded inside this directory.
        let app_dir = config::app_dir(name)?;

        // A declared identity, or `name` by default — validated either way, since the record is one line split
        // on the first space and `name` permits a space.
        let app = app::identity(app, name)?;

        // The first record of this session, naming what every later record is about.
        tracing::info!(
            name,
            config_dir = %app_dir.display(),
            onnx = ONNX_RUNTIME.tag,
            os = std::env::consts::OS,
            arch = std::env::consts::ARCH,
            "initializing"
        );

        // The claim, immediately after — before hardware is probed, before anything moves. Everything the
        // claim protects (cache, model installs, runtime, GPU) is past this point; `--version`, `--help` and a
        // catalogue listing all work while another process holds it.
        //
        // A second initialization in *this* process joins the existing claim rather than contending with it.
        let claim = instance::claim(&app_dir, &app)?;

        let plan = Self::select(gpu::adapters(), std::env::consts::OS, std::env::consts::ARCH)?;

        // Here, before `Self::install` — the first thing that transfers or creates anything — is called.
        Self::report_plan(&plan, on_plan.as_ref());

        // Taken by name before the plan moves into install, so this can't disagree with what actually installs.
        let lib = plan.runtime.lib.expect("every platform the runtime is published for names its library");

        let (mut opai, installed) = Self::install(name, app_dir, claim, plan, models, on_progress).await?;

        // After install (the pinned tag only applies once the runtime is on disk) and before anything can build
        // a session: a compiled provider artifact is only valid for the runtime that built it.
        //
        // On a blocking thread: the common path is a small stamp read, the rare one an unbounded delete of
        // `engines/`.
        let engines_dir = opai.inner.config_dir.clone();
        let app_name = name.to_string();
        spawn_blocking::<_, InitError, _>(move || {
            caches::invalidate_for_runtime(&engines_dir, &app_name, ONNX_RUNTIME.tag)
        })
        .await??;

        // After every install and before returning the handle, so a runtime that can't start is a failed
        // initialization rather than a mid-enhancement surprise.
        let library = runtime::library_path(&installed.runtime, lib);

        // `dlopen` of a ~175 MB library plus environment creation is blocking and syscall-heavy, so it runs on
        // a blocking thread.
        let app_name = name.to_string();
        let webgpu = spawn_blocking::<_, InitError, _>(move || runtime::start(&app_name, &library)).await??;

        // WebGPU is the one provider the install plan cannot decide: its plugin ships inside the runtime's archive, and
        // whether it offers a device is only known once that runtime has loaded it. The handle has not been shared yet.
        let inner = Arc::get_mut(&mut opai.inner).expect("the handle is not shared before initialization returns");
        inner.providers = inner.providers.with_webgpu(webgpu);
        inner.sessions.set_webgpu(webgpu);

        Ok(opai)
    }

    /// What this machine can be asked to run on, as decided during initialization.
    ///
    /// `cuda` and `tensorrt` are true only where the library was actually installed, not merely where hardware
    /// was detected.
    ///
    /// A report of what can be *asked* for, not a promise a session will build on it.
    pub fn providers(&self) -> SupportedProviders {
        self.inner.providers
    }

    /// What is backing the run cache, as decided during initialization.
    ///
    /// [`CacheMode::Disk`] normally; [`CacheMode::Memory`] if the cache directory couldn't be opened;
    /// [`CacheMode::None`] if neither store could be opened.
    ///
    /// Reported, never configured — a front end can warn on anything but `Disk`. This is a different question
    /// from [`ProcessOptions::cache`] (a per-run decision): a run with caching off for itself still reads
    /// `Disk` here.
    pub fn cache_mode(&self) -> CacheMode {
        self.inner.cache.mode()
    }

    /// The application's configuration directory: `<platform config dir>/<name>`.
    pub fn config_dir(&self) -> &Path {
        &self.inner.config_dir
    }

    /// Resolves and creates `sub` under the application's configuration directory.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::Fs`] if `sub` could escape its parent and [`InitError::Io`] if it could not be created.
    pub fn sub_dir(&self, sub: &str) -> Result<PathBuf, InitError> {
        config::sub_dir(&self.inner.config_dir, &self.inner.name, sub)
    }

    /// The session for `artifact`, installing its files first if they are not on disk.
    ///
    /// One place turns a model name into a runnable session, so caching, fallback and installation can't
    /// diverge between callers. A model not on disk is installed and verified (transfer reported through
    /// `on_progress`); sessions are kept resident, reused, released 15 minutes after last use, and never freed
    /// while in use.
    ///
    /// If `requested`'s providers can't open the model, it's rebuilt with nothing attached and the handle
    /// reports what it ran on; the downgrade isn't latched.
    ///
    /// `profile` is the tuning measured for this artifact ([`Operation::profile`]).
    ///
    /// `pub(crate)`: a session isn't held by library callers — [`Opai::process`] acquires and releases one per
    /// operation, which is what makes [`Opai::release_sessions`] have nothing outstanding to wait for.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Install`](error::SessionError::Install) where the files could not be put on disk or
    /// read back — including [`InitError::UnpublishedModel`] for an artifact no source publishes a hash for — and
    /// [`SessionError::Build`](error::SessionError::Build) where the model is on disk and neither the resolved
    /// providers nor the CPU could open it.
    pub(crate) async fn session(
        &self,
        artifact: &ArtifactId,
        profile: &providers::profile::EpProfile,
        requested: ExecutionProvider,
        interest: &sessions::Interest,
    ) -> Result<sessions::SessionHandle<ort::session::Session>, error::SessionError> {
        self.inner.sessions.session(artifact, profile, requested, interest).await
    }

    /// Runs `source` through `operations` and returns the enhanced image beside what it ran on.
    ///
    /// Each operation applies to the result of the one before it, **in [`Family::APPLY_ORDER`]** whatever order they
    /// were given in: the chain is put into that order before anything else happens, so a face recovery always runs
    /// before an upscale that would move its faces, and two front ends listing one stack two ways ask for one result.
    /// Operations of one family keep the order they were given in. An empty chain returns `source` unchanged — a
    /// valid request, sent when a user has toggled every enhancement off.
    ///
    /// `options` is [`ProcessOptions`], or `None` for its defaults (`Auto`, 8-bit, no progress, no
    /// cancellation).
    ///
    /// [`Enhanced`] pairs the picture with a [`ProviderReport`], answering "did this get the GPU it asked
    /// for?" even though a downgraded run looks identical except for speed. It's on the result, not the
    /// progress stream, so it's available even with no callback registered.
    ///
    /// The result's [`identity`](Picture::identity) is composed from `source`'s identity and every operation
    /// applied — not the input's own — so a chained run is never mistaken for the first. What it ran on is
    /// deliberately not part of it. Its [`path`](Picture::path) is the source's.
    ///
    /// # What is refused
    ///
    /// No published enhancement is refused. All seven image families run, each running every variant it
    /// publishes: upscale (Kyoto/Tokyo/Saitama as native passes, Osaka via diffusion), face recovery (align,
    /// run, composite through one contract), light adjustment (both models at a fixed square, applied as a
    /// gain), colour balance (both models at a fixed square, applied as a fitted colour mapping), denoise (all
    /// three models over a tile grid at the photograph's own resolution, then the strength), sharpen (all three
    /// models over a tile grid at the photograph's own resolution, then the strength), and colorization (all
    /// three models once over the whole photograph at a fixed square, the predicted colour put onto the
    /// photograph's own lightness). **Detection is not among them and never will be** — its result isn't an
    /// image, so it's carried in [`Analysis`] instead and can't be put in a chain.
    ///
    /// What can still refuse a run is [`InferenceError::Unsupported`], for a model whose declaration is
    /// incomplete (see [`UnsupportedReason`]), which no shipped variant is. That refusal is checked over the
    /// **whole** chain before anything installs, so a refused second step doesn't cost the first step's
    /// transfer.
    ///
    /// # Errors
    ///
    /// [`InferenceError`], and in every case **no image at all** — not partial, not the cancellation's
    /// half-covered buffer.
    pub async fn process(
        &self,
        source: &Picture,
        operations: &[Operation],
        options: Option<ProcessOptions>,
    ) -> Result<Enhanced, InferenceError> {
        inference::process::process(self, self.inner.cache.handle().as_ref(), source, operations, options).await
    }

    /// Runs one operation whose result is **not** an image over `source`, and returns what it found beside what
    /// it ran on.
    ///
    /// [`process`](Self::process)'s counterpart for the one family with no picture result. `source` is left
    /// untouched.
    ///
    /// **One operation, not a chain** — chaining is a property of pixels, and there's nothing to chain when the
    /// result is a measurement.
    ///
    /// **The result type is the operation's own.** [`Analysis`] carries this path and [`DataOperation`] names
    /// what it produces (e.g. detection produces [`Faces`]); a picture-producing operation can't be put here at
    /// all, since it's typed as [`Operation`] and this takes [`Analysis`] — a compile-time distinction, not a
    /// runtime refusal.
    ///
    /// ```no_run
    /// # async fn run(opai: &opai::Opai, picture: &opai::Picture) -> Result<(), opai::InferenceError> {
    /// use opai::{Detection, Executed, FloatPrecision};
    ///
    /// let detection = Detection::newyork(FloatPrecision::Fp32);
    /// let Executed { value: faces, .. } = opai.execute(picture, &detection, None).await?;
    ///
    /// println!("{} faces", faces.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// `options` is [`ExecuteOptions`] (its own type, since `depth` and other `ProcessOptions` fields wouldn't
    /// mean anything for a run with no pixel output), or `None` for its defaults (`Auto`, no progress, no
    /// cancellation, cache on).
    ///
    /// **A repeated analysis is served, not executed.** Same operation, same image: served from the same store
    /// [`process`](Self::process) uses, under the same 24-hour lifetime, looked up **before** anything
    /// transfers or opens — reporting [`ProviderVerdict::NothingExecuted`], with progress still reaching its
    /// maximum. Set [`ExecuteOptions::cache`] to `false` for a run that must actually execute.
    ///
    /// Losing the store just costs a re-run, never surfaces as an error.
    ///
    /// # Errors
    ///
    /// [`InferenceError`], and in every case **no result at all** — an empty [`Faces`] is a legitimate finding,
    /// so it can't be substituted for an error.
    pub async fn execute(
        &self,
        source: &Picture,
        analysis: &Analysis,
        options: Option<ExecuteOptions>,
    ) -> Result<Executed<Faces>, InferenceError> {
        inference::execute::execute(self, self.inner.cache.handle().as_ref(), source, analysis, options).await
    }

    /// Analyses `source` and returns the enhancements that photograph calls for.
    ///
    /// **Autopilot**: run unprompted when an image opens. Reads seven signals — size, exposure, colour cast, whether it
    /// is grayscale, grain, blur, and whether anyone's in frame — and each suggestion names only the enhancement
    /// **family** plus what the pixels themselves determine. Which model/precision to run is a user preference this
    /// library doesn't read.
    ///
    /// ```no_run
    /// # async fn run(opai: &opai::Opai, picture: &opai::Picture) -> Result<(), opai::InferenceError> {
    /// use opai::Suggestion;
    ///
    /// let suggestions = opai.suggest(picture, None, None).await?;
    ///
    /// for suggestion in &suggestions.suggested {
    ///     match suggestion {
    ///         Suggestion::Denoise => println!("there is visible grain"),
    ///         Suggestion::FaceRecovery => println!("there are faces to restore"),
    ///         Suggestion::Colorization => println!("it is grayscale"),
    ///         Suggestion::LightAdjustment => println!("the exposure is off"),
    ///         Suggestion::ColorBalance => println!("there is a colour cast"),
    ///         Suggestion::Sharpen => println!("it is blurred"),
    ///         Suggestion::Upscale { scale } => println!("worth enlarging {}x", scale.get()),
    ///     }
    /// }
    ///
    /// // A signal that couldn't be read is reported beside the ones that could.
    /// if let Some(error) = &suggestions.incomplete {
    ///     eprintln!("the analysis was incomplete: {error}");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// `families` narrows the analysis to the families a caller cares about:
    ///
    /// - `None` checks every family.
    /// - `Some(families)` checks only those. A family outside the set is never suggested, and a signal only such a
    ///   family needs is never read — no face detection runs unless [`Family::FaceRecovery`] is in the set.
    /// - `Some(&[])` checks **nothing** and returns an empty, complete result. It does not mean "every family".
    /// - [`Family::Detection`] matches nothing, because it is never suggested.
    ///
    /// A signal that was not read because its families weren't asked for never makes the result
    /// [`incomplete`](Suggestions::incomplete).
    ///
    /// ```no_run
    /// # async fn run(opai: &opai::Opai, picture: &opai::Picture) -> Result<(), opai::InferenceError> {
    /// use opai::Family;
    ///
    /// // Only whether to enlarge and whether to colorize: no face detection is run.
    /// let suggestions = opai.suggest(picture, Some(&[Family::Upscale, Family::Colorization]), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// `options` is [`ExecuteOptions`], same type as [`execute`](Self::execute) since every field applies here
    /// too.
    ///
    /// **An empty [`Suggestions::suggested`] is an answer, not a refusal** — an unreadable signal reports via
    /// [`Suggestions::incomplete`] instead.
    ///
    /// **Costs one bounded pass over the pixels and at most one detection run, often none**: the exposure, colour
    /// and grayscale signals read at most about a million samples and run no model, and the face signal goes through
    /// [`execute`](Self::execute)'s cache — and a detection here warms the store for the face-recovery run a suggestion turns into.
    ///
    /// **Holds no session of its own**, so [`release_sessions`](Self::release_sessions) has nothing outstanding
    /// from it.
    ///
    /// # Errors
    ///
    /// [`InferenceError::Cancelled`] or [`InferenceError::Shutdown`] produce **no suggestions at all**. Every
    /// other failure is a signal that couldn't be read, reported in [`Suggestions::incomplete`] instead of
    /// raised as an error nobody asked for.
    pub async fn suggest(
        &self,
        source: &Picture,
        families: Option<&[Family]>,
        options: Option<ExecuteOptions>,
    ) -> Result<Suggestions, InferenceError> {
        autopilot::suggest(self, self.inner.cache.handle().as_ref(), source, families, options).await
    }

    /// Releases every resident session.
    ///
    /// For shutting down, freeing memory on request, or starting a measurement from nothing — decisions the
    /// process owner makes, which is why this is public even though nothing on the running path needs it.
    ///
    /// A session still in use is not freed here and this doesn't wait for it — it goes when its last handle
    /// drops. Where nothing is in use, native resources free **before this returns**.
    pub fn release_sessions(&self) {
        self.inner.sessions.cache().clear();
    }
}

#[cfg(test)]
mod tests;
