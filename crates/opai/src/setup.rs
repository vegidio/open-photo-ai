//! What an initialization installs: choosing the dependencies this machine can use, and putting them on disk.
//!
//! The choosing is this crate's policy and the installing is `rust-sak`'s `ai`: this file decides which folders to
//! name, reports the plan and the progress in the shapes the front ends already read, and maps what can go wrong onto
//! [`InitError`]. See [`deps::folders`].

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rust_sak::ai::gpu::{self as ai_gpu, GpuRefusal};
use rust_sak::ai::{self, Event, Failure, InstallError, State};
use rust_sak::sysinfo::{CudaInfo, GpuInfo};
use tracing::Instrument as _;

use crate::cache::{self, CacheMode, Store};
use crate::deps::{self, ModelTrust, folders};
use crate::error::InitError;
use crate::progress::{Dependency as Which, OnPlan, OnProgress, Phase, PlannedDependency, Progress};
use crate::providers::SupportedProviders;
use crate::sessions::Sessions;
use crate::task::spawn_blocking;
use crate::telemetry::unit::{self, Unit, unit_span};
use crate::{Inner, Opai, instance};

/// What one machine's initialization will install, and where.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    /// The dependency root shared with every application installing from the same folders.
    pub(crate) root: PathBuf,
    /// What to install, in install order: always the runtime first, then the NVIDIA libraries this machine can run.
    pub(crate) dependencies: Vec<(Which, ai::Dependency)>,
}

impl Plan {
    /// The plan for a machine with `adapters`, whose NVIDIA driver reported `cuda`, installing under `root`.
    pub(crate) fn for_machine(root: PathBuf, adapters: &[GpuInfo], cuda: Option<&CudaInfo>) -> Self {
        let dependencies = select(adapters, cuda).into_iter().map(|which| (which.clone(), folders::dependency(&which)));
        Self { root, dependencies: dependencies.collect() }
    }
}

