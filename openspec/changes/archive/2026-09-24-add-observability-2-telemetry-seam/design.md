## Context

`opai::logging::init` builds one subscriber: `tracing_subscriber::fmt()` with this project's `Line` formatter, the
`KeyValues` span-field formatter, a `RUST_LOG`-or-`info,ort=warn` `EnvFilter`, and the synchronous rotating `Sink`.
`format::subscriber` builds it, and the tests build it through the same function, so what they assert on is what
`init` installs. Three binaries call `init`: `gui`, `cli` and `perf`.

At `26.9.9`, `rust-sak` has the `o11y-tracing` bridge (slice 1). `o11y::tracing::layer()` is a
`tracing_subscriber::Layer` with `map_field` and `fold_spans` hooks, and every hook is gated by `o11y`'s own switch.
`o11y::init` can succeed once per process. `o11y::shutdown` is idempotent and blocks for at most one export timeout.
A fix made after the tag, which stops a re-recorded span field from exporting its key twice, is uncommitted in
`../rust-sak`.

All of `opai`'s blocking work goes through `task::spawn_blocking`, a wrapper over `tokio::task::spawn_blocking`.
`tracing`'s current span is thread-local, so a closure run there sees no span today. In the file, a record emitted in
that closure loses its span's fields. In Grafana, it would lose its trace.

**The file loses `ort`'s `id` and `location` today.** `ort` wraps each runtime record in
`span!(Level::TRACE, "ort", id, location)` under its module target `ort::logging`, then emits the event with that span
as explicit parent. The file's `ort=warn` directive disables that TRACE span, so the event arrives with no span to
read. The real file shows it: `level=WARN app=gui target=ort::logging msg="Some nodes were not assigned…"`, with no
`id=` or `location=`. The format tests miss it because they build the span with `warn_span!` and `info_span!` under
target `ort`, a shape `ort` never produces. The existing `logging` requirement "The inference runtime's diagnostics
are captured" is therefore not met. The Grafana layer's fold (D2) needs the same span enabled.

`gui` calls `logging::init` first in `run`, then builds Tauri with the single-instance plugin registered first. That
plugin's setup calls `std::process::exit(0)` on a second launch. `gui` ends with `.run(generate_context!())`, which
has no exit hook.

The Go application created its telemetry in `main`. It used service name `opai`, the version, an `Authorization`
header, and an environment. CI string-replaced `<otel_endpoint>`, `<otel_auth>` and `EnvDevelopment` from the
`OTEL_ENDPOINT` and `OTEL_HEADER_AUTH` secrets. It had no geolocation, and it exported whatever the Analytics switch
said.

## Goals / Non-Goals

**Goals:**

- `opai.log` stays byte-for-byte what it is, proved by the existing format tests passing unchanged against the new
  composition. There is one intended exception: `ort` records regain the `id` and `location` the spec already requires
  (D2).
- One subscriber in every binary. Whether Grafana receives anything is decided by `telemetry::start`, not by which
  subscriber was installed.
- A build without credentials behaves exactly like today's, with no socket, no thread and no error.

**Non-Goals:**

- Honouring the Analytics switch (slice 5).
- Changing which records exist or at what level (slice 3), and adding spans or metrics (slice 4).
- Scrubbing anything from what is sent. See D8.
- Restarting telemetry after `stop`. `o11y` refuses a second `init`, and the series settled on "opt-in takes effect
  at the next launch".
- Sending to Grafana when the file cannot be opened. See Risks.

## Slices

