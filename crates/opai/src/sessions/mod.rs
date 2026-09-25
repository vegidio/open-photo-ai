//! Turning a named model artifact into a loaded, runnable ONNX session, and deciding how long it lives.
//!
//! The seam where `models/`, which names a model and owns no resource, and `providers/`, which decides what a model
//! should run on without a runtime present, both meet a live ONNX Runtime. One place turns a model name into
//! something that can run it, so a caching, fallback or installation decision cannot differ between two callers.
//!
//! # A session is freed when nothing is using it, and that is not a rule anything has to follow
//!
//! The reference implementation's sharpest edge is that an ONNX session can be freed while another goroutine is
//! running inference on it — a native use-after-free that terminates the process — and it holds that off with a
//! refcount, an `evicted` flag, a rule that `Destroy` is never called under the lock, and a wait-for-drained on
//! teardown. None of that exists here. The cache holds one [`Arc`] and each user holds another, so the session is
//! dropped when the last one goes and **not before**. Eviction is a map removal, which drops the cache's `Arc` and
//! nothing else: a user holding one keeps a fully usable session, and a request arriving after the removal builds a
//! fresh one rather than resurrecting a dying one.
//!
//! "In use" is therefore [`Arc::strong_count`] rather than a counter something has to maintain, and the failure it
//! cannot have is the dangerous one — a miscount can at worst keep a session resident a sweep longer, never free one
//! that is in use, because freeing is `Drop` and not a decision.
//!
//! The [`Mutex`](std::sync::Mutex) around each session is not a tax this design pays:
//! `ort::session::Session::run` takes `&mut self`, so some exclusion is required whatever else is true, and
//! serializing two runs of one model against one GPU is not a throughput loss. Two *different* models still run
//! concurrently, which is the case that matters.
//!
//! # Two cache directories on disk, and two invalidations
//!
//! Separately from the sessions held in memory, what a provider *compiles* from a model — a TensorRT engine, a CoreML
//! MLProgram — is kept on disk under `engines/`, and there are two of those directories because they have two
//! lifetimes. `engines/<artifact-id>/` belongs to one model, so what was compiled from one model's weights can never
//! be reused against another's; `engines/.timing/` is the TensorRT timing cache, shared by **every** model, because
//! measured kernel timings carry between graphs and a per-model one costs up to 60% of a cold engine build.
//!
//! Each is discarded by the thing that knows it is stale, and neither check is inferred from the other:
//!
//! - **Per model, at install.** Replacing a model's published files empties that model's engine directory, on the
//!   same path that removes the previous version's files and before any new byte is written — so an interruption
//!   cannot leave new weights beside an engine built from the old ones. An install that is already current discards
//!   nothing.
//! - **Wholesale, at initialization.** `engines/.version` records the ONNX Runtime release the tree was compiled
//!   under, and a launch that finds a different tag — or none — empties the tree, timing cache included, before
//!   writing the new one. What a provider compiles is valid only for the runtime that built it. A durable stamp
//!   rather than "the runtime was just replaced", because a process that died between the two would find a matching
//!   runtime on its next start and never discard them.
//!
//! See [`caches`] for the layout and both checks.
//!
//! # A provider that cannot open a model is a downgrade, not a failure
//!
//! A session that cannot be built on the providers resolved for it is rebuilt with nothing attached, and the handle
//! carries what was asked for beside what it was actually built on. That is what the chain folds a run's
//! [`ProviderReport`](crate::ProviderReport) out of, so a downgrade reaches a caller of
//! [`Opai::process`](crate::Opai::process) as data rather than only as a log line. The downgrade is **not latched**:
//! a later request
//! attempts the provider it asks for, unlike the reference implementation, which latches the first such failure for
//! the rest of the process and clears it only when the user changes processor — an action this project has no
//! settings surface for, so a latched downgrade would outlive the condition that caused it. What bounds the cost is
//! that the fallback re-enters the cache under the CPU key rather than building directly, so a repeated request pays
//! one failed provider attach rather than a rebuild.
//!
//! Only a failure about *building* the session is retried. A model file that is missing or unreadable fails the same
//! way on the CPU, and is reported as a model that could not be put on disk.
//!
//! # What this module writes to the log
//!
//! The four the reference writes, at the points that were left standing for them and without any of them moving:
//!
//! - a **build** starting and the session it produced, at `info`, with the artifact, the provider and what it cost —
//!   inside the single flight, so ten concurrent requests for one model produce one pair rather than ten;
//! - a **resident hit**, at `debug`, since a chain takes a handle per pass;
//! - the **downgrade**, at `warn`, naming the artifact, the provider that would not open it and the error, with the
//!   CPU build after it carrying `requested` beside `provider` so the two read as one event;
//! - an **eviction**, at `info`, naming which artifacts were released and how many, and whether it was the idle sweep
//!   or an explicit release. A sweep that found nothing writes nothing: it runs every few minutes for the life of the
//!   process.
//!
//! Each of those facts is *also* still carried as data, and that is not redundancy. The handle reports
//! [`requested`](SessionHandle::requested) beside [`provider`](SessionHandle::provider) and residency is observable
//! through [`SessionCache::resident`], because the log answers a bug report after the fact and a front end has to be
//! able to tell a user something while it is happening.
//!
//! What has **no** record is the memory budget: `internal/budget.go` and `internal/registry.go` emit twelve about
//! per-pool budgets, leases and admission, and this project has not built that mechanism. They arrive with it.

