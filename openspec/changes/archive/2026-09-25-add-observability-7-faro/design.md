## Context

The state slices 5 and 6 left the window in:

- **Rust decides whether the backend sends.** `setup` reads `analytics.json` and calls `opai::telemetry::start()`
  only when the choice is on. `set_analytics(false)` calls `telemetry::stop()` first, and opting in waits for the next
  launch.
- **The window mirrors its stored choice to Rust** through `lib/analytics.ts`'s `mirrorAnalytics()`, which runs in
  `main.tsx` before React mounts. It sends once at boot and again on every change.
- **Every window failure goes through `lib/report.ts`.** `report` handles warnings and `reportCrash` handles the
  boundary. Both write to the console, skip a tagged command rejection (`alreadyRecorded`), deduplicate per load,
  stop at 50 records, and send the rest to the `log` command.
  - `watchUnhandled()` routes `error` and `unhandledrejection` through `report`.
  - A guard test forbids `console.error` and `console.warn` anywhere else.
- **The `log` command writes under `gui::frontend`.** `opai`'s collector filter (`grafana_directives`: `info,ort=warn`
  plus `opai::telemetry::export=off`) passes it. So a window failure reaches Loki today, through the backend.
- **The CSP allows only `'self'` and the IPC origins in `connect-src`,** in both `csp` and `devCsp`. `lib.rs` has
  guard tests that read `tauri.conf.json`.
- **There is no usage tracking at all.** The Go application's `analytics/index.ts` defined 29 Aptabase events, called
  from about 28 sites. Its own comments record that one of them, `image_processed`, measured the wrong thing for
  months.
- **Faro's web SDK** (2.12) was checked for everything this design uses:
  - `initializeFaro` takes `paused`, and the instance has `pause()` and `unpause()`.
  - `api.pushError` and `api.pushEvent` exist.
  - Instrumentations can be passed one by one (`SessionInstrumentation`, `ViewInstrumentation`) instead of
    `getWebInstrumentations()`.
- **The Grafana stack** has the Frontend Observability plugin (`grafana-kowalski-app` 1.70) enabled. The MCP's service
  account could not create an app, so the owner created `opai` by hand (proposal.md - Impact).
- **The collector's CORS was measured, not assumed.** The app's allowed-origins field accepts only `http`, `https` and
  `chrome-extension` origins, and an empty list blocks every cross-origin request. The window's origin is
  `tauri://localhost` on macOS and Linux, and `http://tauri.localhost` on Windows. With `*` as the only allowed origin,
  the collector answers a preflight from `tauri://localhost`, `null` and `http://tauri.localhost` with 204. It echoes
  the origin and allows `content-type` and `x-faro-session-id`. A real POST from `tauri://localhost` carries the same
  `Access-Control-Allow-Origin`. So the window can send directly, on every platform.

## Goals / Non-Goals

**Goals:**

- Each half sends what is its own: the window its usage, failures and session, the backend its records, spans and
  metrics. Both obey the one Analytics choice, read once per launch, and both stop at once on an opt-out.
- Each window failure appears once in `opai.log` and once in Grafana.
- Every event name is a deliberate choice. Each one answers a question about how the application is used, and none
  repeats a fact Grafana already receives as a record or a metric.
- Nothing the window sends names a file, a path, a folder or a person.

**Non-Goals:**

- Traces, the `traceparent`, and Faro's tracing package (slice 8).
- Web vitals, Faro's console capture, and Faro's automatic error capture.
- Source maps uploaded to Faro. Release stacks name bundled chunks, as slice 6 accepted for the file.
- Resuming Faro when Analytics is turned on mid-session.
- Deleting data already sent after an opt-out.

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
| 7 | `add-observability-7-faro` | `rust-sak`: `session_id()`/`is_enabled()` answer as stopped after `shutdown()` (tag `26.9.10` moved). `opai`: a `file_only` marker the collector filter drops. `gui`: the window's records made file-only; CSP opened for `*.grafana.net`. Frontend: Faro (session and view only) started from the stored Analytics choice and `VITE_FARO_URL`, and paused before an opt-out reaches Rust; window failures sent as Faro exceptions through the reporter; a curated catalogue of usage events | 5, 6 | **current** |
| 8 | `add-observability-8-faro-traces` | `rust-sak`: the bridge parents a span to a remote context, released; pins bumped. `gui`: a `traceparent` read from each invoke's headers and given to the command span. Frontend: Faro's tracing, one span per `invoke` carrying the header, so window and backend spans form one trace | 7 | pending |