Full observability, in order. Only the current slice is proposed; the rest are proposed when they are reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-observability-1-o11y-bridge` | `rust-sak` `o11y`: runtime log emit, owned spans with explicit parents, span context + `traceparent`, links, status, and the `tracing` bridge behind `o11y-tracing`; released as `26.9.9`; pins bumped in `opai`, `gui` and `perf` | - | done |
| 2 | `add-observability-2-telemetry-seam` | `rust-sak`: the `set_attribute` fix released. `opai`: the subscriber becomes a registry with a file layer and a Grafana layer (each with its own filter); `opai::telemetry` start/stop; export failures recorded file-only; span context carried through `task::spawn_blocking`; endpoint and auth baked in via `option_env!`; `logging` spec updated. `gui`: start in `setup`, stop on exit | 1 | **current** |
| 3 | `add-observability-3-record-cleanup` | `opai`: the existing records fixed. Silent failures recorded (initialize, session build, image decode/encode, analysis refusal), duplicates removed, missing fields and durations added, cancellations split from failures, levels corrected | 2 | pending |
| 4 | `add-observability-4-spans-metrics` | `opai`: spans on every unit of work (initialize, install, session acquire/build, enhancement and steps, analysis, autopilot, image IO, cache) and the metric instruments (durations, cache hit/miss, provider fallback, failures by kind, resident sessions) | 3 | pending |
| 5 | `add-observability-5-gui-switch` | `gui`: the Analytics setting mirrored into a Rust-owned file and read at startup; opting out stops sending at once; one wrapper that logs every command's error and opens its span; app exit recorded; the `eprintln!` in `single_instance.rs` fixed; CI secrets wired | 4 | pending |
| 6 | `add-observability-6-frontend-log` | frontend: a log command into `opai.log`; an ErrorBoundary and global `error`/`unhandledrejection` handlers; the errors swallowed today reported | 5 | pending |
| 7 | `add-observability-7-faro` | frontend: Grafana Faro for errors and console, usage events and traces, with a `traceparent` passed into the Tauri commands so frontend and backend spans form one trace; CSP opened for the collector; paused and resumed on the Analytics switch | 5, 6 | pending |

Slice 2's row changed from the one planned in slice 1 in three ways:
- It gained the `set_attribute` release, which slice 1 deferred to "the next pin bump".
- It gained `gui`'s start and stop, which the repository owner chose to wire now rather than leave `start` without a
  caller until slice 5.
- It lost "path fields stripped from the Grafana side", which the owner dropped (D8).

### Decisions already taken for slices 2 to 7

Settled with the repository owner during exploration, and repeated in every slice's design.md. One is revised by this
slice and marked as such.

- **`tracing` stays the one logging API.** Grafana is a second layer beside the file, fed by slice 1's bridge. There
  is no second set of `o11y::log` calls.
- **Analytics defaults to on and sends from the first launch.** Opting out stops sending at once (`o11y::shutdown()`
  and `faro.pause()`). Opting in takes effect at the next launch for the backend; Faro may resume at once.
- ~~**Path fields never reach Grafana.**~~ **Revised in slice 2:** paths are not scrubbed. Records reach Grafana with
  their paths as `opai.log` has them. See D8.
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

### D1 - One registry, two filtered layers, installed by every binary

```
registry()
    .with(fmt::layer().fmt_fields(KeyValues).event_format(Line::for_app(app)).with_writer(sink)
            .with_filter(file_filter()))           // RUST_LOG, else "info,ort=warn"
    .with(o11y::tracing::layer().fold_spans(is_ort_record)
            .with_filter(grafana_filter()))        // fixed
