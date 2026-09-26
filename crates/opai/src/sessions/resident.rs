//! The sessions held in memory, how long each one stays, and the handle a request holds on one.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use rust_sak::o11y::metric::Gauge;
use tracing::{Instrument as _, Span};

use crate::error::SessionError;
use crate::models::ArtifactId;
use crate::providers::ExecutionProvider;
use crate::task::lock;
use crate::telemetry::metrics;
use crate::telemetry::unit::{self, Outcome, Unit, unit_span};

use super::flight::{Flight, Install, Interest};

/// How long a session stays resident after it was last used.
///
/// One number rather than the reference implementation's scaling — a five-minute floor, extended to twenty times what
/// the build cost, capped at thirty minutes — because that scaling exists to cooperate with a memory budget this
/// series defers, and carrying it without its reason would make "why is this model still resident" a calculation
/// rather than a fact. Fifteen minutes is inside the range the reference's own floor and ceiling bracket.
pub(super) const IDLE_LIFE: Duration = Duration::from_secs(15 * 60);

/// How often the background task looks for sessions to release.
///
/// A quarter of [`IDLE_LIFE`] rather than the whole of it, and the two are deliberately different numbers. Sweeping on
/// the same period as the cutoff turns the idle life into the near edge of a window rather than a number: a session
/// falling idle a moment after one tick is not looked at until the next, so "released fifteen minutes after its last
/// use" would mean anything up to thirty. A quarter bounds that overshoot at under four minutes, and what it costs is
/// three extra passes an hour over a map that holds a handful of entries and is already behind a lock a session build
/// takes anyway.
pub(super) const SWEEP_INTERVAL: Duration = Duration::from_secs(IDLE_LIFE.as_secs() / 4);

/// What a resident session is filed under: the artifact, the provider it was actually built on, and whether it was
/// built as a rung of the `Auto` ladder.
///
/// The third field because a rung keeps every accelerator below it attached — `Auto`'s CUDA rung is CUDA with WebGPU
/// behind it — while an explicit request for the same provider attaches it alone. Two different sessions, which one
/// key would hand out as each other. A CPU session attaches nothing either way, so it is never filed as a rung.
///
/// The **artifact** rather than the operation that asked for one, so that two operations needing the same graph share
/// a session — an 8x Kyoto and a 4x Kyoto both need `up_kyoto_4x_fp16`, and the reference implementation, which keys
/// on the operation, builds that graph twice.
///
/// That is sound only while one artifact implies one profile, since the profile decides the options a session is
/// built with and this key does not carry it. It holds by construction — a profile is declared by the variant, and an
/// artifact name is composed from the family, the codename, the native scale and the precision — and there is a test
/// in `tests::resident` that says so, because the diffusion upscaler is the case that tests it: a per-graph override,
/// where one graph of a set wants a setting the others do not.
type Key = (ArtifactId, ExecutionProvider, bool);

/// The key a session for `artifact`, asked for as `requested` and built on `resolved`, is filed under.
fn key(artifact: &ArtifactId, requested: ExecutionProvider, resolved: ExecutionProvider) -> Key {
    let rung = requested == ExecutionProvider::Auto && resolved != ExecutionProvider::Cpu;

    (artifact.clone(), resolved, rung)
}

/// One session held in memory, and what is known about it.
///
/// Everything here is a property of the *session*, and nothing here is a property of a request that was served it:
/// one entry is handed to every request for that artifact on that provider, so a field recording what was asked for
/// would be whatever the request that happened to build it asked, reported to every request after it. That fact
/// belongs to [`SessionHandle`], which is per request.
///
/// [`provider`](Self::provider) is read through [`SessionHandle::provider`], which is what the chain folds its
/// [`ProviderReport`](crate::ProviderReport) out of at the end of a run.
#[derive(Debug)]
pub(super) struct Resident<S> {
    /// The session itself. A [`Mutex`] because `ort::session::Session::run` takes `&mut self`, although ONNX
    /// Runtime's own `Run` is thread-safe.
    session: Mutex<S>,
    /// The provider it was actually built on.
    provider: ExecutionProvider,
    /// When it was last handed out or let go of.
    last_used: Mutex<Instant>,
}

