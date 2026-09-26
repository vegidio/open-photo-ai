//! Keeping what a run produced, so that repeating it is a read rather than a run.
//!
//! An adoption of `rust-sak`'s `memo` rather than a cache written here: the store, its three backing modes, its
//! per-entry lifetime and the sweep that reclaims what expired are all that module's, and what this one adds is the
//! three decisions specific to this project — where the store lives, how a result is turned into bytes and back, and
//! what a failure at either end costs a run.
//!
//! The store holds two kinds of value: a **picture**, which is what each step of an enhancement chain produces,
//! written by [`put`](RunCache::put) and read by [`get_bytes`](RunCache::get_bytes) and [`decode`](RunCache::decode);
//! and a **result that is not a picture** — a set of faces — written by [`put_value`](RunCache::put_value) and read by
//! [`get_value`](RunCache::get_value). Both share one lifetime, one sweep and one set of backing modes, and neither
//! kind can be read as the other: an entry read as the wrong kind is a miss.
//!
//! **No failure of the store reaches the caller.** A store that will not open, a write that is declined, a write that
//! breaks, a read that fails, an entry that no longer decodes: each costs at most running an operation that need not
//! have run, and each is recorded in the log.
//!
//! **Nothing bounds the store by size.** Entries expire after [`ENTRY_TTL`], and the space they hold is reclaimed by
//! the sweep `memo` runs when a disk store opens — so a daily-launched application reclaims yesterday's work on today's
//! first launch, and one session of fifty large enhancements can hold several gigabytes until then.

// Keys. For an enhancement, one entry per **step** of a chain, keyed on the identity of the pixels that step produced
// — `process::identity_after` over the input's identity, the operations applied so far and the resolved depth — so the
// same operation after a different predecessor is a different entry, and a result fed back into `Opai::process` for
// further enhancement looks up exactly the slot its own pixels were stored under. There is one derivation rather than
// two: the read path and the write path cannot disagree about a key they do not each compute.
//
// The execution provider is deliberately **not** part of a key. The same enhancement is the same enhancement whichever
// processor produced it, and a caller that needs each provider to actually run — a benchmark — turns the cache off for
// that run rather than being given a key it can vary.
//
// A size budget with eviction is general-purpose and belongs in `memo` rather than here.
//
// No failure reaches the caller because running the operation is the correct response to every one of them: `put`
// returns `()` and `get_bytes` folds a broken store into a miss, since a `Result` nobody can act on is a signature that
// invites someone to try. `InferenceError` gains nothing on this module's account for the same reason: the cache cannot
// fail a run, so it has nothing to report through the type whose whole job is reporting why a run failed.
//
// **Every record carries `source`**, the identity of the picture the run started from and therefore the value
// `process::process` opens and closes the run with. `key` is folded from that identity through the operations applied
// so far, so it is not a string a reader can match against anything else — `source` is what makes a `grep` over one
// identity return the run and the cache decisions that belonged to it. `decode` is the exception: it is handed bytes
// rather than a slot.
//
// Deliberately not here:
//
// - **Coalescing concurrent identical runs.** `memo` would give it through `get_or_compute_async`, and taking it would
//   make one user's cancellation fail an unrelated run and leave a joined run's progress bar still. Two identical
//   chains genuinely in flight at once each run; the second is served the moment the first writes.
// - **A second store for the results that are not pictures.** They live in this one, under their own encoding and
//   under keys no chain key can produce. A second `Memo` would be a second directory, a second sweep and a second set
//   of failure rules to keep in step, to separate values that were never at risk of meeting.
// - **A version tag beside a stored value.** It would let every result of one kind be invalidated without waiting out
//   the lifetime, which nothing needs and which the enhancement path does not have either. Adding one is an encoding
//   change behind `get_value` and `put_value` that nothing above them would see, so it waits for a need.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::Duration;

use image::DynamicImage;
use rust_sak::image::{EncodeOptions, ImageFormat, PngCompression, PngFilter, decode_bytes, encode_writer};
use rust_sak::memo::{CacheOpts, Memo, MemoError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::image::Picture;
use crate::task::Carried;
use crate::telemetry::unit::{self, Outcome, unit_span};

// A day covers a working session, which is the span over which somebody re-runs the same enhancement on the same
// photograph — dragging a slider, toggling an enhancement off and back on, re-opening a picture after lunch. Past that
// the pixels are cheaper to recompute than to keep, and since nothing bounds this store by size, the TTL is the only
// thing that ever gives the space back.
/// How long a stored result goes on being served.
pub(crate) const ENTRY_TTL: Duration = Duration::from_secs(24 * 60 * 60);

// Distinct from `engines/`, which is what an execution *provider compiled* from a model. That one is keyed on an
// artifact and invalidated by a runtime bump; this one is pixels, keyed on an image and expiring after a day. They
// share nothing but the word "cache".
/// The subdirectory of the application's configuration directory the store lives in.
pub(crate) const CACHE_DIR: &str = "cache";

// `memo` declines a value larger than the whole budget outright, so this admits a denoise at 1:1 and most 2x results
// and declines a large 8x one. That is the right shape: the fallback exists so that a run whose disk store would not
// open is not *slower than it needs to be*, not so that it matches a working cache. The reference's 64 MiB is
// conservative because the fallback is reached when the disk store failed, which in production is often a machine
// that has just run out of something. This is four times that, and still a small fraction of what a single run holds:
// large enough for the common result, small enough not to be the reason a strained machine gets worse.
/// What the memory fallback is sized for.
const MEMORY_BUDGET: u64 = 256 << 20;

/// What is actually backing the run cache after initialization.
///
/// **Reported, never configured.** Which of the three is in force is an outcome of what the machine allowed rather
/// than a preference — a caller that wants no cache turns it off per run through
/// [`ProcessOptions::cache`](crate::ProcessOptions::cache).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CacheMode {
    // It exists because losing the cache is deliberately not an error: without this, "some runs are silently uncached"
    // could not be observed from outside the machine, and a front end could not tell a user that a run was slow
    // because its cache was degraded.
    /// Results are kept on disk and survive a restart. What a healthy machine gets.
    Disk,
    /// The disk store could not be opened, so results are kept for the life of the process only.
    Memory,
    /// Neither store could be opened. Every operation of every run is computed.
    None,
}

