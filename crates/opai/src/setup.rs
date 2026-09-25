//! What an initialization installs: choosing the dependencies this machine can use, and putting them on disk.

use std::path::PathBuf;
use std::sync::Arc;

use rust_sak::sysinfo::GpuInfo;
use tokio_util::sync::CancellationToken;
use tracing::Instrument as _;

use crate::cache::{self, CacheMode, Store};
use crate::deps::artifact::{CUDA, CUDNN, ONNX_RUNTIME, TENSORRT};
use crate::deps::release::Dependency as Descriptor;
use crate::deps::{self, ModelTrust};
use crate::error::InitError;
use crate::progress::{OnPlan, OnProgress, PlannedDependency, Reporter};
use crate::providers::{Provider, SupportedProviders};
use crate::sessions::Sessions;
use crate::task::spawn_blocking;
use crate::{Inner, Opai, config, gpu, instance};

/// What one machine's initialization will install, and everything derivable from that.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    // A named field rather than `plan[0]`: the runtime is always installed and always loaded from here, so
    // naming it removes a positional invariant that spanned two functions.
    /// The ONNX Runtime for this platform. Always present — an unpublished platform fails the selection.
    pub(crate) runtime: Descriptor,
    /// The GPU libraries this machine's hardware and platform actually offer, in install order.
    pub(crate) gpu: Vec<Descriptor>,
}

/// Where an install put each of [`Plan`]'s dependencies.
#[derive(Debug, Clone)]
pub(crate) struct Installed {
    /// The directory the ONNX Runtime was installed into, which
    /// [`runtime::library_path`](crate::runtime::library_path) resolves against.
    pub(crate) runtime: PathBuf,
    /// The directories the GPU libraries were installed into, in install order.
    #[expect(
        dead_code,
        reason = "the GPU directories have no reader until a caller needs a path the install wrote"
    )]
    gpu: Vec<PathBuf>,
}

impl Plan {
    /// Every dependency to install, in install order: the runtime first, then the GPU libraries in pinned-table
    /// order.
    pub(crate) fn all(&self) -> impl Iterator<Item = &Descriptor> {
        std::iter::once(&self.runtime).chain(&self.gpu)
    }

    /// A plan that installs `runtime` and nothing else, which is what a machine with no NVIDIA adapter gets.
    #[cfg(test)]
    pub(crate) fn runtime_only(runtime: Descriptor) -> Self {
        Self { runtime, gpu: Vec::new() }
    }

    /// The rows a plan recipient is handed: every dependency this machine will account for, in install order,
    /// each carrying its total published size.
    ///
    /// Deliberately doesn't say which are already installed — that arrives moments later via the progress
    /// report. Reads nothing from disk, so a launch with nothing to do isn't doubly expensive.
    pub(crate) fn rows(&self) -> Vec<PlannedDependency> {
        self.all().map(PlannedDependency::for_dependency).collect()
    }

    /// What this machine can be asked to run on, derived from what is actually in the plan.
    pub(crate) fn providers(&self) -> SupportedProviders {
        // Folded from the rows so "a provider is claimed only where its library is installed" is structural,
        // not something tests check two hand-maintained things agree on.
        self.gpu
            .iter()
            .filter_map(|descriptor| descriptor.provides)
            .fold(SupportedProviders::detect(), SupportedProviders::with)
    }
}

impl Opai {
    /// Decides what to install on a machine with `adapters`, and what the resulting provider report says.
    ///
    /// A pure function of the adapter list and platform, testable without real GPU hardware.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::UnsupportedPlatform`] only when no **runtime** is published for this platform — an
    /// unpublished GPU library is reported unsupported rather than an error, since it's an optimisation the
    /// user was never going to get anyway.
    pub(crate) fn select(adapters: &[GpuInfo], os: &'static str, arch: &'static str) -> Result<Plan, InitError> {
        Self::select_at(deps::release::RELEASE_BASE_URL, adapters, os, arch)
    }

    /// The testable half of [`Opai::select`], against an explicit base URL so the suite can point a real install at a
    /// local server.
    ///
    /// # Errors
    ///
    /// As [`Opai::select`].
    pub(crate) fn select_at(
        base_url: &str,
        adapters: &[GpuInfo],
        os: &'static str,
        arch: &'static str,
    ) -> Result<Plan, InitError> {
        let runtime = Descriptor::from_release_at(base_url, &ONNX_RUNTIME, os, arch)?;
        let mut gpu = Vec::new();

        // `.ok()` is the whole non-offer rule: no archive for this platform means it never joins the plan, and
        // so its provider is never claimed.
        let published = |release| Descriptor::from_release_at(base_url, release, os, arch).ok();

        if adapters.iter().any(gpu::is_nvidia)
            && let (Some(cuda), Some(cudnn)) = (published(&CUDA), published(&CUDNN))
        {
            // cuDNN after CUDA: it links against the CUDA runtime.
            gpu.push(cuda);
            gpu.push(cudnn);
        }

        // TensorRT is built on CUDA, so it never installs without it — the condition below is read off the CUDA
        // row already being in the plan, not a separate flag.
        if gpu.iter().any(|descriptor| descriptor.provides == Some(Provider::Cuda))
            && adapters.iter().any(gpu::is_tensorrt_capable)
            && let Some(tensorrt) = published(&TENSORRT)
        {
            // Last: at 1.4-2 GB, larger than the other three combined, so an interrupted first launch already
            // has the two that unlock CUDA.
            gpu.push(tensorrt);
        } else if adapters.iter().any(gpu::is_nvidia) && !adapters.iter().any(gpu::is_tensorrt_capable) {
            // Worth a record: from outside, "CUDA but no TensorRT" looks identical to a failed TensorRT install.
            // The pinned release needs compute capability 7.5 (Turing+, the RTX brand), so a GTX card correctly
            // gets CUDA only.
            let cards: Vec<&str> =
                adapters.iter().filter(|gpu| gpu::is_nvidia(gpu)).map(|gpu| gpu.name.as_str()).collect();

            tracing::info!(
                adapters = %cards.join(","),
                tensorrt = TENSORRT.name,
                "no adapter is new enough for the pinned TensorRT release; this machine runs on CUDA"
            );
        }

        Ok(Plan { runtime, gpu })
    }

