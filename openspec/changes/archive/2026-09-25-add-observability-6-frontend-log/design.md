## Context

The state slice 5 left the window in:

- **Every command rejects with a tagged object.** Each error enum that crosses the boundary is
  `#[serde(tag = "kind")]`, so a rejection from a command body is always `{ kind: "…", … }`. Slice 5's D4 classifies
  every variant as a stop, a failure `opai` recorded, or a failure the wrapper recorded. A tagged rejection is
  therefore always already accounted for in `opai.log`.
- **Rejections that never reach a command body are strings.** Tauri produces them when a command name is unknown,
  an argument fails to deserialize, or a plugin command such as `plugin:event|listen` fails. `invoke` throws a
  `TypeError` where there is no Tauri underneath, as with `vite` opened directly and in every test. None of these
  passes through `gui::command`, so none is recorded.
- **24 call sites catch a failure and write it only to the console,** through `console.error` or `console.warn`:
  - `main.tsx`, `App.tsx`, `lib/analytics.ts`
  - `Navbar.tsx` (×2), `AboutDialog.tsx`, `GeneralPage.tsx`, `SetupFailure.tsx`, `FileOptionsMenu.tsx`
  - `ExportDialog.tsx`, `ExportRow.tsx` (×2), `export/batch.ts`
  - `stores/enhancements.ts`, `stores/exportBatch.ts`, `stores/autopilot.ts`
  - `useAutopilot.ts` (×2), `useOpenImages.ts`, `useDroppedImages.ts`, `useEnhancementRun.ts` (×4)

  Most report a command rejection, which is already in the file. The rest are the clipboard refusals, the event
  subscriptions, and any IPC failure that came back as a string. Those are recorded nowhere.
- **Two silent catches are deliberate, and stay:**
  - `particles.tsx`'s `catch {}` is there because there is no Tauri, in tests only.
  - `useEnhancementRun.ts`'s `.catch(() => {})` on an unlisten whose registration failure was already reported.
- **Nothing catches what escapes.** There is no ErrorBoundary and no global handler. A render that throws unmounts
  the whole tree. The window ships hidden (`visible: false`), and `RevealWindow` shows it from a mount effect. The
  Rust fallback in `window.rs` then shows it after its grace period, so a crash on the first render is a blank
  window.
- **The file already fits the records.** `opai`'s file and collector filters both default to `info` for every
  target not otherwise named, so `gui::frontend` reaches both with no change in `opai`. The formatter escapes `\n`
  inside a value, so a stack stays on one line (`format.rs`,
  `a_newline_in_a_value_cannot_split_a_record_in_two`).
- **The Go application** had the same boundary (`features/shared/ErrorBoundary`) and the same two global handlers
  (`main.tsx`), all console-only, plus an Aptabase event. Its `errors.boundary.{title,message,reload}` strings are
  translated in all 13 locales this application ships. This application already has `settings.app.logs.button`
  ("Show logs") and `errors.showLogsFailed`.
- **The Claude Design project has no error screen.** Its 31 screens (01 to 19) cover none. The recovery screen is
  built from the design system's existing tokens and `Button`, in the manner of the setup failure dialog (02b).

## Goals / Non-Goals

**Goals:**

- The window's code decides nothing about the log's shape. Rust writes the record, caps it and gives it its target,
  and the window only says what happened.
- One rule separates "already recorded" from "only the window saw this", and it lives in one function.
- No `console.error` or `console.warn` outside the reporter, enforced by a test, so a call site added later cannot
  silently skip the file.
- A crash on the first render still ends with a visible window.

**Non-Goals:**

- Faro, `traceparent`, and deduplicating against Faro (slice 7).
- Informational or debug records from the window.
- Source maps. A release stack names bundled chunks. It still shows the throwing site's shape and the component path,
  and mapping it back is a developer's job with the matching build.
- Recovering without a reload, such as a per-region boundary that keeps the rest of the window alive. One boundary
  at the root is the Go application's shape, and a finer one is a UX decision for another day.

## Slices

