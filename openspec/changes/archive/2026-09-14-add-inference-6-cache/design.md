## Slices

Running an image through a chain of models estimates far past 15 tasks. It is divided into six
changes, each leaving the project buildable and testable on its own. **This change is slice 6, the
last of them.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-inference-1-models` | A named model's files on disk, verified against the published manifest | the initialization series | **done** |
| 2 | `add-inference-2-providers` | The `ExecutionProvider` type including `Auto`, per-provider default options, and the per-model profile that overrides them | 1 | **done** |
| 3 | `add-inference-3-sessions` | Session construction, the resident session cache with its idle life, and CPU fallback | 2 | **done** |
| 4 | `add-inference-4-tiling` | The shared inference primitives — the tile grid, reflection padding, CHW tensor conversion at either output depth, seam blending — and the tiled driver over them, generic in the per-tile run | none (parallel with 1-3) | **done** |
| 5 | `add-inference-5-kyoto` | The upscale pipeline and the public chain entry point, with progress, cancellation and `OutputDepth` — Kyoto runs end to end | 3, 4 | **done** |
| 6 | `add-inference-6-cache` | The 24-hour per-step image cache, keyed on the input identity plus the operations applied | 5 | **current** |

Slice 4 depended on nothing in 1-3. Everything else is a straight chain, and slice 5 was the first
that needed all of it at once.

**Slice 6's row is the one the previous five wrote, and it is delivered as written** — with one
addition the row could not have foreseen and which is not scope creep but a precondition: the key
this cache reads is `Picture::identity`, and that identity does not currently distinguish two results
that differ in bit depth. D2 is that fix. Without it this slice cannot be built correctly at all.

Deferred past the series, each to its own change when something needs it: the model memory budget
with LRU eviction; **a size budget with eviction for the image cache on disk** (added by this slice —
see D4 and the risk of the same name); alpha preservation through inference; the eleven measured
execution-provider profiles other than Kyoto's; the remaining seven families' pipelines; Osaka's
diffusion pipeline; logging; evicting sessions built on a provider the user has just switched away
from; the per-tile divergence guard; and the front-end wiring — the Tauri command, the event, the
React control, the cache settings toggle and the degradation warning — which is now the whole of what
stands between this library and a working application.

## Context

See proposal.md - Why for motivation, specs/image-cache/spec.md and specs/image-enhancement/spec.md
for the behaviour.

Slice 5 left this slice almost nothing to invent, and said so as it went:

- **`chain::identity_after(identity, operations)`** already folds the input's identity with each
  operation's `cache_tag`, one hash per operation, and its doc comment records that the fold — rather
  than a join — is what makes a chained pair of runs agree with a single run over the concatenated
  operations. That is the cache key, already written.
- **`Operation::cache_tag()`** already folds the per-run parameters a model's identity does not carry
  — the selected faces of a recovery run, a strength, a scale. Slice 1 built it for this.
- **`ProcessOptions`** is a bundle with a `Default` and a documented `..Default::default()` idiom,
  built so *"a cache mode is one more defaulted field rather than a changed signature at every call
  site"*.
- **`OperationProgress`** already owns the mapping from "this operation, 0 to 1" onto the chain's
  single bar, and already clamps so a report cannot go backwards. A hit needs one more method on it,
  not a second weighting.
- **`InferenceError` is `Clone`**, put there by slice 5's D9 in anticipation of a cache that
  single-flighted. D5 is why that anticipation is not taken up, and why the `Clone` stays anyway.
- **`chain::run` already has the seam**: it loops per operation, `await`s the session acquisition on
  the runtime and hands the synchronous work to `task::spawn_blocking`. The cache reads and writes go
  in that same loop, on that same blocking thread.

Six facts about `rust-sak`'s `memo` shape the design and were checked against its source and its
README rather than assumed:

- **`Memo` is `Clone` and holds an `Arc<dyn Store>`; there is no `close`.** The store is released when
  the last handle drops (`memo/mod.rs:71`). This removes the whole of the reference's teardown
  problem — see D11.
- **`Memo::disk` hands back the store this process already has open** for the same directory, matched
  after resolving symlinks, rather than failing on the database's exclusive lock. The reference needed
  a `NewDiskShared` variant and a mutex-guarded check-then-swap to get this; here it is the library's
  own property.
- **It blocks**, and can wait out a teardown of the same directory for up to 50 ms, so it is opened
  from `spawn_blocking` or at start-up — its own documentation says so.
- **`get_bytes`/`set_bytes` are transparent**: bytes written come back exactly, with none of the
  framing `get_or_compute` adds, so nothing constrains what is stored to be `Serialize`.
- **The disk tier is not bounded by size and does not claim to be.** `max_entries` is documented as
  ignored by it outright; `max_capacity` sizes redb's read cache, "not the directory". Expiry is by
  TTL, and the space is reclaimed by a sweep — one in the background when a disk store opens, and one
  per `cleanup()` call.
- **`MemoError::NotAdmitted`** separates a write the store declined — a value larger than the whole
  memory budget, a zero TTL — from `Storage`, a store that actually broke.

The reference's shape for this layer is `Process` reading `cache.GetImage(hash, applied...)` before
each operation and writing `cache.SetImage` after it, over an `internal.Cache` wrapping `go-sak`'s
`memo`. Three details in it are behavioural and are carried: the key being the input hash plus the
operations applied *so far*, the skipped operation still reporting its share of the bar, and a failed
write never failing the operation. Two are carried as **corrections** rather than as parity: what its
key does not contain (D2) and what its capacity figure does not do (D4).

## Goals / Non-Goals

**Goals:**

- The second identical run of a chain runs no model, and the user sees the bar fill rather than
  nothing happen.
- The cache cannot make a run wrong. Every failure mode it has — a store that will not open, a write
  that is refused, an entry that will not decode, a clock that jumped — produces at worst a recompute.
  There is no path from a cache failure to a wrong picture and no path to a failed run.
- The whole of it is exercised on every CI platform with no runtime, no GPU, no model file and no
  network, as the three slices before it managed — including the disk store, which is pure Rust and
  opens in a temporary directory.
- The public error surface does not move. `InferenceError` gains no variant, because the cache has
  nothing it is allowed to fail a run with.
- `Picture::identity` becomes true of every picture rather than nearly every picture.

**Non-Goals:**

- Bounding the store by size, or evicting anything before its TTL. See D4 and the risk.
- Coalescing concurrent identical runs. See D5.
- Caching anything but an image. Face detection produces a list of faces, has no pipeline here yet,
  and would be a different store with a different encoding.
- Any global switch. The cache is a per-run decision on an instance, not a process-wide setting.

## Decisions

### D1. A module of its own at the crate root, and a field on `Inner`

`cache.rs` beside `sessions/`, `providers/` and `models/`: the store, the key derivation, the encode
and decode, and the three-way degradation. Not inside `inference/`, which is where a run is carried
out — the chain *consumes* this the way it consumes the session cache, and a chain that could reach
into the store's internals is how the read path and the write path start disagreeing about a key.

It is the second thing in this crate called a cache and the distinction is worth stating once:
`sessions::caches` is what an *execution provider compiled* from a model, on disk under `engines/`,
keyed on an artifact and invalidated by a runtime bump. `cache` is *pixels*, under `cache/`, keyed on
an image and expiring after a day. They share nothing but the word, and because the first is always
spelled through its parent module the two never appear side by side.

The `Memo` handle is a field on `Inner`, beside `sessions` and for the same reason slice 3 gave for
that one: it is opened against the configuration directory *this* initialization resolved, and two
`Opai`s in one process — which a front end restarting its application layer produces — are two
applications with two directories. A `static` would give them one.

### D2. The resolved depth is folded into the identity, and this fixes a defect that exists today

```
identity_after(identity, operations, depth)
    = operations.fold(identity, |h, op| xxh3(format!("{h}|{}|{depth}", op.cache_tag())))
