//! The chain's tests, and the fake backend and fixtures every group of them runs against.

mod cache;
mod entry;
mod identity;
mod log;
mod plan;
mod report;
mod run;
mod spans;

use super::plan::Planned;
use super::*;
use crate::pipeline::test_support::stub_backend_runs;
use std::collections::HashMap;
use std::sync::Mutex;

use image::{GenericImageView as _, ImageBuffer};

use crate::cache::ENTRY_TTL;
use crate::error::{InitError, SessionError};
use crate::inference::depth::OutputDepth;
use crate::inference::progress::{INSTALL_SHARE, InferenceProgress, Stage};
use crate::inference::test_support::{assert_never_decreases, recording};
use crate::logging::{self, field, records};
use crate::models::ArtifactId;
use crate::models::{FloatPrecision, OsakaPrecision, Scale, Upscale, UpscaleVariant};
use crate::pipeline::session::GraphShape;
use crate::progress::{Dependency, Phase, Progress};
use crate::providers::profile::EpProfile;
use crate::providers::profile::{CoreMlComputeUnits, ExecutionMode};
use crate::sessions::Interest;
use imaging::test_support::{gradient, nearest_neighbour};

/// The shared gradient, behind the `Arc` every chain entry point takes.
fn source(width: u32, height: u32) -> Arc<DynamicImage> {
    Arc::new(gradient(width, height))
}

/// What a run did, recorded across the fake's acquisitions and its tile runs.
#[derive(Debug, Default)]
struct Log {
    /// The artifacts acquired, in the order they were asked for.
    acquired: Vec<String>,
    /// The same acquisitions, each paired with the settings it was opened under — which is the half of the
    /// contract a graph set exercises and a pass sequence does not: three artifacts, three profiles.
    opened_under: Vec<(String, EpProfile)>,
    /// The artifacts a tile was run against, in order.
    ran: Vec<String>,
    /// How many progress reports had already reached the caller when each tile ran.
    reports_at_tile: Vec<usize>,
    /// The artifacts an install was reported for, in order — which is the only path by which a
    /// `Stage::Installing` report can reach a caller.
    installed: Vec<String>,
    /// How many sessions had already been freed when each tile ran. Zero on every tile is the whole of "a model
    /// a run holds cannot be reclaimed under it".
    freed_at_tile: Vec<usize>,
}

impl Log {
    /// The artifacts run against, with consecutive repeats collapsed — one entry per pass rather than per tile.
    fn passes(&self) -> Vec<String> {
        let mut passes: Vec<String> = Vec::new();

        for artifact in &self.ran {
            if passes.last() != Some(artifact) {
                passes.push(artifact.clone());
            }
        }

        passes
    }
}

/// What a fake session holds that a real one would free: created by an open, dropped by the last handle on it.
///
/// Separate from [`FakeSession`] because the point of it is to be *shared* — one of these is served to every
/// request for that artifact, exactly as the resident cache serves one session, so "opened a second time" and
/// "freed while something was still using it" are both observable rather than inferred.
#[derive(Debug)]
struct FakeNative {
    /// Which open produced it, counting from one, so a reopen is visible as a second number.
    open: usize,
    /// Where this records itself when it goes, which is what makes a premature release a checkable event.
    freed: Arc<Mutex<Vec<usize>>>,
}

impl Drop for FakeNative {
    fn drop(&mut self) {
        self.freed.lock().unwrap().push(self.open);
    }
}

/// A fake session: what it was opened for, and everything the tile step needs to reach.
///
/// The state lives on the **session** rather than on the backend because [`Backend::run_tile`] takes no `self` —
/// which is itself deliberate, since the production tile step needs nothing but the handle.
#[derive(Debug)]
struct FakeSession {
    artifact: String,
    log: Arc<Mutex<Log>>,
    /// Whether running a tile against this session fails, standing in for a model that broke partway through.
    fails: bool,
    /// Cancelled when the first tile against this session is reached, which is how a run is stopped from *inside*
    /// a model rather than between operations.
    cancels: Option<CancellationToken>,
    /// The reports the caller has collected so far, read at each tile so that "they arrived while the run was
    /// still going" is a checked property rather than an inference from the order of a finished list.
    reports: Option<Arc<Mutex<Vec<InferenceProgress>>>>,
    /// What this session would free, shared with every other handle on the same artifact.
    native: Arc<FakeNative>,
}

