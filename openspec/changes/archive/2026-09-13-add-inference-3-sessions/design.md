## Slices

Running an image through a chain of models estimates far past 15 tasks. It is divided into six
changes, each leaving the project buildable and testable on its own. **This change is slice 3.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-inference-1-models` | A named model's files on disk, verified against the published manifest | the initialization series | **done** |
| 2 | `add-inference-2-providers` | The `ExecutionProvider` type including `Auto`, per-provider default options, and the per-model profile that overrides them | 1 | **done** |
| 3 | `add-inference-3-sessions` | Session construction, the resident session cache with its idle life, and CPU fallback | 2 | **current** |
| 4 | `add-inference-4-tiling` | The shared inference primitives: the tile grid, CHW tensor conversion, seam blending | none (parallel with 1-3) | pending |
| 5 | `add-inference-5-kyoto` | The upscale pipeline and the public chain entry point, with progress, cancellation and `OutputDepth` — Kyoto runs end to end | 3, 4 | pending |
| 6 | `add-inference-6-cache` | The 24-hour per-step image cache, keyed on the input identity plus the operations applied | 5 | pending |

Slice 4 depends on nothing in 1-3 and can be implemented alongside them. Everything else is a
straight chain.

Deferred past the series, each to its own change when something needs it: the model memory budget
with LRU eviction; alpha preservation through inference; the eleven measured execution-provider
profiles other than Kyoto's, each landing with the family pipeline that can verify it; the remaining
seven families' pipelines; and — added by this slice — logging, and evicting the sessions built on a
provider the user has just switched away from.

## Context

Slice 2 ends at a `SessionPlan`: `requested`, `resolved`, a `Vec<ProviderOptions>` in attach order
each carrying a `BTreeMap<String, String>` in ONNX Runtime's own key spelling, and a typed
`SessionSettings`. It is pure data, produced by `providers::options::resolve(requested, supported,
&profile, &paths)`, and nothing calls it. `CachePaths { engine, timing }` is an argument it takes and
decides nothing about.

Slice 1 ends at `deps::model::install(app_dir, name, &id, on_progress) -> PathBuf`, carrying an
`#[expect(dead_code, reason = "the first caller is the inference slice")]`. It installs into
`models/<artifact-id>/`, verified, and returns that directory.

`Opai::initialize` loads the runtime and starts the `ort` environment before it reports success, so
by the time anything in this slice runs there is a live runtime. `Inner` holds the application name,
the config directory and the `SupportedProviders`, with a comment reading *"Later slices add the
model registry here."*

Three facts about `ort` 2.0.0-rc.13 shape the design and were checked against its source rather than
assumed:

- `Session::run` takes `&mut self`, although ONNX Runtime's own `Run` is thread-safe. A shared
  session therefore needs interior mutability whatever else is true.
- `Session` is `Send + Sync` (`unsafe impl` at `session/mod.rs:723`), so an `Arc<Mutex<Session>>` is
  expressible.
- All three providers this project ships implement `ArbitrarilyConfigurableExecutionProvider`, and
  each one's `register` passes that options map straight to the runtime — CoreML through
  `SessionOptionsAppendExecutionProvider`, the other two through `Update*ProviderOptions` into the
  `_V2` append. A key/value pair is therefore applicable whether or not `ort` has a typed setter.

See proposal.md - Why for motivation, and specs/model-sessions/spec.md for the behaviour.

## Goals / Non-Goals

**Goals:**

- One place turns a model name into something that can run it, so caching, fallback and installation
  cannot differ between two callers.
- A session freed while a run is using it is **unrepresentable**, not merely avoided — which is what
  the project's conventions ask Rust to buy over the reference implementation's locking discipline.
- The cache policy — residency, the idle life, single-flight, the fallback decision — is testable on
  a CI runner with no GPU, no Mac, no ONNX Runtime and no model file.
- Every option slice 2 wrote down reaches a real runtime at least once, somewhere that can say so.

**Non-Goals:**

