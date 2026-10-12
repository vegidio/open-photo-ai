## Context

Slice 3 left every unit of work in `opai` closing in one place, with one `match` over finished, stopped and failed:
- `Opai::recorded` (initialization)
- `deps::install`'s `record_ended`
- the session flight's closure in `SessionCache::get_or_build`
- `process::process` (enhancement)
- `execute::execute` (analysis)
- `autopilot::suggest`
- the six image IO boundaries (`image/error.rs`'s `unreadable`, `unwritable`, `unencodable`)
- the run cache's reads and writes in `cache.rs`

Each arm already holds the duration, the subject and the error. That is where this slice's spans end and its metrics
are recorded.

The facts that shape the approach:

- **The file writes span fields onto records.** `logging::format`'s `Line` appends every enclosing span's formatted
  fields to each event, outermost first. A span on a unit whose records already name its subject would write
  `identity=X` twice on one line, and would add fields to every record inside it, ONNX Runtime's included.
- **Each layer has its own filter.** The file layer is filtered by `RUST_LOG`, else `info,ort=warn`, widened for
  `ort`'s record span. The collector layer is filtered by a fixed `info,ort=warn,opai::telemetry::export=off`, widened
  the same way. A span disabled by one layer's filter is invisible to that layer, while the other still sees it.
- **One span exists today.** It is `session_build` in `sessions/build.rs`, at `info` under that module's target. It
  decorates ONNX Runtime's build-time diagnostics in the file with `artifact` and `provider`, and has reached the
  collector since slice 2.
- **Blocking work already carries its span.** `task::spawn_blocking` enters the caller's span on the blocking thread
  (slice 2). Image IO, session builds, cache reads and a step's pipeline all run there.
- **What `rust-sak` `26.9.10` covers.** Its bridge turns `tracing` spans into `o11y` spans parented by `tracing`'s own
  tree. `on_follows_from` becomes a link, and a span field named `error` sets the span's status to failed. Metric
  instruments (`Counter`, `Gauge`, `Histogram`) are ungated atomics, exported cumulatively once `o11y` has started, with
  1024 series per instrument. Nothing public reads an instrument's value back: `metric::snapshot` is `pub(crate)`.
- **Cache tags carry parameters.** `Operation::cache_tag` spells a strength or a scale (`dn-stockholm-fp32-t40`), so it
  is not a bounded value.
- **There is one session cache per `Opai`.** A process running the application has one. The test suite has many.

## Goals / Non-Goals

**Goals:**

- One enhancement is one trace: the run, its steps, and beneath each step the session request, the build, the install
  and the cache access. It is correctly parented across the blocking hop.
- `opai.log` is byte-for-byte unchanged. The existing format, session and bracket tests prove it by passing unedited.
- Every metric tag comes from a fixed set that the code enumerates.
- Every span ends, with one outcome, in the arm that already writes the closing record.

**Non-Goals:**

- Parenting these units under the front end's request. That is `gui`'s command span, in slice 5.
- Spans inside `imaging`, or per pass, tile or region. The series decided `imaging` stays silent.
- Gating span creation on whether telemetry runs. See Risks.
- Scrubbing span attributes. Slice 2's D8 stands, so a span carries what its records carry.
- A metric per image IO operation beyond its duration and failures, or histograms of image size.

## Slices

Full observability, in order. Only the current slice is proposed; the rest are proposed when they are reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-observability-1-o11y-bridge` | `rust-sak` `o11y`: runtime log emit, owned spans with explicit parents, span context + `traceparent`, links, status, and the `tracing` bridge behind `o11y-tracing`; released as `26.9.9`; pins bumped in `opai`, `gui` and `perf` | - | done |
| 2 | `add-observability-2-telemetry-seam` | `rust-sak`: the `set_attribute` fix released as `26.9.10`. `opai`: the subscriber becomes a registry with a file layer and a Grafana layer (each with its own filter); `opai::telemetry` start/stop; export failures recorded file-only; span context carried through `task::spawn_blocking`; endpoint and auth baked in via `option_env!`; `logging` spec updated. `gui`: start in `setup`, stop on exit | 1 | done |
| 3 | `add-observability-3-record-cleanup` | `opai`: the existing records fixed. Silent failures recorded (initialize, session build, image IO, analysis refusal); each failure recorded once (step folded into run, refusal outside the bracket, unread signal folded into the conclusion); cancellations and shutdowns recorded as stops; durations and subjects added to closing records; `error` reserved for panics. `gui`: its repeats of `opai`'s failures removed | 2 | done |
| 4 | `add-observability-4-spans-metrics` | `rust-sak`: a public read-back on metric instruments, released. `opai`: spans on every unit of work (initialize and the runtime load, install, session request and build, enhancement and steps, analysis, automatic analysis, image IO, cache), sent to the collector only; waiters linked to the build they joined; outcome and failed status on every span; the metric instruments (unit durations, step durations, cache lookups, provider fallbacks, failures by kind, resident sessions) | 3 | **current** |
| 5 | `add-observability-5-gui-switch` | `gui`: the Analytics setting mirrored into a Rust-owned file and read at startup; opting out stops sending at once; one wrapper that logs every command's error and opens its span; app exit recorded; the `eprintln!` in `single_instance.rs` fixed; CI secrets wired | 4 | pending |
| 6 | `add-observability-6-frontend-log` | frontend: a log command into `opai.log`; an ErrorBoundary and global `error`/`unhandledrejection` handlers; the errors swallowed today reported | 5 | pending |
| 7 | `add-observability-7-faro` | frontend: Grafana Faro for errors and console, usage events and traces, with a `traceparent` passed into the Tauri commands so frontend and backend spans form one trace; CSP opened for the collector; paused and resumed on the Analytics switch | 5, 6 | pending |

Slice 4's row changed from the one planned in slices 1 to 3 in three ways:
- It gained the `rust-sak` read-back release. The owner chose to extend `rust-sak` so each instrument is unit-tested,
  rather than check metrics only through the export test.
- "session acquire/build" became a request span and a build span, linked. Slice 1 added links for exactly this.
- The spans reach the collector only, at the owner's choice (D2).

**What this leaves for slice 5:**
- Its command span must use `gui`'s own target, not `opai`'s unit target (D2). Then the file writes its `run` onto
  `opai`'s records, as slice 3 planned, and the collector parents these units under it.
- Its wrapper must not record a failure `opai` already recorded (slice 3's D1).

### Decisions already taken for slices 2 to 7

Settled with the repository owner during exploration, and repeated in every slice's design.md. One was revised in
slice 2 and is marked as such.

- **`tracing` stays the one logging API.** Grafana is a second layer beside the file, fed by slice 1's bridge. There is
  no second set of `o11y::log` calls.
- **Analytics defaults to on and sends from the first launch.** Opting out stops sending at once (`o11y::shutdown()`
  and `faro.pause()`). Opting in takes effect at the next launch for the backend; Faro may resume at once.
- ~~**Path fields never reach Grafana.**~~ **Revised in slice 2:** paths are not scrubbed. Records reach Grafana with
  their paths as `opai.log` has them. This slice's span attributes carry paths on the same terms. Metric tags never
  carry one (D6), for cardinality rather than privacy.
- **ONNX Runtime warnings and above go to Grafana too**, with the same floor as the file.
- **Only the GUI exports.** `cli` and `perf` keep writing the file only.
- **Endpoints and credentials are baked into release builds from CI secrets.** A build without them sends nothing.
  Release builds report `production`, anything else `development`.
- **Faro covers errors and console, usage events, and traces linked to Rust.** Web vitals are out. Frontend errors also
  reach `opai.log` through slice 6's log command, because Faro only sends to Grafana.
- **Divergence from the Go application, deliberate:**
  - It exported to Grafana whatever the Analytics switch said; this honours the switch (from slice 5).
  - It exported error logs only; this adds metrics and traces.
  - It used Aptabase for frontend events; Faro replaces it.
- **`imaging` stays silent.** Its callers in `opai` instrument per pass or per run, never per tile.

## Decisions

### D1 - `rust-sak`: a read-back on each instrument

```rust
Counter::value(&self, tags: &[(&str, &str)]) -> u64          // 0 for a series never written
Gauge::value(&self, tags: &[(&str, &str)]) -> Option<f64>    // None for a series never set
Histogram::count(&self, tags: &[(&str, &str)]) -> u64
Histogram::sum(&self, tags: &[(&str, &str)]) -> f64
```

- **Tags work as they do when writing.** `&[]` is the untagged series. Order does not matter, as it does not when
  writing. A tag set that overflowed into `o11y.series_overflow` reads as never written, which is documented.
- **Documented for tests and diagnostics.** The exporter keeps reading through the registry's snapshot, unchanged.
- **Belongs in `rust-sak`.** Any consumer that wants to test its own metrics needs it, and `opai` cannot build it
  from outside, since the state is private.
- **Released as `26.9.11`** (the CalVer number for September 2026), with the owner's go-ahead before the push. The
  tag is confirmed with `git ls-remote --tags`. The pin moves in `opai`, `gui` and `perf` together, the lock is
  refreshed with `cargo metadata`, and the result is proved with a cold-home `cargo fetch --locked`, as in slices 1
  and 2.

*Alternative:* assert on metrics only through `telemetry`'s in-process collector test. Rejected by the owner: it
proves the instruments are exported, not that a cache hit bumps the hit counter.

*Alternative:* an `opai`-local wrapper type with a test recorder. Rejected, because it is plumbing around a `rust-sak`
module to work around a gap in it, which the project's rules send back to `rust-sak`.

### D2 - Unit spans go to the collector only

Every span this slice adds is created under one target, `opai::unit` (`telemetry::unit::TARGET`), at `info`:

- **The file layer's filter excludes it.** The filter becomes `with_ort_record_span(filter).and(not unit span)`,
  where "unit span" is `metadata.is_span() && metadata.target() == TARGET`. Spans are then invisible to the file
  layer, so `Line`'s walk of an event's scope skips them. No field of theirs reaches a line, whatever `RUST_LOG` says,
  because the exclusion is joined outside the environment's filter, not written as a directive inside it.
- **The collector's filter already passes it.** Its fixed `info` covers every target.
- **`session_build` keeps its own target** and stays visible to both layers. It is the one context that decorated the
  file before this slice, and a user reading a CoreML warning still needs to know which model it was about. In a trace
  it sits as the build span's child, around the runtime's own session construction.
- **`gui` spans in slice 5 use `gui`'s target**, so they reach the file. That is the mechanism by which slice 3
  planned `run=` to appear on `opai`'s records.

This was the repository owner's choice.

*Alternative:* spans in both destinations, carrying only fields their records lack. Rejected, because every
enhancement's lines would change shape, and "no field twice" would be a rule kept by hand as records change.

*Alternative:* spans in both, accepting repeated keys. Rejected, because it makes the file noisier for its human
readers to buy nothing the collector needs.

### D3 - The unit helper: open with empty `outcome` and `error`, end in the closing arm

`telemetry::unit` (crate-private) holds:

```rust
pub(crate) const TARGET: &str = "opai::unit";
macro_rules! unit_span { ($name:literal $(, $fields:tt)*) =>
    tracing::info_span!(target: TARGET, $name, outcome = Empty, error = Empty $(, $fields)*) }

pub(crate) enum Unit { Initialize, RuntimeLoad, Install, SessionBuild, Enhancement, Analysis, AutoAnalysis,
                       ImageLoad, ImageSave, ImageEncode, ImageProbe, ImageProbeRaw, ImageIdentity }
pub(crate) enum Outcome<'a> { Finished, Stopped, Failed { kind: &'static str, error: &'a dyn Display } }

pub(crate) fn ended(unit: Unit, span: &Span, duration: Duration, outcome: Outcome<'_>);
```

`ended` does three things, and every closing arm slice 3 wrote calls it beside its record:
- It records `outcome` (`finished`, `stopped` or `failed`) on the span. It also records `error` when the unit failed,
  which the bridge turns into the span's failed status. A stop records no `error`, so it is never marked failed.
- It records the duration histogram (D6).
- It counts the failure by `kind` (D6).

The duration is the one the closing record already measured, so the record, the span and the metric agree.

**How a span is held.** An async unit wraps its body with `.instrument(span.clone())` and keeps the handle for
`ended`. A blocking unit enters its span with `in_scope`. The span closes when its last handle drops, just after
`ended`. The session request span and the step spans are the only units with no closing record of their own:
- A step ends with the run's `RunFailure` step index. Its span is marked by the step arm, which is D5's `in_step`
  closure.
- A request ends with the value it returns.

Neither writes a record, so slice 3's rule of one record per outcome is untouched.

*Alternative:* `#[tracing::instrument]` on each function. Rejected, because the attribute cannot record the outcome
the closing arm decides, and it would name the span after a private function rather than the unit.

### D4 - Which spans, where, and what each carries

| Unit | Span | Opened in | Fields beyond `outcome`/`error` |
|---|---|---|---|
| initialization | `initialize` | `Opai::recorded` | `name` |
| runtime load | `runtime_load` | `runtime::start` | `library` |
| install | `install` | `deps::install` | `dependency`, `version` |
| session request | `session_request` | `Sessions::session` (per request) | `artifact`, `requested`, `resident` |
| session build | `build` | the flight (D5) | `artifact`, `provider`, `requested` |
| enhancement | `enhancement` | `process::process` | `ids`, `identity`, `provider`, `depth` |
| step | `step` | `process::run`'s loop, per operation not served from the store | `operation`, `index`, `model` |
| analysis | `analysis` | `execute::execute` | `id`, `identity`, `provider` |
| automatic analysis | `automatic_analysis` | `autopilot::suggest` | `identity`, `asked` |
| image IO | `image_load` ... `image_identity` | each public `*_blocking` | `path` or `format` |
| cache read / write | `cache_read`, `cache_write` | `RunCache::get_bytes`, `put`, `get_value`, `put_value` | `key`, `hit` (reads) |

- **The build is two spans.** The new unit span is `build`. The existing module-target `session_build` keeps its name
  and its place in the file, and nests under `build`. Two spans for one build is
  the price of D2 keeping the file's decoration. The inner one covers only the runtime's session construction, and
  the outer one covers the install as well.
- **A step served from the store gets no step span.** Its cache read span says it was served. The step count in a
  trace is therefore the number of operations computed, which is what a trace is read for.
- **Refusals open no span.** A chain or analysis refused before its opening record writes one record and no span,
  matching slice 3's rule that a refusal is not a run that began.
- **The automatic analysis parents its face detection.** The detection runs through `execute`, so its `analysis` span
  is the automatic analysis's child with no extra wiring.
- **`model`** is the operation's catalogue name without run parameters: family, codename and precision, with the scale
  for an upscale whose scale selects the weights (`up_kyoto_x4_fp16`). A crate-private `Operation::model_tag()`
  composes it from what each family already knows. Two operations differing only in a parameter share it. It is never
  `cache_tag`.

### D5 - One build span, and links from every request that waited on it

The flight in `SessionCache::get_or_build` is the one place a build happens, however many requests join it. Here is
the ownership:

- **The request that creates a flight** creates the `build` span, as a child of its own `session_request` span. It
  stores the span's `tracing::Id` in the `Flight` and moves the `Span` handle into a `Mutex<Option<Span>>` on the
  flight.
- **Whichever request runs the cell's initializer** takes the handle out of that slot, instruments the build with it,
  and ends it in the closure's existing `match`. So the span closes when the build ends, not when the flight is
  dropped. The flight never holds a handle past the build, only an `Id`, which does not extend the span's life.
- **A build whose request is dropped part of the way puts the handle back.** The cell then hands the build to another
  waiting request, which takes the handle again and carries the build on under the same span. The trace still holds
  one build, which ends with an outcome, and the links already made to it still point at the build that happened.
- **Every request that joins an existing flight** calls `Span::current().follows_from(id)` on its request span before
  awaiting the cell. The bridge turns that into a link from the waiter's request to the build.

A waiter that joins after the build ended finds the cell initialised and does not wait, so it has nothing to link.
Nothing here touches a `SessionHandle`, an `Arc<Resident>` or a lease. Their lifetimes are exactly what slice 3 left.

*Alternative:* store the `Span` itself in the flight. Rejected, because the span would stay open until the flights
map dropped the entry, and its duration would stop meaning the build's.

### D6 - The instruments

Declared once in `telemetry::metrics` as `LazyLock` statics:

| Instrument | Kind | Tags | Recorded by |
|---|---|---|---|
| `opai_unit_duration_seconds` | histogram | `unit`, `outcome` | `unit::ended` |
| `opai_step_duration_seconds` | histogram | `model`, `provider` | a computed step, with the provider its sessions ran on, or `mixed` |
| `opai_cache_lookups_total` | counter | `kind` (`step`/`analysis`), `result` (`hit`/`miss`) | `process::run` and `execute` at the lookup |
| `opai_provider_fallbacks_total` | counter | `requested`: the provider attempted | the CPU-fallback arm in `sessions/mod.rs` |
| `opai_failures_total` | counter | `unit`, `kind` | `unit::ended` on `Failed` |
| `opai_resident_sessions` | gauge | `provider` | the session cache after every insert, release and sweep |

- **Buckets, in seconds:** `0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60, 120, 300, 600, 1800`.
  These cover a cached step at milliseconds and a TensorRT build or a model download at tens of minutes. `rust-sak`'s
  default bounds (0 to 10 000) suit milliseconds, not seconds.
- **`kind`** is a crate-private `kind(&self) -> &'static str` on `InferenceError`, `InitError`, `SessionError` and
  `ImageIoError`, one stable snake-case name per variant. A variant that wraps another error delegates to it, so
  a failure is counted by its root cause (`download`, `checksum`, `session_build`, `decode`), not by the layer it
  passed through. Stops never reach it.
- **A fallback is counted by the provider that was attempted**, which is the one the request resolved to. An `Auto`
  request counted as `Auto` would not say which accelerator declined. For an explicit request the two are the same.
- **The resident gauge** is set per provider from the cache's own entries, under the lock that mutated them. It
  writes every provider, zeros included, so a provider emptied by a sweep reads `0` rather than its last value.
  There is one cache per application process, so one writer.
- **Every tag value is enumerable.** Each is either an enum's `as_str`, an `ExecutionProvider`'s display, a
  `model_tag` from the catalogue, or one of the literals above. Tests pin the full set of `unit` and `kind` names.

Metrics are recorded in `cli` and `perf` too, at one atomic operation each, and never exported, because neither
starts telemetry. That is `o11y`'s documented behaviour for an instrument in a process that never initialises.

### D7 - Testing

- **Spans** are asserted with a test layer that records the spans it sees, their parents, links and recorded fields,
  under the collector's filter. It extends the recorder `logging::format`'s tests already use for that filter, which
  runs under the suite's recording lock. Tests cover:
  - The trace shape of a fake-backend enhancement that installs, builds and runs two steps.
  - A cancelled run's `stopped` spans with no `error`.
  - A failed step marking both its own span and the run's.
  - Two concurrent requests producing one `build` and one link.
  - The same enhancement over a small and a large image producing the same span count.
- **The file** is proved unchanged by the existing format, session and bracket tests passing unedited. One new test
  emits a record inside a unit span under `RUST_LOG=debug` and asserts that the line carries none of the span's fields.
- **Metrics** are asserted through D1's read-back, as the change across the call under test, under the same
  recording lock. Each instrument has one test beside the code that records it.
- **End to end:** `telemetry`'s existing in-process collector test gains assertions that a span tree and a metric
  arrive at `/v1/traces` and `/v1/metrics`.

### Division across crates

- **`opai`:** everything: the unit helper, the filter change, the spans, the links, the instruments and the kinds.
- **`gui` and `perf`:** the pin.
- **`cli`:** nothing.

### `rust-sak` and crates

This slice uses `o11y-tracing` (already enabled in `opai`) and `o11y::metric`. The one gap, reading a value back, is
filled in `rust-sak` (D1). No crate is introduced.

## Risks / Trade-offs

- **[Risk] Per-layer filtering does not hide a span from the file layer's event scope as expected.** →
  `tracing-subscriber` documents that a span disabled by a layer's filter is skipped by that layer's `Context` lookups.
  I have not yet seen it behave that way against `fmt::layer`'s `FmtContext::event_scope`. The new file test is the
  first thing task 4.1 writes. If it fails, the fallback is to skip spans with the unit target inside `Line`'s scope
  walk. That is one condition in one loop, and the owner's decision stays the same.
- **[Risk] Metric tests read process-global instruments while other tests run in parallel.** A delta could include
  another test's increment. → Metric tests hold the recording lock, and assert on tag sets their own fake backend
  produces. Where a tag set is shared with unlocked tests (the CPU provider), they assert `>=` rather than `==`, and
  say so.
- **[Trade-off] Spans are created in every binary, sending or not.** The collector layer is installed everywhere
  (slice 2's D1), so `tracing`'s registry allocates each unit span even in `cli`. → The count is bounded by units of
  work, and each costs a slab slot and the bridge's one atomic load. A filter that consulted `o11y`'s gate would make
  callsite interest dynamic, which costs every record and not only spans.
- **[Trade-off] Two spans per build in the collector** (`build` and the older `session_build`). → The inner one is
  what keeps the file's runtime diagnostics attributed (D2). Tempo shows it as the build's last phase, which is
  accurate.
- **[Risk] Cache-read spans during a slider drag.** Every cancelled run opens a read span per step. → They are bounded
  by operations per run, as the records are. A drag already cancels runs at the rate a user moves, and each span
  says `stopped` or `finished` honestly.
- **[Risk] A `rust-sak` tag that resolves only from the local cache.** Tags have been deleted from the remote before.
  → The cold-home fetch is part of the adoption task.

## Migration Plan

1. `rust-sak`: the read-back, its tests and README, the five gates, and the release under the owner's go-ahead. Then
   the three pins here.
2. `opai`: the unit helper and the file filter, proved against the unchanged file first. Then the instruments and
   kinds. Then each bracket in the order slice 3 used: initialization, install, sessions, enhancement, analysis,
   automatic analysis, image IO, cache.
3. Validation:
   - Unit tests on the fake backend, on all three CI platforms.
   - A real Grafana check on macOS, handed to the owner per the no-GUI-driving rule. It uses a `gui` debug build with
     the collector variables set, and runs one enhancement on CoreML and one on the CPU. Then Tempo is queried for
     each trace's shape and Mimir for the six instruments.
   - Windows and Linux are covered by CI only. CUDA and TensorRT are untested by hand. Their build and fallback arms
     are the same arms CoreML's take, and the fake backend drives them.

**Rollback:** revert this repository's commits. That returns the pin to `26.9.10`, and nothing in the read-back is
needed by anything else. The `rust-sak` tag stays.
