## Why

Slice 5 made an enhancement runnable end to end, and every run is a full run. A user who drags a
strength slider, toggles an enhancement off and back on, or re-opens a photograph they enhanced ten
minutes ago pays the whole pipeline again — minutes, on the one operation this application asks
somebody to sit and wait for. The reference makes that cheap by memoizing each *step* of a chain on
disk, and `Opai::process` was given the shape for it in advance: the identity it would key on is
already composed, and its options are already a bundle a field can be added to.

This is the last slice of the inference series. It is what turns the second identical run into a
read.

## What Changes

Task estimate: **15**. The inference feature was split into six slices (see design.md - Slices); this
proposal covers slice 6, the last of them.

**In `opai`, a store of processed images and the two places a run touches it:**

- **The per-operation image cache**, on `rust-sak`'s `memo` module — the one the project context
  already names for exactly this: *"memoization with pluggable storage (memory, disk, or
  memory-over-disk) ... the per-operation image cache, including its disk/memory/none fallback
  modes"*. Opened during initialization under `<config>/cache`, degrading rather than failing.
- **Keyed on the identity of what is being cached**, which slice 5 already composes: the input's
  identity folded with each operation's `cache_tag`, so the same operation lands in a different slot
  depending on what preceded it. The cache does not invent a second derivation — it reads the slot
  `Picture::identity` already names, which is what makes a result fed back into `process` look up
  exactly where its own pixels were stored.
- **The resolved output depth folded into that derivation**, which is a correctness fix rather than a
  cache feature — see the divergences below.
- **Read before each operation, written after it.** A hit skips the model entirely and still hands
  back its share of the progress bar, so a chain of cached operations does not stall it.
- **A new `Stage::Cached`**, so a front end can say *"Upscale — cached"* rather than showing a bar
  that jumps. Additive: `Stage` is `#[non_exhaustive]`.
- **`ProcessOptions::cache`**, defaulting to on — the field slice 5's D2 predicted when it made the
  options a bundle: *"a cache mode is one more defaulted field rather than a changed signature at
  every call site."* A caller that must not persist its images, or that is measuring what a model
  costs, turns it off per run.
- **`Opai::cache_mode()`**, reporting `Disk`, `Memory` or `None` — what is actually backing the cache
  after initialization. Reported, never configured. It exists because losing the cache is
  deliberately *not* an error: without something like this, "some runs are silently uncached" is
  unknowable from outside the machine.
- **A failed cache write never fails a run.** The pixels the user asked for are already computed; a
  full or read-only disk is the cache's problem, and the cost of carrying on is a re-run later rather
  than a wrong image. The same holds on the read side: every failure is a miss, including an entry
  that no longer decodes.

## Capabilities

### New Capabilities

- `image-cache`: keeping the result of each step of a chain so that repeating one is a read rather
  than a run — what is stored and under what identity, how long an entry is worth keeping, what
  happens when the store cannot be opened or a write is refused, and when the cache is bypassed
  altogether.

  Distinct from the capabilities around it. `model-sessions` caches *sessions* — what a provider
  compiled from a model, keyed on the model and released on an idle timer; this caches *pixels*,
  keyed on an image and the operations applied to it. `image-enhancement` is the run itself and says
  what a run produces; this says what a repeat of one costs. `image-io` owns the identity a loaded
  image carries, and this capability consumes it rather than widening it.

### Modified Capabilities

- `image-enhancement`: three requirements change.
  - **The identity a result carries now includes the resolved depth.** The existing requirement
    composes it from the input's identity and the operations applied, and that is not enough to
    describe the pixels: the same chain at 8 and at 16 bits produces two different images carrying
    one identity. Uncached that is a mislabel; cached it is the wrong picture served. See the
    divergences.
  - **A run consults the cache**, so an operation whose result is already stored is not run again —
    and the run that skips it still reports its share of the progress, still produces the same
    result, and is still refused where the chain names something unservable.
  - **What a run reports gains a third answer**: a step that was served from the cache rather than
    installed or run.

## Impact

**Crates touched:** `opai` only. `gui` and `cli` are untouched, and deliberately: a cache keyed on an
image's identity, a store that degrades rather than failing, and a per-run bypass are all logic both
front ends would otherwise each build. What the front ends will add is a settings toggle and the
degradation warning the reference's GUI already shows — part of the front-end wiring the series
defers wholesale, because none of it can be written before there is something public to call.

