## Context

Slice 2 gave every record at `info` and above a second destination: `opai.log` and, when telemetry runs, Grafana. The
records themselves are the ones earlier slices wrote, one module at a time. This slice audits them as a set. The
audit covered every `tracing` call under `crates/opai/src`, about 60 outside `logging` and `telemetry`, and the `gui`
records that sit on the far side of an `opai` failure. It found five kinds of fault; see proposal.md - Why.

The facts that shape the approach:

- **The error types already say what was a stop.** No new classification is invented here:
  - `InferenceError::Cancelled` (the caller asked) and `InferenceError::Shutdown` (the runtime is going down).
  - `InitError::Stopped` (nothing left waiting) and `InitError::Cancelled` (shutdown).
  - `ImageIoError::Cancelled`, which only the async twins produce.
  - `From<SessionError> for InferenceError` already unfolds a stopped install into `Cancelled` and a shutdown into
    `Shutdown`.
- **Three brackets already exist, and each has one closing arm too few.**
  - `process` writes "enhancement started" and then "finished" or "failed".
  - `execute` does the same for an analysis.
  - The session flight writes "building session" and then "session ready" or "the build stopped". Its `Err(_)` arm
    is empty.

  `initialize` has an opening record and a success record, and every `?` in between leaves without one.
  `deps::install` is the one place that already wraps its body to record every failure once, through
  `install_inner(..).inspect_err(..)`. This slice reuses that pattern.
- **Some records sit in the wrong place in the flow:**
  - The chain is planned inside `run`, after "enhancement started", so a refusal also closes the bracket as a
    failure.
  - `execute` judges its pipeline before "analysis started", but records nothing when it refuses.
- **Existing tests pin the records being removed:**
  - `a_failed_step_is_distinguishable_from_the_run_that_carried_it` (`inference/process/tests/log.rs`) asserts both
    "step failed" and "enhancement failed". Its comment's reason, that the run record alone would not say which step,
    is answered by moving the step's fields onto the run record.
  - Autopilot's test at `autopilot/mod.rs` asserts "a signal could not be read".
- **The run cache encodes and decodes through `rust-sak`'s `encode_writer` and its own `decode_bytes`,** not through
  `opai::image`'s public functions. The public functions do not call one another. So a record at each public
  boundary is written exactly once per failure, and never on the cache's path, which has its own records.

## Goals / Non-Goals

**Goals:**

- Every unit of work that opens at `info` closes at `info` or above, with one of three outcomes: finished, stopped or
  failed. Each closing record carries its duration and subject.
- One record at `warn` or above per failure per unit of work, across `opai` and `gui` together.
- In `opai`, `error` is written for a panic and nothing else.
- The file for an ordinary successful session changes only by fields added to existing lines.

**Non-Goals:**

- Spans, metrics and trace links (slice 4). This slice adds fields to records, not spans. Where a span would carry a
  field better, the field still goes on the record now, because a record is what Grafana's Loki shows without a trace.
- Records for image IO successes, or new per-step records.
- `gui`'s own failures and levels (slice 5's command wrapper), apart from removing the repeats this slice creates or
  exposes.
- Any change to what an API returns. Every change here is to a record.

## Slices

