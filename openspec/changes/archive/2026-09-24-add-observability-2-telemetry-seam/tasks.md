## 1. Release the `set_attribute` fix in `../rust-sak`

- [x] 1.1 Review the uncommitted `set_attribute` fix in `../rust-sak` (`trace/context.rs`, `trace/current.rs`,
  `trace/owned.rs` and their tests). A re-recorded key must replace the old value, on `OwnedSpan` and `Current` alike.
  Add a `## [Unreleased]` note to the `o11y` README if the module keeps one. Verify all five gates are green:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-features --all-targets -- -D warnings`
  - `cargo test --all-features`
  - `cargo test --doc --all-features`
  - `cargo check --no-default-features --features o11y-tracing`
- [x] 1.2 **Get the repository owner's explicit go-ahead**, then commit, bump the version to the next CalVer number
  (`26.9.10` in September 2026), push, and tag at the pushed commit (design.md D10). Verify that `git ls-remote --tags
  https://github.com/vegidio/rust-sak` lists the tag at that commit.

## 2. Adopt the pin and the feature here

- [x] 2.1 Move the `rust-sak` pin to the new tag in `crates/opai/Cargo.toml`, `crates/gui/Cargo.toml` and
  `crates/perf/Cargo.toml` together, and add `o11y-tracing` to `opai`'s feature list only, with a manifest comment in
  the file's style. Refresh `Cargo.lock` with `cargo metadata`. Verify:
  - `Cargo.lock` holds one `rust-sak` entry.
  - `cargo tree -p opai -e normal --prefix none`, diffed before and after, adds only `rust-sak`'s `o11y` module and
    its `o11y-macros` sibling, plus `machine-uid`, the one third-party crate `o11y` requires (accepted by the owner).
  - `CARGO_HOME=$(mktemp -d) cargo fetch --locked` resolves from a cold cargo home.

## 3. The two-layer subscriber in `opai::logging`

- [x] 3.1 First, correct the two format tests that build `ort`'s span as `warn_span!`/`info_span!` under target `ort`.
  Rebuild them on the real shape from `ort-2.0.0-rc.13/src/logging.rs`:
  `span!(Level::TRACE, target: "ort::logging", "ort", id, location)`, with the event emitted `parent: &span`. Verify
  that they now **fail** against the current subscriber, reproducing the missing `id`/`location` (design.md Context).
- [x] 3.2 Rebuild `format::subscriber` as `registry()` plus the file `fmt::layer` (same `Line`, `KeyValues` and
  writer) under a per-layer filter: `RUST_LOG`, else the default, `.or(filter_fn(is_ort_record_span))` (design.md D1,
  D2). The predicate carries a comment naming `ort`'s `logging.rs`. Verify the following:
  - The tests from 3.1 pass.
  - Every other test in `logging::format`, `logging::panic` and `logging::writer`, and `tests/logging_session.rs`,
    passes **unedited**.
  - `test_subscriber` and `records_of` still build through `format::subscriber`.
- [x] 3.3 Add the Grafana layer to the same registry: `o11y::tracing::layer().fold_spans(is_ort_record_span)` under
  `EnvFilter::new("info,ort=warn,opai::telemetry::export=off").or(filter_fn(is_ort_record_span))`, never read from
  the environment (D2). Unit-test the layer's filter decisions without initialising `o11y`:
  - `debug` is off under `RUST_LOG=debug`-equivalent file directives.
  - `ort::logging` warn is on and info is off.
  - The `ort` record span is on.
  - `opai::telemetry::export` is off at every level.

  Verify the file tests from 3.2 still pass with the layer present and inert.

## 4. `opai::telemetry`

