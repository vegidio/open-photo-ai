//! The one run this application has in flight, and the one result it has produced.
//!
//! Its records name no run: [`Runs::finish`] is reached inside the `enhance` command's span, which carries it, and the
//! file writes it onto each record from there. See `crate::command`.

use std::collections::VecDeque;
use std::sync::Mutex;

use opai::{CancellationToken, Picture};

use crate::sync::lock;

/// The one run this application has in flight, and the one result it has produced.
///
/// ```text
///    enhance(run, source, operations)
///         |
///         v
///    +--------------------------------------------------+
///    |  THE SLOT                                         |
///    |    id:     the window's name for this run         |
///    |    cancel: its token       --> cancel() the one   |
///    |    result: Option<Picture>     it displaces       |
///    +--------------------------------------------------+
///         ^                                   |
///         |                                   v
///    cancel_enhance(run)                 opai:// serve
///    (nothing unless the id matches)     (an Arc clone, never a copy)
/// ```
///
/// What it remembers beyond the run:
///
/// - [`produced`](Held::produced): the identities recently handed back, so a displaced result is
///   distinguishable from one that never existed. Bounded — see [`Produced`].
/// - [`stopped`](Held::stopped): the name of a stop that arrived before its run, so a race between the two
///   still lands correctly.
///
/// Two things let go of the result: a later run displacing it, and the window closing the photograph it was
/// made from, through [`release`](Runs::release).
#[derive(Debug, Default)]
pub(crate) struct Runs {
    // The backend supersedes rather than trusting the window to cancel: the slot exists anyway for holding the
    // result, so cancelling on write is nearly free and turns an invariant the interface could break into one it
    // can't — an unmount or frontend fault can't leave a run holding sessions.
    //
    // Released as well as displaced, because displacement alone would leave a large result resident for the whole
    // session whenever no next run came — a fourfold enlargement of a large scan is gigabytes of pixels.
    //
    // A `std::sync::Mutex` rather than tokio's — nothing holds a guard across an `await`. Poisoning is treated as
    // usable, through `crate::sync::lock`: a panicking holder leaves a perfectly usable slot.
    held: Mutex<Held>,
}

/// The inside of [`Runs`], behind its one lock.
#[derive(Debug, Default)]
struct Held {
    // One struct rather than several mutexes because writing a run updates all of it atomically.
    /// The run in flight, or the one that most recently finished and left its result behind.
    current: Option<Run>,
    /// The enhanced identities this application has recently handed back, whether or not it still holds the
    /// pixels.
    produced: Produced,
    /// The name of a run a stop arrived for before the run did.
    stopped: Option<String>,
}

/// One run: what the window called it, how to stop it, and what it produced.
#[derive(Debug)]
struct Run {
    /// The window's name for this run. What a stop has to match before it does anything.
    id: String,
    /// The identity of the image this run is over. What a release has to match before it drops anything.
    source: String,
    /// The token that stops it, handed to [`opai::ProcessOptions::cancel`].
    cancel: CancellationToken,
    /// What it produced, once it has. `None` while working, or if it never finished.
    result: Option<Picture>,
    // Cancellation is cooperative and checked between tiles, so a run whose photograph has been closed can still
    // answer `Ok` without ever noticing — a fully-cached chain does no tiling at all — and keeping those pixels would
    // leave the result of a closed photograph resident for the session, which is the whole of what the release exists
    // to prevent.
    /// Whether a release has arrived for this run.
    ///
    /// Set by [`release`](Runs::release) and [`release_all`](Runs::release_all) whenever this is the run they
    /// match, working or finished, and read by [`finish`](Runs::finish), which then drops what a run still working
    /// hands back rather than keeping it.
    released: bool,
}

/// What a resolved identity turned out to be: an already-held result, or one no longer held.
#[derive(Debug, Clone)]
pub(crate) enum Resident {
    /// The result this application still holds, shared rather than copied.
    Picture(Picture),
    /// An identity this application produced and no longer holds — a later run having displaced it, or the
    /// image it was made from having been closed.
    Displaced,
}