Full observability, in order. Only the current slice is proposed; the rest are proposed when they are reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-observability-1-o11y-bridge` | `rust-sak` `o11y`: runtime log emit, owned spans with explicit parents, span context + `traceparent`, links, status, and the `tracing` bridge behind `o11y-tracing`; released as `26.9.9`; pins bumped in `opai`, `gui` and `perf` | - | done |
| 2 | `add-observability-2-telemetry-seam` | `rust-sak`: the `set_attribute` fix released as `26.9.10`. `opai`: the subscriber becomes a registry with a file layer and a Grafana layer (each with its own filter); `opai::telemetry` start/stop; export failures recorded file-only; span context carried through `task::spawn_blocking`; endpoint and auth baked in via `option_env!`; `logging` spec updated. `gui`: start in `setup`, stop on exit | 1 | done |
| 3 | `add-observability-3-record-cleanup` | `opai`: the existing records fixed. Silent failures recorded (initialize, session build, image IO, analysis refusal); each failure recorded once (step folded into run, refusal outside the bracket, unread signal folded into the conclusion); cancellations and shutdowns recorded as stops; durations and subjects added to closing records; `error` reserved for panics. `gui`: its repeats of `opai`'s failures removed | 2 | **current** |
| 4 | `add-observability-4-spans-metrics` | `opai`: spans on every unit of work (initialize, install, session acquire/build, enhancement and steps, analysis, autopilot, image IO, cache) and the metric instruments (durations, cache hit/miss, provider fallback, failures by kind, resident sessions) | 3 | pending |
| 5 | `add-observability-5-gui-switch` | `gui`: the Analytics setting mirrored into a Rust-owned file and read at startup; opting out stops sending at once; one wrapper that logs every command's error and opens its span; app exit recorded; the `eprintln!` in `single_instance.rs` fixed; CI secrets wired | 4 | pending |
| 6 | `add-observability-6-frontend-log` | frontend: a log command into `opai.log`; an ErrorBoundary and global `error`/`unhandledrejection` handlers; the errors swallowed today reported | 5 | pending |
| 7 | `add-observability-7-faro` | frontend: Grafana Faro for errors and console, usage events and traces, with a `traceparent` passed into the Tauri commands so frontend and backend spans form one trace; CSP opened for the collector; paused and resumed on the Analytics switch | 5, 6 | pending |

Slice 3's row changed from the one planned in slices 1 and 2 in two ways:
- It gained the removal of `gui`'s repeats. The repository owner chose to remove them here rather than leave every
  failure doubled until slice 5.
- "image decode/encode" became all six public image operations (load, save, encode, probe, probe_raw, identity), also
  at the owner's choice.

**What this leaves for slice 5:** a wrapper that "logs every command's error" would repeat `opai`'s records again. It
must follow D1. It should not record a failure `opai` already recorded, at any level: its span carries `run` onto
`opai`'s record instead. It records only the failures that `gui` produces itself.

### Decisions already taken for slices 2 to 7

Settled with the repository owner during exploration, and repeated in every slice's design.md. One was revised in
slice 2 and is marked as such.

- **`tracing` stays the one logging API.** Grafana is a second layer beside the file, fed by slice 1's bridge. There
  is no second set of `o11y::log` calls.
- **Analytics defaults to on and sends from the first launch.** Opting out stops sending at once (`o11y::shutdown()`
  and `faro.pause()`). Opting in takes effect at the next launch for the backend; Faro may resume at once.
- ~~**Path fields never reach Grafana.**~~ **Revised in slice 2:** paths are not scrubbed. Records reach Grafana with
  their paths as `opai.log` has them. This slice's image IO records carry paths on the same terms.
- **ONNX Runtime warnings and above go to Grafana too**, with the same floor as the file.
- **Only the GUI exports.** `cli` and `perf` keep writing the file only.
- **Endpoints and credentials are baked into release builds from CI secrets.** A build without them sends nothing.
  Release builds report `production`, anything else `development`.
- **Faro covers errors and console, usage events, and traces linked to Rust.** Web vitals are out. Frontend errors
  also reach `opai.log` through slice 6's log command, because Faro only sends to Grafana.
- **Divergence from the Go application, deliberate:**
  - It exported to Grafana whatever the Analytics switch said; this honours the switch (from slice 5).
  - It exported error logs only; this adds metrics and traces.
  - It used Aptabase for frontend events; Faro replaces it.
- **`imaging` stays silent.** Its callers in `opai` instrument per pass or per run, never per tile.

## Decisions

### D1 - The rule: one record per outcome per unit of work

A **unit of work** is anything with its own opening record at `info`:
- initialization
- a dependency or model install
- a session build
- an enhancement
- an analysis
- an automatic analysis (whose only record is its conclusion)

The rules:

- **A unit closes exactly once**, with one of `finished`, `stopped` or `failed`.
- **The failure's origin records it.** The layer where a failure originates records it at `warn`, with the error.
- **An enclosing unit still closes as `failed`, with the error.** Its duration and subject are its own, and the spec
  requires a run's failure in the run's own record.
- **A record that describes the same unit's outcome a second time is a duplicate, and is removed:**
  - "step failed": a step is not a unit, and it has no opening record.
  - "chain refused" followed by "enhancement failed".
  - "a signal could not be read": the face signal *is* the "analysis" unit, which already closed.
  - `gui`'s repeat of any of these.

A failed model download inside an enhancement therefore writes three `warn` records:
- "installing a dependency failed" (the install)
- "session build failed" (the build that needed it)
- "enhancement failed" (the run)

Each has a different duration and subject, and in slice 4 each is a span of one trace.

*Alternative:* record only at the origin, and have enclosing units close without `warn`. Rejected, because a run's
failure would then be read by query as "no failed runs", and the spec requires the run's own outcome to say it failed.

*Alternative:* skip "session build failed" when its cause is an install that already recorded itself. Rejected,
because it leaves "building session" open with no close, which is the silence this slice exists to remove.

### D2 - Stops: `stopped` at `info`, with `reason`

Each bracket's error arm is split on the existing error variants:

- **`InferenceError::Cancelled` and `InitError::Stopped`** close as stopped, with `reason = "cancelled"`.
- **`InferenceError::Shutdown` and `InitError::Cancelled`** close as stopped, with `reason = "shutdown"`.

The classification is one `pub(crate)` method per error type, `stop_reason(&self) -> Option<&'static str>`, which
returns `None` for a real failure. It is crate-private, because no front end needs it: `gui` already matches on the
variants. That keeps it off the public API.

