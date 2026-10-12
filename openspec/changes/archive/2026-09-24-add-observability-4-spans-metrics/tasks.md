## 1. The read-back in `../rust-sak`'s `o11y::metric`

- [x] 1.1 Add `Counter::value`, `Gauge::value`, `Histogram::count` and `Histogram::sum`, each taking a tag set (design.md
  D1). Rules:
  - `&[]` reads the untagged series, and tag order does not matter.
  - A series never written reads as `0` (or `None` for a gauge).
  - A tag set folded into `o11y.series_overflow` reads as never written.

  Unit-test each rule beside each instrument, and document the methods and the overflow caveat in
  `src/o11y/README.md`'s Metrics section. Verify with `cargo test --features o11y metric::` and
  `cargo test --doc --all-features`.
- [x] 1.2 Run `rust-sak`'s five gates and verify all are green:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-features --all-targets -- -D warnings`
  - `cargo test --all-features`
  - `cargo test --doc --all-features`
  - `cargo check --no-default-features --features o11y-tracing`

  Then **get the repository owner's explicit go-ahead**, bump the version to `26.9.11` (the CalVer number for the
  release month), push, and tag. Verify that `git ls-remote --tags https://github.com/vegidio/rust-sak` lists the tag at
  the pushed commit.

  *Done differently, at the owner's direction:* no version bump. The read-back was pushed as `fef766c`, and the
  existing `26.9.10` tag was deleted and re-pushed at that commit, so the pins keep reading `26.9.10`.
- [x] 1.3 Move the `rust-sak` pin to the new tag in `crates/opai/Cargo.toml`, `crates/gui/Cargo.toml` and
  `crates/perf/Cargo.toml` together, with no feature change. Refresh `Cargo.lock` with `cargo metadata`, not
  `cargo update -p`. Verify the following:
  - `Cargo.lock` holds exactly one `rust-sak` entry, and no unrelated package moved.
  - `CARGO_HOME=$(mktemp -d) cargo fetch --locked` resolves from a cold cargo home.
  - `cargo test --workspace --locked` is green.

  *Done as:* no manifest changed, since the tag name did not. `Cargo.lock`'s two `rust-sak` source lines moved from
  `a5de80a` to `fef766c`, and nothing else changed. The cold-home fetch resolved, and the workspace tests passed (2042).

## 2. The unit seam in `opai`

- [x] 2.1 Add `telemetry::unit` (D2, D3): `TARGET`, the `unit_span!` macro, `Unit`, `Outcome`, and `ended`, which
  records `outcome` and `error` on the span. Exclude unit spans from the file layer in `logging/format.rs` by joining
  `not(is unit span)` outside the environment's filter. Then:
  - Write the file test first: a record inside a unit span, under `RUST_LOG=debug`, carries none of the span's fields.
    If per-layer filtering does not hide the span from `Line`'s scope walk, apply design.md Risks' fallback.
  - Test that the collector's filter passes a unit span.
  - Test that `ended` with `Failed` records `error` and with `Stopped` does not.
  - Extend the collector-filter recorder in `logging::format`'s tests to capture parents, links and recorded fields,
    for the tests below to use.

  Verify that every existing test in `logging::format`, `tests/logging_session.rs` and `sessions/tests/log.rs` passes
  **unedited**.
- [x] 2.2 Add `telemetry::metrics` with the six instruments and the bucket bounds in D6. Add the crate-private
  `kind(&self) -> &'static str` on `InferenceError`, `InitError`, `SessionError` and `ImageIoError`: one snake-case name
  per variant, and wrapping variants delegate to the error they wrap. Add `Operation::model_tag()` (D4). Have
  `unit::ended` record the duration histogram and, on `Failed`, the failure counter. Then test:
  - Every variant has a name, and the full set is pinned.
  - Two operations that differ only in a parameter share a `model_tag`.
  - `ended` moves the histogram and counter through the read-back.

  Verify with `cargo test -p opai telemetry:: error::`.

## 3. Spans on the brackets

- [x] 3.1 Initialization and installs (D4):
  - An `initialize` span, opened in `Opai::recorded` and ended in its outcome `match`.
  - A `runtime_load` span in `runtime::start`.
  - An `install` span in `deps::install`, ended by `record_ended` and by the success path.

  Test with the recorder: `initialize` has the right outcome for success, failure (an invalid name) and refusal (a
  held claim), and a failed install's span carries `error`. Verify with `cargo test -p opai initialize deps::install`.
- [x] 3.2 Sessions (D4, D5):
  - A `session_request` span per `Sessions::session`, carrying `resident`.
  - The `build` span created with the flight and parented by the creating request's span. The flight keeps its `Id`,
    and the handle sits in a take-once slot for whichever request runs the initializer. It is ended in the closure's
    existing `match`.
  - `follows_from` from every request that joins an existing flight.
  - `opai_provider_fallbacks_total` in the CPU-fallback arm.

  Test that two concurrent requests for an unbuilt model produce exactly one `build` span and one link, that a failed
  GPU build's `build` span carries `error` and the counter moves, and that no `SessionHandle` or lease lifetime changed
  (the existing session tests pass unedited). Verify with `cargo test -p opai sessions::`.