```

**The defect first, because it is not a cache problem.** `Picture` documents its identity as
describing *those* pixels, and spends its shape enforcing it — private fields, pixels behind an `Arc`
so they cannot be altered while the identity goes on describing what they used to be. Slice 5 then
composed an identity from the input's and the operations applied, and left the depth out. So
`process(pic, [kyoto_4x], Eight)` and `process(pic, [kyoto_4x], Sixteen)` return two pictures with
different pixels, different memory footprints and different export behaviour, carrying **one
identity**. Today that is a mislabel nothing acts on. The moment anything is keyed on it, it is the
wrong picture served — a caller asks for 16 bits and is handed the 8-bit result of an earlier run.

**It is the resolved depth, not the requested one.** `OutputDepth::Source` over a 16-bit photograph
and `OutputDepth::Sixteen` produce the same pixels, so they must produce the same identity and share
one entry; `Source` is resolved against the loaded image before the chain runs (slice 5's D3), and it
is that answer — `ChannelDepth::Eight` or `Sixteen` — that is folded. The spec has a scenario for
each direction.

**Folded per operation rather than appended once, and the difference is not cosmetic.** Appending the
depth to the finished fold — `xxh3(fold(h, ops) | depth)` — preserves the chained-pair property for a
run at one depth, and breaks across a chained pair at two. Take `[a]` at 8 bits, then `[b]` at 16 over
its result: the key is `xxh3(fold(fold(h,a),b) | 16)`. Now take `[a]` at 16 followed by `[b]` at 16:
the key is the same value, and the pixels are not — the first chain's second operation ran over an
8-bit intermediate. Folding the depth at every step makes the intermediate's own identity carry it,
so the two diverge at the first operation and never meet.

The chained-pair property the spec requires survives unchanged, by construction:
`g(g(h,a,d),b,d)` is what both a chained pair at `d` and a single run of `[a,b]` at `d` compute.

An empty chain still leaves the identity untouched, depth or no depth: nothing was applied, so
nothing was derived, and there is nothing to name.

**The cache key of a step is the identity of what that step produces** — `identity_after(source,
operations[..=i], depth)` — which is why there is one derivation rather than two. It is the property
the reference states and the reason it exports its key function under the name `ImageHashAfter`: a
result fed back into `process` looks up exactly the slot its own pixels were stored under. Two
derivations that agreed today would be two things to keep agreeing.

Three alternatives were weighed. **Putting the depth only in the cache key** is the counterexample
above, and leaves `Picture::identity` lying. **Refusing to cache at any depth but the default** would
turn the cache off for exactly the users it is worth most to — the RAW and 16-bit workflow this
project built two decode paths for. **Storing the depth beside the entry and checking it on read** is
a miss that costs a read of the whole value first, and fixes nothing about the identity.

This modifies an archived slice's requirement, which is why it is a `MODIFIED` delta against
`image-enhancement` rather than something this slice absorbs quietly.

### D3. Entries are PNG, at `Fast` compression, through `rust-sak`'s `image`

`rust_sak::image::encode_writer(&result, &mut Vec::new(), ImageFormat::Png, Some(EncodeOptions::Png {
compression: PngCompression::Fast, .. }))` on the way in, `decode_bytes` on the way out. Both are the
in-memory halves of the file functions `image::save` and `image::load` already go through, so the
cache reaches for nothing this crate does not already depend on.

**PNG because it is lossless and carries both depths.** The spec requires a served result to be the
image the run produced — a caller cannot tell a hit from a miss and would be shown a degraded picture
as an enhancement — so a lossy codec is excluded outright. Of what is left, PNG carries 8 and 16 bits
per channel, which are exactly the two this pipeline produces. WebP-lossless is the near miss and is
rejected on the ground slice 5's D5 rejected `imageops::resize`: it is 8-bit only, so it would be the
line on which a 16-bit result the caller asked for silently became an 8-bit one. The reference uses
PNG for the same reason, arrived at from a language whose standard library offered little else.

**`Fast` compression, matching the reference's `png.BestSpeed`,** because this write is on the run's
critical path: the result is encoded and written *inside* the call, and for a large upscale that is a
real fraction of it. The reference's own settings documentation says so in as many words. The default
`Adaptive` filter is kept — it is what the PNG encoder's speed preset expects to sit beside, and the
filter is not where the time goes.

**Raw bytes rather than `get_or_compute`'s framing.** `set_bytes` stores exactly what it is handed,
which matters twice: `DynamicImage` is not `Serialize`, so the typed path could not carry one anyway;
and PNG is self-describing with `decode_bytes` routing on magic, so an entry written by an older
build is read as what it is rather than misinterpreted. The postcard type-fingerprint caveat in
`memo`'s README — that reordering same-typed fields is not caught — cannot apply to an entry with no
header.

Alpha does not arise: slice 4 drops it as a channel, so a result is `Rgb8` or `Rgb16` and PNG carries
both. Storing raw uncompressed pixel buffers instead would be faster to write and roughly three times
larger on a disk nothing bounds, which is the wrong trade in exactly the dimension D4 leaves open.

### D4. Disk, then memory, then nothing — and `memory_disk` is not used

```
Memo::disk(<config>/cache, CacheOpts::new())      -> CacheMode::Disk
  └ on failure: Memo::memory(CacheOpts::new().max_capacity(256 MiB))  -> CacheMode::Memory
      └ on failure: no cache at all               -> CacheMode::None