The records:
- "enhancement stopped", "analysis stopped" and "initialization stopped", each with `reason` and `duration`, plus
  `identity` where the unit has a photograph.
- The install's existing "an install stopped" moves from `debug` to `info`, and gains `reason` and `duration`. Its
  bracket opened at `info` ("installing a dependency"), and a bracket closes at the level it opened. `InitError::Cancelled`
  on an install stops being "installing a dependency failed".
- The session flight's `Err` arm closes as stopped when the build ended in a shutdown.

`info`, not `debug`, because the spec's account of a unit is at the default verbosity, and a run that opened in a
user's file must close there. The volume is bounded by runs, which the opening records already are.

### D3 - Initialization: `initialize` wraps `initialize_inner`

`Opai::initialize` becomes a timer and a record around a private `initialize_inner`, the same shape as `deps::install`.
The clock starts first, so `duration` covers everything, including the claim.

| Outcome | Level | Record |
|---|---|---|
| `Ok` | `info` | "initialized", with `name`, `providers` and **`duration`** |
| `AlreadyRunning` | `info` | "initialization refused: another process holds the claim", with `holder` where known |
| `InitError::Cancelled` / `Stopped` | `info` | "initialization stopped", with `reason` and `duration` (D2) |
| any other `Err` | `warn` | "initialization failed", with `name`, `duration` and `error` |

`AlreadyRunning` is `info`, because it is what `cli` answers when the GUI is open. It is a correct refusal, not a
fault.

`runtime::start` times the `dlopen` and the environment creation, and adds `duration` to "ONNX Runtime started".

### D4 - Session builds: recorded inside the flight, where the duration already is

The flight's closure in `SessionCache::get_or_build` (`sessions/resident.rs`) already measures `started` and holds
the two success records. Its empty `Err(_)` arm becomes:

- **stopped** (`info`, "the build stopped", with `reason = "shutdown"`) when the error is a shutdown.
- **failed** (`warn`, "session build failed", with `artifact`, `provider`, `requested`, `duration` and `error`)
  otherwise.

The cache is generic in its error `E`, which defaults to `SessionError`, and every use takes the default, the tests
included. So the `impl<S, E: Clone>` block holding `get_or_build` narrows to `impl<S> SessionCache<S, SessionError>`.
That gives the arm D2's `stop_reason()` and `Display` with no new trait. If a second error type is ever needed, a
bound can be added then.

**Ownership and lifetime are unchanged.** The record is written inside `flight.cell.get_or_init`, the one closure a
single build runs, however many requests wait on it. Ten requests for a model that fails to build write one
"session build failed", exactly as they write one "building session" today. No handle, `Arc` or lease is touched. The
record reads only the `artifact`, `resolved` and `requested` values the closure already captures.

The CPU fallback's `warn` in `sessions/mod.rs` keeps `artifact` and `provider`, and **drops `error`**. The failed
build's record, written just before it inside the same request, carries the error. The fallback record now says only
what it decides: that the CPU is being tried instead.

### D5 - Enhancement: plan before the bracket, and the failing step on the run's record

- **Planning moves before the opening record.** `plan::<B>(operations)?` moves from `run` into `process`, ahead of
  "enhancement started". Planning is a scan of values already in memory, and its placement inside `run` served
  nothing. A refused chain then writes "chain refused" (`warn`, already there) and nothing else.
