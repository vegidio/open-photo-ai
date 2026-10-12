## Why

Slice 1 built a sink and put a dozen records into it. Everything the log is actually opened for is
still missing from it: what a user's enhancement did, which provider served it, why it ran on the CPU
on a machine with a CUDA card, what the cache served or dropped, and what the installer transferred
before it failed. Today a session that initializes and then processes twenty images writes the same
six lines as one that initializes and sits idle, so `opai.log` answers "did it start" and nothing
after it.

The code says so in four places, in the imperative. `cache.rs`: *"This crate has no logging, so the
distinction has nowhere to go and is not invented as an unread field. **It is the first thing to
reinstate on the day logging lands.**"* `sessions/mod.rs`: *"The reference logs which provider a
session got, what the build cost, and every eviction and downgrade. This project has no logging yet,
so each of those facts is carried as data a caller can read instead… A logging change can later put a
line at each of those points without any of them moving."* `lib.rs` twice, on the cache distinction
and on the downgrade. Those are not comments about a missing feature; they are a list of call sites
left standing for this change, which is the day they described.

The reference implementation's own bug reports are the argument. A downgrade to the CPU is invisible
to a user — the enhancement still completes, five times slower — and the only reason anyone can
answer "why was this slow" there is a line saying which provider the session was built on. This
project carries that fact as `SessionHandle::requested` beside `provider`, which no front end reads
and no bug report can contain.

## What Changes

Task estimate: **15**. This is logging slice 2 of 2, and change 4 of the seven design.md records.

The sweep, against the Go library's own records rather than invented from the Rust code: every
`internal.Log()` call site in the reference, mapped onto the Rust function that replaced it, given a
level, a message and a field set.

**Recount, because slice 1's figure was wrong.** Slice 1 said *"the 84 records the Go library
emits"*, and 84 is the count of `internal.Log().*` — the calls from packages **outside** `internal`.
The `internal` package logs 24 more times through its own bare `Log()`, in `registry.go`,
`budget.go`, `model.go`, `cache.go` and `sysinfo.go`. The real figure is **108**, of which slice 1
covered the part of `init.go` that runs at every launch. design.md carries the full mapping, area by
area, including what has no Rust counterpart and why.

**What gets records, all in `opai`:**

- **An enhancement run** — `inference::chain`, whose module documentation still lists logging under
  *"What is deliberately not here"*. The chain's start and its outcome with a duration at `info`; the
  per-operation cache hit, miss, run and failure at `debug`; a plan refusal at `warn`. This is the
  one area where the level matters as much as the record: the reference logs per-operation at debug
  for exactly this reason.
- **The image cache** — the distinction the module marked for reinstatement: a write the store
  **declined** and one that **broke** are dropped identically today, and *"a run dropping every write
  is caching nothing while looking like it is"*. Plus a lookup that failed and an entry that would
  not decode, which are Go's `cache.go` records.
- **The session registry** — a build starting and the session it produced, with the provider and what
  it cost; the downgrade, which is the record this whole slice is most for; evictions; and the
  registry emptied.
- **The session build and the provider caches** — the TensorRT timing cache dropped after a failed
  build, and the engine tree emptied because the runtime tag moved. Both are minutes of recompilation
  a user pays without being told why.
- **The dependency and model installers** — dependency present, installing, extracting, ready; a hash
  mismatch and the clean pass that follows a resumed transfer; a manifest ignored as malformed. And
  the model listing's three-way resolution — live, cached, compiled-in — where a run silently falling
  back to a manifest compiled in months ago is a thing a bug report needs to say.

**No new dependency, no new public API.** Every record goes through the `tracing` macros slice 1
already wired, into the sink it already built, rendered by the formatter it already wrote. What
changes is the number of call sites, the doc comments that promised them, and the tests that pin
them.

**Nothing in `gui` or `cli`.** Neither has a processing surface yet: the GUI's window never calls
`process`, and the CLI prints a version. Their records arrive with the screens that make them.

## Capabilities

### Modified Capabilities

- `logging`: the capability today says where the file is, what a session header looks like, what
  every line carries and what happens when the sink cannot be installed — all properties of the
  *sink*, and none of the *contents*. What is added is what a reader finds after the application has
  done something: that a run of work is accounted for end to end, that a result served from the cache
  or degraded away from what was asked for says so where it was decided, that an install says what it
  transferred, and that records on paths which repeat per tile or per pixel are below the default
  level so a long session's file stays readable. No existing requirement changes; these sit beside
  them.

No new capability. A record about the image cache is not a new thing the application does, it is the
log having contents — and `image-cache`, `model-sessions`, `model-install` and `inference-tiling`
each already specify the behaviour being recorded. Writing "the cache logs a miss" into
`image-cache`'s spec would scatter one capability across five files and leave no single place that
says what a bug report will contain.

## Impact

**Crates:** `opai` only — the sweep is entirely in library code, which is where every one of these
decisions is made. `gui` and `cli` are untouched, and no front end learns anything new.

**`rust-sak`:** nothing new is needed, and **one gap is found and deliberately left open**. The
reference logs four records from its own retry loop (`internal/deps/retry.go`: a transfer cancelled,
one that will not be retried, one given up on, and each attempt that failed before a retry). This
project has no retry loop — `rust-sak`'s `fetch` has it, behind `Fetch::retries(TRANSFER_RETRIES)` —
and `rust-sak` has no `tracing` dependency and emits no records from any module. So those four have
nowhere to come from without instrumenting `fetch` itself.

Instrumenting it would be the right home for it by this project's own rule, and it is **not** being
done in this slice, on slice 1 D9's terms: the series is not sequenced behind a `rust-sak` release.
What `opai` can see at its own seam is recorded instead — the transfer it started, the mismatch it
got, the clean second pass a resumed transfer earns, and the failure it ends with — which covers the
user-visible outcomes and loses only the per-attempt detail. design.md records it as a candidate for
a later `rust-sak` change rather than as a gap nobody wrote down.

**Behavioural divergence from the Go implementation**, all of it about volume rather than content:

- **Nothing is recorded per tile or per region.** The per-operation records are `debug` and the
  upscale's per-pass record is kept, both matching the reference. What is not carried over is
  `osaka/pipeline.go`'s line per region — a 12,000 × 8,000 photograph is hundreds of tiles, and slice
  1's D7 bought synchronous writes precisely so the last record before a crash is on disk, so a
  per-region record would put a lock and a syscall inside the tile loop to buy it. The Osaka pipeline
  is not implemented here in any case; this fixes the rule before it arrives.
- **The memory-budget records have no counterpart**, because the mechanism does not exist here.
  `internal/budget.go` and `internal/registry.go` emit twelve records about per-pool budgets, leases
  and admission; `sessions/mod.rs` states that the reference's scaling *"exists to cooperate with a
  memory budget this series defers"*. They arrive with the budget, not before it.
- **A model absent from the remote manifest is refused rather than warned about.** The reference
  *"downloads an unknown model unverified and logs a warning"*; `model-install`'s spec already
  records this project's refusal as a deliberate divergence, so the record becomes an error on a path
  that fails rather than a warning on one that continues.
