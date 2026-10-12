## Slices

Seven changes, in order. Single-instance came first because the changes after it are designed
against its guarantee, and logging came next because the benchmark ahead of it wants a sink to see
the library through. **This change is perf slice 1, change 5 of the seven.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-single-instance-1-lock` | At most one OPAI process per user: an advisory lock taken in `Opai::initialize`, the holder recorded, the second process refused | none | **done** (archived `2026-09-14`) |
| 2 | `add-single-instance-2-gui` | The GUI raises its existing window instead of refusing, the ending a double-clicked icon deserves | 1, and a GUI with a window | **done** (archived `2026-09-14`) |
| 3 | `add-logging-1-file` | The sink: `tracing` through `opai`, ORT's C++ diagnostics via `ort`'s `tracing` feature, `<config>/logs/opai.log`, daily rotation, the `---` session divider, the session header, the per-record `app=` field, `APP_NAME`, and the initialization path instrumented | 1 | **done** (archived `2026-09-15`) |
| 4 | `add-logging-2-records` | The instrumentation sweep: the enhancement run, the session registry, the installers, the image cache and the model listing, against the records the Go library emits | 3 | **done** (archived `2026-09-15`) |
| 5 | `add-perf-1-opai` | What a benchmark needs from the library: `release_sessions` made public, the `(requested, actual)` execution provider surfaced out of `process`, skip-verify | 3 | **current** |
| 6 | `add-perf-2-harness` | `crates/perf` → the `perftest` binary: `clap`, catalogue-driven model selection, the warm-up/cold-start/timed-runs loop, statistics, plain and JSON output | 5 | pending |
| 7 | `add-perf-3-tui` | The `ratatui` inline viewport: spinner, per-tile bar, ETA, finished models into scrollback via `insert_before`, TTY detection | 6 | pending |

This slice's row has always said it depends on 3 rather than on 4, and that is still right: it wants
the sink, not the sweep. It inherits slice 4's records anyway, which is where the "verification
skipped" warning below gets its level rule and its formatter for free.

## Context

See proposal.md — Why for the motivation, and the three delta specs for the behaviour. What shapes
this design is five things read out of the code rather than assumed:

- **The facts already exist; only their reach is short.** `SessionHandle` has carried
  `requested()` beside `provider()` since the sessions slice (`sessions/mod.rs`), whose doc comment
  predicted that the accessor would be the one a front end reads. It is not, and the prediction is
  rewritten where it stands: the front end reads `ProviderReport`, whose `requested` half comes
  from the run's own argument rather than from a handle — per D1, because one run asks for one
  provider and an empty chain has no handle to fold it out of. `provider()` is what the report is
  folded from, and `requested()` keeps a narrower answer nothing has asked for yet: *which
  acquisition* was downgraded. `Opai::release_sessions` exists, is tested, writes its record, and
  carries `#[expect(dead_code, reason = "the first caller is the front end that wires shutdown")]`.
  Two of this slice's three deliverables are a visibility change and a return value, not new
  machinery.
- **`chain::run` already accumulates every handle the run took.** `let mut held:
  Vec<SessionHandle<B::Session>> = Vec::new()` exists so that a session a run is using cannot be
  reclaimed underneath it, and it holds every acquisition for the length of the call. The provider
  report is a fold over a vector that is already there — decided in D1.
- **`install_inner` is one function with one identity variable.** `let want =
  manifest::fingerprint(dependency)`, then `current = recorded.fingerprint == want &&
  recorded.intact(dir)`. Everything downstream — the emptying, the derived-cache discard, the
  record — hangs off those two values. Trust is therefore a different `want` and a relaxed
  `current`, not a second pipeline. This is also how the reference does it, and its comment says
  why: *"It takes the same body as everything else — only its identity is different… so the rule
  that derived caches are invalidated when the installed identity moves has one owner rather than
  two."*
- **This project has already spent a change closing the unverified-install path.** `deps/artifact.rs`
  has three separate tests whose comments say *"an empty expected hash means 'do not verify', which
  is how these libraries once shipped unverified"*. Any debug flag that reopens it has to be scoped
  so those tests stay true, which is D4.
