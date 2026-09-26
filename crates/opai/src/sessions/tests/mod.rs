//! The session tests, and the fake sessions, builders and application directories they share.

mod downgrade;
mod flight;
mod install;
mod log;
mod resident;
mod spans;

use super::flight::Flight;
use super::*;
use crate::deps::test_server::{Reply, TestServer, fixture_listing};
use crate::logging::{self, field, records};
use crate::models::catalogue;
use crate::models::test_support::{kyoto, tokyo};
use crate::providers::tests::machine_supporting;
use crate::task::lock;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

/// The application name every test here resolves directories from.
const NAME: &str = "opai-test";

/// A stand-in for an `ort::session::Session`: a serial number, so two sessions can be told apart.
///
/// What the fake buys is the whole of the policy — residency, the idle life, the single flight, the fallback and
/// the install — exercised on a runner with no runtime, no GPU and no model file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fake(usize);

/// A stand-in for a session whose **release** is observable: it records its serial number when it is dropped.
///
/// The one thing [`Fake`] cannot answer. `resident() == 0` says the map no longer names an entry, which is not
/// the requirement: the native resources have to be gone *before `clear` returns*, because a caller that
/// releases everything and builds a model again is entitled to have paid the full construction cost for it. A
/// `Copy` serial number has no moment of release to observe, so the two tests below use this instead — the same
/// device the chain's suite uses for "freed while something was still using it", pointed at the other half of
/// the question.
#[derive(Debug)]
struct Dropping {
    /// Which session this is, so the log says *what* went and not only how many.
    serial: usize,
    /// Where it records itself when its last holder lets go.
    released: Arc<Mutex<Vec<usize>>>,
}

impl Drop for Dropping {
    fn drop(&mut self) {
        lock(&self.released).push(self.serial);
    }
}

/// Builds `serial` into `cache` and lets go of the handle, so the cache's own reference is the only one left.
async fn build_and_release(
    cache: &SessionCache<Dropping>,
    released: &Arc<Mutex<Vec<usize>>>,
    id: &ArtifactId,
    serial: usize,
) {
    let session = Dropping { serial, released: Arc::clone(released) };
    let handle = cache
        .get_or_build(id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &Interest::default(), |_| async {
            Ok::<Option<Dropping>, SessionError>(Some(session))
        })
        .await
        .unwrap()
        .expect("the build was not stopped");

    drop(handle);
}

/// A failure of the kind the provider fallback retries: the model is on disk and the provider would not open it.
fn build_failure(artifact: &ArtifactId, provider: ExecutionProvider) -> SessionError {
    SessionError::Build {
        artifact: artifact.as_str().to_string(),
        provider,
        source: Arc::new(io::Error::other("the provider would not build the model")),
    }
}

/// A failure that is not about the provider: the model file cannot be read, which fails the same way on the CPU.
fn unreadable(model: &Path) -> SessionError {
    SessionError::from(InitError::Io {
        path: model.to_path_buf(),
        source: io::Error::other("the model file could not be read"),
    })
}

/// What a fake builder was asked for, and how it should answer.
#[derive(Default)]
struct Bench {
    /// One entry per build actually performed, in the order they started.
    builds: Mutex<Vec<BuildRequest>>,
    /// Providers whose builds fail as a model that would not open.
    fails_on: Vec<ExecutionProvider>,
    /// Where set, every build fails as a model that could not be read rather than one that would not open.
    unreadable: bool,
}

impl Bench {
    /// The providers every build so far was planned for, in order.
    fn built_on(&self) -> Vec<ExecutionProvider> {
        lock(&self.builds).iter().map(|request| request.plan.resolved).collect()
    }

    /// How many builds have been performed.
    fn builds(&self) -> usize {
        lock(&self.builds).len()
    }
}

/// The builder `bench` describes.
fn builder(bench: &Arc<Bench>) -> Builder<Fake> {
    let bench = Arc::clone(bench);

    Arc::new(move |request: BuildRequest| {
        let provider = request.plan.resolved;
        let artifact = request.artifact.clone();
        let model = request.model.clone();

        let serial = {
            let mut builds = lock(&bench.builds);
            builds.push(request);
            builds.len()
        };

        if bench.unreadable {
            return Err(unreadable(&model));
        }
        if bench.fails_on.contains(&provider) {
            return Err(build_failure(&artifact, provider));
        }

        Ok(Fake(serial))
    })
}

/// An application directory to install and resolve beneath, and the temporary root keeping it alive.
fn app() -> (TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join(NAME);

    (root, app_dir)
}

/// A listing publishing each artifact's graph, pinned to the served fixture.
fn listing_for(artifacts: &[&ArtifactId]) -> Listing {
    fixture_listing(&artifacts.iter().map(|id| format!("{id}.onnx")).collect::<Vec<_>>())
}

/// Sessions installing from `server` against `listing`, building through `bench`, on a machine reporting
/// `supported`.
fn sessions(
    bench: &Arc<Bench>,
    server: &TestServer,
    listing: Listing,
    app_dir: &Path,
    supported: SupportedProviders,
) -> Sessions<Fake> {
    trusting(bench, server, listing, app_dir, supported, ModelTrust::Published)
}

/// [`sessions`] under an explicit declaration about the model files on disk.
fn trusting(
    bench: &Arc<Bench>,
    server: &TestServer,
    listing: Listing,
    app_dir: &Path,
    supported: SupportedProviders,
    trust: ModelTrust,
) -> Sessions<Fake> {
    Sessions::with(
        app_dir.to_path_buf(),
        NAME.to_string(),
        supported,
        trust,
        ModelSource::At { base_url: server.base_url.clone(), listing },
        builder(bench),
    )
}

/// A cache of fakes, and a counter of how many builds it performed.
fn counting_cache() -> (Arc<SessionCache<Fake>>, Arc<AtomicUsize>) {
    (Arc::new(SessionCache::default()), Arc::new(AtomicUsize::new(0)))
}

/// Builds one fake through `cache`, counting the build.
async fn build_through(
    cache: &SessionCache<Fake>,
    builds: &AtomicUsize,
    id: &ArtifactId,
    provider: ExecutionProvider,
) -> Result<SessionHandle<Fake>, SessionError> {
    cache
        .get_or_build(id, provider, provider, &Interest::default(), |_| async {
            Ok(Some(Fake(builds.fetch_add(1, Ordering::SeqCst) + 1)))
        })
        .await
        .map(|handle| handle.expect("the build was not stopped"))
}