- Running inference. Nothing here feeds a tensor to a session; slices 4 and 5 do.
- Any memory accounting. No byte is charged, no budget is consulted, nothing is evicted under
  pressure. The idle life is the only thing that reclaims a session in this slice.
- A public API. The entry point is `pub(crate)`, as slices 1 and 2 were.
- Logging. See D12.

## Decisions

### D1. `sessions/`, a module of its own between `models/` and `providers/`

A directory: `mod.rs` for the cache and the entry point, `build.rs` for turning a plan and a file
into a session, `caches.rs` for the two compiled-artifact directories and their invalidation. Tests
beside each, as the project's convention requires.

Not inside `providers/`, which slice 2 deliberately built to need no runtime, and whose whole suite
is the proof of that — a module that opens a 6.8 GB graph does not belong in it. Not inside `models/`
either, which names models and owns no resource. This is the seam where both meet a live runtime, and
it is the directory slices 4 and 5 grow into.

### D2. The compiled artifacts live in `engines/`, not under `models/`

Under the configuration directory, beside `models/` and `runtime/`:

```
<config>/engines/<artifact-id>/     what a provider compiled from that model
<config>/engines/.timing/           the one TensorRT timing cache, shared
<config>/engines/.version           the runtime version these were compiled under
```

Putting them inside `models/<artifact-id>/` was considered first, since slice 1 gave each model a
directory of its own and that would make invalidation free. It does not survive contact with the
install pipeline, for three reasons found by reading it rather than by reasoning about it:

- `deps::install` finishes with `manifest::record_tree(dir)`, recording **every file under the
  directory** as something it installed. A TensorRT engine compiled later is not something the
  installer downloaded, and a published-file record that names it is wrong.
- The invalidation would not actually be free. On a replacement the pipeline calls `old.remove(dir)`,
  which removes exactly the files the previous manifest recorded — so a compiled engine that appeared
  after that manifest was written would **survive** the replacement, which is the one case that must
  not happen.
- A provider writing into that directory while an install reads it back is a race between a compile
  and the bookkeeping, with no lock between them.

`engines/` also gives the runtime-version wipe somewhere to happen at all: one tree to empty, rather
than a compiled artifact inside each of two dozen model directories.

`.timing` sits **inside** `engines/` rather than beside it, which is what gives it the right lifetime
at both ends: replacing one model's weights clears only that model's subdirectory and the shared
cache survives, while a runtime change empties `engines/` wholesale and takes the timing cache with
it. That second half is the one that matters — a timing cache records tactics for one TensorRT
version, and TensorRT comes from the runtime.

One function resolves `engines/<artifact-id>`, and both the installer that clears it and the session
builder that points a provider at it go through that function, so the two cannot disagree about which
directory it is.

### D3. Two invalidations, each triggered by the thing that knows

**Per model, at install.** The descriptor a model resolves to gains a list of *derived* directories —
directories holding things computed from this dependency rather than downloaded with it — and
`deps::install` empties them on the path where it is already removing the previous version's files,
before anything new is written. Not after: an interruption between the two would leave new weights
beside an old engine, which is the exact pairing that must never exist. The already-current path
returns before reaching it, so an install with nothing to do discards nothing — which is the point,
since those artifacts cost minutes to rebuild.

Emptying goes through the existing `deps::manifest::empty_dir`, the same helper the installer already
uses to clear an unrecorded directory. One place owns what "empty a directory but keep the directory"
means here.

**Wholesale, at initialization.** `engines/.version` records the runtime release tag those artifacts
were compiled under. `Opai::initialize` compares it to the tag pinned in `deps::artifact::ONNX_RUNTIME`
and, when they differ, empties `engines/` and writes the new tag.

A stamp rather than "the runtime was just replaced", because the latter is not durable: a process
that died between replacing the runtime and discarding the engines would find a matching runtime on
its next start, skip the reinstall, and never discard them. The stamp survives that. It is also the
only check available — nobody downloads these files, their names and number cannot be predicted, and
there is no expected hash to compare.

### D4. Ownership: an `Arc` is the lease, and `Drop` is the release