/// Which dependencies a machine with `adapters`, whose NVIDIA driver reported `cuda`, can run, in install order.
///
/// A pure function of the probe's answers, so every branch is driven without GPU hardware. Each refusal is recorded
/// at `info` with its reason: from outside, "an NVIDIA card but no CUDA" looks identical to a failed install.
pub(crate) fn select(adapters: &[GpuInfo], cuda: Option<&CudaInfo>) -> Vec<Which> {
    let mut selected = vec![Which::Runtime];
    let nvidia = || {
        adapters
            .iter()
            .filter(|gpu| crate::gpu::is_nvidia(gpu))
            .map(|gpu| gpu.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };

    match ai_gpu::check_with(adapters, cuda, &folders::CUDA_REQUIREMENT) {
        Ok(()) => selected.extend([Which::Cuda, Which::Cudnn]),
        // Not worth a record: the ordinary state of a Mac, and of any machine without an NVIDIA card.
        Err(GpuRefusal::NoAdapter) => return selected,
        Err(refusal) => {
            tracing::info!(
                adapters = %nvidia(),
                reason = %refusal,
                "the pinned CUDA release cannot run here; this machine runs without CUDA"
            );
            return selected;
        }
    }

    // TensorRT is built on CUDA, so it is asked only once CUDA is in.
    match ai_gpu::check_with(adapters, cuda, &folders::TENSORRT_REQUIREMENT) {
        Ok(()) => selected.push(Which::TensorRt),
        Err(refusal) => tracing::info!(
            adapters = %nvidia(),
            reason = %refusal,
            "the pinned TensorRT release cannot run here; this machine runs on CUDA"
        ),
    }

    selected
}

/// Where an install put the plan's dependencies, and which of them it installed.
#[derive(Debug, Clone)]
pub(crate) struct Installed {
    /// The ONNX Runtime's folder, which the runtime is loaded from.
    pub(crate) runtime: PathBuf,
    /// Whether CUDA (and cuDNN with it) is installed.
    pub(crate) cuda: bool,
    /// Whether TensorRT is installed.
    pub(crate) tensorrt: bool,
}

impl Installed {
    /// What this machine can be asked to run on before the runtime is loaded: the CPU, CoreML on macOS, and the NVIDIA
    /// providers whose libraries are installed. WebGPU is decided by the load.
    pub(crate) fn providers(&self) -> SupportedProviders {
        ai::Supported::detect(self.cuda, self.tensorrt).into()
    }
}

impl Opai {
    /// Installs `plan` and opens the run cache beside it, and builds the handle over both.
    ///
    /// The plan is reported to `on_plan` once, before anything is transferred: every dependency this machine will
    /// account for, in install order, with its archive's size. Then each is reported to `on_progress` as it installs,
    /// or once as already installed.
    ///
    /// `claim` is taken by value, not acquired here: an install literally can't be reached without a claim already in
    /// hand, for any caller.
    ///
    /// # Errors
    ///
    /// [`InitError::Transfer`] where a dependency could not be listed, downloaded or verified,
    /// [`InitError::UnsupportedPlatform`] where no runtime is published for this platform, and [`InitError::Io`] for a
    /// file error, a full disk included.
    pub(crate) async fn install_reporting(
        name: &str,
        app_dir: PathBuf,
        claim: instance::Claim,
        plan: Plan,
        models: ModelTrust,
        on_progress: Option<OnProgress>,
        on_plan: Option<OnPlan>,
    ) -> Result<(Self, Installed), InitError> {
        // Opened while the dependencies install rather than after them: the two share nothing, and a disk store's
        // open can wait out a stale lock and sweep yesterday's expired entries. Spawned so it starts now rather than
        // when it is first awaited, and on a blocking thread for the same reason. A cache that won't open still never
        // fails an otherwise-healthy launch — it degrades to memory, then to none, never an error — and one whose
        // installs fail is simply dropped.
        let cache_root = app_dir.clone();
        let opening =
            tokio::spawn(spawn_blocking::<_, InitError, _>(move || Store::open(&cache_root)).in_current_span());

        let installed = install_dependencies(&plan, on_progress, on_plan.as_ref()).await?;

        let cache = joined(opening.await)?;

        // Logged here (not inside `Store::open`) because the level depends on the outcome: disk is ordinary,
        // anything else is a slowdown nobody at the keyboard can see otherwise.
        let cache_dir = app_dir.join(cache::CACHE_DIR);
        match cache.mode() {
            CacheMode::Disk => tracing::info!(mode = "disk", path = %cache_dir.display(), "run cache ready"),
            CacheMode::Memory => tracing::warn!(
                mode = "memory",
                path = %cache_dir.display(),
                "the disk run cache could not be opened; caching in memory for this run"
            ),
            CacheMode::None => tracing::warn!(
                mode = "none",
                path = %cache_dir.display(),
                "no run cache is available for this run; every operation will be recomputed"
            ),
        }

        // Built against the report this exact install produced, and carrying `models` since the one model
        // installer is behind `Sessions`.
        let sessions = Sessions::new(app_dir.clone(), name.to_string(), installed.providers(), models);
        let inner = Inner { name: name.to_string(), config_dir: app_dir, claim, models, sessions, cache };

        Ok((Self { inner: std::sync::Arc::new(inner) }, installed))
    }
}

#[cfg(test)]
impl Opai {
    /// [`Opai::install_reporting`] with nobody to hand the plan to, as the suites install.
    ///
    /// # Errors
    ///
    /// As [`Opai::install_reporting`].
    pub(crate) async fn install(
        name: &str,
        app_dir: PathBuf,
        claim: instance::Claim,
        plan: Plan,
        models: ModelTrust,
        on_progress: Option<OnProgress>,
    ) -> Result<(Self, Installed), InitError> {
        Self::install_reporting(name, app_dir, claim, plan, models, on_progress, None).await
    }
}

/// Plans `plan` with `rust-sak`, reports the plan, and installs what is missing, reporting each dependency's progress
/// and recording how each install ended.
async fn install_dependencies(
    plan: &Plan,
    on_progress: Option<OnProgress>,
    on_plan: Option<&OnPlan>,
) -> Result<Installed, InitError> {
    let dependencies: Vec<ai::Dependency> =
        plan.dependencies.iter().map(|(_, dependency)| dependency.clone()).collect();
    let planned = ai::plan(&plan.root, &dependencies).await.map_err(|error| failed(plan, error))?;

    // A platform a library's folder publishes nothing for is not offered it: it is in neither the plan nor the report.
    let rows: Vec<(Which, u64)> = plan
        .dependencies
        .iter()
        .zip(planned.dependencies())
        .filter(|(_, row)| row.state != State::Unavailable)
        .map(|((which, _), row)| (which.clone(), row.size))
        .collect();
    if let Some(on_plan) = on_plan {
        let report: Vec<PlannedDependency> = rows
            .iter()
            .map(|(which, size)| PlannedDependency { dependency: which.clone(), size: *size })
            .collect();
        on_plan(&report);
    }

    let mut records = Records::new(plan, &rows, on_progress);
    let outcome = planned.install(|event| records.event(&event)).await;
    match outcome {
        Ok(()) => records.finish(),
        Err(error) => {
            let error = failed(plan, error);
            records.fail(&error);
            return Err(error);
        }
    }

    let has = |wanted: Which| rows.iter().any(|(which, _)| *which == wanted);
    Ok(Installed {
        runtime: ai::dirs::dir_for(&plan.root, &folders::source(&Which::Runtime)),
        cuda: has(Which::Cuda) && has(Which::Cudnn),
        tensorrt: has(Which::TensorRt),
    })
}

/// What `rust-sak`'s `error` means for this initialization.
fn failed(plan: &Plan, error: InstallError) -> InitError {
    let InstallError { dependency, failure } = error;
    let dir = || {
        plan.dependencies
            .iter()
            .find(|(_, candidate)| candidate.name == dependency)
            .map_or_else(|| plan.root.clone(), |(_, candidate)| ai::dirs::dir_for(&plan.root, &candidate.source))
    };
    match failure {
        Failure::UnsupportedPlatform { os, arch } => InitError::UnsupportedPlatform { dependency, os, arch },
        Failure::DiskFull => {
            InitError::Io { path: dir(), source: std::io::Error::from(std::io::ErrorKind::StorageFull) }
        }
        Failure::Io(reason) => InitError::Io { path: dir(), source: std::io::Error::other(reason) },
        failure @ (Failure::Network | Failure::Http(_) | Failure::Damaged | Failure::Malformed(_)) => {
            InitError::Transfer { dependency, reason: failure.to_string() }
        }
        // `Failure` is `rust-sak`'s and may grow; an unknown one is a transfer that did not complete.
        #[allow(unreachable_patterns, reason = "`Failure` is not `#[non_exhaustive]` today, and may become it")]
        failure => InitError::Transfer { dependency, reason: failure.to_string() },
    }
}

/// The progress reports and the install records of one initialization's dependencies, driven by `rust-sak`'s events.
///
/// Each dependency is a unit of its own in the telemetry, as it was when this crate installed them one at a time: it
/// opens on the dependency's first event, a [`Event::Current`] included, and closes on the next dependency's or at the
/// end.
struct Records<'a> {
    plan: &'a Plan,
    sizes: &'a [(Which, u64)],
    on_progress: Option<OnProgress>,
    open: Option<Open>,
}