    /// Hands `on_plan` what this initialization is about to install, once, and does nothing at all where none is
    /// registered.
    ///
    /// A function of its own so the suite can drive this exact call sequence — `initialize` itself loads a real
    /// runtime, which a test can't redirect.
    pub(crate) fn report_plan(plan: &Plan, on_plan: Option<&OnPlan>) {
        let Some(on_plan) = on_plan else {
            return;
        };

        on_plan(&plan.rows());
    }

    /// The body of [`Opai::initialize`], against an explicit application directory and install plan.
    ///
    /// Separated so the suite can drive a real install into a temporary directory against a local server.
    ///
    /// `claim` is taken by value, not acquired here: an install literally can't be reached without a claim
    /// already in hand, for any caller.
    ///
    /// # Errors
    ///
    /// As [`Opai::initialize`], less the selection's own.
    pub(crate) async fn install(
        name: &str,
        app_dir: PathBuf,
        claim: instance::Claim,
        plan: Plan,
        models: ModelTrust,
        on_progress: Option<OnProgress>,
    ) -> Result<(Self, Installed), InitError> {
        let providers = plan.providers();

        // Opened while the dependencies install rather than after them: the two share nothing, and a disk store's
        // open can wait out a stale lock and sweep yesterday's expired entries. Spawned so it starts now rather than
        // when it is first awaited, and on a blocking thread for the same reason. A cache that won't open still never
        // fails an otherwise-healthy launch — it degrades to memory, then to none, never an error — and one whose
        // installs fail is simply dropped.
        let cache_root = app_dir.clone();
        let opening =
            tokio::spawn(spawn_blocking::<_, InitError, _>(move || Store::open(&cache_root)).in_current_span());

        // Each dependency installs into the directory its own row names — never a shared directory that a
        // later bump could delete out from under another dependency.
        let dirs = plan
            .all()
            .map(|descriptor| config::sub_dir(&app_dir, name, &descriptor.dir))
            .collect::<Result<Vec<_>, _>>()?;

        // Whether each is already current, asked of every dependency at once: on a launch with nothing to install
        // this is the whole of the work, a stat per recorded file of a CUDA or TensorRT tree, and the answers do not
        // depend on one another. The installs themselves stay serial below, in plan order — cuDNN links against the
        // CUDA runtime, and progress is reported in the order the plan announced.
        let checks: Vec<_> = plan
            .all()
            .zip(&dirs)
            .map(|(descriptor, dir)| {
                let (descriptor, dir) = (descriptor.clone(), dir.clone());
                tokio::spawn(async move { deps::install::check(&dir, &descriptor).await }.in_current_span())
            })
            .collect();

        let mut installed = Vec::with_capacity(dirs.len());
        for ((descriptor, dir), check) in plan.all().zip(dirs).zip(checks) {
            let checked = joined(check.await)?;

            // One reporter per dependency, so each ends cleanly on `1.0` rather than one composite figure.
            let reporter = Arc::new(Reporter::for_dependency(descriptor, on_progress.clone()));

            // The one terminal report for a dependency with nothing to do, emitted at the point it would
            // otherwise install — so observed order matches install order regardless. Never cancelled: nothing
            // stops initialization today, only a run's own model install. Runtime first, since the plan lists it
            // first — the GPU libraries are useless without it.
            let outcome = deps::install::install_checked(
                &dir,
                descriptor,
                Some(checked),
                Arc::clone(&reporter),
                &CancellationToken::new(),
            )
            .await?;

            if outcome == deps::install::Outcome::AlreadyCurrent {
                reporter.already_installed();
            }

            installed.push(dir);
        }

        let mut installed = installed.into_iter();
        let runtime_dir = installed.next().expect("the plan always carries the runtime");
        let gpu_dirs: Vec<PathBuf> = installed.collect();

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
        let sessions = Sessions::new(app_dir.clone(), name.to_string(), providers, models);
        let inner = Inner { name: name.to_string(), config_dir: app_dir, claim, providers, models, sessions, cache };
        let opai = Self { inner: Arc::new(inner) };

        // Returned alongside the handle so a caller uses the path actually installed to, rather than
        // re-resolving it (and possibly disagreeing).
        let installed = Installed { runtime: runtime_dir, gpu: gpu_dirs };

        Ok((opai, installed))
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