pub(crate) mod build;
pub(crate) mod caches;
mod flight;
#[cfg(test)]
mod live;
mod resident;

pub(crate) use flight::{Install, Interest};
use resident::spawn_sweeper;
pub(crate) use resident::{SessionCache, SessionHandle};

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use crate::deps::ModelTrust;
use crate::deps::model;
#[cfg(test)]
use crate::deps::model::manifest::Listing;
use crate::error::{InitError, SessionError};
use crate::models::ArtifactId;
use crate::providers::options;
use crate::providers::profile::EpProfile;
use crate::providers::{ExecutionProvider, SupportedProviders};
use crate::sessions::build::BuildRequest;
use crate::task::spawn_blocking;
use crate::telemetry::metrics;
use crate::telemetry::unit::{self, Outcome, unit_span};
use tracing::Instrument as _;

/// Where a model's files and the hashes they are verified against come from.
///
/// The one thing a test has to redirect in order to drive a real install: the published listing is resolved at most
/// once per *process*, so a suite that let it be resolved here would hand every test whatever the first one
/// resolved.
enum ModelSource {
    /// The project's published models, with the listing resolved lazily and once per process.
    Published,
    /// An explicit base URL and listing, which is what points a real install at a local server.
    #[cfg(test)]
    At { base_url: String, listing: Listing },
}

/// How a plan and a model file become a session.
///
/// A boxed closure rather than a generic parameter or a `fn` pointer: the production builder is
/// [`build::build`], and a test's has to capture what it is recording.
type Builder<S> = Arc<dyn Fn(BuildRequest) -> Result<S, SessionError> + Send + Sync>;

/// One flight's build: the install, the listing it resolves, and the open.
///
/// A trait object rather than the `async` block's own type, so that proving a run's future `Send` stops here instead
/// of walking the whole transfer stack beneath it. With the unit spans wrapped around each layer, that walk overran
/// the compiler's default recursion limit in a consumer holding an enhancement's future, as `tests/public_api.rs`
/// does.
type BuildFuture<'a, S> = Pin<Box<dyn Future<Output = Result<Option<S>, SessionError>> + Send + 'a>>;

/// Everything one application's session requests share.
///
/// Generic in the session type for the same reason [`SessionCache`] is — the whole of the installation, fallback and
/// caching decision is then drivable against a fake on a runner with no runtime.
pub(crate) struct Sessions<S = ort::session::Session> {
    /// The sessions held in memory.
    cache: Arc<SessionCache<S>>,
    /// The application's configuration directory.
    app_dir: PathBuf,
    /// The application name every directory is resolved from.
    name: String,
    /// What this machine can be asked to run on, as decided during initialization.
    supported: SupportedProviders,
    /// What this process does with a model whose files are already on disk, as its initialization declared.
    ///
    /// Here because this is what owns the one path that installs a model, so the rule is read off the thing that
    /// performs the install rather than fetched from the application handle at each call.
    trust: ModelTrust,
    /// Where the model files come from.
    models: ModelSource,
    /// How a plan and a file become a session.
    build: Builder<S>,
}

