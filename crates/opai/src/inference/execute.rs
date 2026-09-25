//! Running one operation whose result is not an image: what it reports, what stops it, and what comes back beside the
//! result.

// `process`'s sibling, and deliberately a file rather than a function inside it. What the two share is the
// acquisition, and that is shared as `acquire` rather than by either driver reaching into the other.
//
// No second store, and no encoding decided here. The `Memo` is the one the application already opened, and both ends
// go through `RunCache` — so the entry lifetime, the failure rules and the guarantee that a picture entry and a result
// entry cannot be read as one another are `cache`'s, stated once.
//
// A deliberate divergence from the reference, whose `Execute[T]` caches nothing: it selects the model, runs it and
// returns, with no store read or write anywhere in the function — only `Process` touches the cache. The parity target
// is the user-visible result, which is unchanged; what changes is that the second identical analysis is instant
// rather than paying again for a session it already paid for. A reader comparing the two files finds the caching here
// and nothing there: do not remove it as an accident.
//
// No identity composition, and no chain. No new picture is produced, so there is nothing to name — the source's own
// identity is what the store's key is folded from, and it is unchanged by the run. And one operation, because
// chaining is a property of pixels: each enhancement is applied to the picture the one before it produced, and an
// analysis of an image is a single question asked of it.

use std::sync::Arc;

use rust_sak::memo::Memo;
use tokio_util::sync::CancellationToken;

use super::acquire;
use super::process::{ProviderReport, identity_of_data};
use super::progress::{ChainProgress, OnInference};
use crate::cache::RunCache;
use crate::error::InferenceError;
use crate::image::Picture;
use crate::models::DataOperation;
use crate::pipeline;
use crate::pipeline::Backend;
use crate::providers::ExecutionProvider;
use crate::sessions::SessionHandle;
use crate::task::spawn_blocking;
use crate::telemetry::metrics;
use crate::telemetry::unit::{self, Outcome, Unit, unit_span};
use tracing::Instrument as _;

/// Everything about a non-image run that has a sensible default.
///
/// `..Default::default()` is the documented way to set one field:
///
/// ```
/// # use opai::{ExecuteOptions, ExecutionProvider};
/// let options = ExecuteOptions { provider: ExecutionProvider::Cpu, ..Default::default() };
/// ```
///
/// Written that way, a field added later is source-compatible for every caller.
#[derive(Clone)]
pub struct ExecuteOptions {
    // `ProcessOptions`' counterpart, and deliberately **not** a subset of it. `depth` is the field that would be
    // meaningless here — it names bits per channel of pixels this run does not produce — and a field a caller can set
    // that does nothing is a guarantee they can get wrong. Deliberately **not** `#[non_exhaustive]`, for the reason
    // written into `ProcessOptions`.
    /// What to run on. Defaults to [`ExecutionProvider::Auto`], which resolves to the best this machine supports and
    /// falls back to the CPU where a provider cannot open the model.
    pub provider: ExecutionProvider,

    /// Where progress reports go. Defaults to none, and a caller that asks for none is charged for none.
    pub on_progress: Option<OnInference>,

    /// The token that stops the run. Defaults to a fresh one nobody holds, so a caller with no cancel button does not
    /// have to make one.
    ///
    /// Borrowed from the caller in spirit though owned here: pass a clone of the token you kept, and cancelling yours
    /// cancels this run.
    pub cancel: CancellationToken,

    // `false` serves the caller that needs it rather than merely prefers it: a benchmark measuring what a detection
    // costs would otherwise measure a read, and a measurement that silently stored its result would change what the
    // next measurement measured.
    /// Whether this run may read from and write to the store. Defaults to `true`.
    ///
    /// `false` executes the model, returns exactly what a served run would have returned, and leaves **nothing**
    /// behind.
    ///
    /// **Decided once for the whole run**, not separately for the read and the write, so a run cannot be served from
    /// the store and then decline to keep what it produced, or the reverse. It is the same store
    /// [`ProcessOptions::cache`](super::process::ProcessOptions::cache) governs.
    ///
    /// A different question from [`Opai::cache_mode`](crate::Opai::cache_mode), which reports what is *backing* the
    /// store: a run with this set to `false` still reads `Disk` there.
    pub cache: bool,
}

impl std::fmt::Debug for ExecuteOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Written out rather than derived, as `ProcessOptions`'s is: a callback has nothing to print, and the fact a
        // reader actually wants is whether one was registered at all.
        f.debug_struct("ExecuteOptions")
            .field("provider", &self.provider)
            .field("on_progress", &self.on_progress.is_some())
            .field("cancel", &self.cancel)
            .field("cache", &self.cache)
            .finish()
    }
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        Self {
            provider: ExecutionProvider::Auto,
            on_progress: None,
            cancel: CancellationToken::new(),
            // On, as `ProcessOptions`' is and for the same reason.
            cache: true,
        }
    }
}

/// What a non-image run produced: the result, and what it was executed on.
///
/// [`Enhanced`](super::process::Enhanced)'s sibling. The provider report is on the **result** rather than on the
/// progress stream, so a caller that registered no callback still receives it; see [`ProviderReport`].
#[derive(Debug, Clone)]
pub struct Executed<T> {
    // The reference's `Execute[T any]` returns a bare `T`; this is additive over it.
    //
    // Deliberately **not** `#[non_exhaustive]`: the attribute would bar an external crate from destructuring this at
    // all, including the `let Executed { value, .. } = ..` form that is how a caller takes the half it wants.
    /// What the run found — a [`Faces`](crate::Faces) for a detection run, and whatever a later family produces.
    ///
    /// The operation's own type rather than the caller's to choose: see [`DataOperation`].
    pub value: T,

    /// What the run was asked to execute on, beside what it actually executed on.
    pub providers: ProviderReport,
}

