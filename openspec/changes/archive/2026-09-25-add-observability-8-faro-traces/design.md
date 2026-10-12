## Context

The state slice 7 left, and what was measured for this one:

- **The backend roots a trace at each command.** `gui::command`'s `command_span!` opens an `info_span!` under
  `gui::command`, and `traced` instruments the body with it. `opai`'s unit spans take `Span::current()` as their
  parent, so everything a command does is beneath it. 23 of the 25 registered commands are wrapped. `set_analytics` and
  `log` are not. `lib.rs`'s `every_registered_command_opens_its_span` holds that set.
- **The bridge picks every span's parent itself.** In `rust-sak` `26.9.10`, `TracingLayer::new_span` calls
  `trace::start(name, parent, fields)`. `parent` is the nearest exported `tracing` ancestor's context, or else
  `Parent::Current`, the thread's `o11y` stack. There is no way to hand it a context from elsewhere.
- **`o11y` already has the rest.** Slice 1 gave it `SpanContext::from_traceparent`, which rejects all-zero ids and
  malformed lengths and ignores the flags. It also gave it `Parent::Context(SpanContext)`, which `start` reads as
  "this trace, under this span". An `OwnedSpan`'s trace id and parent are fixed in `start`.
- **The file writes a span's creation fields.** `logging` requires that the file carry the request's `run` and "no
  other field of the request's context". So a header recorded as a span field would break that requirement.
- **Tauri 2.11 carries invoke headers on both IPC paths.** `invoke(cmd, args, { headers })` sets them on the
  `ipc://` fetch in `ipc-protocol.js`, and the postMessage fallback carries `options.headers` into the same
  `InvokeRequest`. On the Rust side, `CommandItem.message` is public and `InvokeMessage::headers()` returns the
  `HeaderMap`. So a type implementing `CommandArg` can read a header, owned, beside a command's other arguments.
- **The window calls `invoke` from 13 `ipc/` modules**, directly, each with its own command-name constants. There is
  no shared call site. `lib/faro.ts` imports `appVersion` from `ipc/app.ts`.
- **`@grafana/faro-web-tracing` 2.12.1** was read for what this design uses:
  - `TracingInstrumentation({ instrumentations })` uses the list given, and only falls back to the fetch and XHR
    defaults when the option is absent (`??`). So `[]` installs none.
  - `faro.api.getOTEL()` returns `{ trace, context }`.
  - Its `FaroPausablePropagator` withholds `traceparent` while Faro is paused. This design injects the header itself,
    so it does not depend on that (D5).
- **Faro and the backend send to the same stack.** Faro's traces go to Tempo through the Frontend Observability
  collector, and the backend's go there over OTLP. Joining them needs nothing but a shared trace id.

## Goals / Non-Goals

**Goals:**

- One trace per window request, rooted in the window, with the backend's command span as the child of the window's
  span.
- A header that is absent, malformed or stale never changes how a request is served, and never reaches the file.
- A window that is not sending leaves the backend exactly as slice 7 left it.

**Non-Goals:**

- Fetch, XHR and `opai://` image requests as spans. Faro's user-action spans.
- Tracing progress events and channels. A request's span ends when the invoke is answered.
- A `traceparent` flowing the other way, from the backend into the window.
- Honouring the header's sampled flag. The backend does not sample, and the window sends only sampled sessions.

## Slices

