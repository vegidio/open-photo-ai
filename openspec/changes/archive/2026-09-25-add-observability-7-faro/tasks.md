## 1. Outside the repository

- [x] 1.1 Finish the Frontend Observability app `opai`. The owner created it on 2026-09-25, with `*` as its only
  allowed origin, because the field accepts only `http(s)` origins and the window's origin on macOS and Linux is
  `tauri://localhost` (design.md D4). Its collector is
  `https://faro-collector-prod-eu-north-0.grafana.net/collect/<key>`. What remains is to record that URL as a secret
  beside `OTEL_ENDPOINT` and `OTEL_HEADER_AUTH`, to be mapped onto `VITE_FARO_URL`, and to set it locally for 6.2.
  Verify that a preflight `OPTIONS` with `Origin: tauri://localhost` and `Access-Control-Request-Headers:
  content-type,x-faro-session-id` answers 204 and echoes the origin.

## 2. `rust-sak` and `opai`

- [x] 2.1 Fix `rust-sak`'s `o11y` so `session_id()` and `is_enabled()` answer as stopped once `shutdown()` has run
  (design.md - `rust-sak` and crates). Assert it in `tests/o11y_end_to_end.rs`, and show the assertion fails without
  the fix. At the owner's direction, move the `26.9.10` tag to the fix rather than cut a version, repoint
  `Cargo.lock`, and verify it resolves from a cold cache.
- [x] 2.2 Add the file-only marker (D5). Export `pub const FILE_ONLY: &str = "file_only"` from `opai::logging`. In
  `format.rs`, combine the collector's filter with a `filter_fn` that rejects an event whose metadata declares that
  field, and document the mechanism in the module header. Test, through the existing `sent`/`rendered` helpers,
  that a `warn!(file_only = true, …)`:
  - is written to the file with `file_only=true`
  - is not sent to the collector

  Also test that an unmarked event is still sent. Verify with `cargo test -p opai logging::`.

## 3. `gui` (Rust)

- [x] 3.1 Mark `frontend.rs`'s events with `opai::logging::FILE_ONLY = true` (D5), and rewrite its module doc and
  `log`'s doc: the window's records are the file's alone, and Faro carries them to Grafana. Extend its tests to assert
  the field on both levels, and that the collector filter drops the records. Verify with `cargo test -p gui frontend::`.
  *Note:* the filter is private to `opai`, so `gui` asserts the marker and `opai`'s 2.2 test asserts the drop.
- [x] 3.2 Add `https://*.grafana.net` to `connect-src` in both `csp` and `devCsp` in `tauri.conf.json` (D4). Extend
  `lib.rs`'s `tauri_conf_json` guards to assert that both policies carry it, and that neither carries a bare `*`,
  `https:` or `http:`. Verify with `cargo test -p gui tauri_conf_json`.

## 4. Faro in the window (frontend)

- [x] 4.1 Add `@grafana/faro-web-sdk` ^2.12 to `crates/gui/package.json`, and declare `VITE_FARO_URL` as an optional
  build variable in `vite-env.d.ts` (D1). Verify with `pnpm --dir crates/gui install --frozen-lockfile=false` and
  `pnpm --dir crates/gui exec tsc --noEmit`.
- [x] 4.2 Add `lib/faro.ts` (D1, D2), exporting `startFaro`, `pauseFaro`, `sendError` and `track`:
  - `startFaro(analytics)` starts only when `analytics` is on and `VITE_FARO_URL` is set, asking Rust for nothing but
    the version.
  - Only `SessionInstrumentation` and `ViewInstrumentation`, with `isolate: true`.
  - Every export is a no-op before start, and on a launch that does not start.
  - `pauseFaro` is idempotent, and an opt-out that lands before start prevents it.

  Also add `lib/faro.guard.test.ts`, which fails on any import of `@grafana/faro-` outside `lib/faro.ts`. Call
  `startFaro` first in `main.tsx` with the stored choice, unawaited, with a rejection passed to `report`. Test with a
  mocked SDK and a stubbed `VITE_FARO_URL`:
  - no initialisation with Analytics off, or without a collector
  - the configuration passed otherwise
  - `track` and `sendError` before and after start
  - `pause` called once

  Verify with `pnpm --dir crates/gui test lib/faro`.
- [x] 4.3 In `lib/analytics.ts`, make `mirror(false)` call `pauseFaro()` synchronously before `setAnalytics(false)`
  (D3), and update the module doc. Test that the pause precedes the invoke on an opt-out, and that an opt-in does
  not unpause. Verify with `pnpm --dir crates/gui test lib/analytics`.
