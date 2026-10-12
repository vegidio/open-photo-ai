## Why

Since slice 5, every failure Rust sees is recorded once in `opai.log`, and every command the window makes is a
span. What happens *inside* the window leaves no record at all. It goes to the webview's console, which no user can
open and which is gone when the window closes. That covers:
- a render that throws, which today blanks the whole window with nothing to say why
- an exception in an event handler
- a promise rejection nobody caught
- the dozen failures the code catches and drops with a `console.error`

A user who reports "the window went blank" attaches a log that says nothing happened. This slice gives the window a
way into `opai.log`, and through it into the collector, and uses it for every one of those.

## What Changes

The whole feature needs well over 15 tasks, so it is split into seven slices (see design.md - Slices). This proposal
is the sixth, at **12** tasks.

**A log command (`gui`, Rust):**
- **A new `log` command** writes one record from the window under `gui`'s own `gui::frontend` target, at warning or
  error severity, with the error's text and stack when there are any. It reaches the file and, while telemetry runs,
  the collector, exactly as `gui`'s own records do.
- **It opens no command span.** A record is the whole of what it does, and a trace per record would bury the
  window's real requests. It joins `set_analytics` as the second exemption in the drift test.
- **Each text field is capped in length**, so one record cannot grow the file by megabytes.

**One reporter in the window (`gui`, frontend):**
- **`report(message, error)`** writes the failure to the console, as today, and sends it to the log command, with
  one exception: a failure Rust has already recorded is not sent again.
- **How it tells them apart:** every command rejects with a tagged `{ kind, … }` object, and slice 5 made sure each
  of those is either a stop or already recorded by Rust. Anything else, such as a thrown `Error`, an IPC failure
  that never reached a command body, or a clipboard refusal, is something only the window saw.
- **Repeats are dropped.** The same failure is sent once per page load, and a page load sends at most a fixed number
  of records. A failure that repeats every frame cannot flood the file.
- **A failed send is never reported.** Otherwise a dead bridge would report its own failure forever.

**Catching what nothing catches today (`gui`, frontend):**
- **An ErrorBoundary around the application.** A render crash is recorded at error severity, with the component
  stack. The window shows a recovery screen instead of going blank: a title, what happened, the error's text,
  **Reload**, and **Show logs**.
- **Global `error` and `unhandledrejection` handlers**, installed in `main.tsx` before React mounts, record at
  warning severity what escapes everything else.
- **Every `console.error` and `console.warn` that reports a failure goes through `report`:** 24 call sites across
  `main.tsx`, `App.tsx`, the stores, the hooks and the features. Command rejections among them still record nothing
  new. Clipboard refusals, event subscriptions that could not be made, and IPC failures now reach the file.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `logging`:
  - The window's own failures reach the file and the collector, each recorded once.
  - A failure the window receives from Rust is not recorded a second time.
  - A render crash is recorded at error severity. Other failures only the window sees are recorded at warning.
  - The window's records are bounded per page load.
- `gui-shell`: a render crash shows a recovery screen with Reload and Show logs, instead of a blank window.

## Impact

**Crates touched:**
- `gui` (Rust): a new `frontend` module holding the `log` command. It is registered in `lib.rs`, and the drift
  test's exemption list gains it.
- `gui` (frontend):
  - `ipc/log.ts`
  - `lib/report.ts`
  - the ErrorBoundary and its fallback screen
  - the global handlers in `main.tsx`
  - the 24 call sites
  - three i18n keys in 13 locales
- `opai`: nothing. The file's and the collector's filters already pass `gui::*` at `info`, and the formatter already
  escapes newlines, so a stack stays on one line.
- `cli` and `perf`: nothing. They have no window.

**Why this is in `gui`:** it records a webview's failures, and neither `cli` nor any other consumer of `opai` has a
webview. The command is a thin `tracing` call. The rules about what to send live in the window's reporter, because
only the window can tell a tagged command rejection from its own failure.

**`rust-sak`:** nothing needed. Records reach the collector through `opai`'s existing `tracing` layer (slice 1's
bridge), not through `o11y` directly. No release, no pin moves.

**Dependencies:** none added. The boundary is a class component on React's own API, as the Go app's was.
`react-error-boundary` is not needed for one boundary.

**Parity with the Go application:**
- **Matches:** the ErrorBoundary and its screen (title, message, error text, Reload), with the same translated copy
  in all 13 locales. The global `error` and `unhandledrejection` handlers are installed before mount, as Go's were.
- **Diverges, deliberately:**
  - Go's handlers and boundary wrote only to the console, plus an Aptabase event. Here they reach `opai.log` and the
    collector, which is the point of the series.
  - The recovery screen adds **Show logs**, because the crash has just been written there.
  - Aptabase's `RenderCrashed` event is not ported. Faro replaces Aptabase in slice 7.

**Not in scope:**
- Faro, and a `traceparent` from the window (slice 7). Slice 7 also decides how Faro's own error capture avoids
  sending a failure the log command already sent.
- Informational records from the window. Nothing in it needs one yet.
- The `opai://` scheme handler, which is not the window's code.