impl Runs {
    /// Starts a run called `id` over the image `source`, displacing whatever the slot held.
    ///
    /// Cancels the displaced run's token, drops its result, and installs the new run — all under one lock, so
    /// "one run at a time" is a property rather than a promise.
    ///
    /// If a stop for `id` arrived before this call, the returned token is **already cancelled**.
    pub(crate) fn start(&self, id: &str, source: &str) -> CancellationToken {
        let mut held = lock(&self.held);

        // Before the write: a displaced run must stop regardless of the new run's outcome, and its pixels must
        // go the moment the next run is asked for, not when it completes — bounds resident cost to one.
        if let Some(displaced) = held.current.take() {
            displaced.cancel.cancel();
        }

        let cancel = CancellationToken::new();

        // A stop that overtook its own run: consume it so it doesn't stop a later reuse of the same name.
        if held.stopped.as_deref() == Some(id) {
            held.stopped = None;
            cancel.cancel();
        } else {
            // Leftover from an already-displaced run; discard rather than let it trap a reused name.
            held.stopped = None;
        }

        held.current = Some(Run {
            id: id.to_string(),
            source: source.to_string(),
            cancel: cancel.clone(),
            result: None,
            released: false,
        });

        cancel
    }

    /// Records what the run called `id` produced; does nothing if that run is no longer in flight.
    ///
    /// **A run whose photograph was closed while it worked is dropped on the same terms.**
    ///
    /// Answers whether the result is now held — a run that finished after being displaced or released
    /// produced pixels nobody kept, so it must be reported as stopped rather than as an address the scheme
    /// would answer `GONE` for. The identity is always remembered.
    pub(crate) fn finish(&self, id: &str, picture: Picture) -> bool {
        let mut held = lock(&self.held);

        // Always, since a displaced or released run may answer before noticing.
        held.produced.record(picture.identity());

        // A run displaced while working has already had its pixels dropped by its displacer, so writing them
        // back here would double the resident cost the slot bounds. A released run is dropped for the reason
        // `Run::released` gives: nothing would be left to release what it hands back.

        match held.current.as_mut() {
            Some(run) if run.id == id && run.released => {
                tracing::debug!("a run finished after its image was closed; its result is dropped");

                false
            }
            Some(run) if run.id == id => {
                run.result = Some(picture);

                true
            }
            _ => {
                tracing::debug!("a run finished after being displaced; its result is dropped");

                false
            }
        }
    }

    /// Stops the run called `id`, or remembers the request for a run that hasn't started yet.
    ///
    /// Answers whether anything was stopped.
    ///
    /// **Does nothing when the slot holds another run**, which would be stopping the wrong one.
    pub(super) fn stop(&self, id: &str) -> bool {
        let mut held = lock(&self.held);

        match held.current.as_ref() {
            Some(run) if run.id == id => {
                run.cancel.cancel();

                true
            }
            // Either already displaced, or not started yet — indistinguishable here, so remember the name and
            // let `start` decide.
            _ => {
                held.stopped = Some(id.to_string());

                false
            }
        }
    }

    /// Drops the held result if it was made from the image `source`; does nothing otherwise.
    ///
    /// Answers whether anything was dropped.
    ///
    /// The released result stays in [`produced`](Held::produced), so a pane still drawing its address is told
    /// it is gone rather than that it never existed.
    ///
    /// **A run still in flight is not stopped here**, but it is marked [`released`](Run::released), so what it
    /// hands back later is not kept.
    pub(super) fn release(&self, source: &str) -> bool {
        let mut held = lock(&self.held);

        // Named by the image rather than the result, which is what makes the race safe: closing image A while a
        // run over image B is in flight is ordinary, and a release meaning "drop whatever you hold" would throw
        // away B's result.
        //
        // A run in flight is left to its own path — the window cancels it when the pane goes — rather than
        // stopped here, which would make closing an image a second way to cancel a run.
        match held.current.as_mut() {
            Some(run) if run.source == source => {
                run.released = true;

                run.result.take().is_some()
            }
            _ => false,
        }
    }