Full observability, in order. Only the current slice is proposed; the rest are proposed when they are reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-observability-1-o11y-bridge` | `rust-sak` `o11y`: runtime log emit, owned spans with explicit parents, span context + `traceparent`, links, status, and the `tracing` bridge behind `o11y-tracing`; released as `26.9.9`; pins bumped in `opai`, `gui` and `perf` | - | done |
| 2 | `add-observability-2-telemetry-seam` | `rust-sak`: the `set_attribute` fix released as `26.9.10`. `opai`: the subscriber becomes a registry with a file layer and a Grafana layer (each with its own filter); `opai::telemetry` start/stop; export failures recorded file-only; span context carried through `task::spawn_blocking`; endpoint and auth baked in via `option_env!`; `logging` spec updated. `gui`: start in `setup`, stop on exit | 1 | done |
| 3 | `add-observability-3-record-cleanup` | `opai`: the existing records fixed. Silent failures recorded (initialize, session build, image IO, analysis refusal); each failure recorded once (step folded into run, refusal outside the bracket, unread signal folded into the conclusion); cancellations and shutdowns recorded as stops; durations and subjects added to closing records; `error` reserved for panics. `gui`: its repeats of `opai`'s failures removed | 2 | done |
| 4 | `add-observability-4-spans-metrics` | `rust-sak`: a public read-back on metric instruments, released. `opai`: spans on every unit of work, sent to the collector only; waiters linked to the build they joined; outcome and failed status on every span; the metric instruments | 3 | done |
| 5 | `add-observability-5-gui-switch` | `gui`: the Analytics choice mirrored into a Rust-owned file, read in `setup` before telemetry starts; opting out stops sending at once; one wrapper giving every command a span with its `run` and outcome, and recording once the failures only `gui` observes; `gui`'s blocking hops carry the span; the session's end recorded; the `eprintln!`s in `single_instance.rs` made records | 4 | done |
| 6 | `add-observability-6-frontend-log` | `gui`: a `log` command writing the window's records to the file and the collector, capped, with no span. Frontend: one reporter that skips failures Rust already recorded, deduplicates and bounds per load; an ErrorBoundary with a recovery screen (Reload, Show logs); global `error`/`unhandledrejection` handlers; every console-only failure routed through the reporter | 5 | done |
| 7 | `add-observability-7-faro` | `rust-sak`: `session_id()`/`is_enabled()` answer as stopped after `shutdown()` (tag `26.9.10` moved). `opai`: a `file_only` marker the collector filter drops. `gui`: the window's records made file-only; CSP opened for `*.grafana.net`. Frontend: Faro (session and view only) started from the stored Analytics choice and `VITE_FARO_URL`, and paused before an opt-out reaches Rust; window failures sent as Faro exceptions through the reporter; a curated catalogue of usage events | 5, 6 | done |
| 8 | `add-observability-8-faro-traces` | `rust-sak`: `o11y::tracing::with_parent`, so the bridge parents a span to a remote context; tag `26.9.10` moved to it. `opai`: `telemetry::with_parent` over a `traceparent`'s text. `gui`: a `traceparent` read from each invoke's headers and given to the command span. Frontend: Faro's tracing with no automatic instrumentation, one span per `invoke` carrying the header, so window and backend spans form one trace | 7 | **current** |

This is the last slice in the table. When it is archived the series is complete.

### Decisions already taken for slices 2 to 8

Settled with the repository owner during exploration, and repeated in every slice's design.md. Two were revised, and
are marked as such.

- **`tracing` stays the one logging API.** Grafana is a second layer beside the file, fed by slice 1's bridge. There is
  no second set of `o11y::log` calls.
- **Analytics defaults to on and sends from the first launch.** Opting out stops sending at once (`o11y::shutdown()`
  and `faro.pause()`). ~~Opting in takes effect at the next launch for the backend; Faro may resume at once.~~
  **Revised in slice 7:** opting in takes effect at the next launch for both halves.
- ~~**Path fields never reach Grafana.**~~ **Revised in slice 2:** paths are not scrubbed in the backend's records.
  The window's events carry no paths (slice 7, D7). Its exceptions carry error texts on the same terms as the
  backend's records.
- **ONNX Runtime warnings and above go to Grafana too**, with the same floor as the file.
- **Only the GUI exports.** `cli` and `perf` keep writing the file only.
- **Endpoints and credentials are baked into release builds from CI secrets.** A build without them sends nothing.
  Release builds report `production`, anything else `development`. (No release workflow maps the secrets yet; see
  slice 5.)
- **Faro covers errors, usage events, and traces linked to Rust.** Web vitals are out. Frontend errors also reach
  `opai.log` through slice 6's log command, because Faro only sends to Grafana. From slice 7, the log command's
  records stay in the file and Faro is the window's only path to Grafana.
- **Divergence from the Go application, deliberate:**
  - It exported to Grafana whatever the Analytics switch said; this honours the switch (from slice 5).
  - It exported error logs only; this adds metrics and traces.
  - It used Aptabase for frontend events; Faro replaces it.
- **`imaging` stays silent.** Its callers in `opai` instrument per pass or per run, never per tile.

## Decisions

### D1 - `rust-sak`: `o11y::tracing::with_parent`, a scope rather than a field

```rust
/// Runs `open` so that a span the layer opens inside it, on this thread, with no exported `tracing` ancestor,
/// continues `context`'s trace as a child of `context`'s span.
pub fn with_parent<T>(context: SpanContext, open: impl FnOnce() -> T) -> T;
```

- **A thread-local** holds the context for as long as `open` runs. `new_span` reads it only where it would otherwise
  fall back to `Parent::Current`, and passes `Parent::Context(context)` instead.
- **An exported ancestor still wins.** The remote context stands in for "nothing local above this span". It does not
  re-parent a span that already has a local parent.
- **Restored on exit, by a drop guard.** So nesting restores the outer context, and a panic inside `open` does not leave
  a context behind for the thread's next span.
- **Spans only.** Events emitted inside `open` are routed as today. An event belongs to the span it is emitted in, and
  a remote span is not one this process can emit into.
- **It keeps the bridge's rules:**
  - When tracing is off, `open` runs with nothing set. The layer's hooks are already gated.
  - It never panics, under the module's `unwrap`, `expect` and indexing lints.
- **Documented** in the module header and in `o11y/README.md`, beside the bridge's parentage paragraph.

*Alternative:* a field the bridge consumes, as in `info_span!("enhance", traceparent = %header)`. Rejected. Every other
layer sees the field too, and `opai`'s file layer would write it onto every record in the request. The `logging` spec
forbids that. Hiding it would take a second convention in the file formatter.

*Alternative:* `set_parent(&tracing::Span, SpanContext)` after creation, the shape `tracing-opentelemetry` offers.
Rejected. That crate builds its span at close, so the parent can change until then. The bridge starts its `OwnedSpan`
in `on_new_span`, with its trace id fixed and possibly already inherited by a child. Re-parenting a live span would mean
rewriting its identity.

**Shipped by moving the `26.9.10` tag, at the owner's direction.** Slice 7 did the same for its `rust-sak` fix. The
change is committed on top of the commit `26.9.10` names today (`03bb633`), with no version bump. The tag is then
deleted, locally and on the remote, and recreated on the new commit. Both pushes wait for the owner's go-ahead (the
project's rule for `rust-sak`). No `Cargo.toml` pin moves. `Cargo.lock` records the commit a tag resolved to, so it is
repointed to the new commit. The tag is then proven to resolve from a cold cache.

*Alternative:* cut `26.9.11` and bump the three pins. This was the first plan, and the owner chose the moved tag
instead. It keeps one tag for the series' `rust-sak` work, and `with_parent` is additive, so nothing that built
against the old `26.9.10` breaks.

### D2 - `opai::telemetry::with_parent` takes the header's text

```rust
/// Runs `open` so that the span it opens continues the trace `traceparent` names. Where `traceparent` is `None`, or
/// not a W3C `traceparent`, `open` runs as it would without it.
pub fn with_parent<T>(traceparent: Option<&str>, open: impl FnOnce() -> T) -> T;
```

- **Parsing is `SpanContext::from_traceparent`.** The all-zero and length rules are `rust-sak`'s and are not repeated.
- **A malformed header is silent.** It comes from this application's own window, so a bad one is a bug a test catches.
  It is not a runtime condition worth a record in every request.
- **`gui` calls this, not `rust-sak`.** `gui` does not enable `o11y`, and `opai` stays the one crate that knows how
  telemetry is wired. The function names no window, so `cli` could use it as well.

*Alternative:* return an `opai` context type that `gui` passes back. Rejected. The text is all `gui` has, and the type
would exist only to be round-tripped.

### D3 - `gui`: an argument that reads the header, required by `command_span!`

```rust
/// The `traceparent` header of the request being served, if the window sent one. Never fails a request.
pub(crate) struct Traceparent(Option<String>);

