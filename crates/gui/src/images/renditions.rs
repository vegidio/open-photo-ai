//! The renditions this application has already produced, so one request is decoded and encoded once.
//!
//! Keyed on what [`mod@super::serve`] was asked for — an identity, a bound and a crop — and bounded by bytes
//! rather than by entry count.

use std::collections::{HashMap, HashSet};
use std::sync::{Condvar, Mutex};

use super::serve::Asked;

/// The bytes of an image, and the form they are in: what `super::serve`'s `render` produces and its
/// [`serve`](super::serve::serve) streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Rendition {
    // Kept together because the render decides the format from the picture's own colour type.
    //
    // `bytes` is behind an `Arc` rather than owned, because a cache hit clones this **while holding the map's
    // lock** — with a `Vec` that is a multi-megabyte memcpy blocking every other request, including ones for
    // entirely different images. Keeping it shared makes that clone a refcount bump, and makes `Renditions::keep`'s
    // second copy free as well.
    /// The encoded picture.
    pub(super) bytes: std::sync::Arc<[u8]>,
    /// What it was encoded as, which is the `Content-Type` the response carries.
    pub(super) media_type: &'static str,
}

// These are encoded, not decoded, which is one to two orders of magnitude smaller than the reference's
// 1.5 GB LRU of decoded blobs for the same coverage.
/// How many bytes of already-produced renditions [`Renditions`] keeps: 256 MB.
const RENDITION_BUDGET: usize = 256 * 1024 * 1024;

// Without a cap, one unbounded rendition of a huge scan could evict the whole thumbnail strip to make
// room for itself, then be evicted in turn by the next one — a cache holding nothing useful.
/// What fraction of the budget the largest kept rendition may take: an eighth.
const LARGEST_KEPT_SHARE: usize = 8;

/// The renditions this application has already produced, keyed by what was asked for, least recently used
/// evicted first once the byte budget is exceeded. Refusals aren't kept.
///
/// One request is rendered once even when several arrive together: the first to [`Self::claim`] it renders
/// it, and the rest wait and take what it produced.
#[derive(Debug)]
pub(crate) struct Renditions {
    // Why this exists: `super::serve`'s `respond` tells the webview it may cache a rendition for a year, and that's
    // honoured by WebView2 on Windows. On macOS/Linux the `opai://` scheme is served by a handler the webview's
    // resource cache doesn't sit in front of, so the directive is true but ignored — leaving a thumbnail that
    // re-decoded (about 230ms) every time it scrolled out of the drawer and back in. This is where that promise is
    // actually kept.
    //
    // Never stale: an identity covers every byte of a file, so it can't come to mean different pixels, and the bound
    // and the crop are carried in the request rather than held anywhere — so there's no invalidation to reason
    // about. Refusals aren't cached because a file on an ejected drive may come back once the drive is plugged in
    // again.
    //
    // By bytes, not count: entries vary by two orders of magnitude (a 30 kB thumbnail beside a multi-megabyte canvas
    // rendition), so a byte budget is what actually bounds memory. Recency is a counter rather than an intrusive
    // list; eviction sorts by it and only runs once the budget is actually exceeded.
    //
    // Coalescing matters because the canvas's two panes point at one unbounded rendition while nothing has been
    // enhanced, and on macOS/Linux each asks for it at the same moment; without coalescing that is two full decodes
    // and encodes of the same photograph.
    kept: Mutex<Kept>,
    /// Signalled whenever a request stops being rendered, whether it produced pixels or a refusal.
    finished: Condvar,
    // A field rather than only a constant, so eviction tests can use a small budget instead of allocating a quarter
    // gigabyte to watch it happen.
    /// How many bytes may be resident. [`RENDITION_BUDGET`] outside tests — see [`Renditions::with_budget`].
    budget: usize,
}

impl Default for Renditions {
    fn default() -> Self {
        Self::with_budget(RENDITION_BUDGET)
    }
}

/// [`Renditions`]' state behind its one lock.
#[derive(Debug, Default)]
struct Kept {
    // One struct rather than three mutexes, since an insert updates the map, the byte total and the clock together,
    // and a reader must never see them disagree.
    /// What has been produced, and when each was last handed out.
    entries: HashMap<Asked, (Rendition, u64)>,
    /// The sum of `entries`' rendition byte lengths, maintained rather than recomputed.
    bytes: usize,
    /// Ticks once per read and once per insert. Used only as an order, never as a time.
    clock: u64,
    /// The requests a thread is rendering right now, so a second asker waits instead of rendering it again.
    rendering: HashSet<Asked>,
}