/// The install unit of the dependency the events are about now.
struct Open {
    name: &'static str,
    version: String,
    span: tracing::Span,
    started: Instant,
}

impl<'a> Records<'a> {
    fn new(plan: &'a Plan, sizes: &'a [(Which, u64)], on_progress: Option<OnProgress>) -> Self {
        Self { plan, sizes, on_progress, open: None }
    }

    fn event(&mut self, event: &Event) {
        let (name, progress) = match *event {
            Event::Progress { dependency, phase, bytes, total, fraction } => {
                let phase = match phase {
                    ai::Phase::Download => Phase::Downloading,
                    ai::Phase::Extract => Phase::Extracting,
                };
                (dependency, Some((phase, bytes, (total > 0).then_some(total), fraction)))
            }
            Event::Current { dependency } => {
                let size = self.size(dependency);
                (dependency, Some((Phase::AlreadyInstalled, 0, Some(size), 1.0)))
            }
            Event::Verifying { dependency } => (dependency, None),
        };
        self.open(name);

        let (Some(on_progress), Some(which), Some((phase, bytes, total, fraction))) =
            (&self.on_progress, folders::which(name), progress)
        else {
            return;
        };
        on_progress(&Progress { dependency: which, phase, bytes, total, fraction });
    }

    /// The planned size of the dependency `rust-sak` named `name`.
    fn size(&self, name: &str) -> u64 {
        self.sizes
            .iter()
            .find(|(which, _)| folders::which(name).as_ref() == Some(which))
            .map_or(0, |(_, size)| *size)
    }