impl<S> Resident<S> {
    /// A session just built, stamped as used now.
    fn new(session: S, provider: ExecutionProvider) -> Self {
        Self { session: Mutex::new(session), provider, last_used: Mutex::new(Instant::now()) }
    }

    /// Stamps this entry as used now.
    fn touch(&self) {
        *lock(&self.last_used) = Instant::now();
    }

    /// When this entry was last handed out or let go of.
    fn last_used(&self) -> Instant {
        *lock(&self.last_used)
    }
}

/// A session in use.
///
/// Holding one is what keeps the session alive: the cache holds one [`Arc`] and this holds another, so dropping the
/// last handle is what frees the native resources — including for a session the cache has already let go of.
///
/// Its [`Drop`] stamps the entry as used. Both edges of a use are stamped, the near one when the handle is handed out
/// and the far one here, because stamping only the first would evict a session fifteen minutes after a two-hour run
/// *started*, moments after that run let go of it.
#[derive(Debug)]
pub(crate) struct SessionHandle<S> {
    /// The session being held, shared with the cache and with every other handle on it.
    entry: Arc<Resident<S>>,
}

impl<S> SessionHandle<S> {
    /// Hands out `entry`, stamping it as used.
    fn new(entry: Arc<Resident<S>>) -> Self {
        entry.touch();
        Self { entry }
    }

    /// The execution provider this session was actually built on.
    pub(crate) fn provider(&self) -> ExecutionProvider {
        self.entry.provider
    }

    /// A handle onto `session` as though the cache had just built it on `provider`, for a suite with no runtime.
    ///
    /// The one way to hold a session that the cache did not hand out, and it exists so that the pipeline above this
    /// layer — the chain, the pass sequence, the progress weighting, every refusal — is drivable against a fake
    /// session on a runner with no ONNX Runtime, which is where all of that arithmetic is actually checked.
    #[cfg(test)]
    pub(crate) fn held(session: S, provider: ExecutionProvider) -> Self {
        Self::new(Arc::new(Resident::new(session, provider)))
    }

    /// Borrows the session for a run.
    ///
    /// Exclusive, because `ort::session::Session::run` takes `&mut self`. Two different models still run
    /// concurrently; two runs of *one* model against one GPU are serialized, which is not a throughput loss.
    pub(crate) fn session(&self) -> MutexGuard<'_, S> {
        lock(&self.entry.session)
    }
}

impl<S> Drop for SessionHandle<S> {
    fn drop(&mut self) {
        self.entry.touch();
    }
}

/// The sessions this application is holding in memory.
///
/// Generic in the session type, and building is a closure the caller passes, so the whole of the policy — residency,
/// the idle life, the single flight, eviction — is exercisable on a runner with no runtime, no GPU and no model file.
/// Production instantiates it at `ort::session::Session`; the tests instantiate it at a counting fake.
///
/// Generic in the error type too, which is what the map of builds in flight needs in order to hand one build's
/// failure to every request that was waiting on it.
pub(crate) struct SessionCache<S, E = SessionError> {
    /// The sessions held, by artifact and the provider they were built on.
    entries: Mutex<HashMap<Key, Arc<Resident<S>>>>,
    /// The builds in flight, so that several requests for one model open it once and a failure reaches all of them.
    ///
    /// A separate map rather than a third state in the one above, so that an entry being built is never something the
    /// sweep or [`clear`](Self::clear) has to know about.
    pub(super) flights: Mutex<HashMap<Key, Arc<Flight<S, E>>>>,
    /// The providers that threw an error for an artifact under `Auto`, at build or at run, which its ladder skips from
    /// then on. Emptied by [`clear`](Self::clear), so an explicit release gives every provider another chance.
    declined: Mutex<HashSet<(ArtifactId, ExecutionProvider)>>,
    /// Where the count of resident sessions is published: this process's instrument, or a test's own.
    resident_gauge: Gauge,
}

impl<S, E> std::fmt::Debug for SessionCache<S, E> {
    /// Reports how many sessions are resident rather than what they are: a session is a native handle with nothing to
    /// print, and the count is the fact a reader of this actually wants.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCache").field("resident", &self.resident()).finish()
    }
}