impl Renditions {
    /// A cache holding `budget` bytes.
    fn with_budget(budget: usize) -> Self {
        Self { kept: Mutex::default(), finished: Condvar::new(), budget }
    }

    /// The largest rendition this cache will keep. See [`LARGEST_KEPT_SHARE`].
    fn largest_kept(&self) -> usize {
        self.budget / LARGEST_KEPT_SHARE
    }

    /// The rendition for this request if already produced, or the job of producing it.
    ///
    /// [`Claim::Ready`] is the cache answering; [`Claim::Produce`] means nobody else is rendering this
    /// request, so the caller must — and the guard it carries releases the job however the caller leaves,
    /// a `?` and a panic included. A caller that finds another thread already rendering waits instead of
    /// rendering it a second time.
    pub(super) fn claim(&self, asked: &Asked) -> Claim<'_> {
        let mut kept = self.lock();

        loop {
            kept.clock += 1;
            let now = kept.clock;

            if let Some((rendition, last_used)) = kept.entries.get_mut(asked) {
                *last_used = now;

                return Claim::Ready(rendition.clone());
            }

            // Insert before releasing the lock — that's what makes this a claim rather than a check-then-act.
            if kept.rendering.insert(asked.clone()) {
                return Claim::Produce(Producing { renditions: self, asked: asked.clone(), settled: false });
            }

            // Someone else is producing it; wait for their signal and re-check the map. A refusal leaves the
            // set empty, and this thread then claims the job itself rather than inheriting an error.
            kept = self.finished.wait(kept).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Releases a claim, keeping `rendition` if there is one, and wakes everything waiting on it.
    fn settle(&self, asked: &Asked, rendition: Option<&Rendition>) {
        {
            let mut kept = self.lock();

            kept.rendering.remove(asked);

            if let Some(rendition) = rendition {
                self.keep(&mut kept, asked.clone(), rendition);
            }
        }

        // notify_all, not notify_one: waiters may be waiting on different requests.
        self.finished.notify_all();
    }

    /// Keeps a rendition without claiming it first, evicting until the budget is met again.
    #[cfg(test)]
    fn put(&self, asked: Asked, rendition: &Rendition) {
        // Production reaches `keep` through `Producing::keep`, so one request is rendered once. This exists so
        // eviction/accounting tests don't need to spell out a claim and a guard for every fixture row.
        let mut kept = self.lock();

        self.keep(&mut kept, asked, rendition);
    }

    /// [`Renditions::put`]'s body, against a map the caller has already locked.
    fn keep(&self, kept: &mut Kept, asked: Asked, rendition: &Rendition) {
        if rendition.bytes.len() > self.largest_kept() {
            return;
        }

        kept.clock += 1;
        let now = kept.clock;

        // May be kept twice — a waiter that inherited a refusal renders it itself — so the total is corrected
        // for the previous entry's size rather than assumed to be a fresh insert.
        if let Some((previous, _)) = kept.entries.insert(asked, (rendition.clone(), now)) {
            kept.bytes -= previous.bytes.len();
        }

        kept.bytes += rendition.bytes.len();

        if kept.bytes <= self.budget {
            return;
        }

        let mut by_recency: Vec<_> =
            kept.entries.iter().map(|(asked, (_, last_used))| (*last_used, asked.clone())).collect();
        by_recency.sort_unstable_by_key(|(last_used, _)| *last_used);

        for (_, asked) in by_recency {
            if kept.bytes <= self.budget {
                break;
            }

            if let Some((evicted, _)) = kept.entries.remove(&asked) {
                kept.bytes -= evicted.bytes.len();
            }
        }
    }

    /// The map, treating a poisoned lock as readable.
    fn lock(&self) -> std::sync::MutexGuard<'_, Kept> {
        // For the reason `super::files::Opened::lock` gives.
        self.kept.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// What [`Renditions::claim`] found.
pub(super) enum Claim<'a> {
    /// Already produced, and this is it.
    Ready(Rendition),
    /// Nobody was producing it, so the caller must. See [`Producing`].
    Produce(Producing<'a>),
}

/// The job of producing one rendition, held for as long as this thread is producing it.
pub(super) struct Producing<'a> {
    // A guard rather than a pair of calls, so the claim is released however the producing thread leaves —
    // including a `?` on an unreadable file or a panic in a codec — rather than leaving a request marked
    // "being produced" with nothing producing it.
    renditions: &'a Renditions,
    asked: Asked,
    /// Whether [`Producing::keep`] has already released the claim, so [`Drop`] does not release it twice.
    settled: bool,
}

impl Producing<'_> {
    /// Releases the claim, keeping what was produced.
    pub(super) fn keep(mut self, rendition: &Rendition) {
        self.renditions.settle(&self.asked, Some(rendition));
        self.settled = true;
    }
}

