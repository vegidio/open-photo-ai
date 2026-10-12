## Why

Slice 1 gave `rust-sak` a `tracing` layer that forwards records to `o11y`'s OTLP exporter. Nothing in this
workspace installs it yet, so every record still ends in `opai.log` and reaches us only if a user sends the file.
This slice connects that layer, so a GUI build that carries collector credentials sends its records and spans to
Grafana. It is the first slice anything can be seen in: slices 3 and 4 clean up and add records, and those are only
worth checking once they arrive somewhere.

## What Changes

Task estimate for the whole feature: well over 15, split into seven slices (see design.md - Slices). This proposal
is the second, at **14** tasks.

**In `../rust-sak` (a separate repository):**

- **Release the `set_attribute` fix** found while verifying slice 1. A field recorded twice exported its key twice,
  which OTLP forbids. The fix is replace-or-push on `OwnedSpan` and `Current`, and it sits uncommitted in the
  working tree. It ships as the next CalVer tag, with the owner's go-ahead, before this repository adopts the bridge.
  No other `rust-sak` change is needed.

**In `opai`:**

- **The subscriber becomes a registry of two layers**, each with its own filter:
  - **The file layer** is today's formatter, writer and `RUST_LOG` filter, unchanged, so `opai.log` is byte-for-byte
    what it was, apart from the `ort` fix below.
  - **The Grafana layer** is `rust-sak`'s bridge under a fixed `info,ort=warn` floor that `RUST_LOG` does not move.
    `ort`'s per-record spans are folded into their events. It is installed by every binary, but it is inert until
    telemetry starts: one atomic load per record.
- **`ort`'s `id` and `location` reach the file again.** Today they are lost: `ort` wraps each runtime record in a
  TRACE-level span, which `ort=warn` disables. The tests missed it because they build that span at a different level.
  This is a defect against the existing `logging` spec. It is fixed here because the Grafana layer needs the same span
  enabled in order to fold it.
- **`opai::telemetry`**, with `start()` and `stop()`:
  - `start` starts `o11y` with this project's collector, when the build carries one.
  - `stop` flushes and stops sending. It is idempotent.
  - Telemetry can start once per process, as `o11y` itself allows.
- **The collector is baked in at compile time.** The endpoint and the authorization header come from `option_env!`
  (`OPAI_OTEL_ENDPOINT`, `OPAI_OTEL_AUTH`). A build without them sends nothing, and `start` reports that rather than
  failing. The environment is `production` for a release-profile build and `development` otherwise.
- **Telemetry reports on itself, in the file only.** A session record goes to Grafana when telemetry starts.
  Export failures go to `opai.log`: the first at warning, the rest below the default level. They never go to
  Grafana, which would loop them back into the queue that is failing.
- **`task::spawn_blocking` carries the caller's span** onto the blocking thread. Without this, records emitted there
  lose their enclosing span's fields in the file, and their trace in Grafana. That thread is where image IO,
  session builds and archive expansion run.

**In `gui`:**

- **Telemetry starts in the builder's `setup` hook and stops on `RunEvent::Exit`.** Starting in `setup` means a
  second launch refused by the single-instance plugin never starts it. The Analytics switch is not consulted yet:
  slice 5 mirrors it to Rust. Until slice 5 wires CI secrets, only a developer's build with the two variables set
  sends anything.

## Capabilities

### New Capabilities

None. Sending records to a collector is a new destination for the records `logging` already governs. It is not a
separate capability, so it is specified there, as the series planned.

### Modified Capabilities

- `logging`:
  - Records are also sent to a collector, when the application has started telemetry and the build names one.
  - A floor for what is sent that the environment does not move, and the runtime's context-only spans folded away.
  - Nothing sent by a build without a collector, or by an application that never starts telemetry.
  - Export failures are recorded in the file only.
  - A record emitted on a blocking thread keeps the span it was handed off under.

  The existing requirements on the file are unchanged.

## Impact

**Crates touched:**
- `opai`: `logging` gains a second layer; `telemetry` and the `task::spawn_blocking` change are new code.
- `gui`: two calls in `lib.rs`.
- `perf`: its manifest's pin only.
- `cli`: no change. It installs the same subscriber, never calls `start`, and so sends nothing.

The logic is in `opai`, because it is where the subscriber is built and where the credentials are baked. `gui`
decides only *when* to start and stop.

**`rust-sak`:** `o11y` covers the export, batching, resource enrichment, session id and the gates. Its
`o11y-tracing` bridge covers the layer, parentage across threads, and span folding. Nothing is rebuilt here. The only
`rust-sak` work is releasing the `set_attribute` fix.

**Dependencies:** the `o11y-tracing` feature is added to `opai`'s `rust-sak` line. Its `tracing` and
`tracing-subscriber` are already in the graph, and `o11y` brings `reqwest` and the OTLP encoding, which `fetch`
already compiles. One third-party crate is introduced: `machine-uid`, which `o11y` requires to derive its hashed
machine id, and whose own dependencies are already in the graph. The pin moves in `opai`, `gui` and `perf` together.

**Parity divergence from the Go application, deliberate:**
- It exported error-level logs only. This exports every record at `info` and above, plus spans, as the series
  decided.
- The Analytics switch is ignored until slice 5, as the Go application always ignored it. The difference is that no
  released build carries credentials before slice 5.

**Reverses a recorded series decision:** "Path fields never reach Grafana" is dropped, at the repository owner's
direction. Records reach Grafana with their paths in full: in fields, in error text and in `ort`'s messages. The
Analytics description currently promises that no personal data is collected. See design.md - Risks.

**Not in scope:**
- Cleaning up the existing records (slice 3), and spans and metrics on units of work (slice 4).
- The Analytics switch and CI secrets (slice 5).
- The frontend (slices 6 and 7).