impl<S, E> Default for SessionCache<S, E> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            flights: Mutex::new(HashMap::new()),
            declined: Mutex::new(HashSet::new()),
            resident_gauge: metrics::RESIDENT_SESSIONS.clone(),
        }
    }
}

impl<S, E> SessionCache<S, E> {
    /// A cache publishing its resident count to `gauge` rather than to this process's instrument.
    ///
    /// The suite holds many caches and the process one gauge, which each of them writes whole: a test reading it
    /// would read whichever cache published last.
    #[cfg(test)]
    pub(super) fn publishing_to(gauge: Gauge) -> Self {
        Self { resident_gauge: gauge, ..Self::default() }
    }
}

/// The artifacts a release let go of, as one field value: `up_kyoto_4x_fp16,up_tokyo_2x_fp16`.
///
/// **Sorted**, because the map these come out of has no stable order and a field a reader compares across two
/// sessions of the same application has to have one.
///
/// **Duplicates kept**, because they are not duplicates: one artifact resident on two execution providers is two
/// sessions, and the `released` count beside this says two. Collapsing them would make the field disagree with the
/// count for the one case — a downgrade, which files a CPU session beside a GPU one — that a reader is most likely
/// to be reading this record about.
fn released_artifacts(mut artifacts: Vec<String>) -> String {
    artifacts.sort_unstable();
    artifacts.join(",")
}

impl<S, E> SessionCache<S, E> {
    /// Sets the resident-session gauge from `entries`, one series per provider a session can be built on.
    ///
    /// Called under the lock that just changed them, so two changes cannot publish out of order. Every provider is
    /// written, zeros included, so one emptied by a sweep reads `0` rather than its last count.
    fn publish_resident(&self, entries: &HashMap<Key, Arc<Resident<S>>>) {
        // Every provider a session can be built on, which leaves out `Auto`: that is what a request asks for.
        for provider in ExecutionProvider::BUILT_ON {
            let count = entries.keys().filter(|(_, built_on, _)| *built_on == provider).count();
            let count = u32::try_from(count).unwrap_or(u32::MAX);
            self.resident_gauge.set_with_tags(count, &[("provider", provider.as_str())]);
        }
    }

    /// How many sessions are resident.
    ///
    /// A count a caller can act on, which the eviction records are not: they say what happened and when, and this
    /// says what is true now — a front end drawing "3 models in memory" cannot read a log.
    pub(crate) fn resident(&self) -> usize {
        lock(&self.entries).len()
    }

    /// The session filed under `key`, if one is.
    ///
    /// One map lookup and nothing else — no filesystem work at all, which is what makes a cache hit affordable on a
    /// path the slice that runs inference will take once per tile.
    pub(super) fn get(&self, key: &Key) -> Option<SessionHandle<S>> {
        lock(&self.entries).get(key).map(|entry| SessionHandle::new(Arc::clone(entry)))
    }

    /// Whether `provider` threw an error for `artifact` under `Auto` since the last [`clear`](Self::clear).
    pub(crate) fn is_declined(&self, artifact: &ArtifactId, provider: ExecutionProvider) -> bool {
        lock(&self.declined).contains(&(artifact.clone(), provider))
    }

    /// Records that `provider` threw an error for `artifact` under `Auto`, and lets go of every session of that
    /// artifact filed on it — so the next request builds on the rung below, and the failed session's memory is freed
    /// as soon as whatever is still holding it lets go.
    ///
    /// Returns whether it was newly declined.
    pub(crate) fn decline(&self, artifact: &ArtifactId, provider: ExecutionProvider) -> bool {
        let newly = lock(&self.declined).insert((artifact.clone(), provider));

        let mut entries = lock(&self.entries);
        entries.retain(|(filed, built_on, _), _| !(filed == artifact && *built_on == provider));
        self.publish_resident(&entries);

        newly
    }