/// A backend that opens whatever it is asked for and enlarges each pixel into a block, recording both.
struct Fake {
    log: Arc<Mutex<Log>>,
    /// The artifact whose files cannot be put on disk, standing in for a transfer that failed.
    fails_to_install: Option<String>,
    /// Where set, this model's install is **stopped** rather than failed: what the session layer answers when
    /// every run waiting on a transfer has gone away.
    stops_installing: Option<String>,
    /// The artifact whose acquisition fails, standing in for a model that could not be opened.
    fails_to_open: Option<String>,
    /// The artifact whose tile runs fail.
    fails_to_run: Option<String>,
    /// The artifact whose first tile cancels the run, with the token the caller holds.
    cancels: Option<(String, CancellationToken)>,
    /// What each named artifact's session is built on, standing in for a provider that could open that model.
    ///
    /// Per artifact rather than per backend, because the fact the report exists for is per **model**: a provider
    /// that opens the first model of a chain and not the second is what a run downgraded halfway looks like, and
    /// one answer for the whole fake could not produce it.
    built_on: HashMap<String, ExecutionProvider>,
    /// What an artifact `built_on` names no answer for is built on.
    ///
    /// [`ExecutionProvider::Cpu`] by default, which is what every test written before the report assumed.
    builds_on: ExecutionProvider,
    /// Where the caller's reports land, for the fake to read mid-run.
    reports: Option<Arc<Mutex<Vec<InferenceProgress>>>>,
    /// Whether a model has to be put on disk before it opens, which is what makes the install half of an
    /// operation's progress range actually report. Off by default: most of these tests are about what runs, and a
    /// model already on disk is the commoner case anyway.
    installs: bool,
    /// The sessions this backend has open, keyed on the artifact — the resident cache's own shape, so that a
    /// second request for one model is served the session the first was.
    open: Mutex<HashMap<String, Arc<FakeNative>>>,
    /// How many sessions have been opened, which is what a reopen shows up as.
    opens: Mutex<usize>,
    /// Where a freed session records itself.
    freed: Arc<Mutex<Vec<usize>>>,
    /// Empty the store just before the acquisition at this position, counting from one: `Opai::release_sessions`
    /// arriving from somewhere else while a chain is running. It drops the store's own reference and nothing
    /// else, which is exactly what `SessionCache::clear` does.
    release_before_acquire: Option<usize>,
}

impl Fake {
    fn new() -> Self {
        Self {
            log: Arc::new(Mutex::new(Log::default())),
            fails_to_install: None,
            stops_installing: None,
            fails_to_open: None,
            fails_to_run: None,
            cancels: None,
            built_on: HashMap::new(),
            builds_on: ExecutionProvider::Cpu,
            reports: None,
            installs: false,
            open: Mutex::new(HashMap::new()),
            opens: Mutex::new(0),
            freed: Arc::new(Mutex::new(Vec::new())),
            release_before_acquire: None,
        }
    }

    /// What the run did.
    fn log(&self) -> std::sync::MutexGuard<'_, Log> {
        self.log.lock().unwrap()
    }
}

impl Backend for Fake {
    type Session = FakeSession;
    type Error = std::io::Error;

    async fn acquire(
        &self,
        artifact: &ArtifactId,
        profile: &EpProfile,
        _requested: ExecutionProvider,
        interest: &Interest,
    ) -> Result<SessionHandle<Self::Session>, SessionError> {
        let id = artifact.clone();
        let artifact = artifact.as_str().to_string();
        {
            let mut log = self.log.lock().unwrap();
            log.acquired.push(artifact.clone());
            log.opened_under.push((artifact.clone(), profile.clone()));
        }

        // Checked before the transfer below reports anything: a model whose files never reached the disk has no
        // completed transfer to report, and the failure it produces is the one the session layer folds into
        // `InferenceError::Install` rather than into `Open`.
        if self.fails_to_install.as_deref() == Some(artifact.as_str()) {
            return Err(SessionError::from(InitError::UnpublishedModel { artifact }));
        }

        // Not a failure: the transfer stopped because nothing was left waiting for it, which is what the session
        // layer answers when the last run interested in a model withdrew.
        if self.stops_installing.as_deref() == Some(artifact.as_str()) {
            return Err(SessionError::from(InitError::Stopped));
        }

        // A model that was not on disk, reporting its own transfer. The chain is what hands this callback down,
        // so driving it is the only way the wiring above — that a callback is passed at all, and that the head of
        // the operation's range is divided between the *distinct* artifacts — is exercised rather than assumed.
        if self.installs
            && let Some(report) = &interest.on_progress
        {
            self.log.lock().unwrap().installed.push(artifact.clone());

            for fraction in [0.5, 1.0] {
                report(&Progress {
                    dependency: Dependency::Model(id.clone()),
                    phase: Phase::Downloading,
                    bytes: 0,
                    total: None,
                    fraction,
                });
            }
        }

        if self.fails_to_open.as_deref() == Some(artifact.as_str()) {
            return Err(SessionError::Build {
                artifact,
                provider: ExecutionProvider::Cpu,
                source: Arc::new(std::io::Error::other("the graph would not load")),
            });
        }

        // Something else asking for every session to be released, arriving between two of this chain's
        // operations. After the failures above, so a release is never charged against an acquisition that was
        // going to fail anyway.
        if self.release_before_acquire == Some(self.log.lock().unwrap().acquired.len()) {
            self.open.lock().unwrap().clear();
        }

        let native = {
            let mut open = self.open.lock().unwrap();

            match open.get(&artifact) {
                // One session served to every request for that artifact, which is what the resident cache does.
                Some(native) => Arc::clone(native),
                None => {
                    let mut opens = self.opens.lock().unwrap();
                    *opens += 1;

                    let native = Arc::new(FakeNative { open: *opens, freed: Arc::clone(&self.freed) });
                    open.insert(artifact.clone(), Arc::clone(&native));

                    native
                }
            }
        };

        let cancels = self.cancels.as_ref().filter(|(named, _)| *named == artifact).map(|(_, token)| token.clone());

        // The provider this artifact's session is built on. `SessionHandle::provider` is the only half of the
        // report folded out of the handles, so this is the whole of what a test has to set to drive a downgrade.
        let built_on = self.built_on.get(&artifact).copied().unwrap_or(self.builds_on);

        Ok(SessionHandle::held(
            FakeSession {
                fails: self.fails_to_run.as_deref() == Some(artifact.as_str()),
                artifact,
                log: Arc::clone(&self.log),
                cancels,
                reports: self.reports.clone(),
                native,
            },
            built_on,
        ))
    }