impl CacheMode {
    /// The mode's name: `disk`, spelled as the `serde` renames spell it.
    ///
    /// A bare name rather than a sentence. A caller wanting prose — *"results are kept for this process only"* —
    /// composes it from this.
    pub const fn as_str(self) -> &'static str {
        // Written once, here, and matched by the `serde` renames — the same rule `ExecutionProvider::as_str` states,
        // and for the same reason: a consumer that reports this mode and one that stores it must not be able to spell
        // it two ways. Without it every such consumer transcribes the three words itself, and a mode added later reads
        // as unknown in each of them. How much to explain is a presentation decision, and this is not the place it is
        // made.
        match self {
            Self::Disk => "disk",
            Self::Memory => "memory",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for CacheMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The application's store of processed images.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    // A field on the application's shared state rather than a `static`, for the reason the session cache is one: it is
    // opened against the configuration directory *this* initialization resolved, and two applications in one process —
    // which a front end restarting its application layer produces — are two directories.
    //
    // Teardown is ownership rather than a protocol. `Memo` holds its store behind an `Arc` and has no `close`: the store
    // is released when the last handle drops, and a run holding one *is* a handle. So none of the reference's apparatus
    // for the same property — a global pointer swapped to nil, a nil check per operation, a mutex around a
    // check-then-swap — has anything to do here.
    /// The store, or `None` where neither could be opened.
    memo: Option<Memo>,
    /// Which of the three is in force, which is what [`Opai::cache_mode`](crate::Opai::cache_mode) reports.
    mode: CacheMode,
}

impl Store {
    /// Opens the store under `config_dir`, degrading rather than failing.
    ///
    /// Disk first, then memory, then nothing. **This never fails.**
    ///
    /// It **blocks**, and `memo` documents that it can wait out a teardown of the same directory for up to 50 ms, so
    /// it belongs at start-up or on a blocking thread rather than on a runtime worker.
    ///
    /// Opening a directory this process already has open yields the store that is already open for it rather than
    /// failing on the database's exclusive lock — which is what makes initializing an application twice keep what the
    /// first initialization stored.
    pub(crate) fn open(config_dir: &Path) -> Self {
        // Never failing is the point: an application that would not start because its cache directory was locked would
        // be refusing to do work it is perfectly able to do.
        let directory = config_dir.join(CACHE_DIR);

        // Not `Memo::memory_disk`, though the two-tier store is the module's own recommended default. What is stored
        // here is whole enhanced photographs — hundreds of megabytes each — and a memory tier in front would claim RAM
        // from the one thing already competing for it: an 8x 16-bit run holds well over a gigabyte and nothing budgets
        // it. A promotion tier pays for entries that are small, hot and cheap to hold; these are none of the three,
        // and it would hold one or two before evicting.
        //
        // `max_entries` is left at its default and the fact is worth recording, because the reference's equivalent
        // does nothing: it passes an entry count believing it bounds the store, and the disk tier ignores it outright.
        // A count is the wrong bound for pictures anyway, whose sizes differ by two orders of magnitude.
        if let Ok(memo) = Memo::disk(&directory, CacheOpts::new()) {
            return Self { memo: Some(memo), mode: CacheMode::Disk };
        }

        if let Ok(memo) = Memo::memory(CacheOpts::new().max_capacity(MEMORY_BUDGET)) {
            return Self { memo: Some(memo), mode: CacheMode::Memory };
        }

        // `Memo::memory` cannot fail today and says so — it is fallible only so that gaining a fallible step later is
        // not a breaking change. The arm is written rather than unwrapped because the whole contract of this module is
        // that no failure of the cache reaches a run, and an `unwrap` here would be the one that does.
        Self { memo: None, mode: CacheMode::None }
    }

    /// Which of the three modes is in force.
    pub(crate) fn mode(&self) -> CacheMode {
        self.mode
    }

    /// A handle on the store for one run, or `None` where there is no store.
    ///
    /// Cloning is a refcount bump, and the clone is what keeps the store alive for the length of the run: a teardown
    /// concurrent with that run cannot reach the store it is using.
    pub(crate) fn handle(&self) -> Option<Memo> {
        self.memo.clone()
    }
}

/// The store one run reads and writes, together with the identity every one of its keys is folded from.
///
/// **Bound once at the top of a run**, by [`for_run`](Self::for_run), rather than consulted per operation.
#[derive(Debug, Clone)]
pub(crate) struct RunCache {
    /// The store, held for the length of the run.
    memo: Memo,
    /// The identity of the picture the chain starts from, which every step's key is folded from.
    source: String,
}

impl RunCache {
    /// The store `memo` read and written on behalf of a chain starting from the picture `source` identifies.
    pub(crate) fn new(memo: Memo, source: &str) -> Self {
        Self { memo, source: source.to_string() }
    }

    /// The handle a run holds, or `None` where the run is not to be cached.
    ///
    /// A run cannot be served from the store and then decline to write back to it: both come from this one value.
    /// Holding it for the run keeps the store alive under the run — a [`Memo`] is released when its last handle drops,
    /// and this is a handle.
    pub(crate) fn for_run(cache: Option<&Memo>, enabled: bool, source: &Picture) -> Option<Self> {
        // The one place the three ingredients are combined, so that the rule above is a property of this function
        // rather than of two call sites agreeing: both drivers, `execute` and `process`, are handed the answer and
        // never the ingredients.
        cache.filter(|_| enabled).map(|memo| Self::new(memo.clone(), source.identity()))
    }

    /// The identity of the picture the chain started from, which its keys are folded from.
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    /// The bytes stored under `key`, or `None`.
    ///
    /// A store that broke, an absent key and an elapsed TTL are one answer here, because the response to all three is
    /// the same one and it is always correct: run the operation. **The three are one answer, not one record**: a read
    /// that failed is warned about on its way to being discarded, so a store that has stopped answering is visible in
    /// the log without becoming something a caller has to handle. A plain miss is not recorded at all here — the
    /// chain records it, once, as the step it made run.
    ///
    /// Blocks, for the read.
    pub(crate) fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        let span = unit_span!("cache_read", key, hit = tracing::field::Empty);
        let found = span.in_scope(|| self.read_bytes(key));
        read_ended(&span, found.is_some());