- [x] 3.3 The resident gauge: set `opai_resident_sessions` per provider, zeros included, after every insert, `clear` and
  `sweep`, under the lock that mutated the entries (D6). Test that building one session reads `1` for its provider,
  and that a sweep that evicts it reads `0`. Verify with `cargo test -p opai sessions::`.
- [x] 3.4 Enhancement (D4):
  - An `enhancement` span in `process::process`, ended in its outcome `match`.
  - A `step` span per computed operation in `run`'s loop, marked failed by the `in_step` arm.
  - `opai_step_duration_seconds` for each computed step, with `model_tag` and the provider its sessions ran on, or
    `mixed`.
  - `opai_cache_lookups_total{kind=step}` at the lookup.

  With the fake backend, test:
  - A two-step run whose model installs and builds has the trace shape of the spec's first scenario.
  - A cancelled run's spans say `stopped` with no `error`.
  - A failed step marks both its own span and the run's.
  - A small and a large image produce the same span count.
  - A second identical run counts two step hits and opens no step span.

  Verify with `cargo test -p opai inference::process`.
- [x] 3.5 Analysis and automatic analysis (D4):
  - An `analysis` span in `execute::execute`, with `opai_cache_lookups_total{kind=analysis}` at its lookup.
  - An `automatic_analysis` span in `autopilot::suggest`, ended by its wrapper.

  Test that a refused analysis opens no span, that an automatic analysis parents its face detection's `analysis` span,
  and that a withdrawn one ends `stopped`. Verify with `cargo test -p opai inference::execute autopilot`.
- [x] 3.6 Image IO and cache:
  - A span at each of the six public `*_blocking` image boundaries, ended with the outcome the existing `inspect_err`
    sees.
  - `cache_read` and `cache_write` spans in `RunCache::get_bytes`, `put`, `get_value` and `put_value`, the reads
    carrying `hit`.

  Test that a load through the async twin is one span parented by the caller's span across the blocking hop, that a
  failure marks it failed, and that a cache miss then hit reads `hit=false` then `hit=true`. Verify with
  `cargo test -p opai image:: cache::`.

## 4. End to end and verification

- [x] 4.1 Extend `telemetry/tests.rs`'s in-process collector test to run one fake-backend enhancement under a started
  `o11y`. Assert that `/v1/traces` receives the `enhancement` span with a `step` child sharing its trace id, that
  `/v1/logs` has its records carrying that trace id, and that `/v1/metrics` has `opai_unit_duration_seconds` with
  `unit=enhancement` and `outcome=finished`. Verify with `cargo test -p opai telemetry::`.
- [x] 4.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace --locked`, and verify all three are green. Verify that
  `openspec validate add-observability-4-spans-metrics --strict` passes. Verify with `git diff` that no test under
  `logging/`, `tests/logging_session.rs` or the bracket log tests had an assertion edited.
- [x] 4.3 Hand the owner a manual check on macOS: a `gui` debug build with `OPAI_OTEL_ENDPOINT` and `OPAI_OTEL_AUTH` set,
  enhancing one photograph on CoreML and once on the CPU, then cancelling one by dragging a slider. Then query Grafana:
  - Tempo: each enhancement's trace shape, a `stopped` span with no error status, and a `build` span on the first run
    only.
  - Mimir: the six instruments, with their tag values.

  Compare `opai.log` against a run from before the slice for the same actions. Record in this task's checkbox note what
  was seen.

  *Seen on 2026-09-24 (session `218dc86c`, three enhancements, all on the CPU):*
  - Tempo, first run (`ffdb4bbf…`, face recovery then Tokyo 4x, cancelled): `enhancement` → `cache_read` + `step` per
    operation → `session_request` → `build` → `install` + `session_build`, and `cache_write` under the finished step.
    The run and the second step say `outcome=stopped` with an unset status, and there is no error.
  - Tempo, later runs (`53f528ca…`, `d1c6c968…`): `enhancement` plus one `cache_read` with `hit=true`. No `step` and no
    `build`, so a build appears on the first run only.
  - Mimir: `opai_unit_duration_seconds` (13 units, `finished` and `stopped`), `opai_step_duration_seconds`
    (`model=fr_athens_fp32`, `provider=CPU`), `opai_cache_lookups_total` (`step` and `analysis`, `hit` and `miss`),
    and `opai_resident_sessions` (`CPU=3`, zeros for CoreML, CUDA and TensorRT). `opai_failures_total` and
    `opai_provider_fallbacks_total` have no series, because nothing failed and nothing fell back.
  - `opai.log`: 25 lines. None carries `outcome=`, `resident=` or `hit=`, and the enhancement records have their
    earlier shape.
  - CoreML: the owner ran it and confirmed it on 2026-09-24.