The planned slice 7 was split in two, at the owner's direction, because it came to about 25 tasks:
- **Slice 7 keeps** Faro, errors, the switch, the CSP and the usage events.
- **Slice 8 takes** the traces. That half needs `rust-sak` work: the bridge's `on_new_span` parents a span to the
  nearest exported `tracing` ancestor, or else to the `o11y` thread stack. So a context parsed from a `traceparent`
  cannot become a command span's parent without a new bridge API and a release. Slice 1's D4 anticipated the parse
  but not the parenting.

Three more changes from the planned row:
- **"Errors and console" became "errors, through the reporter".** Faro's console capture would send the reporter's
  own `console.error` a second time (D5).
- **"Paused and resumed on the Analytics switch" became "paused, and resumed at the next launch".** The owner chose
  that the window follows the backend's rule (D2).
- **"Usage events" became "a curated catalogue".** The owner asked for each of the Go application's events to be
  judged rather than ported (D7).

### Decisions already taken for slices 2 to 8

Settled with the repository owner during exploration, and repeated in every slice's design.md. Two were revised, and
are marked as such.

- **`tracing` stays the one logging API.** Grafana is a second layer beside the file, fed by slice 1's bridge. There is
  no second set of `o11y::log` calls.
- **Analytics defaults to on and sends from the first launch.** Opting out stops sending at once (`o11y::shutdown()`
  and `faro.pause()`). ~~Opting in takes effect at the next launch for the backend; Faro may resume at once.~~
  **Revised in slice 7:** opting in takes effect at the next launch for both halves.
- ~~**Path fields never reach Grafana.**~~ **Revised in slice 2:** paths are not scrubbed in the backend's records.
  The window's events carry no paths (D7). Its exceptions carry error texts on the same terms as the backend's
  records.
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

### D1 - Each half sends its own, and the window decides from what it already has

The window starts Faro on its own. It does not ask Rust. It needs three things, and has all of them without Rust:

- **Whether to send:** the Analytics choice. It lives in the window's settings store, which Rust only mirrors, and has
  already rehydrated when `main.tsx` runs.
- **Where to send:** `VITE_FARO_URL`, baked into the bundle at build time and declared in `vite-env.d.ts`.
- **As which build:** the application's version (the existing `version` command), and `production` for a production
  bundle or `development` otherwise.

So the window sends while the stored choice was on at load and the build names a Faro collector. The backend keeps
slice 5's rule for its own records. The two follow the same choice, read once per launch, and stop together on an
opt-out (D3). They are not forced to agree otherwise:

- **A build with a Faro URL but no OTLP collector** sends from the window only. A build with an OTLP collector but no
  Faro URL sends from the backend only. Neither is an error. A release build is given both.
- **A `tauri build --debug`** bundles the window in production mode over a debug backend, so the window reports
  `production` and the backend `development`. Only a developer makes that build.

*Revised during implementation, at the owner's direction.* This slice first had the window ask a `telemetry` command,
which answered the baked `OPAI_FARO_URL` and `opai`'s session id only while the backend was sending. That routed a
frontend concern through the backend: a Rust command, a `gui` module and an `opai` accessor existed only to tell the
window what it already knew. The session id it carried is dropped with it. Slice 8's `traceparent` joins each window
request to its backend trace, which is a finer join than a session.

*Alternative:* the `telemetry` command described above. Rejected for the reason just given.