```
struct Resident<S> {
    session:   Mutex<S>,          // Mutex because ort::Session::run takes &mut self
    provider:  ExecutionProvider, // what it was actually built on
    last_used: Mutex<Instant>,
}

struct SessionHandle<S> {
    entry:     Arc<Resident<S>>,  // Drop stamps last_used
    requested: ExecutionProvider, // what *this* request asked for; differs after a downgrade
}
```

**`requested` belongs to the handle, not to the entry**, which this sketch originally had the wrong
way round. An entry is shared by every request for that artifact on that provider, so a `requested`
recorded there is whatever the request that happened to *build* it asked, reported to every request
after it — and the case that breaks is not hypothetical. The fallback deliberately re-enters the
cache under the CPU key (D6), so a downgrade is served the very session an explicit CPU request
files. On the entry, that handed the downgraded request `Cpu` beside `Cpu`: the exact shape of an
honoured request, and the one signal a broken driver produces, lost precisely when a CPU session
happened to be resident. On the handle it is per request, which is what the spec asks for — *the
resulting session shall carry what was asked for beside what it was actually built on, so that a
caller can tell a downgraded run from an honoured one*. With D12 carrying every fact as data rather
than logging it, this is the only channel that fact has.

This is the project's conventions answered directly. The reference implementation's sharpest edge is
that an ONNX session can be freed while another goroutine is running inference on it — a native
use-after-free that terminates the process — and it holds that off with a refcount, an `evicted`
flag, a rule that `Destroy` is never called under the lock, and a `WaitDrained` for teardown. None of
that exists here: the cache holds one `Arc` and each user holds another, so the session is dropped
when the last `Arc` goes and **not before**. Eviction is `map.remove(key)`, which drops the cache's
`Arc` and nothing else; a user holding one keeps a fully usable session, and a request arriving after
the removal builds a fresh one rather than resurrecting a dying one. "Released exactly once, only
when nothing is using it" is what `Arc` means, so there is nothing to test about it that the compiler
has not already decided — which is why the specs' scenarios for it are about the *observable* halves:
the holder keeps working, and the new request gets its own session.

"In use" for the sweep is `Arc::strong_count(entry) > 1`. Nothing else has to track it, and it cannot
disagree with reality the way a hand-maintained counter can.

`last_used` is stamped when a handle is handed out **and** in the handle's `Drop`. Handing it out is
the near edge of a use and dropping it is the far edge; stamping only the first would evict a session
15 minutes after a two-hour run *started*, moments after that run let go of it.

The reference's leaked-lease diagnostics — a warning for a model held 15 minutes with no sign of
life, and a `Touch` on the hot path to keep a legitimate long run out of it — are deliberately not
carried. They exist to catch a `Release` somebody forgot to write. Here there is nothing to forget.

The `Mutex` around the session is not a tax being paid to the design: `run` takes `&mut self`, so
some exclusion is required regardless, and serializing two runs of one model against one GPU is not a
throughput loss. Two *different* models still run concurrently, which is the case that matters
(detection during autopilot while an enhancement runs).

### D5. The cache is generic over what it holds, so its policy is testable with no runtime

`SessionCache<S, E>` is generic in the session type **and in the error type**, and building is a
closure the caller passes: `get_or_build(artifact, requested, resolved, || async -> Result<S, E>)`.
Production instantiates it at `ort::Session`; the tests instantiate it at a counting fake and drive
residency, the fallback, the single-flight and the sweep on a runner with no runtime, no GPU and no
model file.

**As implemented, the error is a second type parameter** rather than only the closure's, which this
decision originally assumed. The single flight is a `tokio::sync::OnceCell<Result<Arc<Resident<S>>,
E>>` per key, held in a map on the cache — the first request to reach one runs the build and every
other awaits it — so the failure a waiter is handed is stored *in the cache*, and the cache has to
name its type to hold it. That is also what makes "a build that fails fails every request waiting on
it" true rather than each waiter starting its own attempt (spec: *Concurrent requests for one model
are served by a single build*). `E: Clone` is the only bound it costs, which D11 answers.