```

Opened at the end of `initialize`, after everything the application cannot run without has succeeded,
and from `spawn_blocking` because opening blocks. The reference moved this last for the same reason
and left a comment saying why: it used to be first, and a cache that would not open failed a launch
that had nothing else wrong with it.

**Not `Memo::memory_disk`**, though the two-tier store is the module's own recommended default. What
is stored here is whole enhanced photographs — hundreds of megabytes each — and a memory tier in
front would claim RAM from the one thing already competing for it. An 8x 16-bit run holding well over
a gigabyte is the standing risk of this series, named in slice 4 and again in slice 5, and nothing
budgets it. A promotion tier pays off for entries that are small, hot and cheap to hold; these are
none of the three, and it would hold one or two before evicting.

**The memory fallback's budget is 256 MiB, and what that does not cover is stated rather than
discovered.** `memo` declines a value larger than the whole budget with `NotAdmitted`, so this
fallback admits a denoise at 1:1 and most 2x results, and declines a large 8x one. That is the right
shape: the fallback exists so a run whose disk store is locked is not *slower than it needs to be*,
not so that it matches a working cache. The reference chose 64 MiB and its own comment explains the
conservatism — the fallback is reached when the disk store would not open, which in production is
often a machine that has just run out of something. 256 MiB is four times that and still a small
fraction of what a single run holds; the figure has no significance beyond being large enough for the
common result and small enough not to be the reason a strained machine gets worse.

**`max_entries` is left at its default and the fact is recorded, because the reference's equivalent
does nothing.** It passes an entry count of 500 believing it bounds the store; in `go-sak` that
figure is entries per value-log file and in `rust-sak` the disk tier ignores it. A count is the wrong
bound for pictures anyway, whose sizes differ by two orders of magnitude.

**Nothing bounds the disk directory, and this is the accepted trade.** Entries expire after 24 hours
and their space is reclaimed by the sweep `memo` runs when the store opens — so a daily-launched
application reclaims yesterday's work on today's first launch. What is unbounded is one session: fifty
large enhancements can hold several gigabytes until that sweep. A size budget with LRU eviction is
general-purpose, belongs in `rust-sak`'s `memo` rather than here by the project's own rule, and is
deferred to its own change. Recorded as a risk below rather than left to be discovered.

### D5. Read, run, write — and `get_or_compute` is refused on two facts

The per-operation shape is the reference's: look the key up before the operation; on a miss run it and
write the result back; on a hit skip the model entirely.

`memo` offers the better-looking alternative — `get_or_compute_async` would make the read, the run and
the write one call **and** coalesce concurrent identical runs, which is what slice 5's D9 anticipated
when it made `InferenceError` `Clone`: *"a cache single-flighting a run hands one failure to every
waiter, exactly as the session cache does"*. It is not taken up, on two facts about what it does to
behaviour this project has already specified:

- **Cancellation.** Dropping the computing future releases every waiter with `ComputeAbandoned`. One
  user cancelling one export would fail an unrelated run that happened to want the same step. The
  spec requires a cancelled run to stop *itself* and to be distinguishable from a failure; turning one
  gesture into another run's error is neither.
- **Progress.** Only the computing caller's callback runs. A second run joined to it would report
  nothing for the whole operation and then jump — against the requirement that reports reach the
  caller while the run is going, and against the reason that requirement exists.

What is given up is real and small: two identical chains genuinely in flight at once each run. That
happens when a user drags a slider fast enough to overlap two requests, and the second is served from
the cache the moment the first writes. The reference does not coalesce either, for no recorded reason;
here there is one.

`InferenceError` stays `Clone`. It costs nothing, every source it carries is behind an `Arc` already,
and it is the right property for an error type whose values a front end may hold.

### D6. A hit hands back its share of the bar, under a stage of its own

`OperationProgress` gains `cached()`, which emits at fraction 1 under a new `Stage::Cached`. The
weighting arithmetic does not change at all — the operation's share of the chain is already
`(index + fraction) / operations`, and a hit simply reports its fraction as 1 immediately.

**A variant rather than reporting `Running` at 1.0.** A front end that can see the difference can say
*"Upscale — from an earlier run"* instead of showing a bar that jumps a fifth of its width with no
explanation, and reporting `Installing` at 100% for a transfer that never happened would be a report
that is simply false. `Stage` is `#[non_exhaustive]`, so the variant is additive for every caller.