Full observability, in order. Only the current slice is proposed; the rest are proposed when they are reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-observability-1-o11y-bridge` | `rust-sak` `o11y`: runtime log emit, owned spans with explicit parents, span context + `traceparent`, links, status, and the `tracing` bridge behind `o11y-tracing`; released as `26.9.9`; pins bumped in `opai`, `gui` and `perf` | - | done |
| 2 | `add-observability-2-telemetry-seam` | `rust-sak`: the `set_attribute` fix released as `26.9.10`. `opai`: the subscriber becomes a registry with a file layer and a Grafana layer (each with its own filter); `opai::telemetry` start/stop; export failures recorded file-only; span context carried through `task::spawn_blocking`; endpoint and auth baked in via `option_env!`; `logging` spec updated. `gui`: start in `setup`, stop on exit | 1 | done |
| 3 | `add-observability-3-record-cleanup` | `opai`: the existing records fixed. Silent failures recorded (initialize, session build, image IO, analysis refusal); each failure recorded once (step folded into run, refusal outside the bracket, unread signal folded into the conclusion); cancellations and shutdowns recorded as stops; durations and subjects added to closing records; `error` reserved for panics. `gui`: its repeats of `opai`'s failures removed | 2 | done |
| 4 | `add-observability-4-spans-metrics` | `rust-sak`: a public read-back on metric instruments, released. `opai`: spans on every unit of work, sent to the collector only; waiters linked to the build they joined; outcome and failed status on every span; the metric instruments | 3 | done |
| 5 | `add-observability-5-gui-switch` | `gui`: the Analytics choice mirrored into a Rust-owned file, read in `setup` before telemetry starts; opting out stops sending at once; one wrapper giving every command a span with its `run` and outcome, and recording once the failures only `gui` observes; `gui`'s blocking hops carry the span; the session's end recorded; the `eprintln!`s in `single_instance.rs` made records | 4 | done |
| 6 | `add-observability-6-frontend-log` | `gui`: a `log` command writing the window's records to the file and the collector, capped, with no span. Frontend: one reporter that skips failures Rust already recorded, deduplicates and bounds per load; an ErrorBoundary with a recovery screen (Reload, Show logs); global `error`/`unhandledrejection` handlers; every console-only failure routed through the reporter | 5 | **current** |
| 7 | `add-observability-7-faro` | frontend: Grafana Faro for errors and console, usage events and traces, with a `traceparent` passed into the Tauri commands so frontend and backend spans form one trace; CSP opened for the collector; paused and resumed on the Analytics switch | 5, 6 | pending |

Slice 6's row now says where its records go. The planned row said only "a log command into `opai.log`". The owner
chose to send the window's records to the collector as well, as the `logging` spec already requires of "the front
end's" records, and to leave the overlap with Faro's own error capture to slice 7.

### Decisions already taken for slices 2 to 7

Settled with the repository owner during exploration, and repeated in every slice's design.md. One was revised in
slice 2 and is marked as such.

- **`tracing` stays the one logging API.** Grafana is a second layer beside the file, fed by slice 1's bridge. There is
  no second set of `o11y::log` calls.
- **Analytics defaults to on and sends from the first launch.** Opting out stops sending at once (`o11y::shutdown()`
  and `faro.pause()`). Opting in takes effect at the next launch for the backend; Faro may resume at once.
- ~~**Path fields never reach Grafana.**~~ **Revised in slice 2:** paths are not scrubbed. Records reach Grafana with
  their paths as `opai.log` has them. This slice's records carry error texts, which may name paths, on the same terms.
- **ONNX Runtime warnings and above go to Grafana too**, with the same floor as the file.
- **Only the GUI exports.** `cli` and `perf` keep writing the file only.
- **Endpoints and credentials are baked into release builds from CI secrets.** A build without them sends nothing.
  Release builds report `production`, anything else `development`. (No release workflow maps the secrets yet; see
  slice 5.)
- **Faro covers errors and console, usage events, and traces linked to Rust.** Web vitals are out. Frontend errors also
  reach `opai.log` through this slice's log command, because Faro only sends to Grafana.
- **Divergence from the Go application, deliberate:**
  - It exported to Grafana whatever the Analytics switch said; this honours the switch (from slice 5).
  - It exported error logs only; this adds metrics and traces.
  - It used Aptabase for frontend events; Faro replaces it.
- **`imaging` stays silent.** Its callers in `opai` instrument per pass or per run, never per tile.

## Decisions

### D1 - `log`: one command, one record, no span

```rust
// crates/gui/src/frontend.rs
pub(crate) const TARGET: &str = "gui::frontend";

