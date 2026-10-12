## Why

When something goes wrong on a user's machine today, `opai.log` is the only account of it, and only
if the user sends it. The application is to gain full observability: the same records sent to Grafana
alongside metrics and traces, when the user leaves the Analytics setting on. Every record in this
workspace is emitted through `tracing` — `opai`, `gui`, and ONNX Runtime's own diagnostics through
`ort`'s `tracing` feature — so the records reach Grafana with no second set of calls only if a
`tracing` layer can forward them to `rust-sak`'s `o11y` exporter.

That layer cannot be written against `o11y` as it stands. Its log records can only be emitted through
macros that fix their fields at compile time. The one runtime path, `log::__emit`, is `#[doc(hidden)]`
and "not API". Its spans are guards on a thread-local stack, while a `tracing` span is created on one
thread, entered on others and closed wherever its last handle drops. This slice fills those gaps in
`rust-sak`, adds the bridge there, cuts the release, and moves this workspace's pins to it.

## What Changes

Task estimate for the whole feature: well over 15. It spans the rust-sak APIs, the subscriber rework
in `opai`, cleaning up the existing records, instrumenting every unit of work, the gui switch and
command wrapper, the frontend's path into `opai.log`, and Grafana Faro. It is therefore split into
seven slices (see design.md - Slices). This proposal covers the first, at **15** tasks, the ceiling.

**In `../rust-sak` (a separate repository), all within the `o11y` module:**

- **A public, runtime log emit.** `log::emit` (level, message, fields assembled at runtime), and a
  variant that attaches the record to an explicit span context rather than to whatever is on the
  calling thread's stack. The `Fields` alias and the gate checks become public for the same reason.
- **Spans that are not tied to a thread.** An owned span handle opened with an explicit parent (the
  current span, a given context, or none), that can be annotated from any thread and is closed by
  ending or dropping it. The existing guard-and-`instrument` API is unchanged.
- **A public span context.** A trace id and span id pair, with W3C `traceparent` formatting and
  parsing. Slice 7 needs this to join the frontend's traces to the backend's; it is added here so the
  series needs one `rust-sak` release rather than two.
- **Span links and span status.** An owned span can link to other span contexts, and can be marked
  as failed, which exports as OTLP `status.code = ERROR`. Slice 4 needs links for a model build that
  one request starts and others wait on. Status is what lets a trace backend show a failed unit of
  work as failed.
- **The `tracing` bridge, behind a new `o11y-tracing` feature.** A `tracing_subscriber::Layer` that
  forwards events as `o11y` log records and spans as `o11y` spans, parented by `tracing`'s own span
  tree. It has one hook to rewrite or drop a field before it leaves the process, and one to fold
  selected spans into their events instead of exporting them.
- **A release**: a version bump and a new tag, gated on the repository owner's explicit go-ahead.

**In this repository:**

- Bump the `rust-sak` pin to the new tag in `crates/opai/Cargo.toml`, `crates/gui/Cargo.toml` and
  `crates/perf/Cargo.toml` together, so `Cargo.lock` keeps one `rust-sak` entry. No feature is
  enabled and no source changes: `opai` installs the layer in slice 2.

## Capabilities

### New Capabilities

None. After this slice the application records, sends and shows exactly what it did before. Nothing
in `opai`, `gui` or `cli` enables the new feature or calls the new API. The slice produces a
dependency that can forward `tracing` to Grafana, and the pin that reaches it. `.openspec.yaml` sets
`skip_specs: true`, as `add-crop-1-rust-sak` and `add-init-0-rust-sak` did for the same reason.

### Modified Capabilities

None. `logging` changes in slice 2, where the subscriber gains its second layer and the requirements
for what reaches Grafana, and when, can be checked against this application.

## Impact

**Crates touched:** `opai`, `gui` and `perf`, in their manifests and the lock file only: one tag, no
feature change, no source change. `cli` is untouched. No logic goes into a front end, because this
slice puts no logic anywhere in this repository.

**`rust-sak`:** this slice *is* the `rust-sak` work, so the usual question of what it already covers
is answered in reverse. Already there, and **not** rebuilt:
- OTLP/HTTP export on its own thread, with bounded buffers that discard rather than block.
- Resource enrichment: service, version, environment, a scoped machine id, and the session id.
- The enable switch, the level gates, and metrics.
- The span stack and `Instrument`, which the owned span sits beside rather than replaces.

Not there, and added by this slice: runtime-field log emit, spans independent of a thread, span
contexts and `traceparent`, links, status, and the `tracing` bridge.

**Third-party crates introduced:** `tracing` and `tracing-subscriber` (the `registry` and `std`
features), in `rust-sak` only, and named by it only behind `o11y-tracing`. `tracing` already reaches a
plain `o11y` build through `reqwest`'s `hyper` and `h2`; `tracing-subscriber` does not. Both are
already in this workspace's graph through `opai` and `ort`, so the bridge compiles nothing new here.
See design.md for why not `tracing-opentelemetry`.

**Parity divergence:** none in this slice; nothing user-visible changes. The series as a whole
diverges from the Go application in three deliberate ways, each decided and recorded in design.md -
Slices:
- Grafana export honours the Analytics switch. The Go app exported regardless of it.
- Metrics and traces are exported, not only error logs.
- Grafana Faro in the frontend replaces the Go frontend's Aptabase events.

**Not in scope:** any subscriber change in `opai`; which fields are stripped and which spans are
folded (that policy is `opai`'s, in slice 2); endpoints, credentials and CI secrets; the Analytics
switch; and all instrumentation.