### D2 - `lib/faro.ts`: the only module that touches the SDK

```ts
export const startFaro = async (analytics: boolean): Promise<void>;   // starts when `analytics` and VITE_FARO_URL
export const pauseFaro = (): void;                                     // idempotent; a no-op before start
export const sendError = (error: Error, context: ErrorContext): void;  // report.ts wraps a non-Error (D5)
export const track = <E extends EventName>(event: E, params: EventParams[E]): void;
```

- **Started in `main.tsx`, before `mirrorAnalytics()` and `watchUnhandled()`,** with the stored choice, and not
  awaited. It waits only for the version. Anything the window reports before that is dropped, not queued, and still
  reaches the file.
- **`initializeFaro` is called with:**
  - `url`: `VITE_FARO_URL`
  - `app`: `{ name: "opai", version, environment }`
  - `instrumentations: [new SessionInstrumentation(), new ViewInstrumentation()]`
  - `sessionTracking: { enabled: true }`
  - `isolate: true`, so no global `window.faro` exists for other code to reach
- **`pauseFaro` is terminal for the page load.** Nothing calls `unpause`. Opting in waits for the next launch, and by
  then the page has been loaded again. An opt-out that lands while `startFaro` waits for the version wins: Faro is
  never initialised.
- **Every export is a no-op until `startFaro` has initialised Faro**, and forever in a build or launch that sends
  nothing. Callers never check.
- **A guard test** (`lib/faro.guard.test.ts`, the twin of `report.guard.test.ts`) fails on any import of
  `@grafana/faro-*` outside `lib/faro.ts`.

*Alternative:* `getWebInstrumentations()`. Rejected. It installs error, console and web-vitals capture, which this
design turns off on purpose (D5, Non-Goals).

*Alternative:* `@grafana/faro-react`. Rejected. It adds router instrumentation for a window with no router, and a
boundary for a window that has one.

### D3 - The opt-out pauses Faro before the backend is told

`mirrorAnalytics`'s `mirror(false)` calls `pauseFaro()` synchronously, before `setAnalytics(false)` is invoked.
Faro's transport buffers items and sends them in batches. Pausing first means that even the batch in flight is
dropped rather than flushed. Nothing is tracked about the choice itself: the catalogue has no event for it (D7).

`mirror(true)` does nothing to Faro, and the backend does nothing until the next launch. A window reloaded after an
opt-out reads the stored choice, which is off, and never starts.

### D4 - The window sends directly: the CSP allows `https://*.grafana.net`, and the app allows `*`

**The app's allowed origins are `*`.** No narrower entry can name `tauri://localhost`, which the field refuses
(Context). The collector key is shipped in the bundle either way, as with any Faro app. So `*` gives away only the
ability to post from other web pages, not a secret.

*Alternative:* relay Faro's batches through a Rust command, so no browser origin is involved. Rejected by the owner.
The window's data would flow through the backend, which the measured CORS behaviour makes unnecessary.

**The CSP:**

`connect-src` in both `csp` and `devCsp` gains `https://*.grafana.net`. Faro's collector host
(`faro-collector-prod-<region>.grafana.net`) is known only from the build secret, and a CSP in `tauri.conf.json` is
fixed when the bundle is built. The existing guard tests gain one assertion: both policies allow the Faro host
pattern, and neither allows `*` or `https:` on their own.

*Alternative:* inject the exact origin at build time through `TAURI_CONFIG`. Rejected. It is a second mechanism for
one secret, and it breaks the guard tests' premise that `tauri.conf.json` is the whole policy. The wildcard
still confines the window to Grafana Cloud.

### D5 - Window failures: Faro for Grafana, the log command for the file

- **`opai` gains a general file-only marker.** The collector's filter becomes `grafana_directives()` combined with a
  `filter_fn` that rejects any event whose metadata declares a field named `file_only`. The name is exported as
  `opai::logging::FILE_ONLY`.
  - The check reads the callsite's field names, so it costs one metadata scan per event.
  - It is `opai`'s mechanism, and it names no front end.