#[derive(Deserialize)] #[serde(rename_all = "lowercase")]
pub(crate) enum Level { Warn, Error }

#[derive(Deserialize)] #[serde(rename_all = "camelCase")]
pub(crate) struct WindowRecord {
    level: Level,
    message: String,
    error: Option<String>,
    stack: Option<String>,
    component_stack: Option<String>,
}

#[tauri::command]
pub(crate) fn log(record: WindowRecord)
```

- **What it writes.** One `tracing` event under `TARGET`, at the record's level, with `message` as its body and
  `error`, `stack` and `component_stack` as fields when present. It emits through a `match` on the level, since
  `tracing`'s macros take the level at compile time.
- **Infallible, and synchronous.** There is nothing to fail once the argument has deserialized. A malformed argument
  is rejected by Tauri before the body runs, and the reporter drops that rejection (D2).
- **Capped here** (D4), because the record's size is a property of the file, and the file is Rust's.
- **No `command_span!`.** The drift test's exemption list gains `log` beside `set_analytics`, with the reason. A span
  per record would root one trace per failure in Tempo, holding one event and nothing else, and the event is the
  whole of what matters.
- **Only two levels.** The `logging` spec's severity rule gives `error` to what does not survive, which for the window
  is a render crash, and `warn` to every failure. The window has nothing informational to say yet. When it does, a
  third variant is a one-line change on both sides.

*Alternative:* a generic `tracing` bridge such as `tauri-plugin-log`'s `attachConsole`/`log` pair. Rejected. It
writes its own format through its own sinks, beside `opai`'s subscriber rather than into it. Its `webview` target
would forward every `console.*` call, including the debugging noise that a report's reason does not need.

*Alternative:* put the command in `logs.rs`. Rejected. That module is where the log file *is* and how it is shown. A
record written from the window is a different subject, and its target should read as the window's.

### D2 - `report`: one function decides what reaches the file

```ts
// lib/report.ts
export const report = (message: string, error: unknown) => void;          // warn
export const reportCrash = (error: unknown, componentStack?: string) => void; // error, for the boundary alone
```

Both write to the console first, as every call site does today, since that is still what a developer reads in the
webview. Then:

1. **A command's answer is not sent.** `alreadyRecorded(error)` holds for any non-null object with a string `kind`.
   Slice 5 made every such rejection either a stop or a failure Rust recorded (Context). The check is structural
   rather than a list of kinds, so a variant added to an enum later needs no edit here. The drift test and slice 5's
   exhaustive `match` already force that variant to be classified.
2. **The rest is described.**
   - An `Error` gives `name: message` as `error`, and its `stack`.
   - A string is the `error` itself.
   - Anything else is `String(value)`, or `JSON.stringify` for a plain object.
3. **It is deduplicated and counted** (D4), then sent through `ipc/log.ts`.
4. **A send that rejects is dropped without a word.** No console line, no second `report`. A bridge that is gone
   would otherwise report its own failure with every call, and the console already has the original failure.

`reportCrash` skips step 1, because a render error is never a command's answer.

*Alternative:* have each call site decide whether to send. Rejected. It is the same test written 24 times, and the
one that gets it wrong records a failure twice, which the `logging` spec forbids.

*Alternative:* list the tagged kinds known to be recorded. Rejected, because every kind is recorded or is a stop.
The list would be every kind, and would drift.

### D3 - The boundary, the handlers, and where each sits

- **`components/ErrorBoundary.tsx`**, a class component, since React has no hook for `getDerivedStateFromError`.
  - `componentDidCatch` calls `reportCrash(error, info.componentStack)`.
  - It renders `RecoveryScreen` in place of its children.
- **`RecoveryScreen`** is a function component, so it can use `useTranslation`, which the Go boundary could not.
  - It carries the title, the sentence, and the error text in a selectable monospace block.
  - **Reload** calls `window.location.reload()`.
  - **Show logs** calls `revealLog`. A failure is reported through `report` and shown with
    `toast.error(t("errors.showLogsFailed"))`, exactly as `GeneralPage`'s `LogsRow` does.
  - It is laid out on `--background` with the dialog's surface and `Button`, like screen 02b.
- **Placement in `main.tsx`:**
  ```tsx
  <AppProviders>
      <ErrorBoundary><App /></ErrorBoundary>
      <RevealWindow />
  </AppProviders>
  ```
  - **Inside `AppProviders`,** so the recovery screen has the `Toaster` and the tooltip provider.
  - **`RevealWindow` outside the boundary.** A crash on the first render then still commits the fallback and runs
    `RevealWindow`'s mount effect, so the window is shown with the recovery screen on it rather than hidden until
    Rust's grace period, or blank.
- **`lib/unhandled.ts`**'s `watchUnhandled()` is called in `main.tsx` beside `mirrorAnalytics()`, before
  `createRoot`, so a failure in the first render's own effects is caught. It installs:
  - `error`: `report("an error nothing caught", event.error ?? event.message)`
  - `unhandledrejection`: `report("a promise rejection nothing handled", event.reason)`

  Without capture, so resource-load errors on elements, which do not bubble, stay out. A tagged rejection nobody
  handled is still skipped by D2's first rule: Rust has it.
- **React 19 routes errors two ways.** An error a boundary caught goes to `onCaughtError`, which defaults to the
  console, not to `window`. An uncaught one goes through `reportError`, which fires `window`'s `error` event. So a
  crash inside the boundary is recorded once, by `componentDidCatch`. A crash outside it, such as in `AppProviders`
  or `RevealWindow`, is recorded once, by the `error` handler. A test pins the first case to exactly one record.

*Alternative:* `react-error-boundary`. Rejected: a dependency for one class of about twenty lines, which the project
has no second use for.

### D4 - Bounds: repeats and counts in the window, lengths in Rust

- **In the window** (`lib/report.ts`, module state, reset by a page load):
  - A record whose `level + message + error` has been sent already in this load is not sent again.
  - At most **50** records are sent per load. The 51st sends one `warn`, "the window is recording too many failures;
    the rest of this load's are dropped", and every later one is dropped.
  - A `forget()` export, for tests alone, resets both, as `ipc/once.ts` does.
- **In Rust** (`frontend.rs`), each text is cut on a `char` boundary and suffixed with `… (cut short)` when it is
  longer than its cap:
  - `message`: 1 KiB
  - `error`: 4 KiB
  - `stack` and `component_stack`: 8 KiB each

  A capped record is at most about 21 KiB. So a load's worst case is about a megabyte, against the seven days of
  rotation `opai.log` already keeps.

The split follows who can know. Only the window can tell a repeat from a new failure within one load, and only Rust
decides what a line of its file may hold. The window is this application's own code, so the Rust cap is a bound on
size, not a defence against a hostile caller.

### D5 - Every console-only failure goes through `report`

Each of the 24 sites keeps its sentence, which becomes `report`'s `message`, and its surrounding handling: toasts,
store updates and return values are unchanged. `console.warn` sites become `report` too. Two have been `warn`s
because the window carried on, which is what `warn` means in the file anyway.

A vitest guard, `lib/report.guard.test.ts`, reads every non-test `.ts`/`.tsx` under `frontend/`, outside
`node_modules`, and fails on `console.error(` or `console.warn(` anywhere but `lib/report.ts`. It is the frontend twin
of `lib.rs`'s drift test, read from the sources the same way.

The comment in `lib/analytics.ts` that anticipates this slice ("the log command's job, when there is one") is
rewritten to say what now happens.

### D6 - Strings, and nothing new to translate

- **Three keys are ported from the Go locales,** in all 13:
  - `errors.boundary.title`
  - `errors.boundary.message`
  - `errors.boundary.reload`

  The message's promise that settings are kept holds here too, because settings live in `localStorage`, which a
  reload keeps.
- **Show logs reuses `settings.app.logs.button`,** and its failure reuses `errors.showLogsFailed`.
- **`i18n/parity.test.ts`** already fails on a key missing from any locale.

### Division across crates

- **`gui` (Rust):** `frontend.rs` (D1, and D4's caps), and its registration and exemption in `lib.rs`.
- **`gui` (frontend):**
  - `ipc/log.ts`
  - `lib/report.ts` (D2, D4)
  - `lib/unhandled.ts`, `components/ErrorBoundary.tsx` and `RecoveryScreen` (D3)
  - the 24 call sites (D5)
  - the locale keys (D6)
- **`opai`, `cli` and `perf`:** nothing. No logic both front ends need is introduced: `cli` has no window, and the
  record format and filters are `opai`'s already.

### `rust-sak` and crates

- **No `rust-sak` feature is used or missing.** The records reach the collector through `opai`'s `tracing` layer,
  never through `o11y` directly.
- **No crate is introduced**, in Rust or in the frontend. `serde`'s `Deserialize` is already a `gui` dependency.

## Risks / Trade-offs

- **[Risk] A non-command object with a `kind` is thrown and skipped as "already recorded".** → Nothing in the window
  throws such an object today. The window's own tagged unions (`SetupEvent`, `UnknownOperation`) are values, never
  rejections. The rule is documented on `alreadyRecorded`, and a test pins that an `Error`, even one given a `kind`
  property, is sent.
- **[Risk] A crash is recorded twice.** This happens if React ever sends a caught error through `window`'s `error`
  too, or in development, where StrictMode double-renders. → A test renders a throwing child under the boundary with
  the handlers installed and asserts exactly one `log` call. D4's deduplication catches a second one in any case.
- **[Trade-off] A reload keeps Rust's state.** Rust keeps the images it opened, its result slots and its setup. Only
  the window's stores start over. → This is the same path a development reload takes today. `initialize` is
  idempotent (`gui-setup`), so the setup dialog does not reappear, and result slots are released when the next run
  supersedes them.
- **[Trade-off] Release stacks name bundled chunks, not source files.** → Accepted (Non-Goals). The error text and
  component path are what a reader starts from.
- **[Risk] Error texts carry paths and user names to the collector.** → The series' accepted risk, recorded in slice
  5 and revised in slice 2. It is the same terms as `opai`'s records.
- **[Trade-off] 50 records per load may hide a later distinct failure** behind a flood of earlier ones. → The drop
  notice says so, and a reload starts again. A window producing 50 distinct failures has one real problem, which is
  already in the file.

## Migration Plan

1. `frontend.rs` and the `log` command, tested with slice 5's `Recorder` for:
   - level and target
   - fields present and absent
   - the caps at, and one past, each boundary, including a multi-byte character straddling it
   - no span opened

   Then register the command, and add the drift test's exemption.
2. `ipc/log.ts` and `lib/report.ts`, tested against a mocked `invoke`.
3. The boundary, the recovery screen and the handlers, then `main.tsx`'s placement.
4. The 24 call sites, then the guard test.
5. Validation:
   - **CI on Linux, macOS and Windows:** `cargo test --workspace --locked`, `pnpm test`, `tsc --noEmit`, and Biome
     scoped to the touched files. No execution provider is involved.
   - **A manual check on macOS, handed to the owner** (the rule against driving the GUI), with a `gui` debug build
     and the collector variables set:
     - In the webview's inspector, `Promise.reject(new Error("probe"))` writes one `warn` under `gui::frontend`,
       carrying the stack, to `opai.log` and to Loki.
     - A temporary `throw` in a component shows the recovery screen, writes one `error` with `component_stack`,
       and Reload and Show logs both work.
     - Enhancing a photograph that fails writes no `gui::frontend` record.
   - **Windows (WebView2) and Linux (WebKitGTK)** are covered by CI's jsdom tests only. Their stacks differ in format
     from WebKit's, but they are carried as text either way.

**Rollback:** revert this repository's commits. Nothing is persisted, and no other slice depends on the command yet.
Slice 7 will read the reporter, not the command.
