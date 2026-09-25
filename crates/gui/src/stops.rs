//! The jobs in flight that the window can stop by name, and the stops that overtook their own job.
//!
//! [`Stops`] is the table; [`Registration`] is one job's place in it. Autopilot's analyses and exports each have a
//! table of their own — see [`Stops`] for why two and not one.
//!
//! # Why this is its own module
//!
//! It was `autopilot.rs`'s until export became its second caller. Nothing in it is about either kind of job, so it
//! moved here before export was written rather than being copied beside it.

use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::sync::Mutex;

use opai::CancellationToken;

use crate::sync::lock;

/// How many stops that arrived ahead of their job are remembered. See [`Stops`] for why a handful is enough.
const EARLY_STOPS: usize = 16;

/// The jobs of one kind in flight, by the window's name for each, and the stops that overtook their own job.
///
/// ```text
///    <job>(run, ..)                        cancel_<job>(run)
///         |                                    |
///         v                                    v
///    +------------------------------------------------------+
///    |  running: run --> token      stop: cancel + remove  |
///    |  stopped: [run, ..]          (or remember, bounded) |
///    +------------------------------------------------------+
/// ```
///
/// **Not the enhancement slot.** [`Runs`](crate::enhance::Runs) holds one run and supersedes it on the next write;
/// jobs here must not supersede one another, and must not disturb the run in flight. Several can be in flight at
/// once.
///
/// **Keyed by run name rather than by image identity.** The identity is a content identity, so two open files with
/// the same bytes share one, and stopping "the job for this identity" when one of them is closed would stop the
/// other's.
///
/// **One table per kind of job**, told apart by `Of`, because Tauri keys managed state by type. So a stop sent for
/// one kind — `cancel_suggest`, say — can never reach a job of another, even though run names cannot collide.
pub(crate) struct Stops<Of> {
    // A `std::sync::Mutex` rather than tokio's — nothing holds a guard across an `await`. Poisoning is treated as
    // usable, through `crate::sync::lock`: a panicking holder leaves a perfectly usable table.
    held: Mutex<Held>,
    // `fn() -> Of` rather than `Of`, so the table is `Send` and `Sync` whatever the marker is.
    of: PhantomData<fn() -> Of>,
}

impl<Of> Default for Stops<Of> {
    // Written out rather than derived: a derive would demand `Of: Default` of a marker that is never constructed.
    fn default() -> Self {
        Self { held: Mutex::default(), of: PhantomData }
    }
}

impl<Of> std::fmt::Debug for Stops<Of> {
    // Written out for the same reason as `Default`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stops").field("held", &self.held).finish()
    }
}

/// The inside of [`Stops`], behind its one lock.
#[derive(Debug, Default)]
struct Held {
    /// The jobs in flight, and the token that stops each.
    running: HashMap<String, CancellationToken>,
    // Bounded because a stop for a job that already finished lands here too and is never matched: without a bound the
    // set would grow for the whole session. An evicted early stop can only matter if a stop overtakes its own job by
    // more than `EARLY_STOPS` other stops, and the window stops one job per photograph it gives up on.
    /// The names of jobs a stop arrived for before the job did, oldest first.
    stopped: VecDeque<String>,
}

impl<Of> Stops<Of> {
    /// Registers the job called `run`, or answers `None` if a stop for it has already arrived.
    ///
    /// An early stop is **consumed** by this, so it is matched once. The registration deregisters itself when it
    /// is dropped — however the job ends, a panic included — so a finished job never leaves a token behind.
    pub(crate) fn register(&self, run: &str) -> Option<Registration<'_, Of>> {
        let mut held = lock(&self.held);

        if let Some(position) = held.stopped.iter().position(|stopped| stopped == run) {
            held.stopped.remove(position);

            return None;
        }

        let cancel = CancellationToken::new();
        held.running.insert(run.to_string(), cancel.clone());