- **`frontend.rs`'s events declare `file_only = true`.** Their target stays `gui::frontend`. The file writes
  `file_only=true` on those lines, which says to a reader exactly why Loki does not have them.
- **`report` and `reportCrash` call `sendError` in the same step as the log command,** after the same skip,
  deduplication and 50-per-load bound. So Faro receives exactly what the file does:
  - `report` sends `{ type: "warn", message }` as the context.
  - `reportCrash` sends `{ type: "crash", componentStack }`.
  - The `Error` is passed through as itself, so Faro parses the stack. A string or other value is wrapped in an
    `Error` carrying the same text `describe` computes.
  - The flood notice is not sent to Faro. It is a statement about the file.
- **Faro's `ErrorsInstrumentation` and `ConsoleInstrumentation` are not installed.** `watchUnhandled` already routes
  `error` and `unhandledrejection` through `report`. The guard test already forbids `console.error` and
  `console.warn` outside `report`. So their only new captures would be the reporter's own output, sent again.

*Alternative:* keep the log command sending to Loki and give Faro no errors. Rejected by the owner. Faro's exception
view groups by stack and session, which a Loki line cannot.

*Alternative:* a target prefix in place of a field, for example `file_only::gui::frontend`. Rejected. It would rename
the target the file writes, and `EnvFilter` matches a target by string prefix, which is easy to trip by accident.

### D6 - What an event may carry

- **Values are a closed vocabulary**: booleans, counts, durations, enum spellings the window already has (families,
  processors, formats, languages, views) and extension lists. Each event's parameters are a
  TypeScript type in `lib/events.ts`, so `track` accepts nothing else.
- **Never a name, a path, a folder, a user's text or an error message.** `formatList` is ported from the Go
  application's `buckets.ts` with its tests, and is the only derived value. `mpBucket` is not ported: both events
  that sent a size (`preview_rendered`, `export_completed`) are dropped (D7).
- **Durations are milliseconds, unbucketed,** as the Go application decided: a bucket would throw away the p95.

### D7 - The catalogue

The owner asked for the Go application's events to be judged rather than ported. One test decides each: **does it
answer a question about how people use the application that only the window can answer?** An event fails it in
three ways:
- **Grafana already has the fact.** The backend records it, or slice 4's spans and metrics measure it.
- **It counts renders, not decisions.** The Go application's own comment on `image_processed` records that mistake.
- **Its value is too small to act on.**

**Kept, reshaped or added (13, from 29):**