    /// Drops the held result whatever image it was made from, because every image has been closed.
    ///
    /// Answers whether anything was dropped.
    ///
    /// A run in flight is left exactly as [`release`](Self::release) leaves it: still in the slot and marked
    /// released, to be stopped by its own path.
    pub(super) fn release_all(&self) -> bool {
        let mut held = lock(&self.held);

        // Not taken out of the slot: that would throw away the token that stops it.
        let Some(run) = held.current.as_mut() else { return false };

        run.released = true;

        run.result.take().is_some()
    }

    /// What this application holds for `identity`, or `None` if no run ever produced it.
    ///
    /// A clone of the [`Picture`] rather than a guard, so the response can be produced with the lock released
    /// — `Picture` shares pixels via `Arc`, so this is a refcount bump.
    pub(crate) fn resolve(&self, identity: &str) -> Option<Resident> {
        let held = lock(&self.held);

        if let Some(picture) = held.current.as_ref().and_then(|run| run.result.as_ref())
            && picture.identity() == identity
        {
            return Some(Resident::Picture(picture.clone()));
        }

        held.produced.contains(identity).then_some(Resident::Displaced)
    }

    /// Whether the slot holds the run called `id` right now.
    #[cfg(test)]
    pub(super) fn holds(&self, id: &str) -> bool {
        // A method rather than a free helper beside the tests because `Held` and `Run` are this file's own:
        // `super::run`'s tests wait on the slot without the inside of it being visible to them.
        lock(&self.held).current.as_ref().is_some_and(|current| current.id == id)
    }
}

/// How many produced identities are remembered. See [`Produced`] for why a small number is enough.
const REMEMBERED: usize = 64;

// A set that only ever grew would be the obvious shape: one 16-character identity per enhanced result, for the life
// of the process. Small, but unbounded.
//
// Its one job is answering "did this application ever make that?" for an address a window is still holding — and a
// window cannot hold one indefinitely. It holds an address because a pane is drawing it, and a pane is drawing it
// because the run that produced it was recent. So a ring of recent identities answers every question that is
// actually asked, and one that has fallen out of it gets a slightly worse message for an address nothing can still
// be drawing.
//
// A `VecDeque` scanned linearly rather than a deque beside a set: sixty-four comparisons of 16-character strings cost
// nothing beside producing a rendition, and one structure cannot fall out of step with itself.
/// The enhanced identities this application has handed back, oldest first, bounded to [`REMEMBERED`].
///
/// One that has fallen out of it is answered as unknown rather than as [`Resident::Displaced`].
#[derive(Debug, Default)]
struct Produced(VecDeque<String>);

impl Produced {
    /// Remembers `identity` as the most recent, evicting the oldest once the ring is full.
    ///
    /// An identity already in the ring is **moved to the recent end** rather than added again.
    fn record(&mut self, identity: &str) {
        // The same chain run twice over the same photograph produces the same identity twice, and letting it take
        // two places would evict something else for nothing.
        if let Some(position) = self.0.iter().position(|remembered| remembered == identity) {
            self.0.remove(position);
        } else if self.0.len() == REMEMBERED {
            self.0.pop_front();
        }

        self.0.push_back(identity.to_string());
    }