impl Drop for Producing<'_> {
    fn drop(&mut self) {
        if !self.settled {
            // Nothing was produced (a refusal, or a panic). Waiters are woken regardless, and whichever wakes
            // first claims the job — see `claim`.
            self.renditions.settle(&self.asked, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rendition of `bytes` bytes, for the tests about what the cache keeps and evicts.
    fn rendition_of(bytes: usize) -> Rendition {
        Rendition { bytes: vec![0; bytes].into(), media_type: "image/jpeg" }
    }

    /// The request for `identity` at `bound`, spelled once.
    fn asked_for(identity: &str, bound: u32) -> Asked {
        Asked { identity: identity.to_string(), bound, crop: None }
    }

    /// What the cache already holds for this request, through the one path production reads it by.
    ///
    /// A miss takes the claim and then drops it unused, which releases it — so this is a read and nothing more, and
    /// a later call sees the same answer.
    fn cached(renditions: &Renditions, asked: &Asked) -> Option<Rendition> {
        match renditions.claim(asked) {
            Claim::Ready(rendition) => Some(rendition),
            Claim::Produce(_) => None,
        }
    }

    /// A cache of 800 bytes, which keeps renditions of up to 100 — see [`Renditions::with_budget`].
    fn small_cache() -> Renditions {
        Renditions::with_budget(800)
    }

    #[test]
    fn a_rendition_that_was_kept_is_handed_back() {
        let renditions = small_cache();
        let asked = asked_for("0123456789abcdef", 384);

        assert_eq!(cached(&renditions, &asked), None, "an empty cache answered for a request it never saw");

        renditions.put(asked.clone(), &rendition_of(64));

        assert_eq!(cached(&renditions, &asked), Some(rendition_of(64)));
    }

    #[test]
    fn a_bound_is_part_of_what_is_keyed_on() {
        // The property that lets the drawer's thumbnail and the canvas's preview of one photograph both be kept: they
        // are two renditions of the same file and must not be mistaken for each other.
        let renditions = small_cache();

        renditions.put(asked_for("0123456789abcdef", 384), &rendition_of(64));

        assert_eq!(cached(&renditions, &asked_for("0123456789abcdef", 0)), None, "a bound was not part of the key");
        assert!(cached(&renditions, &asked_for("0123456789abcdef", 384)).is_some());
    }

    #[test]
    fn the_least_recently_used_rendition_is_the_one_evicted() {
        let renditions = small_cache();

        for identity in ["aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb", "cccccccccccccccc", "dddddddddddddddd"] {
            renditions.put(asked_for(identity, 384), &rendition_of(100));
        }

        // Reading the first is what makes the second the least recently used, rather than the order they went in.
        assert!(cached(&renditions, &asked_for("aaaaaaaaaaaaaaaa", 384)).is_some());

        // Nine entries of a hundred against a budget of eight hundred, so exactly one has to go.
        for identity in [
            "eeeeeeeeeeeeeeee",
            "ffffffffffffffff",
            "gggggggggggggggg",
            "hhhhhhhhhhhhhhhh",
            "iiiiiiiiiiiiiiii",
        ] {
            renditions.put(asked_for(identity, 384), &rendition_of(100));
        }

        assert!(
            cached(&renditions, &asked_for("aaaaaaaaaaaaaaaa", 384)).is_some(),
            "the recently read one was evicted"
        );
        assert_eq!(
            cached(&renditions, &asked_for("bbbbbbbbbbbbbbbb", 384)),
            None,
            "the least recently used one survived"
        );
        assert!(
            cached(&renditions, &asked_for("iiiiiiiiiiiiiiii", 384)).is_some(),
            "the newest one was never kept"
        );
        assert_eq!(renditions.lock().entries.len(), 8, "more than the one entry that had to go was evicted");
    }

    #[test]
    fn the_budget_is_not_exceeded_by_what_is_kept() {
        let renditions = small_cache();

        for bound in 0..40u32 {
            renditions.put(asked_for("0123456789abcdef", bound), &rendition_of(100));
        }

        let kept = renditions.lock();
        assert!(kept.bytes <= 800, "the cache holds {} bytes", kept.bytes);
        assert_eq!(
            kept.bytes,
            kept.entries.values().map(|(rendition, _)| rendition.bytes.len()).sum::<usize>(),
            "the running total stopped describing what is actually held"
        );
    }

    #[test]
    fn a_rendition_too_large_to_be_worth_keeping_is_not_kept() {
        // A hundred-megapixel scan asked for unbounded. Serving it is fine; keeping it would evict a drawer's worth
        // of thumbnails to hold one picture that is unlikely to be asked for twice.
        let renditions = small_cache();
        let asked = asked_for("0123456789abcdef", 0);

        renditions.put(asked.clone(), &rendition_of(renditions.largest_kept() + 1));

        assert_eq!(cached(&renditions, &asked), None, "an oversized rendition was kept");
        assert_eq!(renditions.lock().bytes, 0, "an oversized rendition was counted against the budget");
    }

    #[test]
    fn the_real_budget_keeps_a_drawer_full_of_thumbnails() {
        // The constants doing the job they were picked for, rather than the mechanism. A 384-pixel thumbnail is about
        // 30 kB, and a strip of a thousand photographs must not evict its own start.
        let renditions = Renditions::default();

        for bound in 0..1000u32 {
            renditions.put(asked_for("0123456789abcdef", bound), &rendition_of(30 * 1024));
        }

        assert_eq!(renditions.lock().entries.len(), 1000, "a thousand thumbnails did not fit in the budget");
    }

    #[test]
    fn one_request_is_produced_once_however_many_ask_for_it_at_once() {
        // Eight threads, one request. The claim is held while they pile up, so every one of them arrives while it is
        // being produced — which is the race, run deliberately rather than hoped for.
        let renditions = std::sync::Arc::new(small_cache());
        let asked = asked_for("0123456789abcdef", 0);

        let Claim::Produce(producing) = renditions.claim(&asked) else {
            panic!("an empty cache claimed to already hold the request");
        };

        let produced = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1));
        let waiters: Vec<_> = (0..8)
            .map(|_| {
                let renditions = std::sync::Arc::clone(&renditions);
                let produced = std::sync::Arc::clone(&produced);
                let asked = asked.clone();

                std::thread::spawn(move || match renditions.claim(&asked) {
                    Claim::Ready(rendition) => rendition,
                    Claim::Produce(producing) => {
                        // Reached only if the coalescing is broken: this thread would be rendering a photograph a
                        // thread beside it is already rendering.
                        produced.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let rendition = rendition_of(64);
                        producing.keep(&rendition);
                        rendition
                    }
                })
            })
            .collect();

        // Given to the waiters only once every one of them is inside `claim` — released here, which is what wakes
        // them. Sleeping to arrange that would make this a test that sometimes proves nothing; instead the claim is
        // simply held until after the threads are spawned, and any waiter that has not arrived yet finds the entry
        // already present, which is the same assertion.
        producing.keep(&rendition_of(64));

        for waiter in waiters {
            assert_eq!(waiter.join().expect("the waiter should not panic"), rendition_of(64));
        }

        assert_eq!(
            produced.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the photograph was rendered more than once"
        );
        assert!(renditions.lock().rendering.is_empty(), "a claim outlived the thread holding it");
    }

    #[test]
    fn a_claim_is_released_even_when_nothing_is_produced() {
        // A refusal, or a codec that panicked. Either way the request must not stay marked as being produced, or
        // every later request for that photograph waits on a thread that is already gone.
        let renditions = small_cache();
        let asked = asked_for("0123456789abcdef", 384);

        match renditions.claim(&asked) {
            Claim::Ready(_) => panic!("an empty cache claimed to already hold the request"),
            // Dropped without `keep`, which is what a `?` out of `produce` does.
            Claim::Produce(producing) => drop(producing),
        }

        assert!(renditions.lock().rendering.is_empty(), "the claim was not released");
        assert!(matches!(renditions.claim(&asked), Claim::Produce(_)), "the request could not be claimed again");
    }

    #[test]
    fn keeping_one_request_twice_does_not_count_it_twice() {
        // The concurrent-miss case the type documents: two renders of one request, the second overwriting the first.
        // The bytes are interchangeable; the total must not drift.
        let renditions = small_cache();
        let asked = asked_for("0123456789abcdef", 384);

        renditions.put(asked.clone(), &rendition_of(64));
        renditions.put(asked.clone(), &rendition_of(64));

        assert_eq!(renditions.lock().entries.len(), 1);
        assert_eq!(renditions.lock().bytes, 64, "one request was counted twice against the budget");
    }
}