    /// Releases every resident session.
    ///
    /// Returns without waiting on a run in progress: it drops the cache's own reference to each session, and one
    /// something is still using is released when that use ends.
    ///
    /// The converse is what a caller measuring a build relies on, and it is the same fact read the other way: an
    /// entry nothing else holds reaches a refcount of zero as this clears the map, so its native session is dropped
    /// *inside this call*. There is no deferred sweep behind it to make a later rebuild look free.
    ///
    /// It also forgets every provider `Auto` had declined, so the next request tries the whole ladder again.
    ///
    /// Reached from [`Opai::release_sessions`](crate::Opai::release_sessions), which is what the application
    /// embedding this library calls.
    pub(crate) fn clear(&self) {
        let mut entries = lock(&self.entries);
        let evicted: Vec<String> = entries.keys().map(|(artifact, _, _)| artifact.to_string()).collect();
        entries.clear();
        self.publish_resident(&entries);
        drop(entries);
        lock(&self.declined).clear();

        let released = evicted.len();
        let artifacts = released_artifacts(evicted);

        // Written even for nothing, because "the application asked for its models back and there were none" is a
        // different thing to read than silence. `info`: it is bounded by what a front end asks for.
        tracing::info!(released, %artifacts, reason = "requested", "released resident sessions");
    }

    /// Releases every entry that nothing is using and that has not been used since `evict_unused_since`.
    ///
    /// The cutoff is a parameter rather than read from the clock here, which is what lets a test evict everything by
    /// passing the present and nothing by passing an hour ago, with no fake clock and no sleeping. The fifteen
    /// minutes is then one constant applied at one call site, which is [`spawn_sweeper`].
    ///
    /// A session something is holding survives whatever its timestamp: a single run can legitimately last far longer
    /// than the interval. The count is read under this lock, and the only thing that can increment it is handing out
    /// a handle, which happens under the same lock.
    pub(super) fn sweep(&self, evict_unused_since: Instant) {
        let mut entries = lock(&self.entries);

        // Collected by the `retain` predicate itself rather than by a pass before or after it. A second walk would
        // have to re-evaluate `strong_count`, which a handle dropped on another thread can change under this lock —
        // `SessionHandle::drop` touches the entry, not this map — so the two passes could disagree about which
        // entries went, and the record would name something that survived.
        let mut evicted: Vec<String> = Vec::new();
        entries.retain(|(artifact, _, _), entry| {
            let keep = Arc::strong_count(entry) > 1 || entry.last_used() > evict_unused_since;
            if !keep {
                evicted.push(artifact.to_string());
            }
            keep
        });
        self.publish_resident(&entries);
        drop(entries);

        let released = evicted.len();
        let artifacts = released_artifacts(evicted);

        // Only when it did something. This runs every few minutes for the life of the process, and a line each time
        // saying nothing happened is how a log stops being read — the volume rule applied to a background task.
        if released > 0 {
            tracing::info!(released, %artifacts, reason = "idle", "released resident sessions");
        }
    }
}