        found
    }

    /// [`get_bytes`](Self::get_bytes)'s body, which [`get_value`](Self::get_value) reads through too without a
    /// second span.
    fn read_bytes(&self, key: &str) -> Option<Vec<u8>> {
        // A result still queued by `put_later` is waited for rather than missed: a run repeated the moment the last
        // one returned — the slider case this cache exists for — must be served what that run produced.
        PENDING.wait_for(key);

        // Separate from `decode` because a chain reads more entries than it decodes: walking a cached prefix, every
        // step but the last is superseded by the one after it, and decoding each on the way past is a
        // several-hundred-megabyte PNG thrown away per step. What this cannot be is a cheap existence probe —
        // `rust_sak::memo::Memo` has none, and `get_bytes` returns the value — which is why the walk is forward.
        Self::read_or_warn(self.source(), key, self.memo.get_bytes(key))
    }

    /// What [`get_bytes`](Self::get_bytes) returns for `outcome`, with a store that *failed* warned about on its way
    /// to being discarded.
    fn read_or_warn(source: &str, key: &str, outcome: Result<Option<Vec<u8>>, MemoError>) -> Option<Vec<u8>> {
        // Split out so the branch is driven rather than described. `memo` exposes no store that can be made to fail on
        // demand — its disk store holds the database open, so neither deleting nor truncating the file underneath it
        // produces an error — and this crate does not reimplement one to test against. Taking the store's answer as a
        // parameter is what lets a test hand this an `Err` and assert on the line the formatter actually wrote, so
        // that deleting or rewording the record below fails that test instead of leaving it passing against a message
        // nothing writes any more.
        match outcome {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!(source, key, %error, "the store failed to read a result");
                None
            }
        }
    }

    /// The image `bytes` hold, or `None` where they no longer decode as one.
    ///
    /// The fourth way a read misses — a truncated write, an interrupted shutdown — and it is a miss for the same
    /// reason the other three are: running the operation is always a correct response to it. It is a miss that is
    /// **warned about**, as a failed read is and an absent key or an elapsed TTL is not.
    ///
    /// Blocks. Decoding a several-hundred-megabyte PNG is exactly the work that must not happen on the async runtime,
    /// which in a Tauri process is the window's event loop.
    pub(crate) fn decode(bytes: &[u8]) -> Option<DynamicImage> {
        // Warned about because an entry that was found and will not read back means the work it stood for is being
        // paid for twice, and the entry went on occupying the store until its TTL elapsed. A plain absence costs one
        // run of one operation and nothing else.
        match decode_bytes(bytes) {
            Ok(image) => Some(image),
            Err(error) => {
                // No key: this is handed bytes rather than a slot, which is what lets a chain read many entries and
                // decode one. The chain's own rewind record says which span of operations it cost.
                tracing::warn!(bytes = bytes.len(), %error, "a stored result would not decode");
                None
            }
        }
    }

    /// What is stored under `key`, read and decoded, or `None` on any of the four ways that misses.
    #[cfg(test)]
    pub(crate) fn get(&self, key: &str) -> Option<DynamicImage> {
        Self::decode(&self.get_bytes(key)?)
    }

    /// Stores `image` under `key` for [`ENTRY_TTL`], discarding every way that can fail.
    ///
    /// **The three outcomes are told apart in the log**:
    ///
    /// - the store **declined** the write — [`MemoError::NotAdmitted`], a result larger than the whole memory budget
    ///   — at `debug`;
    /// - the store **broke**, at `warn`;
    /// - the result would not **encode**, at `warn`.
    ///
    /// Blocks, for the encode as much as the write.
    pub(crate) fn put(&self, key: &str, image: &DynamicImage) {
        let span = unit_span!("cache_write", key);
        span.in_scope(|| self.put_image(key, image));
        // Whatever became of the write: a decline or a break is discarded and warned about, and never fails the run.
        unit::mark(&span, &Outcome::Finished);
    }

    /// [`put`](Self::put), on the store's writer thread rather than the caller's, so a run does not wait for its
    /// result to be encoded and written before handing it back.
    ///
    /// # What a reader sees
    ///
    /// A read of `key` made while the write is queued **waits for it** rather than missing, so a run repeated the
    /// moment the last one returned is served what that run produced, exactly as when the write was synchronous. Only
    /// a reader of the underlying [`Memo`] itself, going round this type, can see the entry absent — which is what
    /// [`settle`] is for.
    ///
    /// # What it holds
    ///
    /// The picture, shared rather than copied, until it has been written. At most [`QUEUED_WRITES`] wait at once;
    /// past that, the caller waits for room, which is what the write used to cost it anyway.
    ///
    /// A writer thread that could not be started, or has gone, is the synchronous write, on the caller's thread.
    pub(crate) fn put_later(&self, key: &str, image: Arc<DynamicImage>) {
        PENDING.begin(key);

        let job = Write { cache: self.clone(), key: key.to_string(), image, carried: Carried::current() };

        let refused = match &*WRITER {
            Some(queue) => queue.send(job).err().map(|refused| refused.0),
            None => Some(job),
        };

        if let Some(job) = refused {
            job.run();
        }
    }

    /// [`put`](Self::put)'s body, inside its span.
    fn put_image(&self, key: &str, image: &DynamicImage) {
        // The pixels are already computed by the time this is called, so there is nothing a failure here could
        // improve: throwing finished work away because a disk filled up is the cache's problem becoming the
        // enhancement's.
        //
        // A failed encode is a `warn` for the reason a broken write is — see `write`.
        let Some(bytes) = encode(image) else {
            tracing::warn!(source = self.source(), key, "a result would not encode and was not kept");
            return;
        };

        self.write(key, &bytes);
    }

    /// Stores `bytes` under `key` for [`ENTRY_TTL`], telling a declined write from a broken one in the log as
    /// [`put`](Self::put) describes.
    fn write(&self, key: &str, bytes: &[u8]) {
        // Shared by both kinds of value rather than copied: the entry lifetime, the `NotAdmitted` `debug`, the
        // broken-write `warn` and the `source` every record carries are rules about *the store*, not about pictures. A
        // second write path that re-derived them would be the place they eventually disagree — one of them gaining a
        // field, or losing the distinction between a decline and a break. What stays in the callers is the only part
        // that is genuinely per-kind: turning the value into bytes, and what a value that will not encode is recorded
        // as.
        match self.memo.set_bytes(key, bytes, ENTRY_TTL) {
            Ok(()) => {}
            // `debug`, because it is ordinary: a memory fallback declines most large results by design.
            Err(MemoError::NotAdmitted) => {
                tracing::debug!(
                    source = self.source(),
                    key,
                    bytes = bytes.len(),
                    "the store declined to keep a result"
                );
            }
            // `warn`, because a run whose every write breaks is caching nothing while behaving exactly as though it
            // were.
            Err(error) => {
                tracing::warn!(
                    source = self.source(),
                    key,
                    bytes = bytes.len(),
                    %error,
                    "the store failed to keep a result"
                );
            }
        }
    }

    /// What is stored under `key` read back as a `T` — the result of a run whose result is **not** a picture — or
    /// `None` on any of the ways a read misses.
    ///
    /// An entry stored as a picture is a miss here, never a value. An absent key, an elapsed TTL and a store that
    /// broke reach a caller exactly as they do through [`get_bytes`](Self::get_bytes), and a value that will not
    /// decode is `warn`ed about on its way to becoming a miss, as [`decode`](Self::decode) does for a picture.
    ///
    /// Blocks, for the read.
    pub(crate) fn get_value<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let span = unit_span!("cache_read", key, hit = tracing::field::Empty);
        let found = span.in_scope(|| self.read_value(key));
        read_ended(&span, found.is_some());

        found
    }

    /// [`get_value`](Self::get_value)'s body, inside its span.
    fn read_value<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        // Neither kind of entry can be read as the other, by two independent mechanisms rather than care at the call
        // sites:
        //
        // - **The keys are disjoint by construction.** A data key is folded by `process::identity_of_data`, which mixes
        //   in a discriminator no chain key can produce, so the two kinds of entry never collide on a key in the first
        //   place.
        // - **The encodings reject each other.** JSON bytes are not a valid PNG and PNG bytes are not valid JSON, so an
        //   entry read as the wrong kind is an `Err` here or in `decode` rather than a value of the wrong kind. That
        //   matters more than it sounds: a caller handed a plausible wrong answer has nothing to compare it against,
        //   while a miss costs a re-run and is always correct.
        //
        // Either one alone would be enough today, and neither is kept alone. The first would make the guarantee depend
        // on a string derivation staying correct forever; the second on two formats never overlapping. They cost
        // nothing to keep together and each covers the other's failure.
        let bytes = self.read_bytes(key)?;

        match serde_json::from_slice(&bytes) {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::warn!(
                    source = self.source(),
                    key,
                    bytes = bytes.len(),
                    %error,
                    "a stored result would not decode"
                );
                None
            }
        }
    }

    /// Stores `value` under `key` for [`ENTRY_TTL`], discarding every way that can fail.
    ///
    /// [`put`](Self::put)'s counterpart for a result that is not a picture, with the same contract and the same
    /// records.
    ///
    /// Blocks, for the encode as much as the write.
    pub(crate) fn put_value<T: Serialize>(&self, key: &str, value: &T) {
        let span = unit_span!("cache_write", key);
        span.in_scope(|| self.write_value(key, value));
        unit::mark(&span, &Outcome::Finished);
    }

    /// [`put_value`](Self::put_value)'s body, inside its span.
    fn write_value<T: Serialize>(&self, key: &str, value: &T) {
        // **JSON**, because `serde_json` is already a dependency and the values are small — a crowded photograph's
        // worth of faces is kilobytes against the megabytes a stored picture holds, so a binary format three times
        // smaller would be saving a rounding error at the cost of a crate. A data family that produced something large
        // — a per-pixel mask — reopens that, with a measurement.
        let bytes = match serde_json::to_vec(value) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    source = self.source(),
                    key,
                    %error,
                    "a result would not encode and was not kept"
                );
                return;
            }
        };

        self.write(key, &bytes);
    }
}