impl<S> std::fmt::Debug for Sessions<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sessions")
            .field("cache", &self.cache)
            .field("app_dir", &self.app_dir)
            .field("supported", &self.supported)
            .finish_non_exhaustive()
    }
}

impl Sessions {
    /// The sessions of an application installed at `app_dir` under `name`, on a machine reporting `supported`,
    /// installing models under `trust`.
    ///
    /// Starts the background sweep, which is why this is only ever called from inside an asynchronous runtime.
    pub(crate) fn new(app_dir: PathBuf, name: String, supported: SupportedProviders, trust: ModelTrust) -> Self {
        Self::with(app_dir, name, supported, trust, ModelSource::Published, Arc::new(build::build))
    }
}

impl<S: Send + 'static> Sessions<S> {
    /// The body of [`Sessions::new`], against an explicit model source and builder.
    fn with(
        app_dir: PathBuf,
        name: String,
        supported: SupportedProviders,
        trust: ModelTrust,
        models: ModelSource,
        build: Builder<S>,
    ) -> Self {
        let cache = Arc::new(SessionCache::default());
        spawn_sweeper(&cache);

        Self { cache, app_dir, name, supported, trust, models, build }
    }

    /// Sessions installing from `base_url` against `listing` and building through `build`, for a suite above this
    /// layer that needs the real request, flight and install around a fake session.
    #[cfg(test)]
    pub(crate) fn serving(
        app_dir: PathBuf,
        supported: SupportedProviders,
        base_url: String,
        listing: Listing,
        build: Builder<S>,
    ) -> Self {
        let models = ModelSource::At { base_url, listing };

        Self::with(app_dir, "opai-test".to_string(), supported, ModelTrust::Published, models, build)
    }

    /// The sessions held in memory, for whoever has to release them or report on them.
    pub(crate) fn cache(&self) -> &SessionCache<S> {
        &self.cache
    }

    /// The session for `artifact`, installing its files first if they are not on disk.
    ///
    /// One entry point, so "a model that is named is a model that can be run" is true at one call site rather than at
    /// every caller. The install happens **inside** the single-flighted build, so ten concurrent requests for a model
    /// that is not on disk perform one install rather than ten — and a request served from the cache does no
    /// filesystem work at all, which matters because the already-current check is one `symlink_metadata` per recorded
    /// file.
    ///
    /// `interest` is what this request brings to that install: where its progress goes, and how long it wants the
    /// install at all. It is told about the transfer whether it started one or merely joined one — a transfer of
    /// gigabytes is the longest part of a first run, and a request handed one already in flight would otherwise sit
    /// through it in silence — and its cancellation withdraws it, which stops the transfer where it was the last
    /// request waiting. See [`SessionCache::get_or_build`].
    ///
    /// `profile` is a parameter rather than looked up from `artifact`, because it is the operation that declares one
    /// and the caller is what holds the operation. That one artifact implies one profile is what the cache key rests
    /// on, and is checked by a test below rather than left to a comment.
    ///
    /// Where the resolved providers cannot open the model, it is built again with nothing attached and the handle
    /// reports both. A missing or unreadable model file is not retried: it fails the same way on the CPU.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Install`] where the model's files could not be put on disk or read back, and
    /// [`SessionError::Build`] where the model is on disk and neither the resolved providers nor the CPU could open
    /// it. A build stopped because nothing was left waiting for it is reported as [`InitError::Stopped`], which the
    /// run layer folds into its own cancellation rather than into a failed install — nothing broke, and nobody was
    /// asking any more.
    pub(crate) async fn session(
        &self,
        artifact: &ArtifactId,
        profile: &EpProfile,
        requested: ExecutionProvider,
        interest: &Interest,
    ) -> Result<SessionHandle<S>, SessionError> {
        // One per request, around the fallback too. It has no record of its own, so it ends with what it returns;
        // `resident` is recorded by the cache, which is what knows.
        let span = unit_span!(
            "session_request",
            artifact = %artifact,
            requested = %requested,
            resident = tracing::field::Empty
        );
        let outcome = self.request(artifact, profile, requested, interest).instrument(span.clone()).await;

        let ending = match &outcome {
            Ok(_) => Outcome::Finished,
            Err(SessionError::Install(install)) if install.stop_reason().is_some() => Outcome::Stopped,
            Err(error) => Outcome::Failed { kind: error.kind(), error },
        };
        unit::mark(&span, &ending);

        outcome
    }

    /// [`session`](Self::session)'s body, inside the request's span.
    async fn request(
        &self,
        artifact: &ArtifactId,
        profile: &EpProfile,
        requested: ExecutionProvider,
        interest: &Interest,
    ) -> Result<SessionHandle<S>, SessionError> {
        let outcome = self.build_on(artifact, profile, requested, requested, interest).await;

        match outcome {
            // A stop, which is not a failure and not a downgrade: the transfer this request was waiting on ended
            // because nothing was left waiting for it, and attempting the CPU would be starting the same install
            // again on behalf of a request that has already gone away.
            Ok(None) => Err(SessionError::from(InitError::Stopped)),
            Ok(Some(handle)) => Ok(handle),
            // The downgrade, and it re-enters the cache rather than building a CPU session directly. That is what
            // makes carrying no latch affordable: without it a machine with a broken driver would rebuild the model
            // from scratch on every request; with it, the second request pays one failed provider attach and then
            // hits the CPU session the first one filed.
            //
            // Only a failure about building the session. A model file that is missing or unreadable reports
            // `Install`, fails the same way on the CPU, and retrying it would double the wait before reporting what
            // was already known.
            // Matched on the provider the failure *carries* rather than on a second resolution of the request. The
            // error names what the plan actually attached (`sessions::build` sets it from `plan.resolved`), so the
            // retry is decided by what was tried; re-resolving here would agree only for as long as nothing else can
            // influence a plan, and the day one can — a per-artifact override, a provider blacklist — the retry
            // would fire against a provider that was never attempted.
            Err(SessionError::Build { provider, .. }) if provider != ExecutionProvider::Cpu => {
                // **The record this whole instrumentation sweep is most for.** A downgrade is invisible to a user —
                // the enhancement still completes, several times slower — so a bug report about speed has nothing to
                // carry unless this line is in the file. `warn`, by the rule: the application is continuing, and the
                // user is paying for something they did not ask for.
                //
                // The `info` for the CPU session that follows carries `requested` beside `provider`, so the two lines
                // read as one event. No `error`: the failed build recorded it, just before this, in the same request.
                tracing::warn!(
                    %artifact,
                    provider = %provider,
                    "the requested execution provider could not open this model; falling back to the CPU"
                );
                // By the provider that was attempted, which is what the request resolved to: an `Auto` request
                // counted as `Auto` would not say which accelerator declined.
                metrics::PROVIDER_FALLBACKS.add_with_tags(1, &[("requested", provider.as_str())]);

                match self.build_on(artifact, profile, requested, ExecutionProvider::Cpu, interest).await? {
                    Some(handle) => Ok(handle),
                    None => Err(SessionError::from(InitError::Stopped)),
                }
            }
            Err(other) => Err(other),
        }
    }

    /// The session for `artifact` under the plan `plan_from` resolves to, reporting `requested` on the handle.
    ///
    /// The two providers are separate because they differ on the retry: the handle still reports what the *caller*
    /// asked for, while the plan — and therefore the key — is built from the CPU.
    async fn build_on(
        &self,
        artifact: &ArtifactId,
        profile: &EpProfile,
        requested: ExecutionProvider,
        plan_from: ExecutionProvider,
        interest: &Interest,
    ) -> Result<Option<SessionHandle<S>>, SessionError> {
        // Keyed on what the machine will actually serve, not on what was asked for: filing a CPU session under a GPU
        // key would make an explicit switch to the CPU build a second identical copy of it.
        let resolved = options::resolve_chain(plan_from, self.supported).resolved;

        self.cache
            .get_or_build(artifact, requested, resolved, interest, |installing| -> BuildFuture<'_, S> {
                Box::pin(async move {
                    // The flight's install rather than this request's: it is performed inside the single flight, so what
                    // it reports goes to every request waiting on that flight, and what stops it is all of them leaving
                    // rather than any one of them.
                    let Some(dir) = self.install(artifact, &installing).await? else {
                        return Ok(None);
                    };

                    let paths = caches::resolve(&self.app_dir, &self.name, artifact)?;
                    let plan = options::resolve(plan_from, self.supported, profile, &paths);

                    // The graph is the artifact's own name, the same rule the descriptor composes a model's version from;
                    // a model too large for the protobuf limit keeps its weights in a sibling the runtime opens itself.
                    let model = dir.join(format!("{artifact}.onnx"));

                    // Not stopped once it is under way, and that is deliberate: opening a model is a compile that can run
                    // to minutes, it is what a later run would have to pay all over again, and there is nothing to
                    // interrupt it at. What a stop saves is the transfer, which is where the gigabytes and the minutes
                    // of waiting actually are.
                    self.open(BuildRequest { artifact: artifact.clone(), plan, paths, model }).await.map(Some)
                })
            })
            .await
    }

    /// Installs `artifact`'s files and returns the directory they landed in, or `None` where the transfer stopped
    /// because nothing was left waiting for it.
    ///
    /// # A stopped transfer is resumed for a request that arrives while it is stopping
    ///
    /// The loop is the whole of that, and it is what keeps "stop it when nobody wants it" from becoming "fail the
    /// next request that does". A stop and a fresh request race by construction — the window's own cleanup and its
    /// next request cross the boundary independently, and the one that displaced the run is exactly the one about to
    /// ask for the same model — so the attempt that stopped asks whether anyone has joined since, and resumes onto
    /// the partial for them if so. `Install::attempt` hands out the token as it stands *now*, which is the fresh one
    /// that request re-armed as it joined.
    ///
    /// Only the transfer is repeated. The bookkeeping either side of it is milliseconds, and a resumed attempt
    /// continues from the bytes already on disk rather than from nothing.
    ///
    /// # Errors
    ///
    /// As [`model::install`], except that [`InitError::Stopped`] is answered as `None` rather than returned: it is
    /// not a failure, and this is the layer that knows it was asked for.
    async fn install(&self, artifact: &ArtifactId, install: &Install) -> Result<Option<PathBuf>, InitError> {
        loop {
            let installing = install.attempt();

            let outcome = match &self.models {
                ModelSource::Published => {
                    model::install(&self.app_dir, &self.name, artifact, self.trust, &installing).await
                }
                #[cfg(test)]
                ModelSource::At { base_url, listing } => {
                    model::install_from(base_url, listing, &self.app_dir, &self.name, artifact, self.trust, &installing)
                        .await
                }
            };

            return match outcome {
                Err(InitError::Stopped) if install.wanted() => {
                    // `debug`: it is the ordinary consequence of a user changing a setting mid-download, and it costs
                    // one range request rather than anything a reader needs warning about.
                    tracing::debug!(%artifact, "a stopped transfer is being resumed for a request that joined since");
                    continue;
                }
                Err(InitError::Stopped) => Ok(None),
                other => other.map(Some),
            };
        }
    }

    /// Runs one build off the asynchronous runtime.
    ///
    /// Opening a graph is anything from a fifth of a second to several minutes of engine compilation, and in a Tauri
    /// process the asynchronous runtime is shared with the window and the IPC plumbing — so this goes to a blocking
    /// thread, as the archive expansion and the runtime load already do.
    ///
    /// # Errors
    ///
    /// As the builder, plus [`SessionError::Install`] carrying [`InitError::Cancelled`] where the runtime shut down
    /// before the build could run.
    async fn open(&self, request: BuildRequest) -> Result<S, SessionError> {
        let build = Arc::clone(&self.build);

        spawn_blocking::<_, SessionError, _>(move || build(request)).await?
    }
}

#[cfg(test)]
mod tests;