    fn run_tile(handle: &SessionHandle<Self::Session>, input: &[f32], output: &mut [f32]) -> Result<(), Self::Error> {
        let session = handle.session();
        let arrived = session.reports.as_ref().map_or(0, |reports| reports.lock().unwrap().len());
        let freed = session.native.freed.lock().unwrap().len();

        {
            let mut log = session.log.lock().unwrap();
            log.ran.push(session.artifact.clone());
            log.reports_at_tile.push(arrived);
            log.freed_at_tile.push(freed);
        }

        if let Some(token) = &session.cancels {
            token.cancel();
        }

        if session.fails {
            return Err(std::io::Error::other("the model failed"));
        }

        nearest_neighbour(input, output);

        Ok(())
    }

    /// The shaped call, recorded the same way [`Fake::run_tile`] records the tiled one and answered with an
    /// output derived from the input rather than with zeros, so a caller that dropped a stage would be visible.
    ///
    /// No `nearest_neighbour` here: the two sides of a shaped call are not one scaled by a whole factor, and the
    /// contract this stands in for is "the graph was run with these two shapes", which is what gets logged.
    fn run_graph(
        handle: &SessionHandle<Self::Session>,
        input: &[f32],
        input_shape: GraphShape,
        output: &mut [f32],
        output_shape: GraphShape,
    ) -> Result<(), Self::Error> {
        let session = handle.session();

        {
            let mut log = session.log.lock().unwrap();
            log.ran.push(session.artifact.clone());
        }

        if session.fails {
            return Err(std::io::Error::other("the model failed"));
        }

        // Stated rather than derived, because a caller that handed a buffer the declared shape does not describe
        // is the mistake this seam exists to make impossible — so the fake refuses it as the real one would.
        if input.len() != input_shape.len() {
            return Err(std::io::Error::other(format!(
                "the input holds {} floats, but {input_shape} needs {}",
                input.len(),
                input_shape.len()
            )));
        }
        if output.len() != output_shape.len() {
            return Err(std::io::Error::other(format!(
                "the output holds {} floats, but {output_shape} needs {}",
                output.len(),
                output_shape.len()
            )));
        }

        // The mean of the input, spread over the declared output: derived from what arrived, so a stage that was
        // skipped or run out of order does not produce what a correct sequence would.
        let mean = input.iter().sum::<f32>() / input.len() as f32;
        output.fill(mean);

        Ok(())
    }

    // The two seams this chain's models never reach — see `stub_backend_runs`.
    stub_backend_runs!(
        "nothing here runs a graph with more than one output, or one fed a value beside its image";
        run_named_outputs,
        run_weighted,
    );
}

/// Runs a chain through the fake with nothing watching it and no cancellation.
///
/// The pixels alone, since what most of these tests are about is what ran; [`run_reporting`] is the same call for
/// the ones about what it ran *on*.
async fn run_with(
    backend: &Fake,
    source: Arc<DynamicImage>,
    operations: &[Operation],
) -> Result<Arc<DynamicImage>, InferenceError> {
    run_reporting(backend, source, operations, ExecutionProvider::Auto)
        .await
        .map(|(pixels, _)| pixels)
}