This is the reference implementation's own shape — `AcquireModel(ctx, id, requested, create func(ep)
(any, error))` — kept for the same reason it has it, and typed rather than `any`.

The alternative, a trait object over a `BuildSession` trait, was rejected: a fake cannot produce an
`ort::Session`, so the trait would have to be generic in its output anyway and would buy only a name.

The sweep takes its cutoff as a parameter — `sweep(evict_unused_since: Instant)` — rather than
reading the clock itself. A test evicts everything by passing `Instant::now()` and nothing by passing
an hour ago, with no fake clock and no sleeping.

**The idle life and the sweep period are two constants, not one**, which the first implementation of
the background task got wrong by using `IDLE_LIFE` for both. A task that looks every 15 minutes does
not see a session that fell idle a moment after a tick until the next one, so residency was anywhere
between 15 and 30 minutes rather than the one number this decision exists to make answerable. The
period is `IDLE_LIFE / 4`, which bounds the overshoot at under four minutes; the cutoff handed to
`sweep` stays `IDLE_LIFE`, so what decides whether a session has gone unused is still how long ago it
was last used and never how long ago the task last looked. A test asserts the two are not the same
number, since the failure is invisible in every test that calls `sweep` directly.

### D6. The fallback re-enters the cache under the CPU key, and nothing is latched

The order is: resolve the chain, key on the **resolved** provider, look up, and build only on a miss.
Keying on the requested provider instead would file a CPU session under `@CUDA` after a downgrade,
and an explicit switch to the CPU would then build a second identical copy of it.

When that build fails and providers were attached, the fallback does not build a CPU session
directly — it **re-enters `get_or_build` with `Cpu`**. That is what makes the no-latch decision
affordable. Without it, a machine with a broken driver would rebuild the model from scratch on every
request; with it, the second request pays one failed provider attach and then hits the CPU session
the first request already filed.

The user chose no latch over the reference's behaviour, and the trade is recorded here rather than
hidden: the cost is one failed provider initialization and one failed session build per request that
asks for a provider this machine cannot serve, against a latch this project has no way to clear,
since the settings surface that clears it in the reference does not exist yet. A latch would be one
`AtomicU8` and a reset the day there is something to reset it from.

Only a failure that is about building the session is retried. A missing or unreadable model file
fails the same way on the CPU, and retrying it doubles the wait before reporting what was already
known.

### D7. Every option is applied through `with_arbitrary_config`, not through typed setters

Slice 2 left the plan carrying ONNX Runtime's own key spelling, and said the typed setter would be
used where one exists. Reading `ort`'s source changes that: `impl_ep!(arbitrary; …)` writes
`with_arbitrary_config` into the **same** `ExecutionProviderOptions` map that every typed setter
writes into, and each provider's `register` hands that map to the runtime verbatim. Using a typed
setter would mean parsing `"EXHAUSTIVE"` or `"CpuAndNeuralEngine"` back out of the string into an
`ort` enum and re-emitting it — a round trip that can only lose, in a match with two dozen arms that
each get one chance to transcribe a value correctly.

So: one loop over the `BTreeMap`, for all three providers. The provider-specific code is then three
constructors and nothing else, and the option table stays comparable line for line with the
provider's documentation and the reference implementation's measured table, which is what slice 2
chose that representation for.

Every dispatch is built with `error_on_failure()`. `ort` defaults to registering silently — logging
and carrying on — which is precisely the failure mode slice 2 flagged: ONNX Runtime rejects an
options update wholesale, so a single mistyped key costs the entire provider and surfaces only as a
run that is ten times slower. With `error_on_failure` it becomes a build failure, and D6 turns that
into a *reported* downgrade rather than a silent one.

### D8. TensorRT builds are serialized, and a failed one drops the shared timing cache

A process-wide mutex held across any session build whose attach list contains TensorRT. The shared
timing cache is one file that ONNX Runtime reads at the start of a build and rewrites at the end with
no locking of its own, and this cache single-flights per artifact rather than globally, so two models
can legitimately build at once. The cost is nothing in the common case — the application builds one
model at a time — and two concurrent engine builds do not finish sooner for sharing one GPU.

When such a build fails, `engines/.timing` is emptied. A half-written timing cache makes TensorRT's
`createTimingCache` return null, which ONNX Runtime turns into a failed session build, so the
provider declines and every model quietly runs somewhere slower until the file is replaced. Dropping
it costs one slow rebuild if the failure was about something else, which is the cheaper way to be
wrong. The removal failure is swallowed: this runs on a path that is already returning an error, and
a cache file that could not be removed is not worth replacing that error with.

### D9. The key is the artifact, and the profile has to be a function of it

`(ArtifactId, ExecutionProvider)`. The reference keys on the operation id, which loads one graph
twice when two operations share it — an 8x Kyoto and a 4x Kyoto both need `up_kyoto_4x_fp16`, and
today the second builds its own copy of it.

That is only sound while **one artifact implies one profile**, since the profile decides the options
a session is built with and the key does not carry it. It holds today by construction: a profile is
declared by the variant, an artifact name is composed from the family, the codename, the native scale
and the precision, and the profile depends on nothing else. It is worth a test rather than a comment
— a traversal of the catalogue asserting that no two operations resolving to one artifact declare
different profiles — because slice 2 already named the thing that will break it: Osaka's per-graph
override, where one graph of a set wants `prefer_nhwc` off while the other two want it on. That
override is per **graph**, and a graph is an artifact, so it is expressible within this key; the test
is what will say so when it lands.

### D10. Install first, then build, in one entry point

`session(&self, artifact, profile, requested, on_progress) -> Result<SessionHandle, SessionError>`
installs through slice 1's path and then builds. This is the reference implementation's
`LoadSessions` shape and it is what makes "a model that is named is a model that can be run" true at
one call site rather than at every caller.

**`profile` is a parameter**, which this decision's first sketch of the signature left out. Nothing
can resolve a `SessionPlan` without one — `providers::options::resolve` takes it — and there is no
artifact-to-profile lookup in the crate: a profile is declared by the *variant*, so the caller
holding the operation is what knows it. D9 is what keeps the artifact-only key sound while the
profile travels beside it, and D9's test is what will say so when a per-graph override lands.

**The build itself runs on a blocking thread.** Opening a graph is anything from a fifth of a second
to several minutes of engine compilation, and in a Tauri process the asynchronous runtime is the
window's event loop — so the build goes through `task::spawn_blocking`, as the archive expansion and
the runtime load already do. Not called out when this was written, and not optional: without it a
TensorRT compile freezes the window for the length of the compile. It is why the builder is an owned
`BuildRequest` and a `dyn Fn` the cache can move, rather than a closure borrowing the plan.

The install is inside the single-flighted build, not before it, so ten concurrent requests for a
model that is not on disk perform one install rather than ten — and a cache hit costs no filesystem
work at all, which matters because the already-current check is one `symlink_metadata` per recorded
file on a path slice 5 will hit once per tile.

Progress carries the existing `OnProgress`, which already covers a model transfer: slice 1 added the
model variant to `progress::Dependency` and spreads the whole 0-1 range over the transfer when there
is no expansion phase. Nothing new is needed, and a build that installs nothing reports nothing.

### D11. Errors

A new `pub(crate) enum SessionError` in `error.rs` beside `InitError`, with `thiserror` as the
project's convention requires:

- `Install(#[from] InitError)` — the model could not be put on disk. This is the distinction the
  specs ask for, and it is free: the install path already returns `InitError`.
- `Build { artifact, provider, source }` — the model is on disk and could not be opened, naming the
  provider that was being attached. Also what the fallback matches on to decide whether a CPU retry
  is worth attempting.

Two things about their shape were settled by implementing them, and neither is cosmetic:

- **Both sources sit behind an `Arc`, so the enum is `Clone`** — `Install(Arc<InitError>)` with a
  hand-written `From<InitError>` rather than `#[from]`. D5's single flight hands one build's failure
  to every request that was waiting on it, and neither `std::io::Error` nor `ort::Error` is `Clone`.
- **`Build`'s source is `Arc<dyn Error + Send + Sync>`, not `Arc<ort::Error>`.** Every `ort::Error`
  constructor goes through the runtime's `CreateStatus`, so one cannot be built at all without a
  loaded runtime — the same fact `runtime.rs` already records. Typing the field as one would put the
  whole of D6's fallback out of reach of the fake-backed suite, which D5 requires to run on a runner
  with no runtime. Nothing downcasts it; the message and the `source` chain both survive.

**A missing or unreadable model file is reported as `Install`, not `Build`.** The builder opens the
file before it touches a `SessionBuilder`, so `Build` means "on disk and would not open" and nothing
else — which is exactly the distinction the specs ask a caller to draw, and it is what lets D6's
retry rule be a match on the variant rather than an inspection of an error code.

Not variants on `InitError`. That type is `pub` and documented as what initialization reports; a
session failure is not an initialization failure, and slice 5 will need a public inference error of
its own that this one folds into rather than a single enum accreting every layer's failures.

### D12. Nothing is logged, and each fact is data instead

The reference logs the provider a session got, the build duration, every eviction, every downgrade,
and a warning per suspected leak. This project has no logging at all — `eprintln!` in two places
that pre-date the runtime — and introducing structured logging is a cross-cutting change that would
consume most of a slice on its own (`rust-sak`'s `o11y` is where it will come from).

So each fact is carried where a caller can read it: the handle reports `requested` and `provider`, so
a downgrade is inspectable rather than announced; residency is observable through the cache's own
accessors, which is what the tests read. Slice 5 is the first caller with a user to tell, and a
logging change can later put a line at each of these points without any of them moving.

Recorded as a parity gap in the proposal rather than pretended away: today, a user whose CUDA is
broken gets a slower run and one boolean nothing displays.

### D13. Platform and provider validation

Everything in D4 through D6 and D9 is exercised by the fake-backed suite on every CI platform. What
needs a real runtime is one integration test that builds a session for a small real model on the CPU
and asserts it reports the CPU — which also validates the CPU path through D7 end to end.

The GPU providers cannot be validated here. CI has no NVIDIA card and no Mac GPU worth the name, so
whether ONNX Runtime accepts slice 2's TensorRT, CUDA and CoreML option maps is **unverified until
somebody runs it on hardware**, which slice 5 is the first change that can. The test is written to be
run by hand on such a machine and left `#[ignore]`d with that stated as the reason, rather than
stubbed into passing — the project's conventions forbid fabricating a model to make a test go green.

**Two things about these tests changed when they were written, and both are worth recording.**

**They are `#[cfg(test)]` modules in the crate, not files in `tests/`.** The entry point is
`pub(crate)` in this slice (Goals - Non-Goals), so nothing in an integration-test binary can reach
it. They are integration tests in substance — real transfers, a real runtime, a real graph, end to
end — and unit tests only in where they live. The slice that gives inference a public surface is what
can move them.

**The CPU test is `#[ignore]`d too, so no CI platform builds a real session.** This paragraph
originally had it running everywhere. It does not, and the reason is the one that already keeps
`runtime.rs`'s live test out of CI: it downloads the pinned ONNX Runtime, ~175 MB on the platforms
that ship it with its execution providers, and the rest of this repository's suite is hermetic — it
serves its own archives from a local listener and contacts nothing. Making one test the exception
would put a network dependency on every `cargo test`, local and CI alike, for a path whose *decision*
CI already covers in full against a fake.

What that costs is stated rather than hidden: **CI proves the plan, the dispatches, the cache, the
single flight, the fallback and the install are right on all three platforms, and proves on none of
them that a graph opens.** The four live tests are the only thing that does, and they are run by
hand.

They have been, on macOS arm64, at the time this slice was implemented: `la_paris_fp16` — 122 KB, the
smallest graph the project publishes — installed from the published listing and opened on the **CPU**
and on **CoreML**. The second of those is the first time any of slice 2's option maps has reached a
live runtime, and with `error_on_failure()` set, so CoreML's map is now verified rather than assumed.
TensorRT's and CUDA's remain unverified, exactly as this decision says.

## Risks / Trade-offs

**A mistyped option key in slice 2's tables costs the whole provider, and this slice is where it
would first show** → `error_on_failure()` (D7) turns it from a silent decline into a build failure,
and D6 turns that into a downgrade recorded on the handle. It is caught rather than suppressed. What
remains is that nothing *displays* it (D12), so on a machine nobody inspects it still looks like a
slow run. The gated integration test (D13) is where the real answer comes from, and it can only be
run by someone with the hardware.

**No latch means a broken driver is paid for on every request** → One failed provider initialization
plus one failed session build per request, rather than per model per run as the reference has it
before its latch engages. Bounded by D6's re-entry, which stops it also being a rebuild. The user
chose this trade-off knowingly; the day there is a settings surface that could clear a latch, adding
one is an `AtomicU8` and a reset.

**A flat 15-minute life is wrong for the extremes in both directions** → Osaka's session is 6.8 GB
and holds it for fifteen minutes after one use with no budget in this series to reclaim it sooner;
Kyoto's rebuilds in a fifth of a second and hardly needed the fifteen. The reference's cost-scaled
interval addresses both and exists to cooperate with the memory budget this series defers — carrying
the scaling without the budget would be carrying the complication without its reason. Revisited with
the budget, which is the change that makes it matter.

**`Arc::strong_count` as the "in use" test is a snapshot** → It is read under the cache's own lock,
and the only thing that can increment it is handing out a handle, which happens under that same lock.
A count of 1 therefore means the cache holds the only reference for as long as the lock is held,
which is long enough to remove the entry. The failure it *cannot* have is the dangerous one: a
miscount can at worst keep a session resident a sweep longer, never free one that is in use, because
freeing is `Drop` and not a decision.

**Serializing TensorRT builds is a process-wide mutex on a path that can take minutes** → Only builds
that attach TensorRT take it, only while building, and a chain builds its graphs in sequence anyway.
The alternative — a per-model timing cache — was measured by the reference and costs up to 60% of a
cold engine build.

**Emptying `engines/` on a runtime change throws away work a user paid minutes for** → Unavoidable
and correct: the artifacts are valid only for the runtime that built them. It happens once per
runtime bump, which is once or twice a year, and the first run after it is slow in exactly the way a
first run is.

**`deps::install` gains a branch, and it is the most load-bearing code in the project** → The derived
directories are a list that is empty for every existing descriptor, emptied on a path that is already
removing files, and the whole existing install suite is what proves nothing else moved. The
alternative — invalidating from the session builder — cannot work: it does not know a replacement
happened.

## Migration Plan

No migration, and nothing on disk is read that did not exist before. `engines/` is created on first
use and its `.version` stamp is absent on a first run, which reads as "these were compiled under a
different runtime" and empties a directory that is already empty.

There is one thing worth knowing for anyone upgrading a development machine across this change: a
`models/` directory from a build predating it holds no compiled artifacts, because nothing in this
project has ever compiled one. So the new tree starts empty for everyone.

Reverting leaves an `engines/` directory nothing reads, and slice 1's install back to being dead
code.

## Open Questions

- **Whether the sweep should also run on a cache miss**, as cheap insurance against a background task
  that has died or was never started. It is one line, it changes no specified behaviour, and it is
  only worth adding if the task turns out to be fragile in the GUI's runtime — which slice 5 is the
  first change that can observe. Answerable then without touching the specs, the approach or the
  tasks.
- **Whether `engines/` should be pruned of models no longer installed.** A model uninstalled by hand
  leaves its compiled artifacts behind, costing disk and nothing else. There is no uninstall path in
  this project yet, so there is nothing to hang the prune off; it belongs with whichever change adds
  one.