```

`format::subscriber` keeps its signature and returns this composition, so `init`, `test_subscriber` and every test
that records through `records_of` build the same thing.

- **The file layer is today's `fmt()` builder taken apart.** The formatter, field formatter, writer and filter are the
  same values. Only the filter moves from global to per-layer, and a per-layer filter makes the same decisions for
  that layer. The existing format and session tests are the proof, and they must pass unedited.
- **The Grafana layer is installed whether or not telemetry will start.** Every bridge hook checks `o11y`'s gate
  first, so before `start`, after `stop`, in `cli` and in `perf`, a record costs one relaxed load there. That keeps
  one subscriber shape for every binary. It also means `start` never has to swap a layer into a subscriber that is
  already global.

*Alternative:* a `reload::Layer` holding `None` until `start` fills it in. Rejected, because it adds a lock on every
record to buy what the gate already does for one atomic load.

*Alternative:* install the Grafana layer only in `gui`. Rejected, because `gui` would have to build `opai`'s
subscriber itself, and the subscriber's composition is `opai`'s.

### D2 - Two filters, both letting `ort`'s record span through

Each layer's filter is an `EnvFilter` joined by `FilterExt::or` to `filter_fn(is_ort_record_span)`. That function
matches a span, not an event, with target `ort::logging` and name `ort`. `FilterExt` and `filter_fn` are in the
`registry` and `std` features `opai` already enables.

- **File:** `RUST_LOG`, else `info,ort=warn`, as today, `.or(ort record span)`.
- **Grafana:** `info,ort=warn,opai::telemetry::export=off`, fixed and never read from the environment,
  `.or(ort record span)`.

Why each piece:

- **`RUST_LOG` is for a user chasing a problem in their own file.** It must not decide how much a whole population
  of installations sends. The spec requires this floor.
- **`ort=warn` matches the file's default.** The series decided on the same floor for both. It also covers
  `ort::logging` by prefix.
- **`opai::telemetry::export` is the target of D5's export-failure records.** Sending those would queue a record
  about the queue failing onto the queue that is failing.
- **The `or` arm is what makes `id` and `location` reach either destination.** Enabling the span costs nothing
  visible:
  - The file layer never writes a span on its own, and only reads its fields into enclosed events.
  - The Grafana layer is given `fold_spans(is_ort_record_span)`, so the span is never exported. Its fields land on the
    event.
  - `FilterFn`'s interest depends only on metadata, so it is cached per callsite. The `or` costs nothing on any
    other record.

  This is the fix for the defect described in Context. The file's `ort` records gain `id=` and `location=`, which the
  existing spec already requires.
- **The tests are rebuilt on `ort`'s real shape:** `span!(Level::TRACE, target: "ort::logging", "ort", …)` with the
  event given `parent: &span`. The two format tests that used `warn_span!` and `info_span!` are corrected to that shape
  (they were the reason the defect passed). A Grafana-layer test asserts the same shape delivers `location` on the
  exported record and exports no span.

*Alternative:* a span-aware `EnvFilter` directive in place of the `or`. Rejected, because in `EnvFilter`'s syntax
`target[span]=level` filters events *inside* a span, not the span itself.

*Alternative:* leave the file's defect to slice 3, which cleans up records. Rejected, because this slice builds the
exact mechanism, and the Grafana fold cannot work without it. Fixing only one side would make the two destinations
disagree about the same record.

### D3 - `opai::telemetry`: `start() -> Result<Sending, TelemetryError>`, `stop()`

```rust
pub enum Sending { Yes, NoCollector }        // NoCollector: this build names none
pub enum TelemetryError { AlreadyStarted, Invalid(O11yError) }
pub fn start() -> Result<Sending, TelemetryError>;
pub fn stop();                               // o11y::shutdown(); idempotent
```

- **Once per process.** `start` latches first, like `logging::init`. A second call is refused with
  `AlreadyStarted` whether the first one sent or found no collector, so a caller cannot read `NoCollector` as "try
  again later".
- **`NoCollector` is not an error.** It is the ordinary state of every development build.
- **`Invalid` wraps `o11y`'s refusal** of a malformed endpoint or header. It can only come from a bad CI secret.
  `gui` logs it and runs on.
- **`start` takes no identity.** The service name is `opai`, as in the Go application, so its dashboards keep
  working. Only `gui` starts telemetry (the series decision), so no record needs `app=` to tell binaries apart.

The configuration:
- `service_name("opai")`.
- `service_version(opai::version())`.
- `environment`: `Production` if `cfg!(not(debug_assertions))`, else `Development`.
- `min_level(Info)`.
- `geolocation`: off, which is the default and matches the Go application.
- `on_export_error`: D5.
- Every other setting keeps `o11y`'s default: 5 s flush, 512 batch, 8192 buffered, 10 s timeout.

*Alternative:* take the environment from a third `option_env!`, as the Go CI's third string replacement did.
Rejected, because the build profile already says it. A developer's local `--release` build with credentials reporting
`production` is a corner case, and the same variables make it deliberate.

### D4 - Credentials from `option_env!`, compiled into `opai`

`OPAI_OTEL_ENDPOINT` is the collector's base URL. `OPAI_OTEL_AUTH` is the whole `Authorization` value, as the Go
secret `OTEL_HEADER_AUTH` was. Both are read with `option_env!` in `telemetry`. Both must be present and non-empty,
or `start` returns `NoCollector`. An endpoint with no auth is treated as no collector, because this project's
collector always authenticates.

- **Rebuilds follow automatically.** `rustc` records `option_env!` reads in dep-info, so cargo rebuilds `opai` when
  either variable changes. No `build.rs` is needed.
- **The variables are prefixed `OPAI_`**, not the bare `OTEL_*` names the Go CI used. `OTEL_EXPORTER_OTLP_*` and
  friends are runtime variables that OpenTelemetry SDKs read, and a compile-time name should not be mistaken for one.
  Slice 5 maps the existing `OTEL_ENDPOINT` and `OTEL_HEADER_AUTH` secrets onto these names in the release job.
- **In `opai`, not `gui`.** The endpoint belongs to the telemetry configuration `opai` owns. `gui` passing strings in
  would split one configuration across two crates.

### D5 - Export failures: the first at `warn`, the rest at `debug`, never to Grafana

The `on_export_error` callback runs on `o11y`'s export thread. It emits
`tracing::warn!(target: "opai::telemetry::export", …)` the first time in a process, and `debug!` after that, using a
`static AtomicBool`.

- **Target and filter.** D2's filter keeps these records out of Grafana. The file filter's default `info` keeps the
  repeats out of an ordinary file, so an unreachable collector costs one line per session, not one every five
  seconds.
- **The callback cannot panic.** It formats an `O11yError` through `%`, which does not panic. A panicking callback
  would kill `o11y`'s worker.

### D6 - A session record when sending starts

After `o11y::init` succeeds, `start` emits `tracing::info!(version, os, arch, "telemetry started")` on target
`opai::telemetry`, which is not excluded. It reaches both layers. The file's session header was emitted before the
Grafana gate opened, so without this, Grafana's first record from a session would be whatever happened next. The
`NoCollector` path emits the same message as `debug!`, with `sending = false`, so the file says why nothing arrived.

### D7 - `task::spawn_blocking` carries the current span

```rust
let span = tracing::Span::current();
tokio::task::spawn_blocking(move || span.in_scope(work))
```

This covers every blocking hop in `opai` from one place: image load, probe, identity, encode and save, session
build, cache open, runtime start, and archive expansion. `Span::current()` is `Span::none()` when there is no span or
no subscriber, and `in_scope` on it is a no-op, so the silent-by-default property holds.

`gui`'s own direct `tauri::async_runtime::spawn_blocking` calls are not changed here. None of them is inside a span
today, and slice 5's command wrapper is where `gui` gets spans.

### D8 - Nothing is scrubbed

The repository owner decided this during this slice's proposal, reversing the recorded "path fields never reach
Grafana". No `map_field` hook is installed, and records reach Grafana with every field, error text and message as the
file has them.

What that sends:
- Paths in path-named fields (`path`, `dir`, `destination`, `config_dir`).
- Paths inside error `Display` text: `failed to decode <path>`, `failed to load the ONNX Runtime at <path>`, and
  `ImageIoError`'s variants.
- Paths that ONNX Runtime writes into its own messages.

A field-name-only strip was weighed and rejected as not keeping the promise it would imply, because the error text
carries the same paths. A full scrub, including values and the message body, needs `rust-sak`'s bridge to pass
`message` through `map_field`. It stays available if the decision is revisited: a later slice could add both halves
without touching this one's structure.

### D9 - `gui`: start in `setup`, stop on `RunEvent::Exit`

- **Start.** `opai::telemetry::start()` is called at the top of the existing `.setup(|app| …)` hook. The result is
  logged: `debug` for `NoCollector`, `warn` for an error. Setup runs after the single-instance plugin's own, which
  exits a second launch, so a refused launch never opens a connection.
- **Stop.** `.run(generate_context!())` becomes `.build(generate_context!())?.run(|_, event| …)`, which calls
  `opai::telemetry::stop()` on `RunEvent::Exit`. That flushes for at most `o11y`'s 10 s timeout, on a window that is
  already closed.

`gui` names no `o11y` type and does not enable the feature itself: it reaches telemetry only through `opai`.

### D10 - The `rust-sak` release and the feature

- **Release.** In `../rust-sak`, commit the `set_attribute` fix already in the working tree, and run its five gates
  (fmt, clippy, tests, doctests, and the `o11y-tracing` check). Then, **with the owner's go-ahead**, bump to the next
  CalVer tag (`26.9.10` in September 2026), push and tag. Confirm the tag with `git ls-remote --tags` before this
  repository names it.
- **Pins and feature.** The pin moves in `opai`, `gui` and `perf` together, with the lock refreshed by
  `cargo metadata` and a cold-home `cargo fetch --locked`, as in slice 1. `opai` gains `o11y-tracing` in its feature
  list. `gui` and `perf` keep theirs.
- **What the feature compiles.** `tracing` and `tracing-subscriber` are already in the graph. `o11y` compiles its
  exporter over `reqwest`, which `fetch` already builds, and its `o11y-macros` proc-macro sibling. So the new
  compilation is `rust-sak`'s own `o11y` module, that macro crate, and one third-party crate: `machine-uid`, which
  `o11y` requires (not optionally) for the HMAC'd machine id it reports. Its own dependencies (`libc`,
  `windows-registry`, `windows-sys`) are already in the graph. The implementation found it with
  `cargo tree -p opai -e normal --prefix none` before and after, and the owner accepted it.

### Division across crates

- **`opai`:** the subscriber composition, the Grafana filter, `telemetry`, the credentials, the export-failure policy,
  and the span carry.
- **`gui`:** when to start and stop.
- **`cli` and `perf`:** unchanged. They install the same subscriber and send nothing because they never start.

The logic stays in `opai` because a future front end that exports would need all of it, and would decide only when.

## Risks / Trade-offs

- **[Risk] The Analytics copy promises what the build no longer keeps.** It currently reads "No images or personal
  data are ever collected", and D8 sends file paths, which carry user names and photo file names. → Nothing is sent
  by any released build before slice 5 wires secrets, so the gap is not yet live. It must be resolved before slice
  5's release, either by rewording the description or by revisiting D8. This slice records it and changes no copy.
- **[Risk] The per-layer filter changes the file's output.** It is not meant to, but a global `EnvFilter` also
  shapes `tracing`'s max-level hint and callsite interest. → The format tests in `logging::format` and
  `tests/logging_session.rs` must pass unedited. The `ask_always` and `RECORDING` machinery in `logging` exists
  precisely for interest-cache effects, and it is kept.
- **[Risk] `ort` changes its record span's name, level or target.** `ort` is pinned exactly at `2.0.0-rc.13`, and
  D2's predicate matches that span's shape. → The predicate names `ort`'s `logging.rs` in a comment. The tests built
  on the real shape pin it, and any `ort` bump is already a deliberate act that re-checks pinned behaviours.
- **[Trade-off] No Grafana when the file cannot be opened.** `init` returns before installing the subscriber when
  `logs/` or `opai.log` cannot be created, so that session sends nothing. → Accepted. A configuration directory that
  cannot be written stops model installation too, so the application is not usable in that state anyway.
- **[Trade-off] A process that aborts loses up to one flush interval of Grafana records.** A native crash, or a panic
  under `panic = "abort"`, does not run `stop`. → The file has every record, since its sink is synchronous, and it is
  still the account of a crash. Flushing from the panic hook was weighed and rejected, because it could block the
  panicking thread, possibly the main one, for the 10 s export timeout.
- **[Trade-off] A developer's build with credentials sends regardless of Analytics until slice 5.** → Only builds
  where the two variables were deliberately set, and it is the path slice 2's real Grafana check uses.

## Migration Plan

1. Commit and gate the fix in `../rust-sak`, release it under the owner's go-ahead, and bump the three pins here.
2. Build the new subscriber composition and prove the file unchanged, then add the Grafana layer and its filter.
3. Add `telemetry`, the span carry, and `gui`'s two calls.
4. Validation:
   - The in-process export test runs on all three CI platforms: a local HTTP listener stands in for the collector,
     driven from a unit test in `telemetry`. The test process initialises `o11y` exactly once, which no other test
     does.
   - A real Grafana check on macOS: a `gui` debug build with the two variables set, and the arrival of the session
     record, a warning and a trace queried through Grafana's Loki and Tempo.
   - Windows and Linux are covered by CI for the export test. They are not run against the real collector in this
     slice, which is platform-independent code with no FFI and no execution provider involved.

**Rollback:** revert this repository's commits, which returns to the `26.9.9` pin and one `fmt` subscriber. The
`rust-sak` tag stays; nothing else depends on it.