- **`run` reports which step failed.** It returns a crate-private `RunFailure { error, step: Option<(String, usize)> }`
  in place of a bare `InferenceError`. `step` is the failing operation's cache tag and index. It is `None` for
  failures outside a step, such as a store read abandoned by shutdown. `process` records from it, then returns
  `error`.
- **"step failed" is removed.** "enhancement failed" gains `operation` and `index`, when `step` is present.
- **The opening record gains `provider`** (the requested one). The closing records gain `identity`.

*Alternative:* keep `InferenceError` as `run`'s error, and have the step arm set a slot that `process` reads.
Rejected, because a side channel is a second way for the two to disagree. The return type already exists to carry
the outcome.

### D6 - Analysis: the refusal is recorded, and the bracket closes three ways

- **The refusal is recorded.** `operation.pipeline::<B>()` gains an `inspect_err` that writes "analysis refused"
  (`warn`, with `id` and `error`). It stays before "analysis started", so a refusal is one record.
- **The bracket follows D2 and D5:**
  - The opening record gains `provider`.
  - The closing records gain `identity`.
  - The error arm splits into "analysis stopped" and "analysis failed".

### D7 - Autopilot: the conclusion carries the incompleteness, and a withdrawal is recorded

- **`record_unread` and its `warn` are removed.** The face detection runs through `execute`, whose "analysis failed"
  already recorded the error at `warn`.
- **The conclusion record gains fields.** "analysis suggested enhancements" gains `incomplete` (the signal's name,
  `face`, the only signal that can be unread today) and `error` whenever `Suggestions::incomplete` is set. It stays at
  `info`. So `autopilot`'s requirement, a record naming the unread signal and its reason, is met by the conclusion.
  That requirement is unchanged, and so is its spec.
- **`suggest` wraps `suggest_inner`, like D3.** The wrapper measures `duration` and records every outcome:
  - `Ok`: the conclusion (moved out of the body, including its early return for an empty image or an empty scope),
    now with `duration`.
  - A stop: "automatic analysis stopped" (`info`, with `identity`, `reason` and `duration`). Today a withdrawn
    analysis records nothing.
  - Any other error: "automatic analysis failed" (`warn`). No path produces one today, but the wrapper is total, so a
    future one is not silent.

### D8 - Image IO: one record at each public blocking boundary

These functions each record a failure once, through an `inspect_err` at the top of their own body:
- `load_blocking`, `save_blocking` and `encode_blocking`
- `probe_blocking`, `probe_raw_blocking` and `identity_blocking`

The async twins only hop threads and call the blocking functions, so they are covered by construction. The one error
they add, `ImageIoError::Cancelled`, is a shutdown and is correctly not recorded.

| Operation | Message | Fields |
|---|---|---|
| load / probe / probe_raw / identity | "an image could not be read" | `op`, `path`, `error` |
| save | "an image could not be written" | `op`, `path`, `format`, `error` |
| encode | "an image could not be encoded" | `op`, `format`, `error` |

All at `warn`. Success writes nothing, as the spec requires. Slice 4 gives these operations spans, which carry their
duration.

*Alternative:* record in the `ImageIoError` constructors (`read`, `decode`, `codec`, ...). Rejected, for two reasons.
A constructor does not know which operation it is serving. And it would fire for errors that are then handled
internally.

### D9 - Levels

- **Unpublished model.** `deps/model/mod.rs`'s refusal of an unpublished model moves from `error!` to `warn!`. The
  divergence its comment defends, refusing rather than downloading unverified, is about the *behaviour*, which is
  unchanged. The level follows D1: a failure returned to the caller.
- **No run cache.** `setup.rs`'s `CacheMode::None` moves from `error!` to `warn!`. The application continues, and the
  spec's rule for a degradation is `warn`.
- **The level rule is written down in `logging`'s module header**, which already documents the file format. Each
  module's header then points there instead of restating it.

### D10 - `gui`: repeats removed

Each record below repeats a failure `opai` now records, and is removed. The code around it stays: the `map_err` or
match arm still turns the failure into the error the window is sent. Only the `tracing` call goes.

