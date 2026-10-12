## Context

Every record in this workspace goes through `tracing`:
- `opai`'s own records.
- `gui`'s, which emits records but deliberately does not configure a subscriber.
- ONNX Runtime's C++ diagnostics, which `ort`'s `tracing` feature turns into events under target
  `ort`. Each runtime record is wrapped in its own TRACE-level `ort` span, carrying `id` and
  `location` and given as the event's explicit parent.

`opai::logging` installs one monolithic `fmt` subscriber that writes `opai.log`.

`rust-sak`'s `o11y` module (tag `26.9.8`) already exports logs, metrics and traces over OTLP/HTTP on
its own thread. It adds resource enrichment (service, version, environment, a scoped machine id and a
session id), has a single enable switch, and never blocks a caller. Three gaps stop a `tracing` layer
from feeding it; see proposal.md - Why:

- **Log records** carry fields fixed at compile time. `log::__emit` exists but is plumbing, and the
  `Fields` alias it takes is `pub(super)`.
- **Spans** are `SpanState`s on a thread-local `STACK`. A `Span` guard pushes and pops one; `Instrumented`
  lifts it off between polls. No span can live independently of a thread, and none can be given a
  parent other than the top of the calling thread's stack.
- **Span records** carry no links and no status, so the export has no `links` and no
  `status.code`. `Current::set_error` records an `exception` event and nothing more.

## Goals / Non-Goals

**Goals:**

- Every piece of `rust-sak` the seven-slice series needs lands in **one** release, so the series
  bumps the pin once.
- A `tracing` layer that turns this workspace's records into `o11y` records with correct trace
  parentage across threads. Correct means the parent comes from `tracing`'s span tree, not from
  whichever thread happened to emit.
- The bridge knows nothing about this application. Which fields are sensitive, and which spans are
  only context carriers, are hooks the consumer fills in.

**Non-Goals:**

- Turning telemetry on or off after `init`. The series settled on "opt-out calls `o11y::shutdown()`,
  opt-in takes effect at the next launch", which today's API already supports.
- Metrics from `tracing`. Metrics stay `o11y::metric` instruments called directly (slice 4); there is
  no `tracing` convention for them worth bridging.
- Sampling. Nothing in the series is high-volume enough to need it once slice 3 has bounded records
  per unit of work.
- Letting `o11y`'s macros see an owned span as "current". An owned span is not pushed onto the
  thread stack, and the bridge never needs it to be.

## Slices

