## Why

`Opai::execute` runs a detection over the same photograph from scratch every time it is asked, because
`add-detection-newyork` specified that nothing a non-image run produces is kept. That requirement's reasoning was
that a result which is not a picture is small and cheap to recompute — which is true of the *result* and false of
the *run*: a repeated detection pays session construction, and that is seconds rather than the ~12 ms the inference
itself costs. Every caller that will exist — per-image face selection, face recovery consuming `Faces`, Autopilot
evaluating a policy over the same image repeatedly — re-asks the same question of the same pixels, and today each
one pays in full.

## What Changes

- **A non-image run reads and writes the store.** `Opai::execute` serves a repeated run over the same image and the
  same operation from the store rather than running the model, under the same 24-hour entry lifetime an enhancement
  already uses.
- **The lookup happens before anything is installed or opened**, so a hit costs neither a transfer nor a session
  build. This is what makes the change worth making at all.
- **A hit reports that nothing was executed.** No session is built, so the provider report says so rather than
  naming the provider the run asked for — the behaviour `data-inference` already requires of a run that built no
  model, reached now by a second route.
- **Every operation on this path gains it, not just detection.** The result type an operation names carries a
  serialization bound, so the next family whose result is not an image is cached without a further change. Detection
  needs no work for this: `Faces` and everything under it already serialize.
- **A caller can turn it off per run.** `ExecuteOptions` gains a `cache` flag defaulting to on, mirroring
  `ProcessOptions::cache` — a benchmark needs every run to actually run.
- **Which way each run went is in the log.** A served analysis and a computed one are each a `debug` record naming
  the operation and the entry, as an enhancement's steps already are — a store quietly caching nothing looks exactly
  like one that is working, and that is the only thing that makes it findable.
- **BREAKING (crate-internal only).** The sealed `DataModel::Output` associated type gains a serialization bound.
  The trait is sealed and this crate is its only implementor, so no external caller can be broken by it; it is marked
  because it narrows a published bound.

**Not in scope:** no second store and no new backing mode — this is the store the application already opens. No
front-end work; neither `gui` nor `cli` calls `execute` yet. No change to how an enhancement is cached, and no change
to the key an enhancement is stored under.

**Estimated task count: 9**, landing at 14 once verification's findings were folded back in as section 6. Within
the 15-task limit either way, so this is one change rather than a series.

## Capabilities

### New Capabilities

None. The behaviour belongs to the path that already has a capability.

### Modified Capabilities

- `data-inference`: the requirement **"Nothing a non-image run produces is cached"** is reversed. It becomes a
  requirement that such a run is served from the store when the same operation has already been run over the same
  image, that a served run builds no model, and that losing or disabling the store costs a re-run rather than an
  error. The four scenarios under it are replaced. The capability's other four requirements are unchanged — in
  particular the provider requirement's *"Where no model was built at all, the report SHALL say so"* is what a hit
  now exercises, and is deliberately left as written.
- `run-cache`: the store is described as holding one kind of value, a picture. It gains a requirement that it
  holds results that are not pictures too, each under its own encoding, and that the two cannot be read as one
  another. No change to the store's lifetime, its backing modes, or what a failure costs.
- `logging`: **"A result that did not come from a model says so"** is broadened. It was written over the steps of an
  enhancement chain, which was every read of the store there was; now that an analysis is served from the same store
  it is subject to the same silence, and a requirement covering one path would make "the store is doing nothing"
  answerable for half the application. Two scenarios are added and the existing three are unchanged. This is the
  requirement's own reasoning applied to a second reader, not a new rule.

**The capability is renamed `image-cache` → `run-cache`.** Its name asserted the very thing this change ends: that
every entry is a picture. The rename is prose and file names only — no requirement text changes on account of it, and
the wording sweep carries through `openspec/config.yaml` and the four other capabilities that named it. In the code
the same rename lands on `cache::ImageCache`, which becomes `cache::Store`, and on the comments that called the
store an image cache. A capability whose name contradicts its newest requirement is how the next reader concludes
there are two stores.

## Impact

**Crates: `opai`, and `perf` for the rename only.** `gui` and `cli` are untouched — neither calls `execute` today,
and the per-image face selection UI that will is its own change. `perf` gains no behaviour: it reaches the store
through `ProcessOptions` as before, and what it sees of this change is the word "image cache" becoming "run cache" in
a help line, a README paragraph and its own comments.

**`rust-sak`: nothing new, and nothing missing.** Checked before proposing anything else. `memo` already provides
the store, its three backing modes, the per-entry lifetime and the sweep, and `crypto`'s `xxh3_string` already
derives the keys an enhancement is stored under — this change reuses both exactly as `cache.rs` reuses them today.
Nothing here is a candidate for extending `rust-sak`: what is being added is the encoding of *this project's* result
types and a key derivation over *this project's* operations, neither of which is general-purpose to anything.

**No new third-party crate.** `serde` and `serde_json` are already dependencies of `opai`.

**Code:**

- `cache.rs` — a value-encoding pair beside the existing picture pair. The read path, the entry lifetime and the
  declined/broken-write logging are already value-agnostic and are reused rather than repeated.
- `inference/execute.rs` — the driver gains the lookup, the store write, the cached-progress report and the pair of
  `debug` records saying which way each run went, which is `logging`'s requirement reaching a second reader of the
  store.
- `inference/chain.rs` — nothing behavioural; the data key derivation sits beside `identity_after` so the two
  cannot silently converge on the same string.
- `models/operation.rs` — the bound on the sealed trait's associated type.
- `lib.rs` — `execute` receives the store handle, as `process` already does; and the crate-level cache section gains
  the second kind of entry, since it is the first thing a consumer of `opai` reads and it currently says the store
  holds photographs.
- `tests/public_api.rs` — the new `ExecuteOptions` field pinned the way `ProcessOptions::cache` is.
- **The rename**, across `cache.rs` and every comment in `opai` and `perf` that called the store an image cache. No
  behaviour, and no log message a test or a spec pins.

**Deliberate divergence from the reference.** The Go implementation's `Execute[T]` does no caching at all: it
selects the model, runs it and returns, with no store read or write anywhere in the function — only `Process` touches
the cache. This change knowingly diverges. The parity target is the user-visible result, and the result is
unchanged; what changes is that the second identical detection is instant rather than paying for a session it
already paid for. The divergence is worth recording because a later reader comparing the two files will find the
caching here and nothing there, and should not remove it as an accident.