impl<'a, R: Runtime> CommandArg<'a, R> for Traceparent { /* command.message.headers().get("traceparent") */ }
```

- **Each of the 23 traced commands gains a `traceparent: Traceparent` argument.** Tauri fills it from the invoke. It is
  owned, so async commands keep their `'static` bodies and their current return types.
- **`command_span!` takes it as its second argument:** `command_span!("enhance", traceparent, run = run)`. The macro
  opens the span inside `opai::telemetry::with_parent(traceparent.as_deref(), || info_span!(...))`. It has no arm
  without the header, so a command that forgets the argument does not compile. The name stays first, so the existing
  guard test goes on reading it unchanged.
- **The header is not recorded on the span.** The span's fields stay `outcome`, `error` and `run` (Context).
- **Tests:**
  - `Traceparent` read from a message with, without and with a non-UTF-8 header.
  - The file line inside a span opened under a header carries `run` and no header, through `test_support::recorded`.
  - Whether the span reaches the collector under the header's trace is `opai`'s test (D2), because the collector's
    test support is private to `opai`. The same split slice 7 made for `file_only`.

*Alternative:* `tauri::ipc::Request<'_>` as the argument. Rejected. It borrows, which forces an async command to
return `Result`, and several do not. It also exposes the raw body, which no command wants.

