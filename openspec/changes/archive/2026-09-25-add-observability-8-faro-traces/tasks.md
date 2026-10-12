## 1. `rust-sak`

- [x] 1.1 In `../rust-sak`, add `o11y::tracing::with_parent(context, open)` (design.md D1). Use a thread-local
  restored by a drop guard. `new_span` passes `Parent::Context` only where it would otherwise use `Parent::Current`.
  Keep the module's no-panic lints. Test in `src/o11y/tracing/tests.rs`:
  - a span opened inside continues the context's trace, under its span
  - an exported ancestor still wins
  - a child of that span inherits the remote trace
  - nesting restores the outer context, and a panic inside `open` restores it too
  - an event inside `open` is routed as before
  - with tracing off, nothing is set

  Verify with `cargo test --features o11y-tracing o11y::tracing`.
- [x] 1.2 Document `with_parent` in the bridge's module header (parentage paragraph) and in `src/o11y/README.md`. Run
  `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings` and `cargo test --all-features` in
  `../rust-sak`, all green.
- [x] 1.3 Move the `26.9.10` tag (design.md D1). Commit 1.1 and 1.2 on top of `03bb633`, with no version bump.
  **Ask the owner before pushing** the commit, and before deleting and recreating the tag. Then delete `26.9.10`
  locally and on the remote, recreate it on the new commit, and push it. Verify with
  `git ls-remote --tags https://github.com/vegidio/rust-sak` that `26.9.10` names the new commit.

## 2. `opai`

- [x] 2.1 Leave the three `Cargo.toml` pins at `26.9.10`, and repoint `Cargo.lock`'s `rust-sak` source to the new
  commit, as slice 7 did. Verify that:
  - `Cargo.lock` holds one `rust-sak`, at the new commit
  - no other package moved
  - `CARGO_HOME=$(mktemp -d) cargo fetch --locked` resolves the tag cold
- [x] 2.2 Add `opai::telemetry::with_parent(traceparent: Option<&str>, open)` (D2), parsing with
  `SpanContext::from_traceparent` and delegating to the bridge. Test through the collector test helpers:
  - a span opened under a valid header is sent in the header's trace, with the header's span as its parent
  - a child unit span stays in that trace
  - a malformed header, an all-zero header, and `None` each root a new trace, and record nothing
  - the file line of a record inside carries no header

  Verify with `cargo test -p opai telemetry::`.

## 3. `gui` (Rust)

- [x] 3.1 In `command.rs`, add `Traceparent` with its `CommandArg` impl (D3). It reads `traceparent` from
  `command.message.headers()`, and never fails. Change `command_span!` to take it as its second argument and open the
  span inside `opai::telemetry::with_parent`. Update the module doc. Test:
  - `Traceparent` read with the header present, absent, and not UTF-8
  - a span opened under a header records only `outcome`, `error` and `run`, through `test_support::recorded`

  Verify with `cargo test -p gui command::`.
- [x] 3.2 Give each of the 23 traced commands a `traceparent: Traceparent` argument, and pass it to its
  `command_span!`. Update any direct calls in tests. Verify that `cargo test -p gui` passes, including
  `every_registered_command_opens_its_span` unchanged.

## 4. Frontend

- [x] 4.1 Add `@grafana/faro-web-tracing` ^2.12 to `crates/gui/package.json`. Verify with
  `pnpm --dir crates/gui install` and `pnpm --dir crates/gui exec tsc --noEmit`.
- [x] 4.2 In `lib/faro.ts` (D4, D5):
  - install `new TracingInstrumentation({ instrumentations: [] })`
  - make `startFaro(analytics, version)` take the version getter, and pass `appVersion` from `main.tsx`
  - add `traced(name, send)`: a `CLIENT` span named `name` with no attributes, a `traceparent` built from its
    context, status `ERROR` with no message on rejection, and the untraced call before start, after pause, or when
    anything about the span throws

  Widen `faro.guard.test.ts` to `@opentelemetry/`. Extend `faro.test.ts` with a mocked SDK:
  - the configuration now includes tracing with no instrumentations
  - `traced` before start and after pause calls `send()` with no headers
  - once started, `traced` passes a well-formed header and ends the span on resolve and on reject

  Verify with `pnpm --dir crates/gui test lib/faro` and `main`.
- [x] 4.3 Add `ipc/invoke.ts` with `call(command, args)` over `traced` (D5). It calls `invoke` without options when
  there is no header. Add `ipc/invoke.guard.test.ts`, which fails on `invoke` imported from `@tauri-apps/api/core`
  outside `ipc/invoke.ts`, `ipc/log.ts` and `ipc/analytics.ts`. Test `call` with and without a header. Verify with
  `pnpm --dir crates/gui test ipc/invoke`.
- [x] 4.4 Switch every `ipc/` module except `log.ts` and `analytics.ts` from `invoke` to `call`. Leave `Channel`,
  `convertFileSrc` and `listen` as they are. Keep each module's tests asserting the command name and arguments. Verify
  that the guard passes, and that `pnpm --dir crates/gui test` and `tsc --noEmit` are green.

## 5. Verification

- [x] 5.1 Run these, and verify all are green:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace --locked`
  - `pnpm --dir crates/gui test`
  - `pnpm --dir crates/gui exec tsc --noEmit`
  - Biome, scoped to the touched files

  Also verify that `openspec validate add-observability-8-faro-traces --strict` passes.
- [x] 5.2 Hand the owner a manual check on macOS (design.md Migration Plan), using a `gui` debug build with
  `OPAI_OTEL_ENDPOINT`, `OPAI_OTEL_AUTH` and `VITE_FARO_URL` set. Record in this task's note what was seen:
  - an enhancement's trace in Tempo is rooted at the window's `enhance` span, with the backend's `enhance` span and its
    unit spans beneath it
  - an image's `reveal_export` or `reveal_image` span carries no path
  - no `set_analytics` or `log` span arrives from either half
  - after Analytics is turned off, no further span arrives from either half

  Seen on 2026-09-25, in one debug session (09:42:06–09:43:56 UTC), read back from Tempo and `opai.log`:
  - Trace `d67e278b87b3ae6e74e5161287d90ae2` starts at the window's `enhance` span (Faro, `SPAN_KIND_CLIENT`). The
    backend's `enhance` span (`run=qlsqk0xj-6`, `outcome=finished`) is its child. `image_load`, `enhancement`, both
    `step`s, and their session, cache and install spans are all beneath it: 18 spans in one trace.
  - Trace `078abb9107f946a806181b3151c5073e`: the window's `reveal_image` span carries only `session.id`. The backend's
    child carries only `outcome`. No path on either.
  - No `set_analytics` or `log` span from either half.
  - Analytics was turned off at 09:43:21 (`opai.log`). Runs 7–10 followed, but nothing reached Tempo from 09:43:21 on.
  - As expected, some traces are rooted in the backend: the first `version` call, which is made before Faro starts (it
    supplies Faro's version), and the `image_load`/`image_encode` spans from `opai://` pixel requests, which are not
    commands.

