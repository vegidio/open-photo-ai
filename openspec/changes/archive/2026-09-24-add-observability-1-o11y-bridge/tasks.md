## 1. Public primitives in `../rust-sak`'s `o11y`

- [x] 1.1 Make `SpanContext` public per design.md D4. It holds a 16-byte trace id and an 8-byte span
  id, with hex accessors, `to_traceparent()` and `from_traceparent()`. Add `trace::current().context()`.
  Unit-test round-trips; the rejection of all-zero ids, bad lengths, non-hex and version `ff`;
  acceptance of an unknown non-`ff` version; and `None` from `context()` with no span open. Verify
  with `cargo test --features o11y`.
- [x] 1.2 Add `log::emit` and `log::emit_in`, make the `Fields` alias public, and add
  `trace::enabled()` (D2). Re-point the log macros at `emit`, then remove `log::__emit` and
  `trace::__enabled`. Test that `emit_in` stamps the given context and not the thread stack's, and
  that `emit` still picks up an open guard span. Verify the existing macro doctests and tests pass
  unchanged.
- [x] 1.3 Add links and status to the span record, and teach the OTLP/JSON encoder to write
  `links[{traceId, spanId}]` and `status{code: 2, message}` (D5). Make `Current::set_error` set the
  status as well as the `exception` event. Test the encoded payload for a linked, failed span. Verify
  that a span with neither property encodes byte-for-byte as it did at `26.9.8`.
- [x] 1.4 Add `OwnedSpan` and `trace::start(name, Parent, fields)` (D3). It is `Send + Sync` and
  holds its state behind a poison-tolerant `Mutex` off the thread stack. It has `context`,
  `set_attribute`, `add_event(_with)`, `add_link`, `set_error` and `end`, and ends on drop. Test the
  following:
  - Parentage under `Parent::Root`, `Parent::Context` and `Parent::Current` (inside a guard span).
  - Ending on a different thread from the one that started it.
  - The no-op handle when tracing is off.
  - That a guard `span!` opened while it is live does not parent to it.

  Verify with `cargo test --features o11y`.

## 2. The `tracing` bridge

- [x] 2.1 Add the `o11y-tracing` feature (`o11y`, `dep:tracing`, and `dep:tracing-subscriber` with
  `registry` and `std`, default features off) and the `o11y::tracing` module skeleton exposing
  `layer()`, `TracingLayer`, `.map_field()` and `.fold_spans()` (D6, D9). Add the feature to CI's
  per-feature `cargo check` matrix. Verify that `cargo check --no-default-features --features
  o11y-tracing` passes, and that a `--features o11y` build compiles no `tracing-subscriber` and gives
  `rust-sak` no `tracing` edge of its own (`cargo tree -e features`). `tracing` itself still arrives
  through `reqwest`'s `hyper` and `h2`, as it did at `26.9.8`.
- [x] 2.2 Implement event forwarding per D6's mapping table:
  - Levels.
  - Message to body, with the callsite-name fallback.
  - Typed field conversion, with `u64` overflow and `Debug` values becoming strings.
  - The `target` field.
  - Context from the nearest exported ancestor through `emit_in`, falling back to `emit`.
  - `map_field` applied to every field, where `None` drops it.

  Test each row of the table against records captured in-process.
- [x] 2.3 Implement span forwarding:
  - `on_new_span` to `trace::start`, parented by the nearest exported ancestor, and kept in the
    span's extensions.
  - `on_record` to attributes.
  - `on_follows_from` to a link.
  - The status set by a field named `error` or by an `ERROR` event inside the span.
  - `on_close` to end.

  Test a span created on one thread, entered on another and closed on a third. It must export once,
  with the right parent and links.
- [x] 2.4 Implement `fold_spans`: folded spans are not exported, their fields land on enclosed events
  (the innermost key wins), and their children parent past them. Test it with a replica of `ort`'s
  shape: a TRACE span named `ort` with `id` and `location`, given as an event's explicit parent inside
  an exported span. The event must carry `location` and the outer span's trace ids, and no `ort` span
  may be exported.
- [x] 2.5 Prove the gate and panic rules:
  - With telemetry never initialised, and after `shutdown()`, the layer records nothing and exports
    nothing.
  - A `map_field` closure that panics does not unwind out of the layer.

  Verify both with tests. Also put `#![deny(clippy::unwrap_used, clippy::expect_used,
  clippy::indexing_slicing)]` on the bridge module, and verify clippy passes with it.
- [x] 2.6 Add `tests/o11y_tracing_end_to_end.rs`, a separate integration binary because `init`
  succeeds once per process. It runs the following through `spawn_recording_collector`:
  - A registry with the layer, a nested span, and a `spawn`ed thread carrying the span.
  - Events at each level, a folded span, a mapped-out field, and a failing span.

  Assert on the captured OTLP payloads (parent ids, trace ids on logs, the link, `status.code`, and
  the missing field). Verify with `cargo test --features o11y-tracing --test o11y_tracing_end_to_end`.
- [x] 2.7 Document it all in `src/o11y/README.md`:
  - The runtime emit.
  - Owned spans and their non-goal of not being "current".
  - `SpanContext` and `traceparent`.
  - Links and status, including the `set_error` behaviour change.
  - A "Bridging `tracing`" section with the feature, the two hooks and the mapping table.

  Verify with `cargo test --doc --all-features`, which covers every new example.

## 3. Release `rust-sak`

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-features --all-targets -- -D
  warnings`, `cargo test --all-features`, `cargo test --doc --all-features`, and `cargo check
  --no-default-features --features o11y-tracing` in `../rust-sak`. Verify all five are green.
- [x] 3.2 **Get the repository owner's explicit go-ahead**, then push, bump the version to the CalVer
  number for the release month, and cut the tag (D10). Verify `git ls-remote --tags
  https://github.com/vegidio/rust-sak` lists the tag at the pushed commit.

## 4. Adopt the pin here

- [x] 4.1 Move the `rust-sak` pin to the new tag in `crates/opai/Cargo.toml`,
  `crates/gui/Cargo.toml` and `crates/perf/Cargo.toml` together. Change no features. Refresh
  `Cargo.lock` with `cargo metadata`, not `cargo update -p`. Verify the following:
  - `Cargo.lock` holds exactly one `rust-sak` entry, and no unrelated package moved.
  - `CARGO_HOME=$(mktemp -d) cargo fetch --locked` resolves from a cold cargo home.
- [x] 4.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo
  test --workspace --locked`, and verify all three are green. Then run `openspec validate --changes
  add-observability-1-o11y-bridge --strict` and verify it passes. Finally verify `opai`'s logging format tests pass unchanged: this slice installs no
  layer, so `opai.log` must not change.