| File | Records |
|---|---|
| `setup.rs` | "initialization failed" |
| `enhance/run.rs` | "an opened image could not be read for an enhancement", "an enhancement failed" |
| `faces.rs` | "... read for a detection", "detecting the faces in a photograph failed" |
| `autopilot.rs` | "... read for an analysis", "an analysis was incomplete…", "analysing a photograph failed" |
| `export/run.rs` | "... read for an export", "an export's chain failed", and `unwritable` on the `opai::image::save` path only |
| `images/serve.rs` | "an opened image could not be read", "... could not be encoded for the window" |
| `images/files.rs` | "the image's dimensions could not be read", "the file could not be read, so it has no identity" |

`export/run.rs`'s `unwritable` closure serves three failures. Claiming the destination and committing it are `gui`'s
own failures and keep their `error` record. Only the save between them is `opai`'s, so its arm builds the same
`ExportError::Write` without recording.

*Alternative:* demote each to `debug`, keeping `gui`'s `run` id beside the failure. Rejected by the repository owner.
The file keeps `info` and above by default, so a `debug` record only appears when a user was asked for more detail, and
then it repeats the error text `opai`'s record already carries. The `run` id is the one thing lost, and only until
slice 5. Its command wrapper opens a span per command carrying `run`, and since slice 2 the file writes a span's fields
onto every record inside it, so `opai`'s own record will carry `run=` by itself.

No logic moves, and no front end gains a decision `opai` should own.

### Division across crates

- **`opai`:** every record added, moved, merged or re-levelled. The classification of stops.
- **`gui`:** 15 records removed, and one closure split. Nothing new.
- **`cli` and `perf`:** no change. They inherit the new `opai` records through the subscriber they already install.

### `rust-sak` and crates

No `rust-sak` feature is added or needed. `o11y`'s bridge carries whatever records `opai` writes. No crate is
introduced.

## Risks / Trade-offs

- **[Risk] A dashboard or query keyed on a record's text breaks.** "step failed" and "a signal could not be read" go
  away, and "enhancement failed" no longer fires for cancellations. → No released build sends anything before slice
  5, so no dashboard can depend on these yet. The file's readers are people, and the new records say more.
- **[Risk] Image IO `warn`s for a folder of unsupported files.** Describing 200 RAW files of an undecodable format
  writes 200 warnings. → Bounded by files the user chose, as `gui`'s existing warnings already are. Each is a file the
  user will see fail. The spec's volume rule is about image size, not file count.
- **[Trade-off] Three `warn`s for one failed download inside an enhancement.** → Deliberate (D1). They are three units
  with three durations, and slice 4 nests them in one trace. A reader or query counting *runs* that failed filters on
  "enhancement failed".
- **[Trade-off] The session cache's build path loses its genericity in the error.** → No caller used it. Recording
  outside the flight instead, which would keep the genericity, would write once per waiting request, which the spec
  forbids.
- **[Risk] Removing a `gui` record leaves a failure `opai` does not actually record.** → Every removed record is
  paired, in D10's table and the tasks, with the `opai` record that now covers it, and each pairing has a test on the
  `opai` side. Removals that are not repeats, such as the export's claim and commit, are excluded.
- **[Trade-off] No `run` id beside a failure until slice 5.** → A debug file could have tied a window request to
  `opai`'s record. `opai`'s record already names the photograph, the operation and the error, and slice 5's command
  span puts `run` back onto it.

## Migration Plan

1. `opai`: the stop classification, then each bracket (initialize, install, session build, enhancement, analysis,
   autopilot), then image IO and levels. Each step comes with its tests updated beside the code.
2. `gui`: the removals, once the `opai` records they defer to exist.
3. Validation:
   - Unit tests assert on the rendered lines through `records_of`, as the existing log tests do. They cover every new
     record, every removed record's absence, and a stop's absence of `warn`.
   - A manual check on macOS (CoreML and CPU), handed to the owner per the no-GUI-driving rule:
     - cancel an enhancement by dragging a slider
     - open a non-image file
     - export to a read-only folder
     - confirm with `RUST_LOG=info` that each failure appears once at `warn`, and each cancellation once at `info`.
   - Windows and Linux are covered by CI's test run. Nothing here is platform- or provider-specific. CUDA and TensorRT
     are untested by hand. Their build failures take the same `Err` arm as CoreML's, which the fake backend drives.

**Rollback:** revert the slice's commits. No API, file format or dependency changes.