        Some(Registration { stops: self, run: run.to_string(), cancel })
    }

    /// Stops the job called `run`, or remembers the request for a job that has not registered yet.
    ///
    /// Answers whether anything was stopped. **Touches no other job.**
    pub(crate) fn stop(&self, run: &str) -> bool {
        let mut held = lock(&self.held);

        if let Some(cancel) = held.running.remove(run) {
            cancel.cancel();

            return true;
        }

        // Either already finished, or not registered yet — indistinguishable here, so remember the name and let
        // `register` decide.
        if !held.stopped.iter().any(|stopped| stopped == run) {
            if held.stopped.len() == EARLY_STOPS {
                held.stopped.pop_front();
            }

            held.stopped.push_back(run.to_string());
        }

        false
    }

    /// Whether no job is registered, so a caller's tests can check a finished or refused job left nothing behind.
    #[cfg(test)]
    pub(crate) fn is_idle(&self) -> bool {
        lock(&self.held).running.is_empty()
    }
}

/// One job's place in [`Stops`]: the token that stops it, held for as long as the job runs.
#[derive(Debug)]
pub(crate) struct Registration<'a, Of> {
    /// The table to leave when dropped.
    stops: &'a Stops<Of>,
    /// The window's name for this job.
    run: String,
    /// The token that stops it, handed to the library's options as `cancel`.
    cancel: CancellationToken,
}

impl<Of> Registration<'_, Of> {
    /// The token that stops this job.
    pub(crate) fn cancel(&self) -> &CancellationToken {
        &self.cancel
    }
}

impl<Of> Drop for Registration<'_, Of> {
    fn drop(&mut self) {
        lock(&self.stops.held).running.remove(&self.run);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A kind of job for the table under test, standing in for either real one.
    enum Job {}

    #[test]
    fn a_stop_cancels_the_named_job_and_leaves_another_alone() {
        let stops = Stops::<Job>::default();

        let first = stops.register("run-1").expect("nothing stopped it");
        let second = stops.register("run-2").expect("nothing stopped it");

        assert!(stops.stop("run-1"), "a stop naming a job in flight did nothing");

        assert!(first.cancel().is_cancelled());
        assert!(!second.cancel().is_cancelled(), "stopping one job stopped another");
    }

    #[test]
    fn a_stop_that_overtakes_its_own_job_still_lands_on_it_once() {
        let stops = Stops::<Job>::default();

        assert!(!stops.stop("run-1"), "nothing was in flight to stop");
        assert!(stops.register("run-1").is_none(), "a job went on after a stop had asked it not to");

        // Consumed: a second registration under the same name is not stopped by the same early stop.
        assert!(stops.register("run-1").is_some(), "an early stop was matched twice");
    }

    #[test]
    fn a_stop_for_a_finished_job_does_nothing_to_a_later_one() {
        let stops = Stops::<Job>::default();

        drop(stops.register("run-1").expect("nothing stopped it"));

        assert!(!stops.stop("run-1"), "a finished job claimed to have been stopped");

        let later = stops.register("run-2").expect("a stop for another job stopped this one");

        assert!(!later.cancel().is_cancelled());
    }

    #[test]
    fn a_finished_job_leaves_no_token_behind_even_when_it_panics() {
        let stops = Stops::<Job>::default();

        drop(stops.register("run-1").expect("nothing stopped it"));
        assert!(stops.is_idle(), "a finished job left its token in the table");

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _registration = stops.register("run-2").expect("nothing stopped it");

            panic!("a job that panics");
        }));

        assert!(panicked.is_err());
        assert!(stops.is_idle(), "a job that panicked left its token in the table");
    }

    #[test]
    fn the_early_stops_remembered_never_exceed_their_bound() {
        let stops = Stops::<Job>::default();

        for index in 0..EARLY_STOPS * 3 {
            stops.stop(&format!("finished-{index}"));
        }

        assert_eq!(lock(&stops.held).stopped.len(), EARLY_STOPS, "the record of early stops grew past its bound");

        // The most recent is still remembered, the oldest is not.
        assert!(stops.register(&format!("finished-{}", EARLY_STOPS * 3 - 1)).is_none());
        assert!(stops.register("finished-0").is_some());
    }
}