/// Runs a chain through the fake asking for `provider`, and hands back what it ran on beside the pixels.
async fn run_reporting(
    backend: &Fake,
    source: Arc<DynamicImage>,
    operations: &[Operation],
    provider: ExecutionProvider,
) -> Result<(Arc<DynamicImage>, ProviderReport), InferenceError> {
    run_chain(
        backend,
        source,
        operations,
        provider,
        ChannelDepth::Eight,
        None,
        &CancellationToken::new(),
        None,
    )
    .await
}

/// [`run`] over `operations`, planned first the way `process` plans them, with the failing step left off the error.
#[expect(clippy::too_many_arguments, reason = "the arguments of `run`, forwarded")]
async fn run_chain(
    backend: &Fake,
    source: Arc<DynamicImage>,
    operations: &[Operation],
    provider: ExecutionProvider,
    depth: ChannelDepth,
    on_progress: Option<OnInference>,
    cancel: &CancellationToken,
    cache: Option<&RunCache>,
) -> Result<(Arc<DynamicImage>, ProviderReport), InferenceError> {
    let planned = plan::<Fake>(operations)?;

    run(backend, source, &planned, provider, depth, on_progress, cancel, cache)
        .await
        .map_err(|failure| failure.error)
}

/// Waits for every result a run has queued for the store to be written.
///
/// A run hands its result back before its writes land, and a read through `RunCache` waits for them — but a test
/// reading or overwriting the `Memo` directly goes round that, so it settles first.
fn settled() {
    assert!(crate::cache::settle(std::time::Duration::from_secs(60)), "the run cache's writes never landed");
}

/// A store held in memory for one test, which every cached run below shares.
///
/// Memory rather than disk not to avoid the disk tier — `cache.rs` opens a real one in a temporary directory —
/// but because what these tests are about is what a hit and a miss do to the *run*, and a store that needs no
/// directory keeps that the only thing in the frame.
fn store() -> Memo {
    Memo::memory(rust_sak::memo::CacheOpts::new()).expect("a memory store opens")
}

/// Runs `chain` over `source` through `backend`, reading and writing `cache`, and returns the result.
async fn cached_run(
    backend: &Fake,
    cache: Option<&Memo>,
    source: &Picture,
    chain: &[Operation],
    depth: OutputDepth,
) -> Result<Picture, InferenceError> {
    cached_enhancement(backend, cache, source, chain, depth).await.map(|enhanced| enhanced.picture)
}

/// [`cached_run`] with the whole bundle kept, for the tests about what a run reports having executed on.
async fn cached_enhancement(
    backend: &Fake,
    cache: Option<&Memo>,
    source: &Picture,
    chain: &[Operation],
    depth: OutputDepth,
) -> Result<Enhanced, InferenceError> {
    process(backend, cache, source, chain, Some(ProcessOptions { depth, ..Default::default() })).await
}

/// A Kyoto operation at `scale`, which is the one operation this slice serves.
fn kyoto(scale: f64) -> Operation {
    Operation::Upscale(Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp16), Scale::new(scale).unwrap()))
}

/// An upscale operation on `variant` at 4x, for the tests that vary the variant rather than the scale.
fn upscale(variant: UpscaleVariant) -> Operation {
    Operation::Upscale(Upscale::new(variant, Scale::new(4.0).unwrap()))
}

/// The artifacts a planned operation would open sessions for, in order.
///
/// What a planned operation is asserted through now that the driver holds a pipeline rather than a shape it can
/// take apart — and it is the same view the acquisition loop has, which is what makes these checks about the
/// thing that actually happens rather than about a field beside it. A pass sequence names weights per native
/// scale and a graph set names its three graphs, so the two contracts are not confusable here.
fn session_artifacts(planned: &Planned<Fake>) -> Vec<String> {
    planned
        .pipeline
        .sessions()
        .iter()
        .map(|(artifact, _)| (*artifact).as_str().to_string())
        .collect()
}

/// The settings each of a planned operation's sessions would be opened under, in the same order.
fn session_profiles(planned: &Planned<Fake>) -> Vec<EpProfile> {
    planned.pipeline.sessions().iter().map(|(_, profile)| (*profile).clone()).collect()
}

/// An image operation as the same subject, for the checks that need one on the data path's own derivation — see
/// [`identity_of_data`] for why that is the case worth checking.
fn as_subject(operation: Operation) -> crate::models::Subject {
    crate::models::Subject::Enhancement(operation)
}

/// A picture of known pixels and a known identity, as `image::load` would have produced one.
fn picture(width: u32, height: u32) -> Picture {
    Picture::new("/pictures/holiday.jpg", source(width, height), "cafebabecafebabe")
}