| Event | Parameters | Fires | Question it answers |
|---|---|---|---|
| `app_ready` | `processor`, `language`, `background`, `autopilot` | once per load, when setup succeeds | What do people run with? A snapshot counts every user. A change event counts only those who changed something. Replaces `app_open` and `app_initialized`. |
| `files_added` | `count`, `source` (`browse` / `empty` / `drop`), `formats` | after images are admitted | How do images come in, and in which formats? |
| `files_refused` | `count`, `formats` | a drop that held unsupported files | Which formats do people want that are not supported? *New.* |
| `enhancement_added` | `family`, `source` (`manual` / `autopilot`) | each operation added by the user or by Autopilot in the sidebar | Which enhancements are used, and how many come from Autopilot? |
| `enhancement_removed` | `family` | the user removes an operation | Beside the one above, which suggestions are rejected. |
| `autopilot_run` | `count` | an interactive analysis that completed, zero suggestions included (not the export's) | How much does Autopilot suggest per image? A zero is an answer: dropping it would inflate the average. |
| `preview_mode_changed` | `mode` | the comparison selector changes | Which comparison views are used? |
| `crop_applied` | `rotated`, `flipped`, `has_ratio` | Apply with a framing that changed something | What is cropping used for? |
| `export_started` | `file_count`, `format`, `processor` | a batch is started | How big are exports, and in which formats? Renamed from `export_batch_started`. |
| `export_finished` | `file_count`, `exported`, `completed`, `duration_ms` | a batch ends, finished or aborted | Do people wait for batches or abort them? Renamed from `export_batch_finished`. |
| `settings_saved` | `changed` (sorted keys of the preferences that differ) | Save that changed something | Which settings do people touch? Replaces the four `*_changed` events. The new values are in the next load's `app_ready`. |
| `tensorrt_prompt_answered` | `accepted` | either button of the TensorRT question | Do people accept the slow first TensorRT build? |
| `update_opened` | none | the navbar's update control opens the releases page | Do people act on the update notice? *Reshaped* from `update_available`: the backend's `is_outdated` span already says a notice was shown. |

**Dropped, each with its reason:**

| Go event | Why it goes |
|---|---|
| `app_open` | Faro's own session start marks it, and the backend records `telemetry started`. |
| `app_initialized` | Folded into `app_ready`. The provider found is in the backend's `initialize` span. |
| `init_failed`, `autopilot_failed`, `process_failed`, `export_failed` | Failures are recorded once, where they happen (slices 3 to 6). An event repeating them counts one failure twice. |
| `provider_fallback` | Slice 4's provider-fallback metric measures exactly this. The window never sees the fallback. |
| `render_crashed` | A Faro exception from `reportCrash` (D5). |
| `preview_rendered` | It counts renders, several per edit, not decisions. Enhancement durations and cache hits are slice 4's metrics and spans. |
| `export_completed` | Per file. The backend's `export` span carries its duration and outcome, and `export_finished` carries the batch. |
| `file_removed`, `files_cleared` | Closing images answers no product question. |
| `crop_cancelled` | In Go it fired on Apply with an unchanged framing, not on a cancel. It measured nothing coherent. |
| `faces_detected` | Detection runs on three paths here (preview, Autopilot, export). Its count is on the backend's `detect_faces` span. |
| `settings_opened` | `settings_saved` measures the engagement that matters. Opening without saving is noise. |
| `execution_provider_changed`, `model_changed`, `quality_changed`, `language_changed` | Replaced by `settings_saved` and `app_ready`, which fire on Save as this window's settings do. Go's fired per control change. |

**Deliberately not added,** although the window has them:
- the Autopilot toggle, the background and the family exclusions, which `app_ready` and `settings_saved` cover
- face skipping, reveals, Show logs and setup's Try again, which are too small to act on
- the Analytics switch, which must never be reported (`gui-settings`)

**Names** are snake_case, as in Go. None of Aptabase's dashboards carry over to Grafana, so an event is renamed
freely where the old name misdescribed it.

## Division across crates

- **`opai`:** `logging::FILE_ONLY` with the collector filter (D5). A telemetry mechanism, with nothing about a window.
- **`gui` (Rust):** `frontend.rs`'s marker (D5). `tauri.conf.json` and its guards (D4). No command is added.
- **`gui` (frontend):** `vite-env.d.ts`'s `VITE_FARO_URL`, `lib/faro.ts` and its guard (D1, D2), `main.tsx`,
  `lib/analytics.ts` (D3), `lib/report.ts` (D5), `lib/events.ts` with `formatList` (D6, D7), and the event sites.
- **`cli` and `perf`:** nothing.

## `rust-sak` and crates

- **`o11y`:** nothing new is used by this slice. Slice 8 needs the remote-parent API.
- **Found during implementation: `session_id()` outlived `shutdown()`.** At `26.9.10` it read `PIPELINE.get()`, a
  `OnceLock` that `shutdown` never empties, so it answered `Some` after a stop, and `is_enabled()` answered `true`
  against its own doc. The first form of D1 depended on it. It no longer does, but the fix stands on its own.
  Fixed in `rust-sak` (`03bb633`): both read the pipeline only while its gates are open, which `init` opens last and
  `shutdown` and the dead worker's `Sentinel` close. At the owner's direction the `26.9.10` tag was moved to that
  commit rather than a new version cut, so no pin moves; `Cargo.lock` was repointed to it and resolves from a cold
  cache.
- **No Rust crate is introduced.**
- **`@grafana/faro-web-sdk` ^2.12** is the one frontend dependency. It is Grafana's own SDK for its own collector,
  and nothing else speaks that collector's protocol. `@grafana/faro-core` comes in with it.

## Risks / Trade-offs

- **[Risk] A launch's earliest window failures are not sent to Faro.** They happen before the version arrives and Faro
  starts. → They still reach the file. The window is sub-second from mount to start.
- **[Risk] Any web page can post to the collector,** because the app allows `*`. → Junk data is the worst case, and the
  key is public in the bundle anyway. If it happens, Frontend Observability's rate limits apply, and the key can be
  rotated with a rebuild.
- **[Risk] Grafana tightens the collector's handling of `*` or of non-`http` origins.** → The window would stop sending,
  and nothing else would break. The manual check (6.2) is the canary, and the relay alternative in D4 is the fallback.
- **[Risk] `*.grafana.net` is broader than one collector.** → It is still only Grafana Cloud, over HTTPS. The window
  loads no third-party code, and `script-src` is unchanged.
- **[Trade-off] Opting in does not resume the window until relaunch.** → This is the owner's choice (D2). It also
  keeps the window from sending traces without their Rust half once slice 8 lands.
- **[Trade-off] A window session cannot be joined to the backend's session until slice 8.** → Faro's sessions carry
  no backend id (D1). Slice 8's `traceparent` joins each window request to its backend trace.
- **[Risk] The two halves disagree about sending** in a build given one collector and not the other. → Each sends
  its own, so the other half is simply absent. Release builds are given both secrets together.
- **[Risk] An event parameter leaks user content.** → D6's closed vocabulary is enforced by types. A test pins that no
  parameter is typed as a bare `string`, only as a union or as `formatList`'s output.
- **[Risk] Error texts sent to Faro carry paths.** → The same terms as the backend's records since slice 2's revision
  and slice 5's accepted risk. The Analytics copy is unchanged, at the owner's direction.
- **[Trade-off] Each window failure is sent to Faro even when the log command fails**, and vice versa. → Each path
  fails silently on its own, as slice 6 required of the log command.

## Migration Plan

1. The owner creates the Frontend Observability app, and records the collector URL as a secret beside
   `OTEL_ENDPOINT`, mapped onto `VITE_FARO_URL`.
2. `rust-sak`: `session_id()` and `is_enabled()` answer as stopped after `shutdown()`. `opai`: the file-only filter,
   with tests on the collector filter.
3. `gui` Rust: the marker in `frontend.rs`, and the CSP with its guard.
4. The frontend: the dependency, `VITE_FARO_URL`, `lib/faro.ts` and its guard, `main.tsx`, then the mirror's pause and
   the reporter's `sendError`.
5. The catalogue, `formatList`, and the event sites.
6. Validation:
   - **CI on Linux, macOS and Windows:**
     - `cargo test --workspace --locked`
     - `pnpm test`
     - `tsc --noEmit`
     - Biome, scoped to the touched files
   - **A manual check on macOS, handed to the owner** (the rule against driving the GUI). It uses a `gui` debug build
     with `OPAI_OTEL_*` and `VITE_FARO_URL` set:
     - A session appears in Frontend Observability, and shows `app_ready`.
     - Opening images and enhancing sends the catalogue's events.
     - `Promise.reject(new Error("probe"))` from the inspector gives one Faro exception, one `gui::frontend` line with
       `file_only=true` in `opai.log`, and nothing in Loki.
     - Turning Analytics off: nothing more arrives from the window. A reload sends nothing. A relaunch after turning
       it back on sends again.
   - **Windows (WebView2) and Linux (WebKitGTK)** are covered by CI's jsdom tests only. No execution provider is
     involved.

**Rollback:** revert this repository's commits. No state is persisted. Faro data already sent stays in Grafana.