// ── Writing off a run's critical path ──────────────────────────────────────────────────────────────────────────────

/// How many results may wait to be written before a run producing another waits for room.
///
/// Two. Each queued result is a whole picture held in memory until it is encoded, so this bounds memory as much as
/// lag: a slider dragged faster than PNG encodes would otherwise queue every intermediate result it passed through.
/// Two lets one run's write overlap the next run without letting a burst pile up.
const QUEUED_WRITES: usize = 2;

/// The one thread every [`RunCache::put_later`] is written on, or `None` where it could not be started.
///
/// One for the process rather than one per store: writes are disk-bound and gain nothing from running side by side,
/// and a single queue is what keeps them in the order they were produced.
static WRITER: LazyLock<Option<SyncSender<Write>>> = LazyLock::new(|| {
    let (queue, writes) = sync_channel::<Write>(QUEUED_WRITES);

    std::thread::Builder::new()
        .name("opai-cache-writer".to_string())
        .spawn(move || writes.into_iter().for_each(Write::run))
        .inspect_err(|error| tracing::warn!(%error, "the cache writer could not start; results are written in line"))
        .ok()
        .map(|_| queue)
});

/// Every key with a write queued or in progress.
static PENDING: LazyLock<Pending> = LazyLock::new(Pending::default);

/// One queued [`RunCache::put_later`].
struct Write {
    /// The store to write to.
    cache: RunCache,
    /// Where.
    key: String,
    /// What, shared with whatever else still holds it.
    image: Arc<DynamicImage>,
    /// The span the write was asked for in, which its own `cache_write` span is opened beneath, and the subscriber
    /// that span belongs to, which a thread of this module's own does not otherwise have.
    carried: Carried,
}

impl Write {
    /// Writes, and says so to anything waiting on the key — even where the write panicked, which would otherwise
    /// leave a reader of the key waiting forever.
    fn run(self) {
        let Self { cache, key, image, carried } = self;

        let wrote = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            carried.in_scope(|| cache.put(&key, &image));
        }));

        PENDING.end(&key);

        if wrote.is_err() {
            carried.in_dispatch(|| {
                tracing::warn!(source = cache.source(), key, "writing a result panicked and it was not kept");
            });
        }
    }
}