- [x] 4.1 Add `crates/opai/src/telemetry.rs` with `start() -> Result<Sending, TelemetryError>` and `stop()`,
  exported from `lib.rs` (design.md D3, D4):
  - A once-per-process latch taken first.
  - `option_env!("OPAI_OTEL_ENDPOINT")` and `option_env!("OPAI_OTEL_AUTH")`, where either missing or empty gives
    `Sending::NoCollector`.
  - The `Config`: service `opai`, `opai::version()`, the environment from `debug_assertions`, `min_level(Info)`, and
    geolocation off.
  - `o11y` errors wrapped as `TelemetryError::Invalid`.

  Keep the body behind a `pub(crate)` function taking the endpoint and auth, so a test can supply them. Unit-test
  that no collector gives `NoCollector` with no socket opened, and that a second `start` is `AlreadyStarted`, testing
  the latch directly as `logging`'s second-install test does. Verify with `cargo test -p opai telemetry`.
- [x] 4.2 Add the export-failure callback (D5) and the session record (D6):
  - The callback records `warn!` on target `opai::telemetry::export` the first time in a process and `debug!` after,
    using a `static AtomicBool`, and cannot panic.
  - `start` emits `info!(version, os, arch, "telemetry started")` on `opai::telemetry` when sending, and `debug!` with
    `sending = false` when there is no collector.

  Unit-test the first-then-quiet policy by calling the callback directly under `records_of_blocking` at `info`: one
  warning line, then none.
- [x] 4.3 Add the export test: a unit test in `telemetry` with a `tokio` `TcpListener` standing in for the collector.
  It captures the `POST`s to `/v1/logs` and `/v1/traces`. It runs the start path against it with the two-layer
  subscriber bound by `set_default`, then emits:
  - An `info` record inside a span.
  - A `debug` record.
  - An `ort`-shaped warning.
  - A simulated export failure.

  After `stop`, assert on the payloads:
  - The session record and the `info` record arrive, carrying the span's trace id.
  - The span arrives as a trace, and no `ort` span does.
  - The `ort` record carries `location`.
  - The `debug` record and the export-failure record are absent.

  The test is the only place in the suite that initialises `o11y`, and a comment says so. Verify with
  `cargo test -p opai telemetry`.

## 5. The span carried through `task::spawn_blocking`

- [x] 5.1 Capture `tracing::Span::current()` in `task::spawn_blocking` and run `work` inside `span.in_scope` on the
  blocking thread (design.md D7). Unit-test it in `task`'s own tests. The file subscriber is bound as the default on
  the calling thread and, inside the closure, on the blocking thread, as the global one `logging::init` installs
  would be. (`records_of` binds to one thread only. The planned `tests/blocking_span.rs` binary could not reach the
  `pub(crate)` helper, and no public function emits a record from inside one.) A record emitted inside a
  `spawn_blocking` closure under an entered span must render that span's fields in the file format. It failed before
  the fix and passes after. Verify with `cargo test -p opai --lib task::`, and that the existing `task` tests pass.

## 6. `gui`: start and stop

- [x] 6.1 In `crates/gui/src/lib.rs`, call `opai::telemetry::start()` at the top of the existing `.setup` hook,
  logging `NoCollector` at `debug` and an error at `warn`. Replace `.run(generate_context!())` with
  `.build(..)` then `.run(|_, event| …)`, calling `opai::telemetry::stop()` on `RunEvent::Exit` (design.md D9).
  Verify:
  - `cargo build -p gui` succeeds.
  - A debug launch without the variables writes the `debug` "telemetry started" record with `sending=false` to
    `opai.log` when run with `RUST_LOG=debug`. Hand this launch check to the repository owner rather than driving the
    GUI.
  - The existing `gui` tests pass.

  **Checked 2026-09-24, 18:26 local:** a build without the variables, with `RUST_LOG=debug`, wrote
  `level=DEBUG target=opai::telemetry msg="telemetry started" … sending=false` and `gui`'s own DEBUG "this build names
  no collector". Loki received nothing from 16:25 UTC on.

## 7. Specs, docs and verification

- [x] 7.1 Update the prose that describes the old shape:
  - `logging/mod.rs`'s module header ("installs the subscriber"; two destinations now).
  - `format.rs`'s header on the two floors, which now also covers the `ort` span arm and why the runtime's records
    had lost their fields.
  - `opai`'s `tracing-subscriber` manifest comment.

  Verify `cargo doc -p opai --no-deps` builds without warnings.