    /// Whether `identity` is still remembered.
    fn contains(&self, identity: &str) -> bool {
        self.0.iter().any(|remembered| remembered == identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The identity of the photograph a run is over, where which one it is is not what the test is about.
    const SOURCE: &str = "5ec0ffee5ec0ffee";

    /// A second photograph, for the tests that are about a release naming the wrong one.
    const OTHER: &str = "facefeedfacefeed";

    /// A picture of a known identity, built rather than decoded so these tests need no fixture.
    fn picture_of(identity: &str) -> Picture {
        Picture::new("/pictures/holiday.jpg", image::DynamicImage::new_rgb8(2, 2), identity)
    }

    #[test]
    fn starting_a_run_cancels_the_one_it_displaces() {
        let runs = Runs::default();

        let first = runs.start("run-1", SOURCE);
        assert!(!first.is_cancelled(), "a run was cancelled before anything displaced it");

        let second = runs.start("run-2", SOURCE);

        assert!(first.is_cancelled(), "the displaced run was left working with nobody waiting for it");
        assert!(!second.is_cancelled(), "the run that displaced it was cancelled too");
    }

    #[test]
    fn starting_a_run_drops_the_result_it_displaces() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa"));

        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Picture(_))),
            "the result of the run that just finished is not being held"
        );

        runs.start("run-2", SOURCE);

        // Dropped when the next run is ASKED FOR, not when it completes — bounds resident cost to one. D4.
        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "a displaced result is still being held, or has stopped being distinguishable from one never produced"
        );
    }

    #[test]
    fn a_displaced_result_is_told_apart_from_one_no_run_produced() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa"));
        runs.start("run-2", SOURCE);

        assert!(matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)));
        assert!(
            runs.resolve("bbbbbbbbbbbbbbbb").is_none(),
            "an identity no run produced was reported as displaced"
        );
    }

    #[test]
    fn a_result_from_a_run_that_was_displaced_while_it_worked_is_dropped() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.start("run-2", SOURCE);
        // The first run finishing late; its pixels were already accounted for.
        runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa"));

        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "a run that finished after being displaced put its result back in the slot"
        );
    }

    #[test]
    fn stopping_names_the_run_and_leaves_another_alone() {
        let runs = Runs::default();

        let first = runs.start("run-1", SOURCE);
        let second = runs.start("run-2", SOURCE);

        // A stop for the run that was just displaced, arriving in the ordinary course of changing an
        // enhancement — acting on it would stop the run the user is waiting for.
        assert!(!runs.stop("run-1"), "a stop naming a displaced run claimed to have stopped something");
        assert!(!second.is_cancelled(), "a stop for a displaced run stopped the one in flight");

        assert!(runs.stop("run-2"), "a stop naming the run in flight did nothing");
        assert!(second.is_cancelled());

        // Already stopped by the displacement itself, not by the stop above.
        assert!(first.is_cancelled());
    }

    #[test]
    fn a_stop_that_overtakes_its_own_run_still_lands_on_it() {
        let runs = Runs::default();

        // The two cross the boundary independently and either may arrive first — D3.
        assert!(!runs.stop("run-1"), "nothing was in flight to stop");

        let token = runs.start("run-1", SOURCE);

        assert!(token.is_cancelled(), "a run went on to do work a stop had already asked it not to");
    }

    #[test]
    fn a_stop_that_found_nothing_does_not_lie_in_wait_for_a_later_run() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        // A late stop for a run already displaced — nothing to act on, nothing to keep.
        assert!(!runs.stop("run-0"));

        let next = runs.start("run-2", SOURCE);

        assert!(!next.is_cancelled(), "a stop naming a run that never started was applied to an unrelated one");
    }

    // ── Releasing ─────────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn closing_the_image_a_result_was_made_from_releases_it() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa"));

        assert!(runs.release(SOURCE), "the result of the photograph being closed was not being held");

        // Released, not forgotten: a pane still drawing the address is told the result is gone rather than
        // that it never existed. D2.
        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "a released result is still held, or has stopped being distinguishable from one never produced"
        );

        assert!(
            !runs.release(SOURCE),
            "a second release of the same photograph claimed to have dropped something"
        );
    }

    #[test]
    fn closing_a_different_image_leaves_the_held_result_alone() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa"));

        // The ordinary case the name exists for: the user closes one photograph while another's result is what
        // the window is drawing. D1.
        assert!(!runs.release(OTHER), "a release naming another photograph claimed to have dropped something");
        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Picture(_))),
            "closing one photograph threw away the result belonging to another"
        );
    }

    #[test]
    fn releasing_the_image_a_run_is_working_on_leaves_the_run_to_its_own_stop() {
        let runs = Runs::default();

        let token = runs.start("run-1", SOURCE);

        // Nothing has been produced yet, so there is nothing to drop — and the run is left for the window's own
        // stop, rather than closing an image becoming a second way to cancel a run. D1.
        assert!(!runs.release(SOURCE), "a run that has produced nothing claimed to have released something");
        assert!(!token.is_cancelled(), "a release stopped the run in flight");
        assert!(runs.holds("run-1"), "a release took the run in flight out of the slot");
    }

    #[test]
    fn a_run_that_finishes_after_its_image_was_closed_does_not_put_its_result_in_the_slot() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.release(SOURCE);

        // Cancellation is cooperative and checked between tiles, so the stop the window sends when the pane
        // goes does not guarantee the run notices — a fully-cached chain does no tiling to notice between.
        // Keeping these pixels would hold a closed photograph's result for the session, which is the whole of
        // what the release is for.
        assert!(
            !runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa")),
            "a run whose photograph was closed reported its result as held"
        );

        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "the result of a closed photograph is being held, or has stopped being distinguishable from one \
             never produced"
        );
    }

    #[test]
    fn a_run_that_finishes_after_every_image_was_closed_does_not_put_its_result_in_the_slot() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.release_all();

        assert!(
            !runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa")),
            "a run left working when the window was emptied reported its result as held"
        );

        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "the result of a closed photograph is being held, or has stopped being distinguishable from one \
             never produced"
        );
    }

    #[test]
    fn a_release_naming_another_image_leaves_the_run_in_flight_free_to_finish() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);

        // The ordinary case: closing one photograph while a run over another works. The mark must not land on
        // it, or closing any image at all would quietly throw away the result the user is waiting for.
        runs.release(OTHER);

        assert!(
            runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa")),
            "closing one photograph threw away the result of a run over another"
        );
        assert!(matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Picture(_))));
    }

    #[test]
    fn closing_every_image_releases_the_held_result() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa"));

        assert!(runs.release_all(), "nothing was released when every photograph was closed");
        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "a released result is still held, or has stopped being distinguishable from one never produced"
        );

        // A run in flight over a photograph that has just gone: left running, and stopped by the window's own
        // cleanup, so the token that stops it is not thrown away along with the pixels.
        let token = runs.start("run-2", OTHER);

        assert!(!runs.release_all(), "a run that has produced nothing claimed to have released something");
        assert!(!token.is_cancelled(), "releasing everything stopped the run in flight");
        assert!(runs.holds("run-2"), "releasing everything took the run in flight out of the slot");
    }

    // ── What is remembered about what was produced ────────────────────────────────────────────────────────────────

    #[test]
    fn a_released_identity_answers_displaced_until_enough_others_have_pushed_it_out() {
        let runs = Runs::default();

        runs.start("run-1", SOURCE);
        runs.finish("run-1", picture_of("aaaaaaaaaaaaaaaa"));
        runs.release(SOURCE);

        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "a result released a moment ago is already reported as one that never existed"
        );

        // A session that goes on enhancing. Nothing can still be drawing the first address by the end of it — a
        // pane draws a result because the run that produced it was recent. D3.
        for index in 0..REMEMBERED {
            let run = format!("later-{index}");
            runs.start(&run, SOURCE);
            runs.finish(&run, picture_of(&format!("{index:0>16x}")));
        }

        assert!(
            runs.resolve("aaaaaaaaaaaaaaaa").is_none(),
            "the record of what was produced is still growing with the session"
        );
        assert!(
            matches!(runs.resolve(&format!("{:0>16x}", REMEMBERED - 1)), Some(Resident::Picture(_))),
            "the most recent result fell out of the record that is supposed to hold it"
        );
    }

    #[test]
    fn one_identity_produced_twice_takes_one_place_in_the_record() {
        // The same chain over the same photograph, run again — the cache makes this cheap and a user changing an
        // option back does it. Two places for one identity would evict something else for nothing.
        let mut produced = Produced::default();

        produced.record("aaaaaaaaaaaaaaaa");
        for index in 0..REMEMBERED - 1 {
            produced.record(&format!("{index:0>16x}"));
        }
        produced.record("aaaaaaaaaaaaaaaa");

        assert_eq!(produced.0.len(), REMEMBERED, "the ring grew past what it is bounded to");
        assert!(produced.contains("aaaaaaaaaaaaaaaa"), "the identity just produced is not remembered");
        assert!(produced.contains("0000000000000000"), "producing a known identity again evicted the oldest");
    }
}
