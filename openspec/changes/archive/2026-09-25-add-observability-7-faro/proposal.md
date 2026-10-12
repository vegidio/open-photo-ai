## Why

Since slice 6, every failure the window observes reaches `opai.log`, and through `gui`'s records the collector. But
Grafana knows nothing else about the window. There is no record of which screens and controls are used, and no
session to read a failure against. The Go application answered the first question with Aptabase, which this rewrite
never ported. The series decided to answer it with Grafana Faro instead, so that a window's usage, its failures and
the backend's records all sit in the same Grafana stack.

## What Changes

The planned slice 7 comes to about 25 tasks, so it is split in two (see design.md - Slices). This change is the first
half, at **14** tasks:
- **Slice 7 (this change):** Faro itself, the window's failures sent through it, the Analytics switch, and the usage
  events.
- **Slice 8 (`add-observability-8-faro-traces`):** the `traceparent` into the commands. That half needs a `rust-sak`
  release, because the `tracing` bridge cannot yet parent a span to a remote context.

**Faro in the window (`gui`, frontend):**
- **Grafana Faro's web SDK** is initialised once, in `main.tsx`, before React mounts. It sends to a Frontend
  Observability app's collector, with the application's name, version and environment.
- **Only two of Faro's instrumentations are installed:** session and view. It sends no web vitals, as the series
  decided. Its automatic error and console capture stay off, because the window already routes every failure through
  one reporter (below).
- **Nothing reaches Faro except through one module**, `lib/faro.ts`. Until Faro has started, and in a build that names
  no collector, every call is a no-op. A guard test forbids importing the SDK anywhere else.

**Each half sends its own (`gui`, frontend):**
- **The window decides from what it already has.** It starts Faro when the stored Analytics choice is on at load and
  the build names a collector. Rust is not asked. So:
  - A launch with Analytics off sends nothing from the window either.
  - A build without a Faro collector sends nothing from the window.
  - Turning Analytics on takes effect at the next launch, for the window as for Rust.
- **The address is baked in at build time** from a new `VITE_FARO_URL`.

**Opting out stops the window at once (`gui`, frontend):**
- **The analytics mirror pauses Faro before it tells Rust.** Nothing about the opt-out, and nothing after it, is sent
  from either half.

**The window's failures reach Grafana once (`gui` and `opai`):**
- **`report` and `reportCrash` send each failure they already send to the log command to Faro as well**, as an
  exception, under the same rules: a command's rejection is skipped, and repeats and floods are bounded.
- **The log command's records become file-only.** `opai`'s collector filter gains a general marker for records that
  stay in the file: a field named `file_only`. `gui::frontend` records carry it. So each window failure is:
  - in `opai.log` once, from the log command
  - in Grafana once, as a Faro exception, with its session, browser and view

  Without the marker, each window failure would reach Grafana twice: once through the log command and once through
  Faro.

**Usage events (`gui`, frontend):**
- **A curated catalogue of 13 events**, typed, sent through `lib/faro.ts`'s `track`. The Go application's 29 Aptabase
  events were each judged, not ported (design.md D7):
  - 10 are kept or reshaped: files added, enhancements added and removed, Autopilot runs, comparison mode, crop,
    export started and finished, the TensorRT answer, and the update notice acted on.
  - `app_ready` (a per-launch snapshot of the chosen processor, language, background and Autopilot) and
    `settings_saved` replace six launch and per-control events.
  - `files_refused` is new. It records which unsupported formats people try to open.
- **Dropped:** every `*_failed` event, `provider_fallback`, `render_crashed`, `preview_rendered`,
  `export_completed` and `faces_detected`. Grafana already has each of those facts as a record, span, metric or Faro
  exception. Also dropped: `file_removed`, `files_cleared`, `crop_cancelled` and `settings_opened`, which answer no
  product question.
- **No event carries a name, a path, a folder or an error text.** Parameters are counts, booleans, enum spellings the
  window already has, and file extensions.

**The CSP opens for the collector (`gui`):**
- **`connect-src` gains `https://*.grafana.net`** in both policies. The exact host is a build secret, and a
  CSP cannot be built from one.

## Capabilities

### New Capabilities

- `gui-analytics`: what the window sends to Grafana while Analytics is on: which usage events, what they may carry,
  the window's failures as exceptions, its session, and what is never sent.

### Modified Capabilities

- `gui-settings`: the Analytics choice governs the window's sending as well as the backend's. Off stops both at
  once, and on takes effect for both at the next launch.
- `logging`:
  - The window's records reach the file only. They are the one exception to "what reaches the file is also sent".
  - A record can be kept out of the collector by the application that emits it.

## Impact

**Crates touched:**
- `opai`:
  - the `file_only` marker in the collector's filter (`logging/format.rs`)
  - the `logging` spec
- `gui` (Rust):
  - `frontend.rs`'s records marked file-only
  - `tauri.conf.json`'s two CSPs and their guard test
- `gui` (frontend):
  - `@grafana/faro-web-sdk`
  - `VITE_FARO_URL` in `vite-env.d.ts`, `lib/faro.ts`, `lib/events.ts`
  - `main.tsx`, `lib/analytics.ts` and `lib/report.ts`
  - the event call sites
  - a guard test
- `cli` and `perf`: nothing. Neither has a window, and neither starts telemetry.

**Why this is in `gui`:** Faro is a browser SDK, and it runs in the one webview this project has. The events name
the window's controls, and the window decides from its own settings store whether to send. Rust's only part is to
keep the window's log records out of its own collector, through `opai`'s general `file_only` marker.

**`rust-sak`:** `o11y` already has `session_id()` and `is_enabled()`, which `session()` wraps. Faro is a browser SDK,
and `rust-sak` has no browser side, so it has nothing to offer for Faro itself. Both answered as running after
`shutdown()`, which implementation found and fixed in `rust-sak`; the owner had the `26.9.10` tag moved to the fix, so
no pin moves in this slice (design.md - `rust-sak` and crates). Slice 8 needs a release.

**Dependencies:** `@grafana/faro-web-sdk` 2.x, in the frontend only. `@grafana/faro-react` is not used: its value is
router instrumentation and a boundary, and this window has no router and already has a boundary.
`@grafana/faro-web-tracing` comes with slice 8.

**Outside the repository:** a Frontend Observability app must be created in the Grafana stack, and its collector URL
set as `VITE_FARO_URL` wherever release builds are made. It could not be created through the Grafana MCP: the
plugin's API refused the MCP's service account. The owner created it by hand, with `*` as its allowed origin. The
field refuses `tauri://localhost`, the window's origin on macOS and Linux, and the collector was measured to answer
that origin's CORS checks under `*` (design.md D4).

**Parity with the Go application:**
- **Matches:** collection stops the moment Analytics is turned off. The kept events keep Go's names and parameters,
  except where D7 renames an event that misdescribed what it measured.
- **Diverges, deliberately:**
  - Aptabase is replaced by Faro, as the series decided.
  - Turning Analytics on waits for the next launch. Go's Aptabase resumed at once. This way the window and the
    backend follow one rule, and the window never sends traces whose Rust half is missing.
  - 19 of Go's 29 events are dropped or folded into two new ones, at the owner's request that each be judged
    (design.md D7). Settings are measured on Save and at launch, not per control change.
  - Window failures reach Grafana as Faro exceptions, not as Loki records from the log command.

**Not in scope:**
- Traces and the `traceparent` into the commands (slice 8).
- Web vitals, Faro's console capture, and its React integration.
- A release workflow that maps `VITE_FARO_URL` (as with `OPAI_OTEL_*`, slice 5).
- Changing the Analytics copy.