/// Runs `operation` over `source` and hands back what it found beside what it ran on.
///
/// The body of [`Opai::execute`](crate::Opai::execute).
///
/// **Every session is held until this returns**, so a model this run holds cannot be reclaimed by the idle sweep, by
/// an explicit release, or by a provider switch.
///
/// # Errors
///
/// [`InferenceError`], and in every case **no result at all** — not a partial one, and not an empty one standing in
/// for the answer.
pub(crate) async fn execute<B: Backend, O: DataOperation>(
    backend: &B,
    cache: Option<&Memo>,
    source: &Picture,
    operation: &O,
    options: Option<ExecuteOptions>,
) -> Result<Executed<O::Output>, InferenceError> {
    // Generic in the backend so that everything this decides — the defaulting of the options, the progress weighting,
    // the cancellation, the session-holding, the provider report — is checked against a fake session on a runner with
    // no runtime.
    let options = options.unwrap_or_default();

    let cache = RunCache::for_run(cache, options.cache, source);

    // The run as everything that reports speaks it. One value, read by the records below and by the progress label,
    // so the two cannot name the run differently.
    let named = operation.as_subject();

    // The pipeline before anything is installed, which is what keeps this refusal structural: a run that reaches the
    // acquisition below has already been judged servable. Recorded here, before the opening record, so a refusal is
    // one record rather than an analysis that began and failed.
    let pipeline = operation.pipeline::<B>().inspect_err(|error| {
        tracing::warn!(id = %named.cache_tag(), %error, "analysis refused");
    })?;

    let span = unit_span!(
        "analysis",
        id = %named.cache_tag(),
        identity = source.identity(),
        provider = %options.provider
    );

    // The same fields `process::process` writes, so one log reads the same for both paths and one `grep` over an
    // operation's id returns every run that touched it — the identity rather than the path, for the reason given there.
    span.in_scope(|| {
        tracing::info!(
            id = %named.cache_tag(),
            identity = source.identity(),
            provider = %options.provider,
            "analysis started"
        );
    });

    let started = std::time::Instant::now();

    let outcome = run::<B, O>(
        backend,
        source,
        pipeline,
        &named,
        options.provider,
        options.on_progress,
        &options.cancel,
        cache.as_ref(),
    )
    .instrument(span.clone())
    .await;

    // Recorded before the `?`, so that a run which failed says so rather than leaving a reader to infer it from a
    // beginning with no end.
    let duration = started.elapsed();
    let identity = source.identity();
    span.in_scope(|| match &outcome {
        Ok(_) => {
            tracing::info!(id = %named.cache_tag(), identity, ?duration, "analysis finished");
            unit::ended(Unit::Analysis, &span, duration, Outcome::Finished);
        }
        Err(error) => match error.stop_reason() {
            Some(reason) => {
                tracing::info!(id = %named.cache_tag(), identity, reason, ?duration, "analysis stopped");
                unit::ended(Unit::Analysis, &span, duration, Outcome::Stopped);
            }
            // `warn` rather than `error`: the failure is returned to the caller, and whoever decides it is fatal is
            // the front end.
            None => {
                tracing::warn!(id = %named.cache_tag(), identity, ?duration, %error, "analysis failed");
                unit::ended(Unit::Analysis, &span, duration, Outcome::Failed { kind: error.kind(), error });
            }
        },
    });

    let (value, providers) = outcome?;

    Ok(Executed { value, providers })
}