**`rust-sak` - what was found before anything was proposed.**

- **`memo` is the module, and adopting it is the substance of this slice.** It offers the three
  stores this needs (`Memo::memory`, `Memo::disk`, `Memo::memory_disk`), a TTL per entry, a
  `cleanup` that reclaims what expired entries hold, `key_from` for composing keys, raw
  `get_bytes`/`set_bytes` that store exactly what is handed to them, and a `NotAdmitted` error
  separating a write the store *declined* from one that broke. It also solves, in the library, the
  problem the reference had to work around by hand: opening a directory this process already has
  open hands back the same store rather than failing on a lock. Slice 5 said this module was *"a
  whole module's worth of adoption, not a corner of this slice"* and deferred it here; that was
  right.
- **One gap, and it is not filled here — see the divergence below.** `memo`'s disk tier is not
  bounded by size, and is not claimed to be.
- `crypto` covers the key composition, unchanged and already enabled. `image` covers encoding a
  result into cache bytes and decoding it back: `encode_writer` and `decode_bytes` are the in-memory
  halves of the file functions `image::save` and `image::load` already go through, so nothing new is
  reached for.
- `fs` supplies the configuration directory the store opens under, through this crate's existing
  `sub_dir`, exactly as `models/`, `runtime/` and `engines/` are resolved.

**New third-party crates:** none. `memo` is a feature flag on a dependency already in the manifest.
It pulls `redb` and `postcard`, both pure Rust with no C toolchain and no build-time downloads.

**Parity divergences from the Go implementation, deliberate:**

- **The output depth is part of what identifies a result, and it fixes a defect.** The reference has
  no depth concept, so its key needs none. Here two runs differing only in requested depth produce
  two different images, and slice 5's composition does not distinguish them. Folding the *resolved*
  depth into the derivation is the fix; leaving it out and adding depth only to the cache key would
  still serve a wrong picture across a chained pair of runs. Design D2 records why.
- **The disk store is bounded by its TTL and by nothing else, and this says so.** The reference
  believes otherwise — its cache passes a 1000 MiB `MaxCapacity` under a comment reading *"bounds the
  store on disk"* — and that is not what the figure does in either toolkit: in `go-sak` it sizes
  Badger's value-log files, in `rust-sak` it sizes redb's read cache, and `max_entries` is documented
  as ignored by the disk tier outright. So the reference's image cache has always been unbounded, and
  this slice inherits that rather than pretending otherwise: entries expire after 24 hours and a
  sweep at start-up reclaims the space. A session that enhances fifty large photographs can hold
  several gigabytes until the next day's sweep. A size budget with eviction is general-purpose and
  belongs in `rust-sak` rather than here; it is named as a risk and deferred to its own change, the
  way the model memory budget was.
- **The execution provider is not part of the key**, carried from the reference unchanged and with
  its consequence carried too: a result computed on the CPU is served to a later request that asked
  for CoreML. That is right for a user — the enhancement is the same enhancement — and wrong for a
  benchmark, which is why the reference's own CLI turns the cache off before measuring providers.
  `ProcessOptions::cache` is what a benchmark here would use.
- **Nothing is logged**, unchanged from slices 3, 4 and 5. The reference logs each hit, each miss, a
  dropped write and a corrupt entry. Each fact is carried as data instead: a hit through the
  progress report's new stage, the backing store through `cache_mode`, and a write that was declined
  is by construction invisible — it costs a future hit and nothing else.
- **Concurrent identical runs are not coalesced.** `memo` would give it for free through
  `get_or_compute`, and the reference does not do it either; design D5 records why taking it would be
  worse here — a cancelled run would fail every other run waiting on it, and a waiting run's progress
  bar would sit still and then jump.

**Not in scope:** a size budget with eviction for the disk store, and the `rust-sak` work it implies;
any `gui` or `cli` change, including the settings toggle and the degradation warning the reference's
GUI shows; caching the result of a *data* operation such as face detection, which is not an image and
has no pipeline here yet; and everything the series already defers — the model memory budget, alpha
preservation, the eleven unmeasured provider profiles, the seven remaining family pipelines, Osaka's
diffusion pipeline, logging, and the per-tile divergence guard.
