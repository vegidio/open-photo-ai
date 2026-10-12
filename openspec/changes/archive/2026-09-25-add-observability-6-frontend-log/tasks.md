## 1. The log command (Rust)

- [x] 1.1 Add `crates/gui/src/frontend.rs` (design.md D1, D4):
  - `TARGET = "gui::frontend"`.
  - `Level { Warn, Error }` and `WindowRecord`, both deserialized in the wire shape D1 gives.
  - The caps (message 1 KiB, error 4 KiB, stack and component stack 8 KiB each), which cut on a `char` boundary and
    append `… (cut short)`.
  - `#[tauri::command] fn log(record: WindowRecord)`, emitting one event per level through a `match`.

  Test with `test_support::Recorder`:
  - `warn` and `error` land at their levels under `gui::frontend`, with the message as the body.
  - `error`, `stack` and `component_stack` appear when given and are absent otherwise.
  - Each cap holds at its length, and cuts one past it. A multi-byte character straddling a cap is not split.
  - No span is opened.
  - The camelCase JSON shape deserializes, and an unknown level is refused.

  Verify with `cargo test -p gui frontend::`.
- [x] 1.2 Register `frontend::log` in `lib.rs`'s `generate_handler!`. Add `log` to `every_registered_command_opens_its_span`'s
  exemptions beside `set_analytics`, and update its doc comment to say why neither is traced. Verify with
  `cargo test -p gui`: the drift test passes, and fails if the exemption is removed.

## 2. The reporter (frontend)

- [x] 2.1 Add `crates/gui/frontend/ipc/log.ts`: the `WindowRecord` type, `log(record)` invoking `log` with
  `{ record }`, and the command-name comment the other bindings carry. Add `ipc/log.test.ts`, pinning the command
  name and the argument's shape as `ipc/analytics.test.ts` does. Verify with `pnpm --dir crates/gui test ipc/log`.
- [x] 2.2 Add `lib/report.ts` (D2, D4) with `report`, `reportCrash`, `alreadyRecorded` and a test-only `forget`.
  Test with a mocked `invoke` and a spied console:
  - Both functions write to the console.
  - A tagged `{ kind }` rejection is not sent. A string, an `Error` (including one carrying a `kind` property) and a
    plain object are sent, described as D2 says, with the `Error`'s stack.
  - `reportCrash` sends at `error` with the component stack.
  - A repeat within one load is sent once.
  - The 51st distinct record sends the one drop notice, and the 52nd sends nothing.
  - A rejected send writes nothing more and does not throw.

  Verify with `pnpm --dir crates/gui test lib/report`.

## 3. Catching what escapes (frontend)

- [x] 3.1 Port `errors.boundary.title`, `errors.boundary.message` and `errors.boundary.reload` from
  `../open-photo-ai/cmd/gui/frontend/src/i18n/locales/*.json` into all 13 locale files under
  `crates/gui/frontend/i18n/locales/` (D6). Verify with `pnpm --dir crates/gui test i18n`, including the parity test.
- [x] 3.2 Add `components/ErrorBoundary.tsx` with its `RecoveryScreen` (D3). The screen carries the title, the
  message, the selectable error text, **Reload** (`window.location.reload()`), and **Show logs** (`revealLog`,
  `settings.app.logs.button`). A failed Show logs goes to `report` and to `toast.error(t("errors.showLogsFailed"))`.
  It uses the design tokens and `Button` in the manner of the setup failure dialog. Test that:
  - a throwing child renders the screen with the error's text
  - `reportCrash` is called once, with the component stack
  - Reload calls `location.reload`
  - Show logs calls `revealLog`, and toasts on a rejection

  Verify with `pnpm --dir crates/gui test ErrorBoundary`.
- [x] 3.3 Add `lib/unhandled.ts` with `watchUnhandled()`, which installs the `error` and `unhandledrejection` handlers
  (D3) and returns their removal. In `main.tsx`:
  - Call it beside `mirrorAnalytics()`, before `createRoot`.
  - Wrap `<App />` in `ErrorBoundary`, with `RevealWindow` left outside it.

  Test that:
  - an unhandled rejection with an `Error` sends one `warn`, and one with a tagged object sends nothing
  - a dispatched `error` event sends one `warn`
  - a throwing child under the boundary, with the handlers installed, sends exactly one record
  - a tree whose `App` throws on the first render still calls `windowReady`

  Verify with `pnpm --dir crates/gui test`.

## 4. Routing every console-only failure (frontend)

- [x] 4.1 Route the startup, shell and settings sites through `report`, keeping each sentence and every toast and
  state update:
  - `main.tsx`, `App.tsx`, `lib/analytics.ts` (and rewrite its comment about the log command)
  - `Navbar.tsx` (×2), `AboutDialog.tsx`, `GeneralPage.tsx`, `SetupFailure.tsx`

  Verify with `pnpm --dir crates/gui test`, whose existing tests of these components pass unchanged.
- [x] 4.2 Route the image, enhancement, Autopilot and export sites through `report` in the same way:
  - `FileOptionsMenu.tsx`, `useOpenImages.ts`, `useDroppedImages.ts`
  - `useEnhancementRun.ts` (×4), `useAutopilot.ts` (×2)
  - `stores/enhancements.ts`, `stores/autopilot.ts`, `stores/exportBatch.ts`
  - `ExportDialog.tsx`, `ExportRow.tsx` (×2), `export/batch.ts`

  Leave `particles.tsx`'s `catch {}` and the unlisten's `.catch(() => {})` as they are (design.md Context). Verify
  with `pnpm --dir crates/gui test`.
- [x] 4.3 Add `lib/report.guard.test.ts` (D5). It reads every non-test `.ts`/`.tsx` under `frontend/` outside
  `node_modules`, and fails on `console.error(` or `console.warn(` anywhere but `lib/report.ts`, naming the file and
  line. Verify with `pnpm --dir crates/gui test lib/report`: it passes, and fails when one call is restored
  temporarily.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --locked`, `pnpm --dir crates/gui test`, `pnpm --dir crates/gui exec tsc --noEmit`, and
  Biome scoped to the touched files, and verify all are green. Verify that
  `openspec validate add-observability-6-frontend-log --strict` passes.
- [x] 5.2 Hand the owner a manual check on macOS (design.md Migration Plan), using a `gui` debug build with
  `OPAI_OTEL_ENDPOINT` and `OPAI_OTEL_AUTH` set. Record in this task's checkbox note what was seen:
  - `Promise.reject(new Error("probe"))` in the webview's inspector writes one `warn` under `gui::frontend`, with
    its stack on one line, to `opai.log` and to Loki. No trace is rooted at `log`.
  - A temporary `throw` in a component shows the recovery screen, in the running language, and writes one `error`
    with `component_stack`. Reload brings the application back, and Show logs reveals `opai.log`.
  - An enhancement that fails writes its record from `opai`, and none under `gui::frontend`.