- [x] 4.4 In `lib/report.ts`, send each record that reaches the log command to `sendError` in the same step (D5).
  - `report` sends a `warn` context and `reportCrash` a `crash` context with the component stack.
  - An `Error` is passed as itself. Any other value is wrapped in an `Error` carrying `describe`'s text.
  - The flood notice is not sent to Faro.

  Extend `report.test.ts`: a tagged rejection sends to neither path, a repeat and the 51st record behave the same on
  both, and a throwing `sendError` still sends the log record. Verify with `pnpm --dir crates/gui test lib/report`.

## 5. Usage events (frontend)

- [x] 5.1 Add `lib/events.ts` (D6, D7): the 13 event names with one parameter type each, using unions of the
  window's existing spellings, and `formatList`, ported with its tests from
  `../open-photo-ai/cmd/gui/frontend/src/analytics/buckets.ts`. `track`'s signature in `lib/faro.ts` takes its types
  from here. Add a test that no parameter type is a bare `string` except `formatList`'s branded output. Verify with
  `pnpm --dir crates/gui test lib/events` and `tsc --noEmit`.
- [x] 5.2 Send the image and enhancement events at the sites D7 names:
  - `files_added`: `useOpenImages` gains a `source` argument (`DrawerHeader` passes `browse`, `PreviewEmpty` passes
    `empty`), and `useDroppedImages` sends `drop`. The count is what was admitted.
  - `files_refused`: from `useDroppedImages`'s refusal branch.
  - `enhancement_added`: from `AddEnhancement` (manual) and `useAutopilot`'s add loop (autopilot). Nothing is sent
    when the export analyses (`notify: false`).
  - `enhancement_removed`: from `EnhancementList`'s `onRemove`.
  - `autopilot_run`: from `useAutopilot`, on the interactive `added` path only.

  Extend each site's test with a mocked `track`. Verify with `pnpm --dir crates/gui test`.
- [x] 5.3 Send the remaining events at the sites D7 names:
  - `app_ready`: from `App.tsx`'s setup success, once per load.
  - `preview_mode_changed`: from `DrawerHeader`.
  - `crop_applied`: from `useCropController`'s changed branch.
  - `export_started` and `export_finished`: from `export/batch.ts`'s `runBatch`, with a start timestamp for
    `duration_ms`.
  - `settings_saved`: from `useSettingsDialog`'s `close(true)`, diffing the draft against the store. Nothing is sent
    when nothing changed, and `analytics` is never among the keys.
  - `tensorrt_prompt_answered`: from `TensorRTDialog`.
  - `update_opened`: from `Navbar`'s update control.

  Extend each site's test with a mocked `track`, including an aborted export and a save that changes nothing.
  Verify with `pnpm --dir crates/gui test`.

## 6. Verification

- [x] 6.1 Run these, and verify all are green:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace --locked`
  - `pnpm --dir crates/gui test`
  - `pnpm --dir crates/gui exec tsc --noEmit`
  - Biome, scoped to the touched files

  Also verify that `openspec validate add-observability-7-faro --strict` passes.
- [x] 6.2 Hand the owner a manual check on macOS (design.md Migration Plan), using a `gui` debug build with
  `OPAI_OTEL_ENDPOINT`, `OPAI_OTEL_AUTH` and `VITE_FARO_URL` set. Record in this task's note what was seen:
  - A Frontend Observability session appears, and shows `app_ready`.
  - Opening, enhancing, cropping and exporting send their events, with no file name in any of them.
  - `Promise.reject(new Error("probe"))` gives one Faro exception, one `file_only=true` line in `opai.log`, and
    nothing in Loki.
  - Turning Analytics off: nothing more arrives, and a reload sends nothing. Turning it on and relaunching sends
    again.

  *Seen on 2026-09-25, macOS, debug build with all three secrets:*
  - Session `imv7PJhFK1` (owner-driven) sent 21 events: `session_start`, `app_ready` and 11 catalogue events,
    with counts matching the backend's records and no file name in any parameter.
  - `Promise.reject(new Error("probe-rejection"))` and a thrown `probe-throw` each gave one Faro `kind=exception`
    with its context, one `gui::frontend … file_only=true` line in `opai.log`, and nothing in the backend's stream.
  - Analytics off at 07:42:32Z: from then until quit, nothing from the window (a Save, a reload, and a failure
    after the reload included) and nothing from the backend. On again, quit, relaunch: new session `AwnGE7Eh7h`
    with `session_start` and `app_ready`, and the backend's `telemetry started`.
  - Found along the way, outside this slice: one abort ("panic in a function that cannot unwind", 09:38:20) while
    the inspector was being driven, not reproduced; a Tauri `listeners[eventId]` TypeError after a reload; the
    Faro collector adding city and ASN geolocation; and a 429 on the backend's metrics from the stack's rate limit.