// Narrowed to `SessionError` rather than generic in the error, because the build records its own failure and needs to
// read it: whether it was a stop, and what it says. Every cache in the crate already takes the default.
impl<S> SessionCache<S, SessionError> {
    /// The session for `artifact` on `resolved`, building it through `build` if none is resident.
    ///
    /// Concurrent requests for one key are served by a **single** build: the first to arrive runs `build` and every
    /// other awaits it, so a model that is not resident is opened exactly once — and a build that fails fails every
    /// request waiting on it rather than each of them starting its own attempt at something that has just proved
    /// impossible. A later request builds again, which is what makes a transient failure recoverable.
    ///
    /// `requested` is not part of the key and is not recorded on the entry or the handle: what a session *is* depends
    /// on the provider it was built on, not on what was asked for. It is carried only into the build's records, so a
    /// downgrade's CPU build reads as one event with the provider that would not open the model; a caller that needs
    /// the requested half of a downgrade takes it from its own argument, as the run's report does.
    ///
    /// # What a request's own cancellation does here
    ///
    /// It **withdraws** the request, and nothing else. It does not end this call, because the request may be the one
    /// driving the build and dropping a build half a dozen requests are waiting on — or one that is minutes into
    /// compiling an engine — would be a cure worse than the wait. What withdrawing does is take the request out of
    /// the audience, and a build whose audience empties stops transferring: see
    /// [`Audience`](super::flight::Audience) and [`Install`].
    ///
    /// So a cancelled request keeps waiting, and what it is waiting for is now over in milliseconds rather than
    /// gigabytes. It is handed `None` — the build stopped, there is no session and nothing failed — which its caller
    /// reports as the stop it is.
    ///
    /// # Errors
    ///
    /// Returns whatever `build` reported, to every request that was waiting on it.
    pub(super) async fn get_or_build<F, Fut>(
        &self,
        artifact: &ArtifactId,
        requested: ExecutionProvider,
        resolved: ExecutionProvider,
        interest: &Interest,
        build: F,
    ) -> Result<Option<SessionHandle<S>>, SessionError>
    where
        F: FnOnce(Install) -> Fut,
        Fut: Future<Output = Result<Option<S>, SessionError>>,
    {
        // The request's span, where the caller opened one: whether it was served from memory, and which build it
        // waited on.
        let request = Span::current();

        let key = key(artifact, requested, resolved);

        if let Some(handle) = self.get(&key) {
            // `debug`: a chain acquires a handle per pass, so this repeats within one run. It is the record that
            // answers "why was the second image instant" — matching the level the reference gives its own.
            tracing::debug!(%artifact, provider = %resolved, "session already resident");
            request.record("resident", true);
            return Ok(Some(handle));
        }
        request.record("resident", false);

        let flight = {
            let mut flights = lock(&self.flights);

            match flights.get(&key) {
                // A spent flight is not joined — see [`Flight::spent`] — and is replaced rather than waited for,
                // which is safe because its build has finished: nothing it owned is still writing to the disk.
                Some(current) if !current.spent() => {
                    // Linked rather than parented: the build is in the trace of the request that started it, and
                    // this one waited on it. One that is already over has nothing to wait on, and nothing to link.
                    if !current.cell.initialized()
                        && let Some(build) = &current.build_id
                    {
                        request.follows_from(build.clone());
                    }
                    Arc::clone(current)
                }
                _ => {
                    // Created by the request that starts the flight, as its child, and carried out by whichever
                    // request runs the build. See `Flight::build_span`.
                    let build = unit_span!(
                        "build",
                        artifact = %artifact,
                        provider = %resolved,
                        requested = %requested
                    );
                    let fresh = Arc::new(Flight::new(build));
                    flights.insert(key.clone(), Arc::clone(&fresh));
                    fresh
                }
            }
        };

        // Joined before the cell is awaited, so a request that joins a transfer already in flight both hears the
        // rest of it and keeps it running. The guard gives the place back when this call returns however it returns
        // — including when the future it is in is dropped rather than awaited to the end.
        let mut waiting = flight.join(interest);
        // Composed out here rather than inside the closure below, which borrows the cell it is passed to.
        let installing = flight.installing();

        let started = Instant::now();

        // Every request holding this cell awaits the one initialization, and each is handed the same outcome.
        //
        // The pair of records is **inside** the flight, which is what makes them an account of the work rather than
        // of the requests: ten concurrent requests for one model produce one "building" and one "ready", because
        // nine of them await this cell rather than entering the closure.
        let building = flight.cell.get_or_init(|| {
            let held = flight.take_build_span();
            let instrumented = held.span().clone();

            async move {
            tracing::info!(%artifact, provider = %resolved, "building session");

            let built = build(installing)
                .await
                .map(|session| session.map(|session| Arc::new(Resident::new(session, resolved))));

            match &built {
                // `requested` beside `provider`, always — so the CPU build that follows a downgrade reads as one
                // event with the warning above it, and an honoured request shows the two agreeing.
                Ok(Some(_)) => {
                    let duration = started.elapsed();
                    tracing::info!(%artifact, provider = %resolved, %requested, ?duration, "session ready");
                    unit::ended(Unit::SessionBuild, held.span(), duration, Outcome::Finished);
                }
                // `info` rather than `warn`, and it is the record that answers "why is there no model on this
                // machine after all that": the transfer stopped because the user stopped asking for it, which is not
                // a failure and leaves a partial to resume onto. The same message as a shutdown's below, told apart by
                // `reason`, so one query over stopped builds finds both.
                Ok(None) => {
                    let duration = started.elapsed();
                    tracing::info!(%artifact, provider = %resolved, reason = "cancelled", ?duration, "the build stopped");
                    unit::ended(Unit::SessionBuild, held.span(), duration, Outcome::Stopped);
                }
                // Recorded here, inside the one closure a build runs, so ten requests waiting on a build that fails
                // write one record rather than ten. The error is on this record and not on the fallback's that may
                // follow it: that one says only what it decides.
                Err(error) => {
                    let duration = started.elapsed();
                    match error {
                        SessionError::Install(install) if let Some(reason) = install.stop_reason() => {
                            tracing::info!(%artifact, provider = %resolved, reason, ?duration, "the build stopped");
                            unit::ended(Unit::SessionBuild, held.span(), duration, Outcome::Stopped);
                        }
                        _ => {
                            tracing::warn!(
                                %artifact,
                                provider = %resolved,
                                %requested,
                                ?duration,
                                %error,
                                "session build failed"
                            );
                            unit::ended(
                                Unit::SessionBuild,
                                held.span(),
                                duration,
                                Outcome::Failed { kind: error.kind(), error },
                            );
                        }
                    }
                }
            }
            held.ended();

            built
            }
            .instrument(instrumented)
        });

        // The wait, with this request's own cancellation watched beside it. The branch is disabled once the request
        // has withdrawn, because a cancelled token resolves immediately and would otherwise spin this loop for as
        // long as the build takes.
        let outcome = {
            tokio::pin!(building);

            loop {
                tokio::select! {
                    outcome = &mut building => break outcome.clone(),
                    () = interest.cancel.cancelled(), if waiting.waiting() => waiting.withdraw(),
                }
            }
        };

        // Filed once however many requests were waiting, so all of them are served the one session. A key that was
        // swept in between is refilled with what this build produced rather than built a second time.
        //
        // Before the flight is dropped below, and that order is the whole of what closes the window: a request
        // arriving between the two would otherwise find neither map holding this key and start a second build of
        // what had just been built — a redundant engine compile of minutes, for a model already in hand.
        let filed = outcome.map(|built| {
            built.map(|built| {
                let mut entries = lock(&self.entries);
                let entry = Arc::clone(entries.entry(key.clone()).or_insert(built));
                self.publish_resident(&entries);
                entry
            })
        });

        // By identity, so a request that merely waited cannot remove a flight that a later request has since
        // installed for the same key. On the failed and the stopped paths too: nothing was filed, and a later
        // request is meant to build again rather than be handed an outcome that is over.
        {
            let mut flights = lock(&self.flights);
            if flights.get(&key).is_some_and(|current| Arc::ptr_eq(current, &flight)) {
                flights.remove(&key);
            }
        }

        Ok(filed?.map(SessionHandle::new))
    }
}

/// Starts the background task that releases sessions left unused.
///
/// Holds a [`Weak`] rather than the cache, so the task ends when the application it belongs to is dropped rather than
/// keeping it alive for the life of the process.
pub(super) fn spawn_sweeper<S, E>(cache: &Arc<SessionCache<S, E>>)
where
    S: Send + 'static,
    E: Send + Sync + 'static,
{
    let cache = Arc::downgrade(cache);

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        // `interval` fires immediately the first time, and there is nothing to sweep at the moment it is started.
        ticker.tick().await;

        loop {
            ticker.tick().await;

            let Some(cache) = Weak::upgrade(&cache) else { break };

            // The cutoff is [`IDLE_LIFE`] and the period is [`SWEEP_INTERVAL`]: what decides whether a session has
            // gone unused is how long ago it was last used, never how long ago this task last looked.
            //
            // `checked_sub` because a monotonic clock younger than the idle life is possible — a freshly booted
            // container — and nothing can have been unused for longer than the clock has run.
            if let Some(cutoff) = Instant::now().checked_sub(IDLE_LIFE) {
                cache.sweep(cutoff);
            }
        }
    });
}