- [x] 7.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace --locked`, and verify all three are green. Then run `openspec validate
  add-observability-2-telemetry-seam --strict` and verify it passes.
- [x] 7.3 Real Grafana check, on macOS, with the owner's collector credentials exported as `OPAI_OTEL_ENDPOINT` and
  `OPAI_OTEL_AUTH`. The owner launches a `gui` debug build and enhances one image with CoreML, which produces `ort`
  warnings. Then verify through Grafana's Loki and Tempo:
  - The "telemetry started" record arrives under `service_name="opai"` with `deployment_environment="development"`.
  - An `ort` warning arrives carrying `location`.
  - No `ort` span appears in Tempo.
  - After quitting, the last records before exit are present, which proves `stop` flushed.

  Record the query used and the result in this task's checkbox note.

  **Run of 2026-09-24, session `2d27ec81-299b-48a1-9f24-28bfa0a184c3` (3 of 4 checks pass, one not exercised):**
  - Loki `{service_name="opai"}`, 16:15–16:30 UTC: 14 records, every file record after the gate opened. The first is
    "telemetry started" (`deployment_environment_name="development"`, `service_version=26.10.0`). Only the file's
    pre-gate "Open Photo AI starting" header is absent, as designed. ✓
  - The last record before quitting ("enhancement finished", 18:16:53 local) is present, so `stop` flushed. ✓
  - Tempo `{resource.service.name="opai"}`: one trace, `session_build` (artifact `up_tokyo_4x_fp32`, provider
    CoreML, 7.9 s), a single span. `{name="ort"}` returns nothing. ✓
  - An `ort` warning carrying `location`: **not exercised.** The run emitted no `ort` records at all, in the file or in
    Loki (`| target=~"ort.*"` is empty). Face recovery and colour balance came from the run cache, so the only
    session built was Tokyo's, which attaches cleanly. Re-run with a model that warns on CoreML (for example
    `fr_athens_fp32`, `dn_stockholm_fp16` or `la_paris_fp16`, on an image not already cached).

  **Second run, 18:22–18:24 local, with credentials and `RUST_LOG=debug`:**
  - The file: 62 DEBUG records, and 5 `ort` INFO records. The `ort::logging` ones now carry
    `id=io.vinicius.opai location="device_discovery_common.cc:34 operator()"`, the D2 fix seen on the real runtime.
  - Loki (`sum by (detected_level, target) (count_over_time({service_name="opai"}[2m]))`): 14 records, all INFO,
    all `opai::*`. That is exactly the file's INFO records minus the pre-gate header. None of the debug records and
    none of the `ort` INFO records were sent, so the floor holds under `RUST_LOG=debug`. ✓
  - Tempo: one `session_build` trace, a single span, no `ort` span. ✓
  - An `ort` warning: still not exercised. Tokyo was again the only session built.

  **Third run with credentials, 19:12–19:13 local, session `23bed9f5-1d81-4d5a-a8a9-38996351f2bc`, a new image with
  face recovery (Athens FP32, CoreML):**
  - Loki `{service_name="opai"} | target=~"ort.*"`: the three `ort::logging` WARN records arrived, each with its
    `location` (`coreml_execution_provider.cc:113 GetCapability`, `session_state.cc:1367`/`1369
    VerifyEachNodeIsAssignedToAnEp`). ✓ `ort`'s INFO device-discovery records were not sent. ✓
  - They carry `trace_id` `d5f676f7472341c1c906b86549b8cf66`, which Tempo holds as Athens's `session_build` (22.8 s,
    `artifact=fr_athens_fp32`, `provider=CoreML`): a single span, with no `ort` span. ✓
  - `id` does not appear in Loki: the runtime supplies an empty logger id for session-level records (the file shows
    `id=""`), and Loki drops empty structured metadata. Its non-empty ids (`io.vinicius.opai` on device discovery) are
    carried as normal.
