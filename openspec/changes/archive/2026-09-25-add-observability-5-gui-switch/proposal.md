## Why

Since slice 4, a build that names a collector sends `opai`'s records, spans and metrics, but `gui` still has three gaps:

- **It ignores the Analytics switch.** The switch lives only in the window's `localStorage`, which Rust cannot read
  when it starts telemetry in `setup`. A user who turned Analytics off is sent from anyway.
- **The window's requests are not in the traces.** Each enhancement is a trace of its own, with nothing tying it to
  the command that asked for it. Slice 3 removed `gui`'s repeated failure records on the promise that a command span
  would put `run=` back onto `opai`'s records. No such span exists yet.
- **Some of `gui`'s own failures are never recorded.** A link that would not open and a log file that could not be
  revealed leave no record. The session's end leaves none either, and a second launch's raise failures go to
  `stderr`, which nobody reads.

This slice closes these gaps. It is the last `gui` slice of the series: the frontend's records and Faro come next.

## What Changes

The whole feature needs well over 15 tasks, so it is split into seven slices (see design.md - Slices). This proposal
is the fifth, at **13** tasks.

**The Analytics switch reaches Rust (`gui`, Rust and frontend):**
- **A file mirrors the Analytics choice** in `opai`'s configuration directory, owned and written by `gui`'s Rust
  side.
- **`setup` reads the file before starting telemetry**, and does not start it when the choice is off. So a launch
  after an opt-out sends nothing at all.
- **The window mirrors the stored choice through a new `set_analytics` command**: once at boot, and again whenever a
  Save changes it.
- **Turning Analytics off stops sending at once**, through `opai::telemetry::stop()`. Turning it on takes effect at
  the next launch, as the series decided, because `o11y` starts once per process.
- **Turning the choice off is itself never sent.** Telemetry is stopped before anything about the opt-out is
  recorded, and `set_analytics` opens no command span.

**One wrapper for every command (`gui`):**
- **Every Tauri command except `set_analytics` runs inside a span named after the command**, under `gui`'s own
  target. The span carries the command's `run` when it has one. It therefore reaches the file, which writes `run=`
  onto `opai`'s records inside it, as slice 3 planned. It also reaches the collector, which parents `opai`'s unit
  spans under it, as slice 4 planned.
- **The span says how the command ended:** finished, stopped or failed, and a failure marks it as failed.
- **Each command error says who recorded it.** Every error variant is classified in one exhaustive match:
  - a stop
  - a failure `opai` already recorded, which the wrapper does not record again
  - a failure only `gui` saw, which the wrapper records once, at `warn`

  Records inside command bodies that now repeat the wrapper's are removed. So are the explicit `run` fields that the
  span now supplies. The export's `error` record for a destination it could not claim or commit becomes the
  wrapper's `warn`.
- **`gui`'s own blocking hops carry the command span**, as `opai`'s `task::spawn_blocking` has done since slice 2.
  So an image described on a blocking thread stays in its command's trace.

**Smaller `gui` fixes:**
- **The session's end is recorded.** An `info` record with the session's uptime is written on `RunEvent::Exit`,
  before telemetry stops, so it reaches the file and the collector.
- **The three `eprintln!`s in `single_instance.rs` become `warn` records.** `lib.rs`'s `eprintln!` for a log sink
  that could not be installed stays, because no subscriber exists at that point.

**Dropped from the planned row:** "CI secrets wired". This repository has no release workflow to wire them into.
The change that adds one must map `OTEL_ENDPOINT` and `OTEL_HEADER_AUTH` onto `OPAI_OTEL_ENDPOINT` and
`OPAI_OTEL_AUTH` (slice 2's D4). Until then, no released build sends anything.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-settings`: the Analytics choice is now honoured rather than only recorded. Turning it off stops sending at
  once and for every later launch, and turning it on takes effect at the next launch.
- `logging`:
  - A front end's request is a unit of work of its own. Its span parents the library's units and says how the
    request ended.
  - The front end records the failures only it observes, once each.
  - The request's own identifier is added to the file's records, as the one context allowed beside those that already
    decorated it.
  - The end of a graphical session is recorded.

## Impact

**Crates touched:**
- `gui` (Rust):
  - new `command` and `analytics` modules
  - `lib.rs` (`setup`, the exit hook, the handler list)
  - every command's body
  - `single_instance.rs`
  - `rust-sak`'s `fs` feature added to its pin
- `gui` (frontend): an `ipc/analytics.ts` binding, and the mirror at boot and on Save.
- `opai`: nothing. `telemetry::start` and `stop` already have the shape this slice needs. The file layer's filter
  already passes `gui`'s target, and the collector's already passes `info`.
- `cli` and `perf`: nothing. Neither starts telemetry, so neither has a switch to honour.

**Why this is in `gui`:**
- The switch is a preference of the graphical application, stored by its window.
- The wrapper wraps Tauri commands.
- Only `gui` exports, so neither would have a second caller in `opai`.
- The telemetry logic stays in `opai`. `gui` decides only whether and when to call it, as slice 2 set out.

**`rust-sak`:**
- `fs` covers resolving the configuration directory (`user_config_dir`) and the temporary file the choice is written
  through (`mk_temp_file_in`). `gui` enables the feature. It is already compiled through `opai`, so nothing new is
  built.
- `o11y` needs nothing new. `opai::telemetry` already wraps its `shutdown`.
- No `rust-sak` release, and no pin moves.

**Dependencies:** none added. `rust-sak`'s `fs` feature is newly named by `gui`, and it is already in the graph. `serde_json`
moves from `gui`'s dev-dependencies to its dependencies; `tauri` already depends on it.

**Parity divergence from the Go application, deliberate and already recorded by the series:**
- It exported to Grafana whatever the switch said. This honours the switch.
- Its frontend's Aptabase honoured the switch at once, both ways. Here opting in waits for the next launch, because
  `o11y` starts once per process.

**Accepted risk, at the repository owner's direction:**
- The Analytics description still reads "No images or personal data are ever collected".
- Records still reach the collector with file paths, per slice 2's D8.
- Neither is changed here (design.md - Risks).

**Not in scope:**
- CI secrets and a release workflow (above).
- The frontend's own records (slice 6).
- Faro, and a `traceparent` from the window (slice 7).
- Changing the Analytics copy.
