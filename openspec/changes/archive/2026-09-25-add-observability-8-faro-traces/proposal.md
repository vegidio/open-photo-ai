## Why

Since slice 7, Grafana holds two unconnected pictures of one request. Faro has the window's session, its events and
its exceptions. Tempo has the backend's trace for each command, rooted at the command. Nothing joins them. An
enhancement that took eight seconds cannot be read from the click that asked for it through to the model that ran it.
A window failure cannot be matched to the backend work it was waiting on.

Slice 7 dropped the session id it had first planned to share, on the grounds that this slice's `traceparent` makes a
finer join (slice 7, D1). This slice delivers that join.

## What Changes

This is slice 8 of `add-observability`, and the last slice in its table. It was split out of the planned slice 7
because it needs a change in `rust-sak` (slice 7, Slices). It has **13** tasks.

**`rust-sak` (`o11y-tracing`), with the `26.9.10` tag moved to it:**
- **The `tracing` bridge can parent a span to a remote context.** Today the bridge parents a new span to its nearest
  exported `tracing` ancestor, or else to the thread's `o11y` stack. A context parsed from a `traceparent` has no way
  in. A new `o11y::tracing::with_parent(context, open)` runs `open` so that a span it creates with no exported ancestor
  continues `context`'s trace, under `context`'s span.
- **The `26.9.10` tag is deleted and recreated on the new commit**, as slice 7 did for its `rust-sak` fix. No version
  is cut and no `Cargo.toml` pin moves. `Cargo.lock` is repointed to the new commit.

**`opai`:**
- **`opai::telemetry::with_parent(traceparent, open)`** takes the header's text, parses it with `rust-sak`'s
  `SpanContext::from_traceparent`, and wraps the bridge's call. An absent or malformed header opens the span exactly as
  today. `gui` does not take `rust-sak`'s `o11y` feature: `opai` stays the one owner of telemetry.

**`gui` (Rust):**
- **Every traced command reads the request's `traceparent` header**, through a small argument type that never fails.
  `command_span!` opens the command span under it. The header is not a span field, so it never reaches `opai.log`.
- **`command_span!` has no form without the header**, so a command that does not read it does not compile. The
  existing guard test, that every registered command opens its span, is unchanged.

**`gui` (frontend):**
- **`@grafana/faro-web-tracing`** is added, with no automatic instrumentation. There are no fetch or XHR spans: the
  window's traced unit is the request to the backend, not the HTTP traffic beneath it.
- **One wrapper around `invoke`.** Each request the window makes is one Faro span, named after the command. Its
  `traceparent` goes in the invoke's headers. So the window's span is the root, and the backend's command span and
  everything under it are its children, in one trace.
- **Two requests stay unwrapped**, the same two the backend exempts: `set_analytics`, because nothing about an opt-out
  may leave the process, and `log`, because a window failure is not a request. A guard test fails on `invoke` being
  imported anywhere else.
- **Nothing changes while the window is not sending.** Before Faro starts, after an opt-out, and in a build with no
  Faro collector, the wrapper sends no header. The backend then roots its trace at the command, as it does today.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-analytics`: the window's requests are sent as spans, and each carries its trace into the backend. What a
  request's span may carry is bounded as events are.
- `logging`: a graphical application's request continues the trace its window started, when the window names one. It
  is rooted at the request only when the window does not.

## Impact

**Crates touched:**
- `rust-sak` (its own repository): `o11y::tracing::with_parent`, its tests and README section, and the `26.9.10` tag
  moved to it.
- `opai`:
  - `telemetry::with_parent`
- `gui` (Rust):
  - `command.rs`: the header argument and `command_span!`
  - the 23 traced commands' signatures
- `gui` (frontend):
  - `@grafana/faro-web-tracing`
  - `lib/faro.ts`
  - a new `ipc/invoke.ts` and its guard test
  - the `ipc/` modules that call `invoke`
  - `main.tsx`
- `Cargo.lock`: `rust-sak` repointed to the commit the moved tag names.
- `cli` and `perf`: nothing.

**Why the frontend half is in `gui`:** Faro is a browser SDK, and the wrapper names the window's requests. The Rust
half splits along the usual line:
- Parsing the header and parenting a span to it is a telemetry mechanism that names no window, so it is in `opai`.
- Reading a Tauri request's headers is Tauri's, so it is in `gui`.

**`rust-sak`:** `o11y` already has `SpanContext`, `from_traceparent` and `Parent::Context` (slice 1, D4). The `tracing`
bridge does not reach them: its `on_new_span` picks the parent itself. Parenting a `tracing` span to a remote context
is general-purpose. Any service that receives a W3C `traceparent` and logs through `tracing` needs it. So it belongs in
`rust-sak`, not in `opai`. At the owner's direction it ships by moving the `26.9.10` tag rather than cutting a
version, as slice 7's fix did (design.md D1).

**Dependencies:** `@grafana/faro-web-tracing` 2.x, in the frontend only. It brings OpenTelemetry's web SDK. No Rust
crate is added.

**Outside the repository:** nothing. Faro's collector already accepts traces for the `opai` app, and the CSP already
allows its host (slice 7, D4).

**Parity with the Go application:** it had no traces, in either half. This slice goes past parity, as the series
decided ("Faro covers errors, usage events, and traces linked to Rust").

**Not in scope:**
- Faro's fetch and XHR instrumentation, and user-action spans.
- Tracing Tauri events and channels (`enhance:progress`, the setup channel). The window's span covers its request
  from invoke to answer. Progress is not traced.
- Sampling. Every session that sends is traced, as every session sends its events.