**The skipped operation reports through the same path as every other**, which is the detail the
reference calls out and the reason its bar works: the operation-to-chain mapping stays in one place,
and a chain whose every operation was cached still ends on exactly 1 rather than never reporting at
all.

**The lookup walks forward, prefix by prefix**, rather than probing for the longest stored prefix
first. Longest-first would avoid running operation 1 in the case where the `[1,2]` entry survived and
the `[1]` entry did not — which a shared TTL makes rare, since the longer prefix is written later and
expires later. What settles it is the cost of asking: `get_bytes` returns the value, so probing a
longer prefix speculatively reads a several-hundred-megabyte blob off disk to find out whether it is
there. A forward walk reads exactly the entries it uses.

### D7. Where the cache sits in the loop, and what it does not get to skip

Per operation, in order:

1. **Refusals first, for the whole chain, unchanged.** `plan` already runs over every operation before
   anything else happens. A chain naming an unservable operation is refused even when every earlier
   step is cached — a cache must not turn a bad request into a partial success.
2. **Read and decode on a blocking thread.** Decoding a 300 MB PNG is exactly the kind of work slice 5
   moved off the runtime, and for the same reason: in a Tauri process the runtime is the window's
   event loop.
3. **On a hit**, report `Stage::Cached` and continue. **No session is acquired**, which is what makes
   a hit actually cost nothing — acquiring would install a model for an operation that is not going to
   run.