*Alternative:* read the header once in the `invoke_handler` closure. Rejected, for slice 5's reason: the closure returns
before an async command's body runs, on another thread, so the value has no way to reach the span.

### D4 - Faro's tracing, with nothing automatic

`startFaro` adds `new TracingInstrumentation({ instrumentations: [] })` beside session and view.

- **No fetch or XHR instrumentation.** Tauri's `invoke` is itself a fetch to `ipc://localhost`, and the canvas loads
  pixels over `opai://`. The defaults would give every request a second, anonymous HTTP span and inject a header of
  their own. The window's unit is the request to the backend (D5), traced once.
- **`startFaro(analytics, version)`** takes the version getter from `main.tsx` rather than importing `ipc/app.ts`. D5
  routes `ipc/app.ts` through `ipc/invoke.ts`, which imports `lib/faro.ts`. Without this change the imports would form
  a cycle.
- **The guard test** (`faro.guard.test.ts`) widens to `@opentelemetry/` too. `lib/faro.ts` stays the only module that
  touches either SDK.

*Alternative:* the default instrumentations, with `propagateTraceHeaderCorsUrls` matching `ipc://localhost`. Rejected.
It traces the transport, not the request. The span would be named after an HTTP method and URL rather than the
command, and whether the header rode along would depend on which IPC path Tauri took.

### D5 - `ipc/invoke.ts`: one call site for every traced request

```ts
// lib/faro.ts
export const traced = <T>(name: string, send: (headers?: Record<string, string>) => Promise<T>): Promise<T>;

// ipc/invoke.ts
export const call = <T>(command: string, args?: InvokeArgs): Promise<T> =>
    traced(command, (headers) => (headers ? invoke<T>(command, args, { headers }) : invoke<T>(command, args)));
```

- **`traced`, when Faro is running,** starts a span named after the command, with kind `CLIENT`, through
  `getOTEL().trace.getTracer("opai")`. It builds `00-<trace id>-<span id>-<flags>` from the span's own context, and
  ends the span when `send` settles. A rejection sets status `ERROR` with no message: the backend's span carries the
  reason (spec, gui-analytics). The span has no attributes.
- **When Faro is not running** (before start, after `pauseFaro`, or in a build or launch that sends nothing), `traced`
  calls `send()` with no headers. Callers never check, as with every other export.
- **Headers only when there is one**, so `invoke` is called exactly as today when nothing is traced. The existing
  `ipc/*.test.ts` assertions on `invoke(name, args)` hold unchanged.
- **Every `ipc/` module calls `call`, except `log.ts` and `analytics.ts`,** which keep `invoke`. These are the same
  two commands the backend leaves untraced. `set_analytics` must never be reported. `log` would root a window span per
  failure record, beside the Faro exception that already carries it.
- **A guard test** (`ipc/invoke.guard.test.ts`) fails when `invoke` is imported from `@tauri-apps/api/core` anywhere
  other than `ipc/invoke.ts`, `ipc/log.ts` and `ipc/analytics.ts`. It is the window's twin of
  `every_registered_command_opens_its_span`.
- **`traced` never throws into the caller.** A failure to start a span or to build the header falls back to the
  untraced call. A request is worth more than its span, as `track` already decides for events.

*Alternative:* a `traceparent` argument added to each command's arguments object. Rejected. It puts telemetry in every
command's wire type, and each command's Rust signature would have to name it, where a header travels beside the
arguments and only the wrapper reads it.

### D6 - Opting out

Nothing new is needed. `pauseFaro` clears the instance, so the next `traced` sends no header, and Faro's paused
transport drops any span that ends after the pause. A request already in flight at the opt-out finishes. Its window
span is dropped and its backend span is not sent, because `o11y` has already shut down.