/// The keys with a write outstanding, counted, and the signal that one has finished.
///
/// Keyed on the key alone rather than on the store as well: a key is folded from a source's content and a chain, so
/// two stores holding the same key is two stores of the same result, and a reader of one waiting out the other's
/// write loses at most that wait.
#[derive(Default)]
struct Pending {
    keys: Mutex<HashMap<String, usize>>,
    ended: Condvar,
}

impl Pending {
    fn begin(&self, key: &str) {
        *crate::task::lock(&self.keys).entry(key.to_string()).or_default() += 1;
    }

    fn end(&self, key: &str) {
        let mut keys = crate::task::lock(&self.keys);

        if let Some(count) = keys.get_mut(key) {
            *count -= 1;
            if *count == 0 {
                keys.remove(key);
            }
        }

        self.ended.notify_all();
    }

    /// Waits until nothing is queued or being written under `key`. Immediate where nothing is.
    fn wait_for(&self, key: &str) {
        let keys = crate::task::lock(&self.keys);
        let _unblocked = self
            .ended
            .wait_while(keys, |keys| keys.contains_key(key))
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }

    /// Waits until nothing is queued or being written at all, or `timeout` passes. Whether it got there.
    fn settle(&self, timeout: Duration) -> bool {
        let keys = crate::task::lock(&self.keys);
        let (_keys, waited) = self
            .ended
            .wait_timeout_while(keys, timeout, |keys| !keys.is_empty())
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        !waited.timed_out()
    }
}

/// Waits for every result [`RunCache::put_later`] has queued to be written, or for `timeout` to pass, and says which.
///
/// What a process calls on its way out, so the last run's results are not lost with the writer thread; and what a
/// test reading a [`Memo`] directly calls before it looks.
pub(crate) fn settle(timeout: Duration) -> bool {
    PENDING.settle(timeout)
}

/// Closes a read's span, saying whether the store served it. Never failed: every way a read breaks is a miss.
fn read_ended(span: &tracing::Span, hit: bool) {
    span.record("hit", hit);
    unit::mark(span, &Outcome::Finished);
}