4. **On a miss**, acquire the sessions on the runtime as today, then run the passes, encode the result
   and write it — the encode and the write inside the same `spawn_blocking` as the run, so the large
   buffers stay on one thread and there is no second hop.

A cancelled or failed run writes nothing: it never reaches step 4's tail, because `run_passes` returns
the error instead of a picture. Nothing needs to check the token a second time.

### D8. The cache is a per-run field, read once

`ProcessOptions` gains `cache: bool`, defaulting to `true` — the field slice 5's D2 predicted, and the
reason that type is a bundle with a `Default`. `ProcessOptions { cache: false, ..Default::default() }`
is the whole of turning it off.

**Read once at the top of the run**, not per operation, which is the reference's own correction to
itself: a run must not read from the cache and then decline to write back to it. Here that is
`Option<Memo>` bound once for the call, which also removes the second half of what the reference's
comment guards — the store cannot be closed out from under the loop, because the run holds a handle
(D11).

**A `bool`, not an enum.** An enum would be right if there were a third mode anyone had asked for;
there is not, and `cache: false` at a call site says exactly what it means. **Per run, not a global
setter**, unlike the reference's `SetImageCacheEnabled`: `Opai` is an instance rather than a
process-global, and a hidden global that a run cannot see the value of is how the reference ended up
having to bind it to a local and comment about why.