- **Nothing in the workspace calls `initialize` or `process` yet.** `crates/cli/src/main.rs` prints
  a version; `crates/gui/src/lib.rs` installs a log sink and builds a Tauri app. The only thing that
  pins either signature is `tests/public_api.rs`, deliberately, through function pointers. The one
  breaking change — `process`'s return type — costs one test file; `initialize` ends up not breaking
  at all, per D3.

## Goals / Non-Goals

**Goals:**

- Three library capabilities a benchmark cannot be written without, each shaped for the front ends
  that will want it too rather than for the harness that asked first.
- A debug mode whose blast radius is written into the type system: a process that did not ask for it
  at startup has no way to reach it, and no run's options can express it.
- No new pipeline in the installer. One function, one identity, one place the derived caches are
  discarded.

**Non-Goals:**

- **`crates/perf` itself.** No `clap`, no sweep loop, no statistics, no output format, no TUI. Those
  are slices 6 and 7, and nothing here is shaped around a command-line flag.
- **A latch, and a call to clear one.** The reference latches "this provider is unusable" across
  models and needs `ResetProviderFallback` to forget it. This project's fallback is explicitly not
  latched (`sessions/mod.rs`: *"The downgrade is **not latched**: a later request attempts the
  provider it asks for"*), so there is nothing to reset and no API for resetting it.
- **A global fallback handler.** The reference's `SetFallbackHandler` is process-wide, fires only
  for the first model downgraded, and has to be reset between models by hand — three properties the
  return value in D2 does not have.
- **Model memory budgets and pools**, which the reference's benchmark header reports through
  `ModelMemoryStats`. This project has not built them; inventing a report for a mechanism that does
  not exist would be worse than the harness omitting the line.
- **Trusting anything but model files.** The runtime and the GPU libraries stay pinned, hashed and
  verified in every process.

## Decisions

### D1. The provider report is folded out of `held`, the vector the chain already keeps

`chain::run` holds every `SessionHandle` it acquires until the call returns. Each one carries the
provider it was built on and the provider that was asked for. So at the end of the run the report is
a fold over `held` — the requested provider (one value, the run's own `provider` argument) and the
set of providers that were actually built on — and not one line of plumbing is added to the loop, to
`Backend::acquire`, or to the pass sequence.

This is why a chain downgraded for its second model and not its first reports both, which the
reference cannot do: its handler fires once, for the first downgrade, and is then quiet because the
latch makes every later model build on the CPU without another failed attempt to report.

`actual` is a set rather than a list. A chain acquiring four handles on one provider — which is what
a two-operation, two-pass upscale does — would otherwise report that provider four times, and
nothing a caller wants to do with the answer is served by the multiplicity. It is ordered so that
two runs of the same shape produce the same report and a test can compare them directly.

**A run that acquired nothing reports an empty set**, and that is a third outcome rather than a
degenerate case of "ran on the CPU". An empty chain and a fully-cached run both take zero handles;
reporting the requested provider as though it had been used would tell a benchmark it measured
something it did not. The spec makes this explicit because it is exactly the case a caller gets
wrong by writing `report.actual.first().unwrap_or(report.requested)`.

**Alternatives considered.** A callback in `ProcessOptions`, which is the reference's shape moved
per-run: rejected because a caller that registered no handler would never learn its run was
downgraded, and the fact is a property of the outcome rather than an event during it. A new `Stage`
on the existing progress stream: rejected for the same reason plus a worse one — it files a provider
fact under progress reporting, and `ProcessOptions::on_progress` defaults to `None`, so the default
caller is the one that loses it.

### D2. `process` returns a bundle, mirroring `ProcessOptions` on the way in

`Opai::process` returns a small owned struct carrying the `Picture` and the provider report, rather
than a tuple. The reasoning is the one already written into `ProcessOptions`' own doc comment: a
bundle with named fields is what lets a later slice add a third thing a run wants to report —
whether any step was served from the store, how long inference took as distinct from encoding —
without moving a single call site. A tuple gets exactly one addition before it becomes unreadable.

The provider report is its own named type inside that bundle rather than two loose fields, so that
the pair travels together. A front end showing *"ran on CPU because CUDA could not open this
model"* needs both halves; splitting them across the result invites a caller to render one.

The result's identity is unchanged and deliberately does not include the provider — same pixels,
same name. The spec says so in its own scenario, because a result computed on a GPU and one computed
on the CPU are interchangeable and the image cache is keyed on that identity. A provider in the
identity would halve the cache's hit rate on a machine whose GPU is sometimes busy, for no
correctness gain at all.

`chain::run` returns the pixels beside the report and `process` composes the public bundle from it,
which keeps the whole of D1 inside the generic, fake-backed half of the module — so "a chain
downgraded for one model and not another" is a test on a runner with no GPU.

### D3. Trust is declared at `initialize`, as a value, not as a flag or a setter

The declaration is a field of `InitOptions` — `models` — and is stored on `Inner` beside the other
things this initialization decided — the provider report, the cache mode, the claim. It reaches the
installer the way everything else does: `Opai::session` → `Sessions` → `deps::model::install`, which
already carries `app_dir` and `name` along that path.

It is a two-valued enum rather than a `bool`. `initialize(name, app, true, on_progress)` at a call
site says nothing about what is true, and this is the one argument in the signature where a reader
getting it backwards installs unverified weights.

**Why not `ProcessOptions`.** That is where `cache` went, and the precedent is written into the code:
*"per run rather than a setter on the application because a hidden global whose value a run cannot
see is how the reference ended up having to bind it to a local and comment about why."* That
reasoning is about **visibility**, and it points the other way here. `cache` is a property of the
run — two runs in one process can legitimately differ, and a caller measuring one wants it off for
that call alone. Trust is a property of the **machine's state**, not of a run: it decides what is on
disk, which every later run in every later process inherits. A per-run flag would let one run install
unverified weights that the next run, with the flag off, would then find recorded and use. Putting
it at initialization is what makes D6 expressible at all.

**Why not a setter.** `Opai::set_model_trust` would be the reference's shape, and it has the property
this project has already rejected once: a value a run cannot see, changeable between the check and
the install.

**Why not an environment variable.** Zero API surface and unreachable from a front end, which is
genuinely attractive. Rejected because it is precisely the invisible global the paragraph above
argues against, with the extra defect that no test can scope it — it is process-wide state a test
must set and unset around itself, in a suite that runs tests in parallel threads.

### D4. Trust decides what is read off the disk, never what is accepted off the network

The reference's flag does both. `internal/deps/download.go`'s `acquire` takes `skipVerify` and, on a
hash mismatch, logs *"hash mismatch ignored"* and returns the file. This project carries the on-disk
half and refuses the network half, which is the behavioural divergence the proposal calls out.

The workflow the flag exists for is a file the developer exported and copied into the model
directory. That is fully served by the on-disk half. The network half adds nothing to it and
reopens, behind a debug flag, exactly the hole `deps/artifact.rs` spent a change closing — with the
aggravating factor that bytes arriving over a network are the ones an attacker can choose, which a
file the operator placed by hand is not. `install.rs`'s existing hash check and its one-retry
recovery are untouched by this change.

There is a second reason, visible only in the reference: its mismatch-ignored branch runs
`removePart` and then returns a `File` describing a path it never renamed into place. The on-disk
path has no such corner because it moves nothing.

### D5. A trusted model's identity is stamped from the files, and a stat is the whole cost

On the trusted path there is no published hash to compare against, so `want` becomes a stamp
computed over each source's name, size and modification time as they sit on disk, instead of
`manifest::fingerprint(dependency)`. Everything downstream is unchanged: the same comparison against
the recorded fingerprint, the same `dependency.derived` emptying when it moves, the same record
written at the end.

That gives both halves of the spec's requirement at once. Editing a weights file moves its size or
its mtime, the stamp moves, the comparison fails, and the provider's compiled engine is discarded —
which matters most in exactly this workflow, since re-exporting a model is what changes weights most
often and a TensorRT engine reused against weights it was never built for produces an image nothing
reports as wrong. Touching nothing leaves the stamp where it was, so an iteration loop does not pay
a multi-minute engine rebuild on every launch.

Size and mtime rather than a hash of the bytes, which is the reference's reasoning and is worth more
here than there: this runs on every acquisition of the model, the weights run to gigabytes, and
`symlink_metadata` is the call `Manifest::intact` already makes per file on every launch. A hash
would be strictly more precise about a file edited in place within one filesystem timestamp tick,
which is not a threat model — an operator who replaced a file and wants it measured is not trying to
defeat their own stamp — at a cost of re-reading every byte of every model on every run.

`rust-sak`'s `crypto` module is what would provide such a hash and is deliberately not reached for
here; nothing in this decision needs a `rust-sak` feature that is not already enabled.

### D6. A trusted install records no files, so trust does not leak into the next process

The record written for a trusted model names the stamp as its fingerprint and an **empty file list**.
Two consequences, both required by the spec:

- `Manifest::intact` treats an empty list as "not installed", so the next process — one that did not
  declare trust — finds the record not current and installs the model from the published sources,
  verified. Without this, one debugging session would silently poison that machine: the unverified
  weights would sit under a record indistinguishable from a real install, and every later launch
  would run the unpublished model believing it was the published one.
- Which is exactly why `intact` must be **skipped** on the trusted path itself. Left in, it would
  report "not installed" on every launch, and the derived-cache discard the stamp is there to gate
  would fire every time — the engine rebuild D5 exists to avoid. So the trusted comparison is
  `recorded.fingerprint == want` alone, and the ordinary one keeps both halves.

The reference reaches the same two conclusions and its comments are the clearest statement of why;
this is one of the places the project context means by *"its comments carry most of the reasoning"*.

The `warn` record is written every time the trusted path is taken, not once per process. A model
whose verification was skipped is the first thing to suspect in a report of wrong output, and the
level puts it in the file a user attaches without being asked to reproduce anything.

### D7. `release_sessions` becomes public, and Rust already gives the property the reference had to wait for

The method, its body, its record and its tests are unchanged. What comes off is `pub(crate)`, the
`#[expect(dead_code)]` on it, the matching one on `SessionCache::clear`, and the comments on both
predicting this slice — `sessions/mod.rs` currently says *"A logging change can later put a line at
each of those points without any of them moving"* and *"reachable from `Opai::release_sessions`,
which no caller reaches yet"*. Comments that describe an ended state are rewritten rather than left
standing, as slice 4 did for its four.

**Ownership and lifetime.** The cache holds an `Arc<Resident<S>>` per entry and every `SessionHandle`
holds another. `clear()` drops the map's references; an entry nothing else holds reaches refcount
zero inside that call and its native session is dropped there, synchronously, before `clear` returns.
An entry a run is still holding survives to that run's last handle drop. That is precisely the
requirement the spec now states — freed before the call returns when nothing is in use, no waiting
when something is — and it falls out of ownership rather than being enforced.

It is worth naming what that replaces. The reference's `CleanRegistry` *waits* for the models it
removed to actually be destroyed, and its benchmark comments explain that this is what stops a "cold"
run from quietly reusing a still-resident session. Here the wait is unnecessary in the case the
benchmark is in — between models, with nothing in flight — because the last reference is the cache's
own. This is the project context's *"design for that rather than porting the Go locking discipline"*
applied to the one case where it produces a smaller API instead of a safer one.

No new API for the benchmark's other need — that each model's cold start pays a full build — because
the existing contract already delivers it: release everything, then acquire, and the acquisition
finds nothing resident.

### D8. Trust is carried on the `Dependency`, set only where a model's descriptor is built

`deps::release::Dependency` gains the field, defaulted to ordinary verification, and it is set in
`deps/model/descriptor.rs` where a model's descriptor is composed — never on the `const` release rows
for the runtime, CUDA, cuDNN or TensorRT, which have no field to set it in and are built by
`Descriptor::from_release_at`. That is the spec's "model files only" rule enforced by where the code
can reach rather than by a branch that has to remember, and it means `install_inner` reads one field
without knowing what kind of dependency it has.

## Risks / Trade-offs

**An unverified model is recorded on disk and a later process runs it** → This is D6's whole subject
and is closed at the record: no files are listed, so a process without the declaration reinstalls and
verifies. The `warn` per acquisition is the second line of defence, in the file a user already
attaches.

**A breaking signature change with no consumer to catch a mistake** → `tests/public_api.rs` pins
`process` through a function pointer, which is what it was built for; the compile failure there is
the review. `gui` and `cli` call neither it nor `initialize`, so nothing silently keeps compiling
against an old meaning. `initialize` is no longer among the breaking ones — see D3 — and its new
field is checked in the same file, both in the `..Default::default()` form and for its default.

**`actual` as a set loses which operation ran on what** → Deliberate. The question a caller asks is
"was this run downgraded, and to what", which a set answers. Per-operation attribution would mean a
report shaped like the chain, which nothing has asked for; the log already carries the per-model
answer at `warn`, from the session registry, for whoever is debugging one.

**A stamp of size and mtime can miss an in-place edit inside one timestamp tick** → Accepted, per
D5. The flag is a developer convenience and its user is not adversarial to it; the cost of the exact
answer is re-hashing gigabytes on every acquisition.

**A trusted model completed from the network is a mix of trusted and verified files** → It is not:
trust applies only where *every* backing file is present, and a model missing one is installed and
verified whole. This is the reference's `sourcesPresent` rule and the spec states it directly.

**`crates/perf` may want something this slice did not expose** → Possible, and cheap to find out
late: slice 6 depends on 5 and would be blocked by a missing accessor. Mitigated by having read the
reference harness end to end first — `cmd/perf` needs `CleanRegistry`, the fallback report,
`SetSkipModelVerification` and `SetImageCacheEnabled`, and the fourth already exists as
`ProcessOptions::cache` from the cache slice. The two the reference also uses and this project
cannot supply, `ModelMemoryStats` and `ResetProviderFallback`, are non-goals above with the reason.

## Crates, dependencies and validation

**Crate split.** `opai` only. Every deliverable is library logic both front ends would want — a
shutdown gesture, a downgrade to show the user, a debug mode for whoever is working on models — so
none of it could live in a front end even if there were one to put it in. `gui` and `cli` are
untouched by this change; `crates/perf` is slice 6.

**`rust-sak`.** No new feature and no new use. `crypto` already backs the installer's hashing and is
deliberately not reached for by D5's stamp; `fetch` owns the transfer and is untouched by D4; `fs`
and `memo` are not involved. Nothing here is a gap in `rust-sak`, so there is nothing to extend
there and no third-party crate to weigh against it.

**Other crates.** None introduced. The stamp is `std::fs::symlink_metadata`, the report is `std`
collections, and `tracing` is already a dependency from slice 3.

**Platforms and execution providers.** Everything in D1, D2, D6 and D7 is exercised against the
`Backend` fake and a local test server, so it is checked on every CI platform with no runtime, no
GPU, no model file and no network — including the downgrade paths, which the fake produces on demand.
What that does *not* cover is a real downgrade: the report's contents on an actual CUDA or TensorRT
failure are checked on macOS/CoreML and CPU only, since those are the providers this machine has.
CUDA, cuDNN and TensorRT on Linux and Windows are untested here as they have been in every slice so
far. D5's stamp is validated on macOS and on CI's Linux runners; Windows is untested, and is the one
platform where the mtime granularity behind D5 differs enough to be worth naming — NTFS resolves to
100ns, which is finer than the ext4 and APFS cases, so it is not a weaker case than the ones tested.