/// Encodes `image` into what is stored, or `None` if it cannot be encoded.
fn encode(image: &DynamicImage) -> Option<Vec<u8>> {
    // **PNG, because it is lossless and carries both depths.** A served result has to be the image the run produced —
    // a caller cannot tell a hit from a miss and would be shown a degraded picture as an enhancement — so a lossy codec
    // is excluded outright. Of what is left, PNG carries 8 and 16 bits per channel, which are exactly the two this
    // pipeline produces. WebP-lossless is the near miss and is rejected for being 8-bit only: it would be the line on
    // which a 16-bit result a caller asked for silently became an 8-bit one.
    //
    // **`PngCompression::Fast`**, matching the reference's own `BestSpeed`, because this write is on the run's critical
    // path: the result is encoded and written *inside* the call, and for a large upscale that is a real fraction of it.
    // The default `PngFilter` is kept — it is what the encoder's speed preset expects to sit beside, and the filter is
    // not where the time goes.
    //
    // Raw bytes rather than `get_or_compute`'s framing, which matters twice: `DynamicImage` is not `Serialize`, so the
    // typed path could not carry one anyway; and PNG is self-describing with `decode_bytes` routing on magic, so an
    // entry written by an older build is read as what it is rather than misinterpreted.
    let mut bytes = Vec::new();
    let options = EncodeOptions::Png { compression: PngCompression::Fast, filter: PngFilter::default() };

    encode_writer(image, &mut bytes, ImageFormat::Png, Some(options)).ok()?;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView as _, ImageBuffer, Rgb};

    // The shared gradient, under the name this module's round-trip tests read it by: an 8-bit picture whose every
    // pixel is distinct, so a round trip that reproduces it read the right bytes.
    use crate::logging::{self, field, records};
    use crate::models::face::{Confidence, Face, Faces, Point, Rect};
    use imaging::test_support::gradient as eight_bit;

    /// A store that was never opened, which is the `None` arm reached honestly rather than by injection.
    fn nothing() -> Store {
        Store { memo: None, mode: CacheMode::None }
    }

    /// The same at 16 bits per channel, with values above 255 so a truncation to 8 bits is visible.
    fn sixteen_bit(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([(x * 257 % 65536) as u16, (y * 511 % 65536) as u16, 65535 - ((x + y) % 65536) as u16])
        }))
    }

    /// What a detection run produces, as the value tests of the second kind of entry store and read back.
    ///
    /// The crate's own result type rather than a stand-in, because what has to round-trip is what is actually kept.
    fn faces() -> Faces {
        Faces::new([Face::new(
            Rect::new(Point::new(12.0, 34.0), Point::new(56.0, 78.0)),
            [Point::new(1.5, 2.5); Face::LANDMARKS],
            Confidence::new(0.875).expect("a confidence in range"),
        )])
    }

    /// A run's view of `cache`, for a chain starting from a picture identified as `source`.
    fn run_cache(cache: &Store, source: &str) -> RunCache {
        RunCache::new(cache.handle().expect("this store was opened"), source)
    }

    #[test]
    fn a_store_opens_on_disk_and_says_so() {
        let root = tempfile::tempdir().unwrap();

        let cache = Store::open(root.path());

        assert_eq!(cache.mode(), CacheMode::Disk);
        assert!(root.path().join(CACHE_DIR).is_dir(), "the disk arm did not create its directory");
        assert!(cache.handle().is_some());
    }

    #[test]
    fn a_directory_that_cannot_be_created_falls_back_to_memory() {
        // The trigger this reproduces is not the production one — that is another copy of the application holding the
        // directory, which CI cannot stage — but the fallback it exercises is the same one: a file where the
        // directory would go cannot be created or resolved, so the disk arm fails and the memory arm answers.
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(CACHE_DIR), b"not a directory").unwrap();

        let cache = Store::open(root.path());

        assert_eq!(cache.mode(), CacheMode::Memory);
        assert!(cache.handle().is_some(), "the memory fallback opened no store");
    }

    #[test]
    fn the_memory_fallback_still_serves_what_it_was_given() {
        // Degrading is only worth doing if the degraded thing works: results are kept for the life of the process.
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(CACHE_DIR), b"not a directory").unwrap();
        let cache = Store::open(root.path());
        let run = run_cache(&cache, "cafebabecafebabe");

        run.put("step", &eight_bit(32, 24));

        assert_eq!(run.get("step").map(|image| image.dimensions()), Some((32, 24)));
    }

    #[test]
    fn a_store_that_was_never_opened_serves_nothing_and_keeps_nothing() {
        let cache = nothing();

        assert_eq!(cache.mode(), CacheMode::None);
        assert!(cache.handle().is_none(), "the `None` arm handed out a store");
    }

    #[test]
    fn opening_one_directory_twice_yields_one_store_that_still_serves_what_the_first_wrote() {
        // The spec's "an application initialized twice" scenario, and `memo`'s own property rather than something
        // arranged here: the database takes an exclusive lock, and a plain second open would fail with an error
        // indistinguishable from another *process* holding it.
        let root = tempfile::tempdir().unwrap();

        let first = Store::open(root.path());
        run_cache(&first, "cafebabecafebabe").put("step", &eight_bit(32, 24));

        let second = Store::open(root.path());

        assert_eq!(second.mode(), CacheMode::Disk, "the second open fell back rather than sharing the store");
        assert_eq!(
            run_cache(&second, "cafebabecafebabe").get("step").map(|image| image.dimensions()),
            Some((32, 24)),
            "the second opening discarded what the first had stored"
        );
    }

    #[test]
    fn a_round_trip_preserves_the_pixels_exactly_at_both_depths() {
        // The property that lets a hit be indistinguishable from a miss. Asserted on the pixels themselves rather
        // than on the dimensions, because a lossy codec would pass a dimensions check unchanged.
        for original in [eight_bit(37, 23), sixteen_bit(37, 23)] {
            let bytes = encode(&original).expect("a picture this pipeline produces encodes");
            let served = decode_bytes(&bytes).expect("what was just encoded decodes");

            assert_eq!(served.dimensions(), original.dimensions());
            assert_eq!(served.as_bytes(), original.as_bytes(), "the round trip altered the pixels");
        }
    }

    #[test]
    fn a_sixteen_bit_result_does_not_come_back_eight_bit() {
        // The failure a lossy or 8-bit-only codec would introduce, and the reason WebP-lossless is rejected: a caller
        // that asked for 16 bits would be handed 8 with nothing to tell it apart from a computed result.
        let original = sixteen_bit(40, 30);

        let served = decode_bytes(&encode(&original).unwrap()).unwrap();

        assert!(matches!(served, DynamicImage::ImageRgb16(_)), "a 16-bit result came back as {served:?}");
    }

    #[test]
    fn a_stored_result_comes_back_as_the_picture_that_was_stored() {
        let root = tempfile::tempdir().unwrap();
        let run = run_cache(&Store::open(root.path()), "cafebabecafebabe");
        let original = sixteen_bit(48, 32);

        run.put("step", &original);
        let served = run.get("step").expect("what was just stored is served");

        assert_eq!(served.dimensions(), original.dimensions());
        assert_eq!(served.as_bytes(), original.as_bytes());
    }

    #[test]
    fn a_queued_write_is_waited_for_by_a_read_rather_than_missed() {
        // The promise `put_later` makes in place of writing synchronously: a read of the key, however soon after, is
        // served what was queued. Several in a row, so the queue's bound is reached and the caller waits for room.
        let root = tempfile::tempdir().unwrap();
        let run = run_cache(&Store::open(root.path()), "cafebabecafebabe");
        let original = Arc::new(sixteen_bit(48, 32));

        for step in 0..(QUEUED_WRITES * 2) {
            run.put_later(&format!("step{step}"), Arc::clone(&original));
        }

        for step in 0..(QUEUED_WRITES * 2) {
            let served = run.get(&format!("step{step}")).expect("a queued write is served once it lands");
            assert_eq!(served.as_bytes(), original.as_bytes());
        }

        assert!(
            settle(Duration::from_secs(60)),
            "nothing should be left queued once every key has been read back"
        );
    }

    #[test]
    fn a_read_says_whether_the_store_served_it_and_a_value_read_is_one_span() {
        let root = tempfile::tempdir().unwrap();
        let run = run_cache(&Store::open(root.path()), "cafebabecafebabe");

        let (_, observed, ()) = crate::logging::traced_of_blocking("info", || {
            assert!(run.get_bytes("step").is_none());
            run.put("step", &sixteen_bit(48, 32));
            assert!(run.get_bytes("step").is_some());

            assert!(run.get_value::<Faces>("faces").is_none());
            run.put_value("faces", &faces());
            assert!(run.get_value::<Faces>("faces").is_some());
        });

        let reads = observed.named("cache_read");
        let hits: Vec<Option<&str>> = reads.iter().map(|read| read.field("hit")).collect();
        assert_eq!(hits, [Some("false"), Some("true"), Some("false"), Some("true")], "{:#?}", observed.spans);
        assert!(reads.iter().all(|read| read.parent.is_none()), "a value read nested a second read span");

        let writes = observed.named("cache_write");
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].field("key"), Some("step"));
        assert!(reads.iter().chain(&writes).all(|span| span.field("outcome") == Some("finished")));
    }

    #[test]
    fn an_absent_key_a_broken_store_and_an_undecodable_entry_are_all_one_miss() {
        // All three are answered the same way — run the operation — and that answer is always correct, so nothing
        // above this has to be able to tell them apart.
        let root = tempfile::tempdir().unwrap();
        let cache = Store::open(root.path());
        let run = run_cache(&cache, "cafebabecafebabe");

        // Never written.
        assert!(run.get("absent").is_none());

        // Written, but not as an image any longer: a truncated write, or an interrupted shutdown.
        run.memo.set_bytes("corrupt", b"\x89PNG\r\n\x1a\n and then nothing", ENTRY_TTL).unwrap();
        assert!(run.get("corrupt").is_none(), "an entry that will not decode was served");

        // And a store that was never opened at all, which is the broken one this crate can actually construct:
        // `RunCache` cannot be built without a `Memo`, so the shape of "no store" is having none.
        assert!(nothing().handle().is_none());
    }

    #[test]
    fn a_declined_write_leaves_the_caller_holding_its_result() {
        // `NotAdmitted`, reproduced with a value larger than the whole budget — which is also the production case
        // this fallback actually meets: a large 8x result against a 256 MiB memory tier.
        let memo = Memo::memory(CacheOpts::new().max_capacity(1024)).unwrap();
        let run = RunCache::new(memo, "cafebabecafebabe");
        let result = eight_bit(256, 256);

        run.put("too-large", &result);

        // Nothing was kept, and nothing reached the *caller*: `put` returns `()`, so there is no channel by which a
        // declined write could reach the run that produced the pixels. What it does reach is the log — see
        // `a_write_the_store_declines_is_recorded_below_the_default_level`, which is where the distinction lives.
        assert!(run.get("too-large").is_none(), "a value over the budget was admitted after all");
        assert_eq!(result.dimensions(), (256, 256), "the caller's own result was affected by the write");
    }

    #[test]
    fn an_entry_written_with_no_life_left_is_dropped_rather_than_served() {
        // The other `NotAdmitted`: a zero TTL is a write that expires the instant it lands, and `memo` says so
        // instead of doing the I/O. It reaches `put` the same way every other decline does — dropped, and recorded
        // at debug.
        let memo = Memo::memory(CacheOpts::new()).unwrap();
        let run = RunCache::new(memo.clone(), "cafebabecafebabe");

        assert!(memo.set_bytes("expired", b"anything", Duration::ZERO).is_err());
        assert!(run.get("expired").is_none());
    }

    #[test]
    fn the_entry_lifetime_is_a_day() {
        // Pinned on its own because it is the only thing that ever gives the disk space back: nothing bounds this
        // store by size, so a TTL that drifted would change how much of a machine the application can hold.
        assert_eq!(ENTRY_TTL, Duration::from_secs(86_400));
    }

    #[test]
    fn a_run_carries_the_identity_its_keys_are_folded_from() {
        let memo = Memo::memory(CacheOpts::new()).unwrap();

        assert_eq!(RunCache::new(memo, "cafebabecafebabe").source(), "cafebabecafebabe");
    }

    #[test]
    fn a_write_the_store_declines_is_recorded_below_the_default_level() {
        // Ordinary rather than wrong: the memory fallback declines any result larger than its whole budget, which for
        // a large 8x run is the designed outcome. A user's file should not fill with it.
        let memo = Memo::memory(CacheOpts::new().max_capacity(1024)).unwrap();
        let run = RunCache::new(memo, "cafebabecafebabe");

        let (log, ()) = logging::records_of_blocking("debug", || run.put("too-large", &eight_bit(256, 256)));

        let declined = records(&log, "the store declined to keep a result");
        assert_eq!(declined.len(), 1, "a declined write was recorded {} times:\n{log}", declined.len());
        assert!(declined[0].contains("level=DEBUG"), "an ordinary decline is above debug: {}", declined[0]);
        assert!(declined[0].contains("key=too-large"), "the decline does not name the entry: {}", declined[0]);
        assert_eq!(field(declined[0], "source"), Some("cafebabecafebabe"), "{}", declined[0]);

        // And at the default level it is absent, which is the half that keeps a session's file readable.
        let (ordinary, ()) = logging::records_of_blocking("info", || run.put("too-large", &eight_bit(256, 256)));
        assert!(records(&ordinary, "the store declined to keep a result").is_empty(), "{ordinary}");
    }

    #[test]
    fn a_result_that_will_not_encode_is_a_warning_and_still_not_the_run_s_problem() {
        // An image with no pixels, which the PNG encoder refuses. `put` still returns `()`: the distinction lands in
        // the log rather than in the signature.
        let memo = Memo::memory(CacheOpts::new()).unwrap();
        let run = RunCache::new(memo, "cafebabecafebabe");
        let unencodable = DynamicImage::ImageRgb8(ImageBuffer::new(0, 0));

        let (log, ()) = logging::records_of_blocking("info", || run.put("empty", &unencodable));

        let refused = records(&log, "a result would not encode and was not kept");
        assert_eq!(refused.len(), 1, "a failed encode was recorded {} times:\n{log}", refused.len());
        assert!(refused[0].contains("level=WARN"), "work being redone is not a warning: {}", refused[0]);
        assert!(refused[0].contains("key=empty"), "the record does not name the entry: {}", refused[0]);
        // `key` names the entry and is a fold nothing else in the file spells; `source` is what a reader joins to the
        // run's own pair of records.
        assert_eq!(field(refused[0], "source"), Some("cafebabecafebabe"), "{}", refused[0]);

        // Nothing was kept, and the store is otherwise untouched.
        assert!(run.get("empty").is_none());
    }

    #[test]
    fn an_entry_that_will_not_read_back_says_so_where_it_was_found() {
        // A truncated write or an interrupted shutdown. Distinguished from a plain miss in the log precisely because
        // it is not one: the work was paid for, stored, and is now being paid for again.
        let memo = Memo::memory(CacheOpts::new()).unwrap();
        let run = RunCache::new(memo, "cafebabecafebabe");
        run.memo.set_bytes("corrupt", b"\x89PNG\r\n\x1a\n and then nothing", ENTRY_TTL).unwrap();

        let (log, served) = logging::records_of_blocking("info", || run.get("corrupt"));
        assert!(served.is_none(), "an entry that will not decode was served");

        let broken = records(&log, "a stored result would not decode");
        assert_eq!(broken.len(), 1, "an undecodable entry was recorded {} times:\n{log}", broken.len());
        assert!(broken[0].contains("level=WARN"), "{}", broken[0]);
        assert!(broken[0].contains("bytes="), "the record does not say how much was stored: {}", broken[0]);

        // A plain miss is recorded by nothing here: the chain records it, once, as the step it made run.
        let (quiet, ()) = logging::records_of_blocking("debug", || {
            assert!(run.get("never-written").is_none());
        });
        assert!(!quiet.contains("msg="), "a plain miss wrote a record:\n{quiet}");
    }

    #[test]
    fn a_result_that_is_not_a_picture_round_trips() {
        // The second kind of value the store holds, over the backing a CI runner always has. What a detection run
        // produces is asserted rather than a stand-in type, because the encoding has to carry what this crate
        // actually stores.
        let run = RunCache::new(Memo::memory(CacheOpts::new()).unwrap(), "cafebabecafebabe");
        let found = faces();

        run.put_value("analysis", &found);

        let served: Faces = run.get_value("analysis").expect("what was just stored is served");
        assert_eq!(served, found, "the round trip altered the result");
    }

    #[test]
    fn neither_kind_of_entry_can_be_read_as_the_other() {
        // `run-cache`'s requirement, at the one place the two kinds meet. An entry read back as the wrong kind is a
        // *wrong answer* rather than a miss, and a wrong answer here is invisible: a caller handed a plausible value
        // has nothing to compare it against. So both directions are driven, and both have to be misses.
        //
        // Written by the real `put` and `put_value` rather than by hand-built bytes, so this breaks if either
        // encoding changes — which is the only way it could stop being a test of what is actually stored.
        let run = RunCache::new(Memo::memory(CacheOpts::new()).unwrap(), "cafebabecafebabe");

        run.put("a-picture", &eight_bit(32, 24));
        run.put_value("a-result", &faces());

        // Both entries are there and each reads back as itself, so what follows is about the *kind* rather than about
        // an empty store.
        assert!(run.get("a-picture").is_some());
        assert_eq!(run.get_value::<Faces>("a-result").as_ref(), Some(&faces()));

        // A picture read as a result: PNG bytes are not valid JSON.
        let (as_value, served) = logging::records_of_blocking("info", || run.get_value::<Faces>("a-picture"));
        assert!(served.is_none(), "a stored picture was decoded as a result");
        assert_eq!(records(&as_value, "a stored result would not decode").len(), 1, "{as_value}");

        // And a result read as a picture: JSON bytes are not a valid PNG.
        let (as_picture, served) = logging::records_of_blocking("info", || run.get("a-result"));
        assert!(served.is_none(), "a stored result was decoded as a picture");
        assert_eq!(records(&as_picture, "a stored result would not decode").len(), 1, "{as_picture}");

        // Both fold into a miss rather than an error: neither call can return one, which is the signature saying so.
        // The response to each is running the operation, and that is always correct.
    }

    #[test]
    fn a_value_is_kept_for_the_same_lifetime_a_picture_is() {
        // `memo` exposes no way to read an entry's remaining life, so the lifetime is not observable from out here —
        // what is observable is that both kinds of value are written by one function, which is what makes it one
        // lifetime rather than two that agree today. Driven through a store that declines the write: the record is
        // `RunCache::write`'s, so a `put_value` that grew a `set_bytes` call of its own would stop producing it.
        let memo = Memo::memory(CacheOpts::new().max_capacity(8)).unwrap();
        let run = RunCache::new(memo, "cafebabecafebabe");

        let (from_value, ()) = logging::records_of_blocking("debug", || run.put_value("analysis", &faces()));
        let (from_picture, ()) = logging::records_of_blocking("debug", || run.put("step", &eight_bit(64, 64)));

        for log in [&from_value, &from_picture] {
            let declined = records(log, "the store declined to keep a result");
            assert_eq!(declined.len(), 1, "a declined write was recorded {} times:\n{log}", declined.len());
            assert!(declined[0].contains("level=DEBUG"), "{}", declined[0]);
            assert_eq!(field(declined[0], "source"), Some("cafebabecafebabe"), "{}", declined[0]);
        }

        // And the constant both of them pass is the one the module pins at a day — see `the_entry_lifetime_is_a_day`.
        assert_eq!(ENTRY_TTL, Duration::from_secs(86_400));
    }

    #[test]
    fn a_value_that_will_not_serialise_is_a_warning_and_still_not_the_run_s_problem() {
        // A map whose keys are not strings, which is JSON's own limit rather than a contrivance: `serde_json` refuses
        // it at the encode. `put_value` still returns `()` — the distinction lands in the log, exactly as an image
        // that would not encode does — and nothing is stored under the key.
        let run = RunCache::new(Memo::memory(CacheOpts::new()).unwrap(), "cafebabecafebabe");
        let unencodable = std::collections::BTreeMap::from([([1_u8, 2], 3_u32)]);

        let (log, ()) = logging::records_of_blocking("info", || run.put_value("analysis", &unencodable));

        let refused = records(&log, "a result would not encode and was not kept");
        assert_eq!(refused.len(), 1, "a failed encode was recorded {} times:\n{log}", refused.len());
        assert!(refused[0].contains("level=WARN"), "work being redone is not a warning: {}", refused[0]);
        assert!(refused[0].contains("key=analysis"), "the record does not name the entry: {}", refused[0]);
        assert_eq!(field(refused[0], "source"), Some("cafebabecafebabe"), "{}", refused[0]);

        assert!(
            run.get_value::<std::collections::BTreeMap<[u8; 2], u32>>("analysis").is_none(),
            "a value that would not encode was stored anyway"
        );
    }

    #[test]
    fn a_read_the_store_fails_is_a_warning_rather_than_a_miss() {
        // No store fails on demand (see `RunCache::read_or_warn`), so the store's answer is handed to it directly, which
        // is the branch `get_bytes` takes and not a re-emission beside it: delete the record there and this fails.
        let (failed_read, served) = logging::records_of_blocking("info", || {
            RunCache::read_or_warn("cafebabecafebabe", "a-step", Err(MemoError::NotAdmitted))
        });
        assert!(served.is_none(), "a store that failed served bytes");

        let failed = records(&failed_read, "the store failed to read a result");
        assert_eq!(failed.len(), 1, "{failed_read}");
        assert!(failed[0].contains("level=WARN"), "{}", failed[0]);
        assert!(failed[0].contains("key=a-step"), "{}", failed[0]);
        assert!(failed[0].contains("error="), "the record does not say what the store said: {}", failed[0]);
        // The run it belongs to, which is what a reader joins to the enhancement's own pair of records.
        assert_eq!(field(failed[0], "source"), Some("cafebabecafebabe"), "{}", failed[0]);

        // And the arm beside it stays silent, so a plain miss and an absent value are not warned about on the way
        // through: only a store that answered with an error is.
        let (quiet, served) = logging::records_of_blocking("debug", || {
            RunCache::read_or_warn("cafebabecafebabe", "a-step", Ok(Some(b"stored".to_vec())))
        });
        assert_eq!(served.as_deref(), Some(b"stored".as_slice()));
        assert!(!quiet.contains("msg="), "a successful read wrote a record:\n{quiet}");
    }
}