What this serves is the two callers the reference named: a benchmark, which must not measure a read or
charge a PNG encode to a model, and an embedder processing images it must not persist.

### D9. `CacheMode` is reported, never configured

`pub enum CacheMode { Disk, Memory, None }`, `#[non_exhaustive]`, returned by `Opai::cache_mode()`.
Parity with the reference's `ImageCacheMode`, and it exists for the reason that one does: losing the
cache is deliberately not an error, so without something like this "some runs are silently uncached"
is unknowable from outside the machine. The reference's GUI raises a warning on anything but `Disk`;
doing that here is the front-end slice's work, and this is what it will read.

It is not settable. What backs the cache is an outcome of what the machine allowed, not a preference —
a caller that wants no cache turns it off per run (D8), which is a different question with a different
answer.

### D10. Every cache failure is the same failure to a run, and none of them is public

A write that was **declined** (`NotAdmitted`), a write that **broke** (`Storage`), a read that
**failed**, and an entry that **will not decode** are all ignored identically: the run carries on and
produces a correct result. The pixels are already computed by the time a write is attempted, and
throwing finished work away because a disk filled up is the cache's problem becoming the
enhancement's.

The reference separates declined from broken, and only to log them at different levels — a run
dropping every write is caching nothing while looking like it is, which is worth being able to find.
This project has no logging, so the distinction has nowhere to go and is deliberately not invented:
carrying it as an unread field would be the appearance of a decision rather than one. It is the first
thing to reinstate on the day logging lands, and the module says so.

**`InferenceError` therefore gains no variant.** The cache cannot fail a run, so it has nothing to
report through the type whose whole job is reporting why a run failed.

### D11. Ownership and lifetime: a run holds the store it is using

The `Memo` lives on `Inner`, so every clone of an `Opai` shares one handle, one store and one
in-flight table. Cloning is a refcount bump. A run clones the handle it needs for the call.

**That is the whole of the teardown safety**, and it is worth contrasting with what the reference
needs for the same property. There, `Destroy` swaps a global pointer to nil and closes the store,
`Process` binds it to a local and nil-checks it, a mutex serializes a check-then-swap in
`setupImageCache`, and a comment explains that re-reading the pointer per operation could pass the
check and then find a closed store further down the loop. Here there is no pointer, no close and no
window: the store is released when the last handle drops, and a run holding one is a handle. A
concurrent teardown cannot reach it.

Re-initializing under the same name reopens nothing — `Memo::disk` returns the store already open for
that directory — so an application initialized twice keeps what it stored, which is the spec's own
scenario. Re-initializing under a *different* name opens a second store under that name's directory,
which is correct: two names are two applications with two configuration directories.

**What the cache adds to a run's peak footprint** is one encoded buffer: on a miss, the PNG bytes
exist alongside the result while it is written; on a hit, the bytes exist alongside the decoded image
while it is decoded. Both are well under the image itself. Nothing here holds a session, takes a lock
across a pass, or touches a provider — the cache never sees one, which is also why this slice has no
provider matrix to validate (D12).

### D12. Platform and provider validation

**Everything in this slice runs on all three CI jobs, and the disk store is not stubbed for it.**
`memo`'s disk tier is redb — pure Rust, no C toolchain, no download — so it opens in a `tempfile`
directory on a runner with no runtime, no GPU, no model file and no network. The key composition and
its chained-pair property, hit and miss, the depth distinction in both directions, the bypass leaving
nothing behind, a write that fails not failing the run, an undecodable entry being a miss, the
progress a hit reports, and the three-way degradation are all checked properties on ubuntu, macOS and
Windows, against slice 5's existing fake backend.

The three-way degradation is reached honestly rather than by injecting a failure: `Memo::memory` is
constructed directly for the memory arm, and the `None` arm is a store that was never opened. What is
not reproducible in CI is the disk store *failing* to open for the reason it fails in production —
another copy of the application holding the directory — so the fallback's trigger is covered by
pointing the open at a path that cannot be created.