## Division across crates

- **`rust-sak`:** `o11y::tracing::with_parent` (D1).
- **`opai`:** `telemetry::with_parent` (D2). A telemetry mechanism that names no window.
- **`gui` (Rust):** `Traceparent`, `command_span!`, and the 23 commands' signatures (D3).
- **`gui` (frontend):** the dependency, `lib/faro.ts`'s `TracingInstrumentation` and `traced` (D4), and `ipc/invoke.ts`
  with its guard and the `ipc/` modules (D5). Also `main.tsx`'s version getter.
- **`cli` and `perf`:** nothing. `perf`'s pin names the same tag and resolves through the same `Cargo.lock` entry.

## `rust-sak` and crates

- **`o11y` + `o11y-tracing`:** `SpanContext::from_traceparent`, `Parent::Context`, and the new `with_parent`, through
  `opai`. `gui` keeps its current `rust-sak` features. No pin moves. `Cargo.lock`'s one `rust-sak` entry is repointed to the
  moved tag's commit.
- **No Rust crate is introduced.**
- **`@grafana/faro-web-tracing` ^2.12** is Grafana's own tracing package for the Faro SDK already in use. It brings
  `@opentelemetry/api` and `sdk-trace-web`, which nothing else here needs directly. `lib/faro.ts` imports only from
  `@grafana/faro-*`.

## Risks / Trade-offs

- **[Risk] The window's clock and the backend's disagree,** so a child span appears to start before its parent in
  Tempo. → They run on one machine, with one clock. The window's `performance` time origin is the only skew, and it is
  sub-millisecond.
- **[Risk] Faro's collector and the OTLP endpoint land traces in different Tempo tenants,** and the join is invisible.
  → The manual check (6.2) looks up one trace id and expects both services in it. If they are split, the fix is stack
  configuration, not code.
- **[Risk] A Tauri update drops custom headers on one IPC path.** → Both paths were read in 2.11.5 (Context). A dropped
  header degrades to slice 7's behaviour, a backend-rooted trace, and fails nothing.
- **[Trade-off] Long requests hold a window span open for their whole run**, an export for minutes. → That is the span's
  meaning: from asking to being answered. Faro batches the span when it ends.
- **[Trade-off] Progress events are not in the trace.** → The backend's unit spans already say where the time went.
- **[Risk] A cache holds `26.9.10` at its old commit,** such as CI's restored git db or another machine's
  `~/.cargo/git`. → `Cargo.lock` names the new commit, so cargo fetches it rather than trusting the cached tag. The
  cold-cache fetch (task 2.1) proves the new commit is reachable from the tag on the remote.
- **[Risk] A future command added without `Traceparent`.** → `command_span!` does not compile without it, and the guard
  test already fails on an unwrapped command.

## Migration Plan

1. `rust-sak`: `with_parent`, its tests, and README, committed on top of `03bb633`. Then, with the owner's go-ahead,
   push the commit, delete `26.9.10` locally and on the remote, and recreate and push it on the new commit.
2. Repoint `Cargo.lock` to the new commit, and prove `26.9.10` resolves to it from a cold `CARGO_HOME`.
3. `opai`: `telemetry::with_parent`, with tests through the collector helpers.
4. `gui` Rust: `Traceparent`, `command_span!`, and the 23 signatures.
5. Frontend: the dependency, `TracingInstrumentation` and `traced`, `ipc/invoke.ts` and its guard, then the `ipc/`
   modules.
6. Validation:
   - **CI on Linux, macOS and Windows:** `cargo test --workspace --locked`, `pnpm test`, `tsc --noEmit`, and Biome
     scoped to the touched files.
   - **A manual check on macOS, handed to the owner** (the rule against driving the GUI). It uses a debug build with
     `OPAI_OTEL_*` and `VITE_FARO_URL` set: an enhancement's trace in Tempo holds the window's `enhance` span as its
     root, with the backend's `enhance` span and its unit spans beneath it. A `reveal_export` span carries no path.
     After Analytics is turned off, no further spans arrive.
   - **Windows (WebView2) and Linux (WebKitGTK)** are covered by CI's tests only. No execution provider is involved.

**Rollback:** revert this repository's commits. `Cargo.lock` then names `03bb633` again, which is still reachable
from the moved tag because the new commit sits on top of it. The tag stays where it is: `with_parent` is additive.
