## Slices

Seven changes, in order. Single-instance came first because the changes after it are designed against
its guarantee — the logging change puts three binaries in one file and needs to know whether a second
writer is possible, and the benchmark needs to know nothing else is competing for the GPU, the cache
or the model directory while it is measuring. **This change is logging slice 2, change 4 of the
seven.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-single-instance-1-lock` | At most one OPAI process per user: an advisory lock taken in `Opai::initialize`, the holder recorded, the second process refused | none | **done** (archived `2026-09-14`) |
| 2 | `add-single-instance-2-gui` | The GUI raises its existing window instead of refusing, the ending a double-clicked icon deserves | 1, and a GUI with a window | **done** (archived `2026-09-14`) |
| 3 | `add-logging-1-file` | The sink: `tracing` through `opai`, ORT's C++ diagnostics via `ort`'s `tracing` feature, `<config>/logs/opai.log`, daily rotation, the `---` session divider, the session header, the per-record `app=` field, `APP_NAME`, and the initialization path instrumented | 1 | **done** (archived `2026-09-15`) |
| 4 | `add-logging-2-records` | The instrumentation sweep: the enhancement run, the session registry, the installers, the image cache and the model listing, against the records the Go library emits | 3 | **current** |
| 5 | `add-perf-1-opai` | What a benchmark needs from the library: `release_sessions` made public, the `(requested, actual)` execution provider surfaced out of `process`, skip-verify | 3 | pending |
| 6 | `add-perf-2-harness` | `crates/perf` → the `perftest` binary: `clap`, catalogue-driven model selection, the warm-up/cold-start/timed-runs loop, statistics, plain and JSON output | 5 | pending |
| 7 | `add-perf-3-tui` | The `ratatui` inline viewport: spinner, per-tile bar, ETA, finished models into scrollback via `insert_before`, TTY detection | 6 | pending |

Row 5 depends on 3 rather than on this change, and still does: `perftest` wants a sink to see the
library through. It benefits from this slice — a benchmark run whose provider was downgraded now says
so — but is not blocked on it, so the two could be taken in either order.

## Context

See proposal.md — Why for the motivation and specs/logging/spec.md for the behaviour. Slice 1's
design is the one to read beside this: it decided the sink, the line format, the two filter dials and
the synchronous writer, and none of that is reopened here.

Five facts about the code, read rather than assumed, shape this design:

- **The call sites were left standing for this change, in writing.** `cache.rs`, `lib.rs` (twice) and
  `sessions/mod.rs` each say the fact is carried as data *because* there is no logging, and
  `sessions/mod.rs` goes as far as *"A logging change can later put a line at each of those points
  without any of them moving."* `inference/chain.rs` lists logging under "What is deliberately not
  here". This slice is those four comments coming due, and every one of them is rewritten rather
  than left describing a state that has ended.
- **The reference logs 108 times, not 84.** Slice 1's figure counts `internal.Log().*` — the calls
  from packages outside `internal`. Twenty-four more are made inside the `internal` package through
  its own bare `Log()`, in `registry.go` (6), `model.go` (8), `budget.go` (6), `cache.go` (2) and
  `sysinfo.go` (2). The mapping in D1 is against all 108.
- **This project has ported roughly half the subsystems those records come from.** There is no
  autopilot, no memory budget, no Osaka pipeline, no wired face-recovery inference and no legacy
  sweep. Twenty-eight of the 108 records are about machinery that does not exist here, and inventing
  a record for a mechanism this project has not built would be worse than leaving it out.
- **`rust-sak` emits nothing.** It names no `tracing` or `log` dependency in its manifest and there
  is not one logging call in `src/`. So every record this project gets is one `opai` emits at its own
  seam, and anything that happens inside `fetch`, `memo` or `fs` is invisible — which is what decides
  D7.
- **Slice 1's formatter renders span fields**, walking `ctx.event_scope()` for each span's
  `FormattedFields`. A span is therefore a way to attach context to records this project did not
  emit, which is what D5 spends.

## Goals / Non-Goals

**Goals:**

- A file that answers the three questions a bug report actually arrives with: why was it slow, why
  did nothing get faster the second time, and where did the install stop.
- One level rule, applied to every record, so that "why is this at debug" has an answer that is not
  the author's taste on the day.
- A field vocabulary shared across records, so `opai.log` can be grepped for an artifact or an
  operation and return every record about it.
- The four doc comments that promised these records rewritten to describe what is there.
- A default-level session whose length is bounded by what the user did, not by how large their
  photograph was.

**Non-Goals:**

- **New behaviour.** Nothing changes about what runs, what is cached, what is installed or what is
  refused. The one thing that moves is a distinction the image cache currently discards, and it
  moves into a record rather than into an API.
- **Front-end records.** `gui` and `cli` have one call each and no processing surface; their records
  arrive with the screens that make them.
- **Instrumenting `rust-sak`.** D7.
- **A record for a mechanism this project has not built.** The memory budget, the lease registry,
  autopilot and the Osaka pipeline are named in D1's table as absent, with the change that will bring
  them, rather than approximated.
- **Structured or machine-read output.** Unchanged from slice 1: the file is read by people.
- **Telemetry.** Unchanged from slice 1: `rust-sak`'s `o11y` and an OTLP exporter remain a separate
  decision, and nothing here forecloses it.

## Decisions

### D1. The sweep is driven by the reference's call-site list, mapped area by area

Not by walking the Rust code and asking "would a record be nice here", which produces a log that
agrees with whoever wrote it last. Each of the reference's 108 sites is resolved to one of three
outcomes: **ported** (this project has the code, and the record goes at the Rust site that replaced
it), **absent** (this project has not built the mechanism, named with the change that will), or
**divergent** (this project does the thing differently, so the record differs and the reason is
written down).

| Go source | Records | Rust site | Outcome |
|---|---|---|---|
| `init.go` (launch path) | 12 | `lib.rs::initialize`, `cache.rs`, `runtime.rs`, `instance.rs` | **done in slice 1** |
| `init.go` (engine cache) | 1 | `lib.rs:642` → `sessions::caches::invalidate_for_runtime` | ported, `info` |
| `init.go` (teardown) | 4 | `lib.rs::release_sessions`, `SessionCache::clear` | ported in part — D6 |
| `init.go` (legacy sweeps) | 2 | — | absent: no Go installation to migrate from, since `APP_NAME` is `io.vinicius.opai` (slice 1 D8) |
| `inference.go` | 15 | `lib.rs::process`, `inference/chain.rs` (`process`, `run`, `plan`) | ported — D2, D3 |
| `internal/cache.go` | 2 | `cache.rs` (`get_bytes`, `put`) | ported — D4 |
| `internal/model.go` | 8 | `sessions/mod.rs` (`session`, `build_on`, `get_or_build`) | ported — D6 |
| `internal/utils/session.go` | 6 | `sessions/build.rs::build`, `sessions/mod.rs::open` | ported — D5, D6 |
| `internal/utils/ep_profile.go` | 5 | `sessions/build.rs::serialized`, `SessionCache::get_or_build` | ported in part: the timing-cache drop is a record of its own, and the provider decline is carried by `requested` beside `provider` on the build record rather than by a site in `providers/` — see below. The reference's "excluded for this model; substituting" and "no appender is registered" are absent — `execution-providers`' spec folds both outcomes into one resolution that attaches nothing |
| `internal/registry.go` | 6 | `SessionCache::sweep` | 2 ported (eviction, destruction); 4 absent with the memory budget |
| `internal/budget.go` | 6 | — | absent: `sessions/mod.rs` records that the reference's scaling *"exists to cooperate with a memory budget this series defers"* |
| `internal/deps/install.go` | 6 | `deps/install.rs::install`, `expand` | ported — D8 |
| `internal/deps/download.go` | 5 | `deps/install.rs::acquire`, `transfer` | ported — D8 |
| `internal/deps/retry.go` | 4 | — | absent: the retry loop is `rust-sak`'s — D7 |
| `internal/deps/manifest.go` | 3 | `deps/manifest.rs::read` | ported, `warn` |
| `internal/deps/archive.go` | 1 | `deps/install.rs::expand` | ported, `debug` |
| `internal/deps/model.go` | 1 | `deps/model/mod.rs::install` | **divergent**: the reference downloads an unverified model and warns; `model-install`'s spec already fixes this project's refusal, so it is an error on a path that fails |
| `internal/utils/model_data.go` | 4 | `deps/model/manifest.rs::resolve_once` | ported — D9 |
| `internal/sysinfo.go`, `utils/nvidia.go` | 3 | `gpu.rs::adapters`, `lib.rs::select_at` | ported, `warn`/`info` |
| `autopilot.go` | 3 | — | absent: no autopilot |
| `internal/utils/tiling.go` | 1 | `inference/grid.rs` | absent: a grid that cannot be laid out is unrepresentable here rather than warned about |
| `internal/utils/ort_path_windows.go` | 2 | — | absent: no 8.3 short-path handling in this project |
| `models/upscale/process.go` | 1 | `inference/upscale.rs::run_passes` | ported, `debug` |
| `models/upscale/osaka/*` | 3 | — | absent: Osaka's diffusion pipeline is refused by `chain::plan` |
| `models/facerecovery/inference.go` | 1 | — | absent: face recovery is not wired into the chain |

Roughly 30 records land, against 108 sites: 12 already done, 28 absent for a mechanism, and the rest
ported — several of them collapsing two Go sites into one, because this project's error handling
already joins cases the reference logs separately.

### D2. Three levels, and one rule that decides which

- **`info` — a fact about this session a bug report needs, produced a bounded number of times.** A
  run beginning and ending; an install beginning and ending; a model made ready and what it cost;
  compiled artifacts discarded.
- **`debug` — true, useful when chasing something, and produced once per operation or more.** Every
  per-step cache decision; a session-cache hit; a dependency found already present; the live listing
  read successfully; an upscale pass finishing.
- **`warn` — the application is continuing, and a user is paying for something they did not ask
  for.** A downgrade to another provider; a cache write that failed; a stored entry that would not
  decode; a transfer repeated; a listing fallback; a manifest ignored.

`error` stays where slice 1 put it: a panic, and the image cache being unavailable for the whole run.
An operation that fails is a `warn` and not an `error`, matching the reference — the failure is
returned to the caller as an `InferenceError`, and a caller that decides it is fatal is the front
end, which does not exist yet.

The rule that matters is the middle one, because it is the only one that cannot be reconsidered
cheaply later: a record written at `info` on a per-operation path is in every user's file forever,
while one written at `debug` costs a global load and a branch. When in doubt this slice takes
`debug`, which is also where the reference put its own per-operation records.

### D3. The run is bracketed at the chain, not at `Opai::process`

`inference::chain::process` gets the pair, not `Opai::process`, even though the second is the public
entry point. Two reasons: `chain::process` is where the operations, the resolved depth and the
cache handle are all in scope together, so the opening record can say what the run will actually do
rather than what it was handed; and `Opai::process` is one line of delegation, so a record there and
a record here would be two answers to "when did the run start" separated by a function call.

The opening record carries the operation count, the operation ids, the source's identity hash and the
resolved depth. The closing record carries the elapsed time and the operation count again, so a
`grep` for the message finds a pair of lines that can be read together without scrolling.

**The identity hash is the field that makes the log joinable.** It is what the cache keys are folded
from and what names the result, so it is the one value that ties a run's records to the cache
records, and to a second run over the same photograph an hour later.

`duration` is rendered through `Duration`'s `Debug`, which prints `1.523s` and `250ms` — close enough
to Go's `time.Duration` spelling that a support instruction written for one log reads against the
other, and without a formatting helper of this project's own.

### D4. The image cache's dropped distinction becomes two records, not an API

`put` has three outcomes and discards two of them: the image would not encode, `set_bytes` returned
an error, or it worked. `get_bytes` is `self.memo.get_bytes(key).ok().flatten()`, which folds a store
that **broke** into a key that is **absent** — and the module says why: *"A store that broke, an
absent key and an elapsed TTL are one answer here, because the response to all three is the same one
and it is always correct: run the operation."*

That reasoning is still right and the return type does not change. What changes is that the error is
recorded on its way to being discarded, so the distinction lands in the log rather than in a
signature:

- a write the store **declined** — `MemoError::NotAdmitted`, a result larger than the whole memory
  budget or one with no life left in it → `debug`, naming the key and the size. This is the half of
  the distinction that is *ordinary*: the memory fallback declines most large results by design, so a
  `warn` would put a line in every user's file for the designed outcome of an 8x run. The
  declined/broken split is the point of D4, and the levels are where it lands;
- a write that **broke** — any other `set_bytes` error — or one that failed to encode → `warn`,
  naming the key and the error, because a run whose every write breaks is caching nothing while
  behaving exactly as though it were;
- a read whose store returned an error → `warn`, distinguished in the record from a plain miss, which
  is not recorded at all at the cache layer (the chain records the miss);
- a stored entry that would not decode → `warn`, since it means a truncated write is being paid for
  twice;
- a run with no store at all — caching off for the call, or `CacheMode::None` → `debug`, once per
  run rather than once per operation.

**Alternative considered:** making `put` return a `Result` and `get_bytes` distinguish its three
misses, then logging in the chain. Rejected because it pushes the decision to the caller and the
caller has nothing to do with it: the module's own argument is that running the operation is always
the correct response, and a `Result` nobody can act on is a signature that invites someone to try.

### D5. A span around the session build, so the runtime's own complaints say which model they are about

The one place this slice spends a span rather than an event. `sessions::build::build` runs
synchronously on a blocking thread, and it is inside that call that ONNX Runtime emits the
diagnostics slice 1 wired up — *"Some nodes were not assigned…"*, a provider declining to attach, a
TensorRT engine being rebuilt. Those records arrive with the runtime's own `location` and `id` and
nothing that says which artifact or which provider was being built, because the C++ side does not
know.

A span entered for the build, carrying `artifact` and `provider`, is rendered onto every record
emitted inside it by slice 1's formatter. This is what the reference reconstructed by hand: its
parser attached `source=onnxruntime` and its own context to text it had scraped off descriptor 2.

**What this does and does not cover, stated rather than implied.** A span's fields reach an event
only on the thread that entered it. ORT emits the records that matter here — session construction,
provider attachment, graph partitioning — on the calling thread, so they are covered. Anything the
runtime emits later from one of its own worker threads, during inference rather than during the
build, is not, and arrives with the runtime's fields alone as it does today. That is a narrowing of
the reference's coverage, and it buys back the entire capture stack slice 1 deleted.

### D6. The downgrade is the record this slice is most for, and it goes where the retry is decided

`Sessions::session` matches `Err(SessionError::Build { provider, .. }) if provider !=
ExecutionProvider::Cpu` and rebuilds on the CPU. Two records there: a `warn` when the resolved
provider fails, naming the artifact, the provider and the error; and the `info` for the CPU session
that succeeds, which is the ordinary build record carrying `requested` beside `provider` so the pair
reads as one event.

`SessionHandle::requested` stays exactly as it is. The comment on it — *"The one place a downgrade is
reported at all: this project has no logging"* — is rewritten to say that the log now reports it too
and that the accessor remains the one a front end will read, since a bug report is not how a user
should find out their run was downgraded.

**There are two downgrades, and only one of them is a `warn`.** The one above is a *build* failure: a
provider the machine reports as supported that then will not open this model, which is a broken
driver or an unsupported operator set and is something gone wrong. The other is a *resolution*: a
named provider the machine does not support at all, which `options::resolve_chain` folds to the CPU
before anything is built. Nothing is wrong there — the answer is correct and immediate, and a `warn`
per session for "this Mac has no CUDA" is noise — so it gets no record of its own. It is still in the
file, because `requested` is carried beside `provider` on **every** build record rather than only
after a retry: a resolution downgrade reads as `session ready provider=Cpu requested=Cuda` with no
warning above it, and a build downgrade reads as the same line with one. That is why D1's
`ep_profile.go` row names the build record rather than a site in `providers/options.rs`, which emits
nothing.

The rest of the registry: a resident hit at `debug` (matching `internal/model.go`'s own level); a
build starting and finishing at `info` with its duration; an eviction from the sweeper and a release
from `clear` at `info`, naming how many were released. `Opai::release_sessions` gets the "releasing
everything" record that `init.go`'s teardown block has — its three companions there (the image cache
failing to close, the timeout waiting for models, the ONNX teardown skipped) have no counterpart,
because this project has no explicit teardown to fail: `Drop` releases the cache and Rust's ownership
model is what replaced the reference's wait-for-release loop.

### D7. The four retry records are not written, and `rust-sak` is not instrumented in this slice

`internal/deps/retry.go` logs each attempt that failed, the one that will not be retried, the
cancellation and the give-up. This project has no retry loop: `Fetch::retries(TRANSFER_RETRIES)`
hands it to `rust-sak`, which has no `tracing` dependency and emits nothing from any module.

Three options, and the middle one is taken:

1. **Instrument `rust-sak`'s `fetch`.** The right home by this project's own rule — a retry loop that
   says what it is doing is general-purpose, and any consumer would want it. It means adding
   `tracing` to a published crate, agreeing what it emits, releasing it, and then depending on the
   release. Deferred, on slice 1 D9's terms: *"the series is not sequenced behind a `rust-sak`
   release."* Written down here so it is a candidate rather than a gap nobody recorded.
2. **Record what `opai` can see at its own seam** — taken. `acquire` knows whether a partial existed
   before the transfer, which mode each pass used, what hash arrived, and whether the clean second
   pass recovered. That is a record when a transfer is repeated and a record when it finally fails,
   both naming the artifact: every user-visible outcome of the retry loop, minus the per-attempt
   detail.
3. **Reimplement the retry locally to log it.** Rejected outright — it is the module this project's
   rules single out by name.

What is genuinely lost is the shape of a flaky connection: a transfer that succeeded on the third
attempt looks, in this log, exactly like one that succeeded on the first.

### D8. The installer's records are the reference's, at the same levels

`deps/install.rs` maps almost one-to-one: dependency present (`debug`), installing (`info`, with the
dependency and its version), extracting (`info`, with the archive), ready (`info`, with the file
count), the archive that could not be sized (`debug`). `deps/manifest.rs::read` returning `None` on a
malformed or wrong-schema record is a `warn` naming the directory, which is `internal/deps/manifest.go`'s.

The hash mismatch is where this project's structure gives a better record than the reference's.
`acquire` runs a resumed pass and then, only if a partial existed, a clean one — so the record can
say *why* the second pass is happening, which `download.go`'s four separate mismatch warnings each
say a piece of.

Model installs (`deps/model/mod.rs`) get the same three — installing, ready, failed — keyed on the
artifact id rather than on a dependency name, so an `opai.log` grepped for `up_kyoto_4x_fp16` finds
the install, the build, the session and every run that used it.

### D9. The listing's three-way resolution is recorded at the point it falls back

`Listing::resolve_once` tries the live listing, then a cached copy, then the one compiled in. The
first is a `debug` with the entry count; the second and third are `warn`s naming what failed, because
a run working from a manifest compiled in months ago will eventually fail to find a model that
exists, and by then the fallback is long past.

This is `internal/utils/model_data.go`'s four records exactly, including the warn when the manifest
could not be written back to the cache.

### D10. The test for a record is the line in the file, not a collector

Slice 1 built `logging::test_sink` and `logging::test_subscriber` — the real rotator, the real
formatter, at a chosen filter — and `lib.rs`'s tests wrap them in a `records_of` helper that returns
the file's text, because *"what is asserted below is the text a reader of `opai.log` would actually
find — not a field set observed through a test-only collector that could pass while the file said
something else."*

That helper is promoted out of `lib.rs`'s test module into `logging` under `#[cfg(test)]`, so the
five modules this slice instruments assert the same way. It takes the filter directives as an
argument, which is what lets the volume requirement — the same run at `info` and at `debug`
producing different line counts — be a test rather than a claim.

Its one constraint travels with it and is documented rather than rediscovered: `set_default` binds
the subscriber to the calling thread, so a test must stay on it. The suite's runtime is the
current-thread one, and a record emitted from `spawn_blocking` will not be seen — which is exactly
where the session build's records are, so those are asserted through the builder seam the session
tests already use rather than through a real blocking thread.

## Risks / Trade-offs

**A record on a per-operation path turns out to be on a per-tile path** → The chain's loop is per
operation and the tile loop is inside `run_tiled`, two layers below, so the boundary is structural
rather than a matter of care. The one record near it is the upscale pass (`debug`, at most three per
operation). Mitigated by the spec's own requirement — the same enhancement over a small image and a
large one produces the same number of default-level records — which is a test rather than a review
note.

**Synchronous writes make a chatty record a measurable cost** → Slice 1 D7 bought durability with a
lock and a `write(2)` per record on the emitting thread, and named this slice as the risk: *"If slice
4's sweep makes a hot path chatty, the answer is the level of that record, not a buffer between the
record and the disk."* Taken as written. At `info` a run produces two records plus one per
installation; at `debug` a long chain produces a few dozen, which is a few dozen syscalls against an
inference run measured in seconds.

**Thirty new call sites are thirty chances to log something a user did not consent to sharing** →
What goes in: operation ids, artifact ids, provider names, durations, counts, hashes of image
content, and filesystem paths beneath the configuration directory. What does not: the contents of an
image, and any path a user chose — a source photograph's path is in `Picture`, and it is deliberately
not on the run's records. The identity hash is of the pixels and the operations, which is what the
cache is already keyed on, and it is not reversible to a picture.

**A record and the comment that promised it disagree afterwards** → Four doc comments describe the
absence this slice ends, and one of them is a module header that a reader reaches before the code.
Rewriting them is a task rather than a side effect of the task that adds the record, so it cannot be
half-done by a change that ran out of room.

**The reference is the source of truth for the record set, and it is ahead of this project** →
Twenty-eight of the 108 sites are for mechanisms not built here, and each will need its record when
the mechanism arrives. The mapping table in D1 is written into this design so that the next change to
touch one of those areas has the list rather than rediscovering it from the Go source.

**The ORT span covers the build thread and not the runtime's own threads** → D5, stated there rather
than hidden. A diagnostic emitted during inference rather than during the build arrives with the
runtime's fields alone, as it does today; nothing regresses, and the improvement is bounded.

## Platforms

The design targets **macOS, Windows and Linux**, and nothing in it has a platform surface: every
record goes through `tracing` macros into slice 1's sink, which is pure Rust over `std::fs`. The one
platform-specific Go record set — `ort_path_windows.go`'s two 8.3 short-path warnings — is in D1's
table as absent, because this project has no such handling to warn about.

**Validated on none of them yet** — this is a design, and the honest statement is the one slices 1
and 2 made: this project's live checks have only ever run on macOS arm64. What CI checks
hermetically on all three is every record in this slice except the ones that need a loaded runtime:
the chain's records run against the fake `Backend` the suite already has, the cache's against a
temporary store, the installer's against `TestServer`, and the listing's against its three sources.

**Execution providers:** no provider is resolved, configured or built differently. What changes is
that the downgrade path — a provider that will not attach, and the CPU session that follows — is
written down. Which provider a machine actually downgrades on is untested here for the same reason it
always has been: it needs the hardware, and the live checks that have it are macOS arm64.

## Division across the crates

**`opai` holds all of it.** Every record is emitted at a decision this library makes, and both front
ends would want every one of them — which is the test this project sets for where code lives.

**`gui` and `cli` are untouched.** Neither gains a call, a field or a dependency. Their records
arrive with the screens that make them: a GUI that processes an image will want to say which image
and at whose request, and that record belongs to the change that adds the screen.

**`perf` does not exist yet** (slice 6) and inherits this slice's records for free, which is most of
what makes a benchmark run readable: the session build's cost, the provider it landed on, and whether
a run was served from the cache are exactly what a timing harness needs to know it measured what it
thought it did.

**`rust-sak` features used: none new**, and one gap left open on purpose — D7. No third-party crate
is introduced, and no dependency is added to any manifest: `tracing` is already there from slice 1,
and this slice is call sites, comments and tests.