**No execution provider is involved at any point**, which is the first time in this series that has
been true: the cache sits above the session layer, never names one, and a stored result is served
identically whatever computed it. So there is nothing provider-specific to validate and nothing
provider-specific left untested.

One `#[ignore]`d measurement is worth having and is not a correctness test: a real Kyoto run over a
real photograph, twice, recording what the PNG encode and write cost against the run they save, at
both depths. That is the number the reference's settings documentation asserts without a figure, and
it is what would justify revisiting D3.

## Risks / Trade-offs

**The disk cache is bounded by time and not by size** → Accepted deliberately (D4, and the user's
call on this slice's scope). A session that enhances fifty large photographs can hold several
gigabytes until the next launch's sweep reclaims what has expired. Mitigated only by the 24-hour TTL,
the start-up sweep, and the fact that the reference has always behaved this way while believing
otherwise. The honest response if it bites is a size budget with LRU eviction in `rust-sak`'s `memo`,
as its own change — named in the Slices section as deferred rather than left to be rediscovered.

**The write is on the run's critical path** → A large upscale pays a PNG encode and a disk write
before `process` returns, and the caller sees it as part of the enhancement. `PngCompression::Fast` is
the mitigation the reference chose and this carries; the bypass (D8) is the escape for the one caller
— a benchmark — for which it is not a trade but a measurement error. The `#[ignore]`d measurement in
D12 is what turns this from an argument into a number.

**A 16-bit PNG encode of a 1.5-gigapixel result is slow and allocates** → The same class as the
standing 8x memory risk, now with a second large buffer beside the result. Nothing budgets it, and the
measurement above is the first chance to see it. If it is bad, the local fix is D3's — a cheaper
lossless encoding for the cache, which is two functions in one module and no change to anything that
calls them.

**A clock jump can expire an entry early or resurrect one briefly** → `memo`'s disk deadlines are
wall-clock, because they have to survive a restart. The consequences here are a recompute and, in the
other direction, an entry served slightly past its day. A resurrected entry is still the correct
result for its key — it is the same pixels the same run produced — so the worse direction is the
harmless one.

**The identity of every enhanced picture changes** → D2 alters what `process` returns for pictures it
computes. Nothing persists a `Picture` identity yet, no cache exists whose entries could be orphaned
by it, and no front end has been written against the old value; this is the last moment it is free.
The cost of *not* doing it is in D2.

**One more thing in `ProcessOptions` a caller can get wrong** → `cache: false` silently makes every
run slow, and a front end that sets it while debugging and forgets would look like a performance
regression with no other symptom. Mitigated by the default being `true` and by `cache_mode()` being
the thing a front end reports — which reports what is *backing* the cache and would still say `Disk`.
That asymmetry is deliberate: what backs the store and whether a run uses it are different questions
(D9), and conflating them into one reporter would be the reference's global setter by another name.

## Migration Plan

No migration. The cache directory is new and nothing reads it before this change creates it. No
existing format is read or written differently, and every public item added is new — `CacheMode`,
`Opai::cache_mode`, `ProcessOptions::cache` and `Stage::Cached` — so nothing that compiles today stops
compiling. `Stage` and the two new enums are `#[non_exhaustive]`, so a caller matching on `Stage`
already has a catch-all arm.

The one behavioural change to something already shipped is D2's identity, and it needs no migration
because nothing persists one.

Reverting removes the module, the field and the four public items, and leaves the cache directory on
disk holding entries nothing will ever read. They expire after a day and the directory can be removed
by hand; nothing else in the application looks at it.

## Open Questions

- **What the PNG encode actually costs against the run it saves**, at each depth. Answered by D12's
  ignored measurement rather than by argument, and it changes no specified behaviour: a different
  lossless encoding is local to two functions in one module.
- **Whether 256 MiB is the right memory-fallback budget.** Answerable only by finding out how often
  the disk store fails to open in the field, which needs the front end and the telemetry neither this
  slice nor this series builds. The figure is a constant with its reasoning written beside it.