    /// Opens `name`'s unit, closing the one before it as finished.
    fn open(&mut self, name: &'static str) {
        if self.open.as_ref().is_some_and(|open| open.name == name) {
            return;
        }
        self.finish();
        let version = self.version(name);
        let span = unit_span!("install", dependency = name, version = %version);
        self.open = Some(Open { name, version, span, started: Instant::now() });
    }

    /// `<folder>/<version>` of the dependency named `name`, as in `cuda/13.3.0`: what its record says it installed.
    fn version(&self, name: &str) -> String {
        self.plan
            .dependencies
            .iter()
            .find(|(_, dependency)| dependency.name == name)
            .map(|(_, dependency)| format!("{}/{}", dependency.source.name(), dependency.source.version()))
            .unwrap_or_default()
    }

    /// Closes the open unit as finished.
    fn finish(&mut self) {
        if let Some(open) = self.open.take() {
            unit::ended(Unit::Install, &open.span, open.started.elapsed(), unit::Outcome::Finished);
        }
    }

    /// Closes the failed dependency's unit with `error`, opening it first if no event came before it failed.
    fn fail(&mut self, error: &InitError) {
        let name = match error {
            InitError::Transfer { dependency, .. } | InitError::UnsupportedPlatform { dependency, .. } => {
                Some(*dependency)
            }
            _ => self.open.as_ref().map(|open| open.name),
        };
        if let Some(name) = name {
            self.open(name);
        }
        let Some(open) = self.open.take() else { return };
        open.span.in_scope(|| {
            let duration: Duration = open.started.elapsed();
            deps::install::record_ended(open.name, &open.version, duration, error);
            let ending = match error.stop_reason() {
                Some(_) => unit::Outcome::Stopped,
                None => unit::Outcome::Failed { kind: error.kind(), error },
            };
            unit::ended(Unit::Install, &open.span, duration, ending);
        });
    }
}

/// The answer of a task spawned while [`Opai`] installs its dependencies, with its panic resumed and a runtime that
/// shut down under it reported as the cancellation it is.
fn joined<T>(joined: Result<Result<T, InitError>, tokio::task::JoinError>) -> Result<T, InitError> {
    match joined {
        Ok(answer) => answer,
        Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
        Err(_) => Err(InitError::from(crate::task::Cancelled)),
    }
}

/// The dependency root under the user's configuration directory, created if it is missing.
///
/// # Errors
///
/// [`InitError::Fs`] if the platform has no configuration directory or the root could not be created.
pub(crate) fn dependency_root() -> Result<PathBuf, InitError> {
    Ok(rust_sak::fs::mk_user_config_dir(folders::ROOT, "")?)
}

/// The version folders of every dependency under `root`, in install order, whether or not they are installed.
pub(crate) fn dependency_dirs(root: &Path) -> Vec<PathBuf> {
    folders::dirs_under(root)
}