/// [`execute`]'s body, once the options are resolved and the log bracket is open.
///
/// `cache` is the store this run reads and writes, already decided by [`execute`] for the whole call — see
/// [`RunCache::for_run`]. A run already cancelled is handed neither a transfer nor a stored result. A stored result is
/// served with a [`Stage::Cached`](super::progress::Stage::Cached) report and holds no session at all; otherwise the
/// model runs, and only a successful run is stored.
#[expect(
    clippy::too_many_arguments,
    reason = "each is one independent decision about the run; bundling them would build a struct whose only reader is               the next line"
)]
async fn run<B: Backend, O: DataOperation>(
    backend: &B,
    source: &Picture,
    pipeline: pipeline::SharedData<B, O::Output>,
    operation: &crate::models::Subject,
    provider: ExecutionProvider,
    on_progress: Option<OnInference>,
    cancel: &CancellationToken,
    cache: Option<&RunCache>,
) -> Result<(O::Output, ProviderReport), InferenceError> {
    // Its own function so `execute`'s bracket has exactly one outcome to record, which is what keeps a failed run's
    // closing record on the same path as a successful one's.
    //
    // A one-operation chain is exactly what this is, so the install/run split, the monotonic clamp and the property
    // that a model already on disk does not give up the head of its range all come for free and are already tested.
    let progress = ChainProgress::new(on_progress, 1);
    let reporter = Arc::new(progress.operation(0, operation.clone()));

    // Checked before anything is installed **and before the lookup**: a caller that stopped the run must not first
    // pay for a transfer, and must not be handed a stored result as though the run had completed either.
    if cancel.is_cancelled() {
        return Err(InferenceError::Cancelled);
    }

    // The store and the slot this run's result lives in, derived once and carried as one value. A pair rather than two
    // `Option`s held side by side: the key exists exactly when the store does, so two options would be a pairing
    // re-established by a tuple match at each of the three places that need it, and a state that cannot happen would
    // still have to be written out.
    let store = cache.map(|cache| (cache.clone(), identity_of_data(cache.source(), operation)));

    // Above `acquire`, and that placement is the load-bearing part: a hit transfers nothing, opens no session
    // and holds no handle. What a repeated analysis actually pays for is putting the model into memory, which is
    // seconds; the arithmetic the model then performs is milliseconds. A lookup made after the model was open would
    // save the cheaper half and pay the expensive one, which is an optimisation that measures well and does nothing.
    if let Some((cache, key)) = &store {
        // On a blocking thread, as the chain's read is: a store backed by disk is I/O, and in a Tauri process the
        // async runtime is the window's event loop.
        let reading = cache.clone();
        let wanted = key.clone();
        let stored = spawn_blocking::<_, InferenceError, _>(move || reading.get_value::<O::Output>(&wanted)).await?;
        metrics::cache_lookup(metrics::ANALYSIS, stored.is_some());

        if let Some(value) = stored {
            // `debug`, matching the chain's per-step record: a front end asking the same question repeatedly would
            // otherwise fill a user's file with it.
            tracing::debug!(operation = %operation.cache_tag(), key, "analysis served from the store");

            // Its full share of the bar, through the same clamped path every other report goes through — so a run
            // that executed nothing still ends on exactly 1 rather than never reporting at all.
            reporter.cached();

            // The same fold the miss path uses, over no handles at all. `ProviderVerdict::NothingExecuted` then falls
            // out of `ProviderReport`'s own logic rather than being a second place that decides what a served run
            // reports.
            //
            // Holding no handle is correct: a run that did no inference has no claim on a session, so a served run
            // neither keeps a model resident nor prevents the idle sweep reclaiming one.
            return Ok((value, ProviderReport::new(provider, std::iter::empty())));
        }
    }

    // The same block the chain runs, over the head of this operation's range. See `acquire`.
    let sessions = acquire::acquire(backend, pipeline.as_ref(), &reporter, provider, cancel).await?;

    let input = source.shared_pixels();
    let cancelled = cancel.clone();
    let reported = Arc::clone(&reporter);
    let wanted = progress.wanted();

    // Moved into the closure below — the same pair the read above used, so the two ends cannot disagree about where
    // this result lives. Cloned rather than moved only so the record at the tail can name the slot a computed result
    // went into, which is what lets one `grep` pair a miss with the hit it later serves.
    let keeping = store.clone();

    // Acquired on the runtime and run off it, exactly as the chain does and for the same reason: in a Tauri process the
    // async runtime *is* the window's event loop. The hop is written out here rather than shared with the chain because
    // the two closures have nothing in common but their shape — the chain's moves whole decoded pictures and encodes
    // one back, this one moves a value and serialises one.
    //
    // The sessions travel into the blocking closure and back out of it, so they are owned throughout the run and are
    // held until this function returns — there is no window in which one could be found reclaimed. The error type is
    // named rather than inferred: `Cancelled` converts into three of this crate's errors, and which one a shutdown is
    // reported as is the choice being made here.
    let (held, produced) = spawn_blocking::<_, InferenceError, _>(move || {
        let report = |fraction: f64| reported.running(fraction);
        let report = wanted.then_some(&report as &dyn Fn(f64));
        let stopped = || cancelled.is_cancelled();

        let produced = pipeline.run(&input, &sessions, report, &stopped);

        // Inside the same blocking call the run happened on, as the chain's write is: encoding and storing is I/O and
        // belongs nowhere near the runtime, and there is a blocking thread here already.
        //
        // **Only a successful run writes.** A failed one and a cancelled one both arrive here as an `Err` — the
        // pipeline returns the error rather than a value — so neither has a result to store, and a later run is never
        // served something that was never produced. Nothing checks the token a second time to get that.
        if let (Some((cache, key)), Ok(value)) = (&keeping, &produced) {
            cache.put_value(key, value);
        }

        (sessions, produced)
    })
    .await?;

    let value = produced?;

    // The counterpart of the hit above, at the same level and with the same fields, so one `grep` over an operation's
    // id returns every analysis of every run that touched it and says which way each went — and so a store that is
    // quietly caching nothing, which looks exactly like one that is working, is something a reader of `opai.log` can
    // find. Written after the `?`, so a run that failed says *that* through the bracket's `warn` rather than claiming
    // to have computed something.
    //
    // `key` is absent where the run was asked not to use the store: there is no slot, and a field naming one would be
    // inventing it. The record is still written, because what has to be visible is that the model ran.
    tracing::debug!(
        operation = %operation.cache_tag(),
        key = store.as_ref().map(|(_, key)| key.as_str()),
        "analysis computed"
    );

    // Folded out of the handles the run was already holding, rather than accumulated by a line added to the
    // acquisition: `held` carries every session this run took, each stamped with the provider it was built on, so the
    // report costs no plumbing at all. An empty `held` reports that nothing was executed rather than claiming the
    // requested provider — the distinction `ProviderReport::actual` exists to keep.
    let report = ProviderReport::new(provider, held.iter().map(SessionHandle::provider));

    Ok((value, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::test_support::stub_backend_runs;

    use std::sync::Mutex;

    use image::DynamicImage;
    use rust_sak::memo::{CacheOpts, Memo};

    use crate::error::SessionError;
    use crate::inference::progress::{InferenceProgress, Stage};
    use crate::inference::test_support::assert_never_decreases;
    use crate::logging::{self, field, records};
    use crate::models::artifact::Family;
    use crate::models::face::{Confidence, Face, Faces, Point, Rect};
    use crate::models::{Analysis, ArtifactId, Detection, FloatPrecision, Precision};
    use crate::pipeline::{DataPipeline, Model, SharedData};
    use crate::progress::{Dependency, Phase, Progress};
    use crate::providers::profile::EpProfile;
    use crate::task::lock;

    /// What a run did, so a test can say what was installed, opened and run rather than inferring it from the result.
    #[derive(Default)]
    struct Log {
        /// Every artifact a session was asked for, in order.
        acquired: Vec<String>,
        /// How many times the pipeline actually ran.
        runs: usize,
    }

    /// A backend that opens fake sessions and runs nothing: the pipeline below is the thing under test's counterpart,
    /// and what this stands in for is the acquisition.
    struct Fake {
        log: Arc<Mutex<Log>>,
        /// Whether an acquired model reports a transfer, which is what decides the install/run progress split.
        installs: bool,
        /// The provider every session is built on, which is what a downgrade is expressed as.
        built_on: ExecutionProvider,
        /// An artifact whose session refuses to open.
        fails_to_open: Option<&'static str>,
        /// A token cancelled while a session is being acquired, which is how a run is stopped *in flight* rather
        /// than before it starts.
        cancels: Option<CancellationToken>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                log: Arc::default(),
                installs: false,
                built_on: ExecutionProvider::Cpu,
                fails_to_open: None,
                cancels: None,
            }
        }

        fn log(&self) -> std::sync::MutexGuard<'_, Log> {
            lock(&self.log)
        }
    }

    impl Backend for Fake {
        type Session = ();
        type Error = std::io::Error;

        async fn acquire(
            &self,
            artifact: &ArtifactId,
            _profile: &EpProfile,
            _requested: ExecutionProvider,
            interest: &crate::sessions::Interest,
        ) -> Result<SessionHandle<Self::Session>, SessionError> {
            lock(&self.log).acquired.push(artifact.as_str().to_string());

            // Stopped while the model is being opened, which is the only way this suite can reach the check the
            // pipeline itself makes: a token cancelled before the call is caught by the driver's own first check.
            if let Some(token) = &self.cancels {
                token.cancel();
            }

            // A model that was not on disk, reporting its own transfer. The driver hands this callback down, so
            // driving it is the only way the install half of the range is exercised rather than assumed.
            if self.installs
                && let Some(report) = &interest.on_progress
            {
                for fraction in [0.5, 1.0] {
                    report(&Progress {
                        dependency: Dependency::Model(artifact.clone()),
                        phase: Phase::Downloading,
                        bytes: 0,
                        total: None,
                        fraction,
                    });
                }
            }

            if self.fails_to_open == Some(artifact.as_str()) {
                return Err(SessionError::Build {
                    artifact: artifact.as_str().to_string(),
                    provider: self.built_on,
                    source: Arc::new(std::io::Error::other("the graph would not load")),
                });
            }

            Ok(SessionHandle::held((), self.built_on))
        }

        stub_backend_runs!(
            "this suite drives the driver, not a model";
            run_tile,
            run_graph,
            run_named_outputs,
            run_weighted,
        );
    }

    /// What the pipeline under the driver does when it runs.
    #[derive(Clone, Copy, PartialEq)]
    enum Behaviour {
        /// Reports three fractions and hands back one face.
        Succeeds,
        /// Fails partway, so the driver has a failure to fold.
        Fails,
        /// Reports nothing and hands back an empty set, which is a legitimate finding.
        FindsNothing,
    }

    /// A data pipeline standing in for New York: it names artifacts, reports fractions, and answers with faces.
    struct FakePipeline {
        log: Arc<Mutex<Log>>,
        artifacts: Vec<ArtifactId>,
        profile: EpProfile,
        behaviour: Behaviour,
    }

    impl FakePipeline {
        /// A pipeline over `artifacts` behaving as `behaviour`, recording into `log`, as the driver holds one.
        fn shared(log: &Arc<Mutex<Log>>, artifacts: &[Precision], behaviour: Behaviour) -> SharedData<Fake, Faces> {
            Arc::new(Self {
                log: Arc::clone(log),
                artifacts: artifacts
                    .iter()
                    .map(|precision| ArtifactId::new(Family::Detection, "newyork", None, *precision))
                    .collect(),
                profile: EpProfile::default(),
                behaviour,
            })
        }
    }

    impl Model<Fake> for FakePipeline {
        fn required(&self) -> &[ArtifactId] {
            &self.artifacts
        }

        fn sessions(&self) -> Vec<(&ArtifactId, &EpProfile)> {
            self.artifacts.iter().map(|artifact| (artifact, &self.profile)).collect()
        }
    }

    impl DataPipeline<Fake> for FakePipeline {
        type Output = Faces;

        fn run(
            &self,
            _input: &DynamicImage,
            sessions: &[SessionHandle<()>],
            progress: Option<&dyn Fn(f64)>,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<Faces, InferenceError> {
            lock(&self.log).runs += 1;

            assert_eq!(sessions.len(), self.artifacts.len(), "the driver acquired the wrong number of handles");

            if cancelled() {
                return Err(InferenceError::Cancelled);
            }

            match self.behaviour {
                Behaviour::Succeeds => {
                    for fraction in [0.2, 0.6, 1.0] {
                        if let Some(progress) = progress {
                            progress(fraction);
                        }
                    }

                    Ok(Faces::new([Face::new(
                        Rect::new(Point::new(1.0, 2.0), Point::new(3.0, 4.0)),
                        [Point::new(0.0, 0.0); Face::LANDMARKS],
                        Confidence::new(0.9).expect("a confidence in range"),
                    )]))
                }
                // Partway through, so there is something already reported for a failure to have to discard.
                Behaviour::Fails => {
                    if let Some(progress) = progress {
                        progress(0.2);
                    }

                    Err(InferenceError::Run {
                        operation: "New York (FP32)".to_string(),
                        tile: 0,
                        source: Arc::new(std::io::Error::other("the model failed")),
                    })
                }
                Behaviour::FindsNothing => Ok(Faces::empty()),
            }
        }
    }

    /// The operation every run below is reported as, in the carrier its own path takes.
    fn detection() -> Analysis {
        Detection::newyork(FloatPrecision::Fp32)
    }

    /// The same, as the [`Subject`](crate::models::Subject) the driver is handed — which is the form the key
    /// is folded from and the form the records name.
    fn asked() -> crate::models::Subject {
        crate::models::Subject::Analysis(detection())
    }

    /// A picture to run over. Nothing here reads its pixels; what matters is that it has an identity to record.
    fn picture() -> Picture {
        Picture::new(
            "/pictures/portrait.jpg",
            Arc::new(DynamicImage::ImageRgb8(image::RgbImage::new(64, 48))),
            "cafebabecafebabe",
        )
    }

    /// Options collecting every report, beside the sink they land in.
    fn recording() -> (Arc<Mutex<Vec<InferenceProgress>>>, ExecuteOptions) {
        let reports: Arc<Mutex<Vec<InferenceProgress>>> = Arc::default();
        let sink = Arc::clone(&reports);

        let options = ExecuteOptions {
            on_progress: Some(Arc::new(move |report: &InferenceProgress| lock(&sink).push(report.clone()))),
            ..Default::default()
        };

        (reports, options)
    }

    /// Drives the driver's body directly, which is what lets a test supply a pipeline rather than reach one through
    /// the operation's own seam — the seam itself is `models::operation`'s to check.
    async fn drive(
        backend: &Fake,
        pipeline: SharedData<Fake, Faces>,
        options: ExecuteOptions,
    ) -> Result<(Faces, ProviderReport), InferenceError> {
        drive_over(backend, pipeline, options, None).await
    }

    /// The same, over `memo` as the application's store — which is the only ingredient `drive` withholds.
    ///
    /// The store is resolved through [`RunCache::for_run`], the function the entry point itself calls, so a run asked
    /// not to use the store is refused it here for the same reason it would be there rather than by a second rule
    /// written into the suite.
    async fn drive_over(
        backend: &Fake,
        pipeline: SharedData<Fake, Faces>,
        options: ExecuteOptions,
        memo: Option<&Memo>,
    ) -> Result<(Faces, ProviderReport), InferenceError> {
        drive_asking(backend, pipeline, options, memo, &picture(), &asked()).await
    }

    /// The same again, over the photograph the run is asking about and the question it is asking of it — the two
    /// remaining ingredients, and between them the whole of what decides which slot the driver looks in.
    ///
    /// Separate from the store so that "a different question of the same photograph" and "the same question of a
    /// different photograph" are driven *through the driver* rather than asserted about the keys it would derive.
    /// Those are not the same test: a driver that derived the right key and then looked under another would pass the
    /// second and fail this.
    async fn drive_asking(
        backend: &Fake,
        pipeline: SharedData<Fake, Faces>,
        options: ExecuteOptions,
        memo: Option<&Memo>,
        source: &Picture,
        operation: &crate::models::Subject,
    ) -> Result<(Faces, ProviderReport), InferenceError> {
        let cache = RunCache::for_run(memo, options.cache, source);

        run::<Fake, Analysis>(
            backend,
            source,
            pipeline,
            operation,
            options.provider,
            options.on_progress,
            &options.cancel,
            cache.as_ref(),
        )
        .await
    }

    #[tokio::test]
    async fn progress_never_decreases_and_reaches_one() {
        // The property a front end actually depends on: a bar that retreats is worse than one that jumps. Driven
        // across the handover an install makes to a run, which is where it would go backwards if the two each
        // reported over the whole range.
        let mut backend = Fake::new();
        backend.installs = true;

        let (reports, options) = recording();
        drive(&backend, FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds), options)
            .await
            .expect("a run");

        let reports = lock(&reports);
        assert!(!reports.is_empty(), "nothing was reported");

        assert_never_decreases(&reports, "across the install handover");

        assert_eq!(reports[reports.len() - 1].chain_fraction, 1.0, "the bar did not reach its maximum");
    }

    #[tokio::test]
    async fn a_model_already_on_disk_does_not_give_up_the_head_of_its_range() {
        // The split is decided by whether an install actually happened rather than by whether one was possible, so a
        // model already on disk runs over the whole range instead of starting a fifth of the way along.
        let backend = Fake::new();
        assert!(!backend.installs, "the fixture installed something");

        let (reports, options) = recording();
        drive(&backend, FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds), options)
            .await
            .expect("a run");

        let reports = lock(&reports);
        assert!(
            reports.iter().all(|report| !matches!(report.stage, Stage::Installing(_))),
            "a model already on disk reported a transfer"
        );

        // The pipeline's own first report is 0.2, and with no install it is 0.2 of the whole range rather than 0.2 of
        // the four fifths an install would have left.
        assert_eq!(
            reports[0].chain_fraction, 0.2,
            "the run gave up the head of its range to a transfer that never happened"
        );
        assert_eq!(reports[reports.len() - 1].chain_fraction, 1.0);
    }

    #[tokio::test]
    async fn a_stopped_run_produces_no_result() {
        // Stopped before anything is installed: a caller that cancelled must not first pay for a transfer.
        let backend = Fake::new();
        let cancelled = ExecuteOptions { cancel: CancellationToken::new(), ..Default::default() };
        cancelled.cancel.cancel();

        let outcome =
            drive(&backend, FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds), cancelled)
                .await;

        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "a cancelled run produced a result");
        assert!(backend.log().acquired.is_empty(), "a cancelled run installed a model on its way to stopping");

        // And stopped after the sessions are open, which is the other point a token can be read at: the pipeline sees
        // it and returns nothing rather than the faces it had found so far.
        let token = CancellationToken::new();
        let mut backend = Fake::new();
        backend.cancels = Some(token.clone());

        let options = ExecuteOptions { cancel: token, ..Default::default() };
        let pipeline = FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds);

        let outcome = drive(&backend, pipeline, options).await;

        assert!(matches!(outcome, Err(InferenceError::Cancelled)));
        assert_eq!(backend.log().runs, 1, "the pipeline was not reached, so this checked the wrong cancellation");
    }

    #[tokio::test]
    async fn a_failed_run_produces_no_result_rather_than_an_empty_one() {
        // The distinction that matters: an empty set is a legitimate finding — an image with no face in it — so a
        // failure that returned one would be indistinguishable from a successful detection of nothing, and a caller
        // would show it to a user as one.
        let backend = Fake::new();
        let (_, options) = recording();

        let outcome =
            drive(&backend, FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Fails), options).await;

        let Err(InferenceError::Run { operation, .. }) = outcome else {
            panic!("a failed run produced a result");
        };
        assert_eq!(operation, "New York (FP32)", "the failure did not name the operation");

        // And the other half of the distinction: a run that genuinely found nothing succeeds with an empty set.
        let backend = Fake::new();
        let (found, _) = drive(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::FindsNothing),
            ExecuteOptions::default(),
        )
        .await
        .expect("an image with no face is a successful detection");

        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn a_failure_to_open_the_model_is_reported_rather_than_served_as_no_faces() {
        let mut backend = Fake::new();
        backend.fails_to_open = Some("dt_newyork_fp32");

        let outcome = drive(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            ExecuteOptions::default(),
        )
        .await;

        assert!(
            matches!(outcome, Err(InferenceError::Open { .. })),
            "a model that would not open produced faces"
        );
        assert_eq!(backend.log().runs, 0, "the pipeline ran against a session that was never built");
    }

    #[tokio::test]
    async fn the_report_names_what_was_asked_for_beside_what_was_built() {
        // The downgrade a run is otherwise silent about: the same faces, in the same way, several times slower.
        let mut backend = Fake::new();
        backend.built_on = ExecutionProvider::Cpu;

        let options = ExecuteOptions { provider: ExecutionProvider::Cuda, ..Default::default() };
        let (_, report) =
            drive(&backend, FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds), options)
                .await
                .expect("a run");

        assert_eq!(report.requested, ExecutionProvider::Cuda);
        assert_eq!(report.actual, vec![ExecutionProvider::Cpu]);
        assert_eq!(
            report.verdict(),
            crate::inference::process::ProviderVerdict::Downgraded {
                requested: ExecutionProvider::Cuda,
                actual: vec![ExecutionProvider::Cpu],
            }
        );
    }

    #[tokio::test]
    async fn a_run_that_built_no_session_says_nothing_was_executed() {
        // Never "it ran on what was asked for", which would claim a measurement that was never taken. Detection takes
        // exactly one session, but nothing in the driver depends on that — this is the pipeline that takes none.
        let backend = Fake::new();

        let options = ExecuteOptions { provider: ExecutionProvider::Cuda, ..Default::default() };
        let (_, report) = drive(&backend, FakePipeline::shared(&backend.log, &[], Behaviour::FindsNothing), options)
            .await
            .expect("a run");

        assert!(report.actual.is_empty(), "a run that built nothing named a provider it ran on");
        assert_eq!(report.verdict(), crate::inference::process::ProviderVerdict::NothingExecuted);
    }

    /// A result that is plainly not the one `Behaviour::Succeeds` produces, so a served run and an executed one are
    /// told apart by the value rather than only by the acquisition log.
    fn stored_faces() -> Faces {
        Faces::new([Face::new(
            Rect::new(Point::new(100.0, 200.0), Point::new(300.0, 400.0)),
            [Point::new(9.0, 9.0); Face::LANDMARKS],
            Confidence::new(0.5).expect("a confidence in range"),
        )])
    }

    /// The slot a run over [`picture`] with [`detection`] stores its result under.
    ///
    /// Through the real derivation rather than a literal: a key written out here would go on matching after an edit
    /// that changed what the driver actually looks under.
    fn slot() -> String {
        identity_of_data(picture().identity(), &asked())
    }

    /// A memory-backed store holding `found` as the result of that run, as a previous run would have left it.
    fn store_holding(found: &Faces) -> Memo {
        let memo = Memo::memory(CacheOpts::new()).expect("a memory store");
        RunCache::new(memo.clone(), picture().identity()).put_value(&slot(), found);
        memo
    }

    #[tokio::test]
    async fn a_run_served_from_the_store_transfers_opens_and_executes_nothing() {
        // Asserted against the fake's **acquisition log** rather than against the result: a driver whose lookup sat
        // below `acquire` would return the same value while having paid for the session, which is the
        // expensive half.
        let memo = store_holding(&stored_faces());
        let backend = Fake::new();
        let (reports, options) = recording();

        let (found, report) = drive_over(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            options,
            Some(&memo),
        )
        .await
        .expect("a run");

        assert_eq!(found, stored_faces(), "the run produced its own answer rather than the stored one");
        assert!(backend.log().acquired.is_empty(), "a served run opened a session");
        assert_eq!(backend.log().runs, 0, "a served run executed the model");

        // A served run built no model, so the report says so rather than naming the provider it would have used.
        assert!(report.actual.is_empty(), "a served run named a provider it ran on");
        assert_eq!(report.verdict(), crate::inference::process::ProviderVerdict::NothingExecuted);

        // And a caller listening to it is told it finished: a bar left short because the work was skipped is
        // indistinguishable from a run that stalled.
        let reports = lock(&reports);
        assert_eq!(reports[reports.len() - 1].chain_fraction, 1.0, "a served run never reached its maximum");
        assert!(
            reports.iter().any(|report| matches!(report.stage, Stage::Cached)),
            "a served run did not report that it was served"
        );
    }

    #[tokio::test]
    async fn a_miss_keeps_what_it_produced_so_the_next_run_is_served() {
        // The read and the write compute the same key from the same call, so a run that stored a result and a run that
        // looks one up cannot disagree about where it lives. Asserted as "the model ran once across the two", which is
        // the only thing a caller could actually observe.
        let memo = Memo::memory(CacheOpts::new()).expect("a memory store");
        let backend = Fake::new();

        let mut produced = Vec::new();
        for _ in 0..2 {
            let (found, _) = drive_over(
                &backend,
                FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
                ExecuteOptions::default(),
                Some(&memo),
            )
            .await
            .expect("a run");

            produced.push(found);
        }

        assert_eq!(backend.log().runs, 1, "the second run executed the model rather than being served");
        assert_eq!(backend.log().acquired.len(), 1, "the second run opened a session");
        assert_eq!(produced[1], produced[0], "the served run did not produce what the first run had");
    }

    #[tokio::test]
    async fn a_failed_and_a_cancelled_run_each_keep_nothing() {
        // A later run must not be served a result that was never produced. Both arrive at the write as an `Err`, so
        // this is the property that the arm reads the outcome rather than merely the token.
        let failed = Memo::memory(CacheOpts::new()).expect("a memory store");
        let backend = Fake::new();
        drive_over(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Fails),
            ExecuteOptions::default(),
            Some(&failed),
        )
        .await
        .expect_err("this pipeline fails");

        let token = CancellationToken::new();
        let cancelled = Memo::memory(CacheOpts::new()).expect("a memory store");
        let mut stopping = Fake::new();
        stopping.cancels = Some(token.clone());
        drive_over(
            &stopping,
            FakePipeline::shared(&stopping.log, &[Precision::Fp32], Behaviour::Succeeds),
            ExecuteOptions { cancel: token, ..Default::default() },
            Some(&cancelled),
        )
        .await
        .expect_err("this run is stopped in flight");

        for memo in [&failed, &cancelled] {
            let stored: Option<Faces> = RunCache::new(memo.clone(), picture().identity()).get_value(&slot());
            assert!(stored.is_none(), "a run that produced no result stored one anyway");
        }
    }

    #[tokio::test]
    async fn a_run_cancelled_before_the_lookup_is_not_handed_a_stored_result() {
        // A caller that stopped the run must not be handed a result as though the run had completed — which is why
        // the token is read above the read rather than only above the acquisition.
        let memo = store_holding(&stored_faces());
        let backend = Fake::new();

        let cancelled = ExecuteOptions { cancel: CancellationToken::new(), ..Default::default() };
        cancelled.cancel.cancel();

        let outcome = drive_over(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            cancelled,
            Some(&memo),
        )
        .await;

        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "a cancelled run was served a stored result");
        assert!(backend.log().acquired.is_empty());
    }

    #[tokio::test]
    async fn a_different_operation_and_a_different_image_are_each_their_own_entry() {
        // Both halves of "stored under the image together with the operation", each driven *through the driver*: it
        // is the driver that has to look in its own slot and find nothing there, and a test that only compared the
        // two keys would pass against a driver that derived one key and then read under another.
        let memo = store_holding(&stored_faces());

        // A different question of the same photograph. The two `NewYork` precisions are genuinely two operations —
        // they carry different cache tags — which is what lets this be a run rather than an assertion about a string.
        let other = crate::models::Subject::Analysis(Detection::newyork(FloatPrecision::Fp16));
        assert_ne!(other.cache_tag(), asked().cache_tag());

        let backend = Fake::new();
        let (found, _) = drive_asking(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            ExecuteOptions::default(),
            Some(&memo),
            &picture(),
            &other,
        )
        .await
        .expect("a run");

        assert_ne!(found, stored_faces(), "a second operation was served the first's answer");
        assert_eq!(backend.log().runs, 1, "a second operation did not run its own model");

        // The same question of a different photograph, which is a second identity rather than a second question —
        // the other half of what the slot is folded from.
        let elsewhere = Picture::new(
            "/pictures/harbour.jpg",
            Arc::new(DynamicImage::ImageRgb8(image::RgbImage::new(64, 48))),
            "f00df00df00df00d",
        );
        assert_ne!(elsewhere.identity(), picture().identity());

        let backend = Fake::new();
        let (found, _) = drive_asking(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            ExecuteOptions::default(),
            Some(&memo),
            &elsewhere,
            &asked(),
        )
        .await
        .expect("a run");

        assert_ne!(found, stored_faces(), "a second photograph was served the first's answer");
        assert_eq!(backend.log().runs, 1, "a second photograph did not run its own model");

        // And the entry that *is* there is still served, so all of the above is about which slot rather than about a
        // store that turned out to be empty.
        let backend = Fake::new();
        let (found, _) = drive_over(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            ExecuteOptions::default(),
            Some(&memo),
        )
        .await
        .expect("a run");

        assert_eq!(found, stored_faces());
        assert_eq!(backend.log().runs, 0);
    }

    #[tokio::test]
    async fn a_run_that_asks_for_no_store_executes_every_time_and_leaves_nothing_behind() {
        // The benchmark's case: a measurement that read a stored result would measure a read, and one that silently
        // stored its result would change what the next measurement measured. Asserted against the store byte for
        // byte rather than against the result, because a run that happened to compute what a served one would have
        // is not evidence that it did not read.
        let memo = store_holding(&stored_faces());
        let before = memo.get_bytes(&slot()).expect("the store reads back");
        assert!(before.is_some(), "the fixture did not populate the store");

        let backend = Fake::new();

        for _ in 0..2 {
            let (found, _) = drive_over(
                &backend,
                FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
                ExecuteOptions { cache: false, ..Default::default() },
                Some(&memo),
            )
            .await
            .expect("a run");

            assert_ne!(found, stored_faces(), "a run with the store off was served a stored result");
        }

        assert_eq!(backend.log().runs, 2, "a run with the store off was served rather than executed");
        assert_eq!(backend.log().acquired.len(), 2, "a run with the store off skipped its session");

        let after = memo.get_bytes(&slot()).expect("the store reads back");
        assert_eq!(after, before, "a run with the store off wrote to it");
    }

    #[tokio::test]
    async fn a_store_that_could_not_be_opened_costs_a_run_and_no_error() {
        // The `CacheMode::None` machine, reached honestly: `RunCache::for_run` is handed no `Memo` at all, which is the
        // shape "no store" actually has — a `RunCache` cannot be built without one.
        let backend = Fake::new();

        for _ in 0..2 {
            drive_over(
                &backend,
                FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
                ExecuteOptions::default(),
                None,
            )
            .await
            .expect("a machine with no store still analyses its photographs");
        }

        assert_eq!(backend.log().runs, 2, "a run without a store was served from something");

        // A store that *failed a read* is the third shape of the same answer and is not reachable from here: `memo`
        // exposes no store that can be made to fail on demand, and `RunCache::get_bytes` folds the error into `None`
        // before the driver sees it — which is the branch `cache`'s own `a_read_the_store_fails_is_a_warning_rather
        // _than_a_miss` drives directly. What this file is responsible for is that `None` costs a run, which is what
        // the loop above and the undecodable-entry test each assert.
    }

    #[tokio::test]
    async fn an_entry_that_will_not_read_back_costs_a_run_rather_than_an_error() {
        // A truncated write, or an entry written by a build whose encoding differed. It is a miss like any other, and
        // the response to a miss is to execute the model — the result is still obtainable, which is why none of this
        // reaches the caller.
        let memo = Memo::memory(CacheOpts::new()).expect("a memory store");
        memo.set_bytes(&slot(), b"{ not json, and not a picture either", crate::cache::ENTRY_TTL)
            .expect("the fixture stores an entry");

        let backend = Fake::new();
        let (found, report) = drive_over(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            ExecuteOptions::default(),
            Some(&memo),
        )
        .await
        .expect("an entry that will not read back is not an error");

        assert_eq!(backend.log().runs, 1, "the model did not run to replace what could not be read");
        assert_eq!(found.len(), 1);
        assert_eq!(report.actual, vec![ExecutionProvider::Cpu], "a run that executed reported nothing executed");
    }

    #[tokio::test]
    async fn a_served_run_still_reports_reaching_the_end() {
        // Separated from the acquisition test because it is a different requirement with a different failure: a bar
        // left short because the work was skipped is indistinguishable from a run that stalled, and a caller has no
        // other way to learn that an instant run finished.
        let memo = store_holding(&stored_faces());
        let backend = Fake::new();
        let (reports, options) = recording();

        drive_over(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            options,
            Some(&memo),
        )
        .await
        .expect("a run");

        let reports = lock(&reports);
        assert!(!reports.is_empty(), "a served run reported nothing at all");
        assert_never_decreases(&reports, "across a served run");
        assert_eq!(reports[reports.len() - 1].chain_fraction, 1.0);
        assert_eq!(reports[reports.len() - 1].operation_fraction, 1.0);
    }

    #[tokio::test]
    async fn whether_an_analysis_was_served_or_computed_is_in_the_log() {
        // A served analysis is indistinguishable from a computed one in everything a caller sees except the time it
        // took — which is the whole point of the store and also what makes one that is quietly doing nothing
        // impossible to notice. So both ways are recorded, and *both* are asserted: a file carrying only the hits
        // says nothing about the run that never got one.
        let memo = Memo::memory(CacheOpts::new()).expect("a memory store");
        let backend = Fake::new();

        let (miss, outcome) = logging::records_of("debug", || async {
            drive_over(
                &backend,
                FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
                ExecuteOptions::default(),
                Some(&memo),
            )
            .await
        })
        .await;
        outcome.expect("a run");

        let computed = records(&miss, "analysis computed");
        assert_eq!(computed.len(), 1, "a computed analysis was recorded {} times:\n{miss}", computed.len());
        assert_eq!(field(computed[0], "operation"), Some(asked().cache_tag().as_str()));
        assert_eq!(field(computed[0], "key"), Some(slot().as_str()), "the record does not name the slot it filled");
        assert!(computed[0].contains("level=DEBUG"), "a per-run record is above debug: {}", computed[0]);
        assert!(records(&miss, "analysis served from the store").is_empty(), "an empty store served something");

        // The second run, which the first one's write makes a hit — so the pair is asserted over one store rather
        // than over two fixtures that might disagree about what a run leaves behind.
        let (hit, outcome) = logging::records_of("debug", || async {
            drive_over(
                &backend,
                FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
                ExecuteOptions::default(),
                Some(&memo),
            )
            .await
        })
        .await;
        outcome.expect("a run");

        let served = records(&hit, "analysis served from the store");
        assert_eq!(served.len(), 1, "a served analysis was recorded {} times:\n{hit}", served.len());
        assert_eq!(field(served[0], "operation"), Some(asked().cache_tag().as_str()));
        assert_eq!(field(served[0], "key"), Some(slot().as_str()), "the two records name different slots");
        assert!(served[0].contains("level=DEBUG"), "a per-run record is above debug: {}", served[0]);
        assert!(
            records(&hit, "analysis computed").is_empty(),
            "a served run claimed to have computed its answer"
        );
    }

    #[tokio::test]
    async fn an_ordinary_session_is_not_told_which_way_a_run_went() {
        // The volume rule the records above rest on: they repeat once per run, so a front end asking the same
        // question of the same photograph repeatedly writes none of them at the default level. Asking for the detail
        // is one environment variable rather than a rebuild.
        let memo = store_holding(&stored_faces());
        let backend = Fake::new();

        let (log, outcome) = logging::records_of("info", || async {
            drive_over(
                &backend,
                FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
                ExecuteOptions::default(),
                Some(&memo),
            )
            .await
        })
        .await;
        outcome.expect("a run");

        assert!(
            records(&log, "analysis served from the store").is_empty(),
            "a per-run record at the default level"
        );
        assert!(records(&log, "analysis computed").is_empty(), "a per-run record at the default level");
    }

    #[tokio::test]
    async fn every_session_is_held_until_the_run_returns() {
        // The guarantee is ownership rather than a rule any code follows: the handles travel into the blocking
        // closure and back out of it, so there is no window in which one could be found reclaimed. What is checkable
        // here is that the pipeline is handed exactly the handles the model asked for, which the fake asserts, and
        // that the report is folded out of handles the driver still had.
        let backend = Fake::new();

        let (_, report) = drive(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32, Precision::Fp16], Behaviour::Succeeds),
            ExecuteOptions::default(),
        )
        .await
        .expect("a run");

        assert_eq!(
            backend.log().acquired,
            vec!["dt_newyork_fp32", "dt_newyork_fp16"],
            "the order was not preserved"
        );
        // Two handles on one provider are named once: a property of the report rather than of how the run
        // accumulated them.
        assert_eq!(report.actual, vec![ExecutionProvider::Cpu]);
    }

    #[tokio::test]
    async fn a_caller_that_asks_for_no_reporting_is_charged_for_none() {
        let backend = Fake::new();

        let (found, _) = drive(
            &backend,
            FakePipeline::shared(&backend.log, &[Precision::Fp32], Behaviour::Succeeds),
            ExecuteOptions::default(),
        )
        .await
        .expect("a run");

        assert_eq!(found.len(), 1, "a run with no callback produced no result");
    }

    /// An analysis whose pipeline is refused, whatever the backend: the refusal no shipped operation can reach, which
    /// is what the record for it is asserted against.
    struct Refused;

    impl crate::models::operation::sealed::DataModel for Refused {
        type Output = Faces;

        fn as_subject(&self) -> crate::models::Subject {
            asked()
        }

        fn pipeline<B: Backend>(&self) -> Result<SharedData<B, Faces>, InferenceError> {
            Err(InferenceError::Unsupported {
                operation: "New York (FP32)".to_string(),
                reason: crate::error::UnsupportedReason::IncompleteGraphSet { missing: "detector" },
            })
        }
    }

    #[tokio::test]
    async fn a_refused_analysis_is_one_record_and_never_a_run_that_began() {
        let backend = Fake::new();

        let (log, outcome) =
            logging::records_of("info", || async { execute(&backend, None, &picture(), &Refused, None).await }).await;
        assert!(matches!(outcome, Err(InferenceError::Unsupported { .. })), "{:?}", outcome.err());

        let refused = records(&log, "analysis refused");
        assert_eq!(refused.len(), 1, "{log}");
        assert!(refused[0].contains("level=WARN"), "{}", refused[0]);
        assert_eq!(field(refused[0], "id"), Some(asked().cache_tag().as_str()));
        assert!(field(refused[0], "error").is_some(), "the refusal does not say why: {}", refused[0]);

        assert!(records(&log, "analysis started").is_empty(), "a refused analysis was recorded as begun:\n{log}");
        assert!(records(&log, "analysis failed").is_empty(), "a refusal was recorded twice:\n{log}");
        assert!(backend.log().acquired.is_empty(), "a refused analysis installed a model");
    }

    #[tokio::test]
    async fn a_refused_analysis_opens_no_span() {
        let backend = Fake::new();

        let (_, observed, outcome) =
            logging::traced_of("info", || async { execute(&backend, None, &picture(), &Refused, None).await }).await;
        assert!(matches!(outcome, Err(InferenceError::Unsupported { .. })), "{:?}", outcome.err());

        assert!(observed.named("analysis").is_empty(), "a refusal opened a span: {:#?}", observed.spans);
    }

    #[tokio::test]
    async fn a_cancelled_analysis_is_a_span_that_says_it_stopped() {
        let backend = Fake::new();
        let options = ExecuteOptions { cancel: CancellationToken::new(), ..Default::default() };
        options.cancel.cancel();

        let (_, observed, outcome) = logging::traced_of("info", || async {
            execute(&backend, None, &picture(), &detection(), Some(options)).await
        })
        .await;
        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "{:?}", outcome.err());

        let analysis = observed.only("analysis");
        assert_eq!(analysis.field("id"), Some(asked().cache_tag().as_str()));
        assert_eq!(analysis.field("identity"), Some(picture().identity()));
        assert_eq!(analysis.field("outcome"), Some("stopped"));
        assert_eq!(analysis.field("error"), None, "a cancellation was marked failed");
        assert_eq!(observed.event("analysis stopped").and_then(|event| event.span), Some(analysis.index));
    }

    #[tokio::test]
    async fn a_cancelled_analysis_is_recorded_as_stopped_and_not_as_a_failure() {
        let backend = Fake::new();
        let options = ExecuteOptions { cancel: CancellationToken::new(), ..Default::default() };
        options.cancel.cancel();

        let (log, outcome) = logging::records_of("info", || async {
            execute(&backend, None, &picture(), &detection(), Some(options)).await
        })
        .await;
        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "{:?}", outcome.err());

        let stopped = records(&log, "analysis stopped");
        assert_eq!(stopped.len(), 1, "{log}");
        assert!(stopped[0].contains("level=INFO"), "{}", stopped[0]);
        assert_eq!(field(stopped[0], "reason"), Some("cancelled"));
        assert_eq!(field(stopped[0], "identity"), Some(picture().identity()));
        assert!(field(stopped[0], "duration").is_some(), "{}", stopped[0]);
        assert!(!log.contains("level=WARN"), "a cancellation wrote a warning:\n{log}");
    }

    #[tokio::test]
    async fn a_failed_analysis_is_recorded_once_as_a_failure() {
        let mut backend = Fake::new();
        backend.fails_to_open = Some("dt_newyork_fp32");
        let options = ExecuteOptions { provider: ExecutionProvider::CoreMl, ..Default::default() };

        let (log, outcome) = logging::records_of("info", || async {
            execute(&backend, None, &picture(), &detection(), Some(options)).await
        })
        .await;
        assert!(matches!(outcome, Err(InferenceError::Open { .. })), "{:?}", outcome.err());

        // It began, naming what was asked for.
        let started = records(&log, "analysis started");
        assert_eq!(started.len(), 1, "{log}");
        assert_eq!(field(started[0], "provider"), Some(ExecutionProvider::CoreMl.as_str()));

        let warnings: Vec<&str> = log.lines().filter(|line| line.contains("level=WARN")).collect();
        assert_eq!(warnings.len(), 1, "{log}");
        assert!(warnings[0].contains(r#"msg="analysis failed""#), "{}", warnings[0]);
        assert_eq!(field(warnings[0], "identity"), Some(picture().identity()));
        assert!(field(warnings[0], "error").is_some(), "{}", warnings[0]);
        assert!(field(warnings[0], "duration").is_some(), "{}", warnings[0]);
        assert!(records(&log, "analysis stopped").is_empty(), "{log}");
    }
}