Full observability, in order. Only the current slice is proposed; the rest are proposed when they are
reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-observability-1-o11y-bridge` | `rust-sak` `o11y`: runtime log emit, owned spans with explicit parents, span context + `traceparent`, links, status, and the `tracing` bridge behind `o11y-tracing`; released; pins bumped in `opai`, `gui` and `perf` | - | **current** |
| 2 | `add-observability-2-telemetry-seam` | `opai`: the subscriber becomes a registry with a file layer and a Grafana layer (each with its own filter); `opai::telemetry` start/stop; path fields stripped from the Grafana side; span context carried through `task::spawn_blocking`; endpoint, auth and environment baked in via `option_env!` from CI secrets; `logging` spec updated | 1 | pending |
| 3 | `add-observability-3-record-cleanup` | `opai`: the existing records fixed — silent failures recorded (initialize, session build, image decode/encode, analysis refusal), duplicates removed, missing fields and durations added, cancellations split from failures, levels corrected | 2 | pending |
| 4 | `add-observability-4-spans-metrics` | `opai`: spans on every unit of work (initialize, install, session acquire/build, enhancement and steps, analysis, autopilot, image IO, cache) and the metric instruments (durations, cache hit/miss, provider fallback, failures by kind, resident sessions) | 3 | pending |
| 5 | `add-observability-5-gui-switch` | `gui`: the Analytics setting mirrored into a Rust-owned file and read at startup; opting out stops sending at once; one wrapper that logs every command's error and opens its span; app exit recorded; the `eprintln!` in `single_instance.rs` fixed; CI secrets wired | 4 | pending |
| 6 | `add-observability-6-frontend-log` | frontend: a log command into `opai.log`; an ErrorBoundary and global `error`/`unhandledrejection` handlers; the errors swallowed today reported | 5 | pending |
| 7 | `add-observability-7-faro` | frontend: Grafana Faro for errors and console, usage events and traces, with a `traceparent` passed into the Tauri commands so frontend and backend spans form one trace; CSP opened for the collector; paused and resumed on the Analytics switch | 5, 6 | pending |

### Decisions already taken for slices 2 to 7

Settled with the repository owner during exploration, and repeated in every slice's design.md.

- **`tracing` stays the one logging API.** Grafana is a second layer beside the file, fed by this
  slice's bridge. There is no second set of `o11y::log` calls.
- **Analytics defaults to on and sends from the first launch.** Opting out stops sending at once
  (`o11y::shutdown()` and `faro.pause()`). Opting in takes effect at the next launch for the backend;
  Faro may resume at once.
- **Path fields never reach Grafana.** File and directory paths are stripped on the Grafana side;
  `opai.log` keeps them in full. The Analytics description promises no personal data.
- **ONNX Runtime warnings and above go to Grafana too**, with the same floor as the file.
- **Only the GUI exports.** `cli` and `perf` keep writing the file only.
- **Endpoints and credentials are baked into release builds from CI secrets.** A build without them
  sends nothing. Release builds report `production`, anything else `development`.
- **Faro covers errors and console, usage events, and traces linked to Rust.** Web vitals are out.
  Frontend errors also reach `opai.log` through slice 6's log command, because Faro only sends to
  Grafana.
- **Divergence from the Go application, deliberate:**
  - It exported to Grafana whatever the Analytics switch said; this honours the switch.
  - It exported error logs only; this adds metrics and traces.
  - It used Aptabase for frontend events; Faro replaces it.
- **`imaging` stays silent.** Its callers in `opai` instrument per pass or per run, never per tile.

## Decisions

### D1 - One release carries everything the series needs from `rust-sak`

`traceparent` (slice 7) and links (slice 4) are not needed by the bridge's first consumer. They are
in this slice anyway, because each `rust-sak` release costs a tag, bumps in three manifests, and a
cold-resolve check against a remote whose old tags have been deleted before. Both are small, and both
sit on types this slice has to make public anyway (`SpanContext`, the span record).

*Alternative:* ship only what slice 2 uses and release again later. Rejected, because it costs a
second release for about forty lines of code.

### D2 - A public runtime log emit, with an explicit-context variant

```
o11y::log::emit(level, message, fields)                  // context: this thread's o11y span stack
o11y::log::emit_in(context: SpanContext, level, message, fields)
o11y::log::Fields = Vec<(Cow<'static, str>, Value)>      // made public
o11y::log::enabled(level)                                // already public; documented as the pre-check
o11y::trace::enabled()                                   // replaces the hidden __enabled
```

Neither function checks the gate itself, and both must stay cheap when called after a
`log::enabled` check. The macros are re-pointed at `emit`, and `__emit` is removed: it was
`#[doc(hidden)]` and named only by the macros.

`emit_in` exists because the bridge knows its record's span from `tracing`'s context, which may have
no counterpart on the `o11y` thread stack. This happens when the event is emitted on a
`spawn_blocking` thread, or when `ort` emits from an intra-op thread under an explicit parent.

*Alternative:* one `emit` with an `Option<SpanContext>`. Rejected, because `None` would be ambiguous
between "use the stack" and "attach to nothing".

### D3 - Owned spans: opened with an explicit parent, ended anywhere

```
let span = o11y::trace::start(name, Parent::Current | Parent::Context(ctx) | Parent::Root, fields);
span.context() -> Option<SpanContext>    // None when tracing was off at start
span.set_attribute(k, v); span.add_event(..); span.add_link(ctx); span.set_error(&e);
span.end();                              // or drop
```

- **Holds its own state.** An `OwnedSpan` holds its `SpanState` behind a `Mutex` rather than on
  `STACK`, and is `Send + Sync`. That is what lets the bridge keep it in a `tracing` span's
  extensions, which any thread may touch.
- **Exported on end.** Ending it exports the same `SpanRecord` a guard would, with the same
  enrichment.
- **When tracing is off.** It is created as a no-op, and every method on it is silent, matching
  `Current`.
- **Never pushed onto `STACK`.** So a guard-based `span!` opened while an owned span is live does
  not parent to it. That is a documented non-goal: the bridge supplies parentage itself.

`Parent::Current` reads the top of the `o11y` stack, so an owned span opened inside a guard or
`Instrumented` span still nests under it.

*Alternative:* a way to push an owned span onto `STACK` for a scope. Rejected, because nothing in the
series needs it, and it reopens the thread-hop problem D3 exists to avoid.

### D4 - A public `SpanContext`, with W3C `traceparent`

`SpanContext { trace_id: [u8; 16], span_id: [u8; 8] }`, which is `Copy`, with:
- **Hex accessors:** `trace_id_hex()` and `span_id_hex()`.
- **`to_traceparent()`:** always produces `00-<32 hex>-<16 hex>-01`.
- **`from_traceparent(&str) -> Option<Self>`:**
  - It accepts any two-hex-digit version other than `ff`, as W3C requires of a parser.
  - It rejects all-zero ids and malformed lengths.
  - It ignores the flags.
- **`trace::current().context()`** returns the innermost stack span's context.

`TraceId` and `SpanId` stay internal; the context is the public face of both. There is deliberately no
public constructor from raw bytes: a context is either read from a live span or parsed from a
`traceparent`, which is all the series needs, and the parser is where the all-zero rule is enforced.

### D5 - Links and status on the span record

`SpanRecord` gains `links: Vec<SpanContext>` and a status that is unset or `Error(message)`. The OTLP
encoder writes these as `links: [{traceId, spanId}]` and `status: {code: 2, message}`, and writes
nothing for an unset status, so today's spans export byte-for-byte as they do now.

`set_error`, on both `OwnedSpan` and the existing `Current`, now sets the status as well as recording
the `exception` event. Only the second half is new on `Current`. It is a behaviour change to an
existing public method, recorded in the README. It is also the meaning every trace backend gives
"this span failed", which an event alone does not carry.

### D6 - The `tracing` bridge

Behind a new feature, `o11y-tracing = ["o11y", "dep:tracing", "dep:tracing-subscriber"]`, so an
`o11y` user who does not use `tracing` compiles none of it.

The public surface is `o11y::tracing::layer() -> TracingLayer`, a `tracing_subscriber::Layer<S>` for
any `S: Subscriber + for<'a> LookupSpan<'a>`, with two builder hooks:

- **`.map_field(|name, value| -> Option<Value>)`** rewrites or drops each field of an event or span
  before it leaves the process. Slice 2 uses it to strip paths.
- **`.fold_spans(|metadata| -> bool)`** marks spans that are not exported. Their fields are copied
  onto the events inside them, and their children parent to the nearest exported ancestor. Slice 2
  uses it for `ort`'s per-record span: without it, every runtime diagnostic would become a
  zero-length span in Tempo, and its `location` would be missing from the log line.

**Mapping:**

| `tracing` | `o11y` |
|---|---|
| `ERROR` / `WARN` / `INFO` | `Error` / `Warn` / `Info` |
| `DEBUG`, `TRACE` | `Debug` (`o11y` has no finer level) |
| event `message` field | log body; an event with none uses its callsite name |
| other event fields | log fields, plus `target` = the event's target |
| `i64`/`u64`/`f64`/`bool`/`&str` fields | the matching `Value` variant directly; `u64` above `i64::MAX` and `record_debug` values become strings |
| event parent (explicit, else contextual) | walk up to the nearest non-folded span holding an `OwnedSpan`; `emit_in` its context, or `emit` if there is none (so an `o11y` guard span still correlates) |
| `on_new_span` | `trace::start` with the nearest exported ancestor's context as parent, else `Parent::Current`; stored in the span's extensions |
| `on_record` | attributes; a field named `error` also sets the status |
| `on_follows_from` | `add_link` |
| an `ERROR` event inside a span | sets the nearest exported span's status |
| `on_close` | `end` |

**Rules:**

- **Gating.** Every hook checks `log::enabled(level)` or `trace::enabled()` first and does nothing
  more when the gate is shut. After `o11y::shutdown()` the layer costs one atomic load per callsite.
- **Filtering belongs to the consumer.** It is set as a per-layer `Filter`, the way slice 2 will
  give the Grafana layer a different filter from the file layer.
- **Span duration** is from creation to close, the wall-clock life of the unit of work, not its busy
  time. That is what a trace view is expected to show.
- **Nothing in the layer may panic.** It runs inside `ort`'s logging callback, on threads under
  `extern "system"` frames that cannot unwind. This is the same rule `opai`'s file formatter already
  obeys. Every lock recovers from poisoning, and a missing extension is skipped.

### D7 - Why not `tracing-opentelemetry`

`tracing-opentelemetry` plus `opentelemetry-otlp` is the ecosystem's stock answer, and it would need
none of D2 to D5. It is rejected for three reasons:

- **It duplicates `o11y`.** It brings its own exporter, batch processor and resource model. That is
  a third-party stack duplicating a `rust-sak` module, which the project's rules forbid.
- **Its resource model does not match `o11y`'s** scoped machine id and session id. Grafana would then
  receive two differently-shaped resources from one application.
- **It needs more than `o11y` does.** Its batch processor wants an async runtime or its own threads,
  and it adds a dependency tree the size of `opentelemetry_sdk`. `o11y`'s whole premise is "no async
  runtime, one thread".

### D8 - The bridge lives in `rust-sak`, not in `opai`

Decided by the repository owner. Forwarding `tracing` to `o11y` has nothing to do with photographs:
any first-party program that logs through `tracing` and wants `o11y`'s exporter needs the same
layer. Kept in `opai`, it would need D2 to D5 public in `rust-sak` anyway, and would sit one crate
away from the internals it depends on.

### D9 - Crates introduced, and `rust-sak` features used

**In `rust-sak`, behind `o11y-tracing` only:**
- `tracing` 0.1.
- `tracing-subscriber` 0.3, with `default-features = false` and the `registry` and `std` features,
  for `Layer`, `LookupSpan` and span extensions.

These are the smallest set that names a `Layer`. Both are already in this workspace through `opai`
(`tracing` 0.1.44, `tracing-subscriber` 0.3.23) and `ort`, so adopting the feature in slice 2
compiles nothing new here.

**In this repository, no feature changes in this slice:**
- `opai` keeps its current features.
- `gui` keeps `crypto`, `github` and `image`.
- `perf` keeps `fs`.

Slice 2 adds `o11y-tracing` to `opai`, the only crate that installs a subscriber. `gui` never names
the feature: it reaches telemetry through `opai`'s API.

### D10 - Release shape

The version moves to the next CalVer number for the release month (`26.9.9` if released in September
2026), tagged at the pushed commit, with the owner's go-ahead before the push. Before bumping here,
`git ls-remote --tags` confirms the tag is on the remote.

This repository then bumps **three** pins together: `opai`, `gui` and `perf`, since `gui` has pinned
`rust-sak` directly since the update-check slice. The lock file is refreshed with `cargo metadata`
rather than `cargo update -p`, so it holds exactly one `rust-sak` entry, and the result is proved to
resolve from a cold cargo home.

## Risks / Trade-offs

- **[Risk] Two span models in one process could mis-nest.** An `o11y` guard span and a `tracing`
  span may coexist. → The bridge only ever parents to `tracing` ancestors, falling back to the `o11y`
  stack only when `tracing` has none. Tests cover both orders.
- **[Risk] A `tracing` span that is never closed never exports.** For example, a span held by a
  leaked future. → Accepted: that is `tracing`'s own semantics, and the file log is unaffected.
  Dropping an `Instrumented` future still closes its span.
- **[Risk] Mapping `TRACE` onto `Debug` loses a level.** → The consumer's per-layer filter decides
  what reaches the bridge. Slice 2 floors the Grafana layer at `info`, so the collapse is never seen.
- **[Risk] The layer's cost lands on hot paths.** The records live beside inference. → A shut gate
  costs one relaxed load. A kept record costs the same bounded-queue push `o11y`'s macros already
  pay. Slices 3 and 4 keep records off per-tile loops.
- **[Trade-off] `set_error` on `Current` now also sets status.** A caller relying on "event only"
  would see spans marked failed. → No consumer of `rust-sak` calls it today. It is recorded in the
  README.
- **[Risk] A re-recorded field duplicated a span attribute.** Found at verification, after `26.9.9` was
  tagged: `set_attribute` appended, so a `tracing` field recorded twice exported the key twice,
  which OTLP forbids. → Fixed in `rust-sak` after `26.9.9` (replace-or-push on
  `OwnedSpan` and `Current` alike). It reaches this workspace with the next pin bump, which slice 2
  makes anyway, and nothing records a span field twice before slice 4 instruments units of work.
- **[Risk] A pin that resolves only from the local cache.** `rust-sak`'s old tags have been deleted
  from the remote before. → A cold-home resolve is part of the verification (task 4.1).

## Migration Plan

1. Implement in `../rust-sak`, with tests beside the code, and release under the owner's go-ahead.
2. Bump the three pins here with no feature or source change, and run the full workspace suite.

**Rollback:** revert the three pin lines and the lock file. Nothing in this repository calls the new
API until slice 2.

**Validation:**
- `rust-sak`'s CI runs Linux only, and so does its per-feature `cargo check` matrix, which gains
  `o11y-tracing`.
- This repository's CI builds and tests on Linux, macOS and Windows, which covers the pin bump on all
  three.
- The bridge is platform-independent Rust with no FFI, so no execution provider is involved.
- The end-to-end export test runs against `rust-sak`'s existing local OTLP test collector, not a real
  Grafana. The first real Grafana check is slice 2's.
