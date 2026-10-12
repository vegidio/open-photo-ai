## 1. Window and platform

- [x] 1.1 Add `"titleBarStyle": "Overlay"`, `"hiddenTitle": true` and
  `"trafficLightPosition": { "x": 9, "y": 26 }` to the `main` window in
  `crates/gui/tauri.conf.json`. Verify with `pnpm tauri dev` on macOS that no title bar is drawn
  above the webview and the traffic lights sit over the top 48px, and confirm the existing
  `tauri.conf.json` drift guard in `crates/gui/src/lib.rs` still passes under `cargo test`.

- [x] 1.2 Register `tauri-plugin-os` in `crates/gui/src/lib.rs` (after the single-instance plugin, per
  the ordering rule recorded there) and add `@tauri-apps/plugin-os` to `crates/gui/package.json`.
  Do not grant the plugin's `os:default` in `capabilities/default.json` - `platform` is an
  init-script property, not a command. Verify `cargo build` and `pnpm install` succeed and that
  `window.__TAURI_OS_PLUGIN_INTERNALS__` is populated in the dev window's console.

- [x] 1.3 Add `frontend/ipc/os.ts` wrapping the plugin's synchronous `platform()` and exporting the
  one question the frontend asks (is this macOS). Verify with `frontend/ipc/os.test.ts`, which stubs
  `window.__TAURI_OS_PLUGIN_INTERNALS__` and asserts the wrapper for `"macos"` and for one non-macOS
  value.

## 2. Shared frontend pieces

- [x] 2.1 Add `--success-bright: #4ade80` to `:root` in `frontend/style.css`, beside `--success`, and
  re-export it through the `@theme inline` block. Verify `pnpm build` emits both the custom property
  and a `text-success-bright` utility.

- [x] 2.2 Add `frontend/lib/constants.ts` with `APP_NAME` and the dotted-canvas class string (the
  `--surface-dot` radial gradient at 48px), and `frontend/components/icons/` with the three
  preview-mode icons drawn from the design's paths at lucide's geometry (24x24 viewBox,
  `currentColor`, stroke from the provider). Verify `pnpm typecheck` passes and a test renders each
  icon and asserts it emits an `<svg>` inheriting `currentColor`.

- [x] 2.3 Add `frontend/stores/enhancements.ts` - `autopilot` defaulting to `true`, `setAutopilot`,
  `toggle`, persisted under `enhancements-storage` with a `partialize` keeping only `autopilot`.
  Verify with a test that the default is on, that `toggle` flips it, and that a store rehydrated from
  a persisted `false` comes up off.

## 3. The four regions

- [x] 3.1 Add `core:window:allow-start-dragging` to `crates/gui/capabilities/default.json` - it is
  NOT in `core:window:default`, and without it `data-tauri-drag-region` is inert and fails silently.
  Then build `frontend/features/navbar/Navbar.tsx`: 48px tall on `--card` with a `--border` bottom
  rule, `data-tauri-drag-region` on its container, 86px left padding only when `ipc/os` reports macOS,
  the application name, and Settings, About and the `navbar.version` string on the right. Verify with
  a test that mocks `@/ipc/os` and `@/ipc/app` and asserts the inset class is present for macOS,
  absent otherwise, and that the version the `version` command returns is rendered. Verify the drag
  itself against the running window - a unit test cannot see a permission that is missing.

- [x] 3.2 Build `frontend/features/preview/Preview.tsx` and `PreviewEmpty.tsx`: the canvas absolutely
  positioned with a 48px bottom inset on the dotted background, and inside it the `FolderOpen` icon at
  72px/stroke 1.25 in `--primary`, `preview.empty.title` through `<Trans>` so the catalogue's `<br/>`
  survives, `common.or`, and the `preview.empty.browse` button. Verify with a test that both lines of
  the invitation render as two lines and that the Browse images button is present and enabled.

- [x] 3.3 Build `frontend/features/drawer/Drawer.tsx` and `DrawerHeader.tsx`: 176px tall, absolutely
  positioned at the bottom, translated down 128px while folded, with the 48px header carrying the
  fold toggle (`ChevronsUp` folded, `ChevronsDown` unfolded), Add images, Select all, the
  preview-mode toggle group with its tooltips, and the zoom slider between `Minus` and `Plus`. Every
  control except Add images takes `disabled` from a prop. Verify with a test that passes the
  no-files state and asserts the fold toggle, Select all, all three preview modes and the zoom slider
  are disabled while Add images is not.

- [x] 3.4 Build `frontend/features/sidebar/Sidebar.tsx`: 256px wide on `--card` with a `--border` left
  rule; a 144px preview area showing `sidebar.noPreview` in `--foreground-faint`; the 24px-padded
  Autopilot block on `--background` with the label in `--success-bright`, the Switch bound to the
  store, and the disabled `enhancements.add` button; a spacer; and the 48px `sidebar.exportImage` bar
  on `--secondary`, disabled. Verify with a test that Autopilot toggles the store and that Add
  enhancement and Export image are both rendered and disabled.

## 4. Composition

- [x] 4.1 Rewrite `frontend/App.tsx` as the shell - navbar above a row of (canvas with drawer over it)
  and sidebar - passing the no-files state down as the single place it is stated, and delete the
  placeholder's `frontend/App.test.tsx`. Verify with a new `App.test.tsx` that all four regions are
  present at once and that every file-dependent control listed in the spec is disabled.

## 5. Verification

- [x] 5.1 Run `pnpm lint`, `pnpm typecheck`, `pnpm test` and `pnpm build` in `crates/gui`, and
  `cargo fmt --check`, `cargo clippy` and `cargo test` at the workspace root. Then launch
  `pnpm tauri dev` on macOS and compare the window against screen 01: traffic lights inside the
  navbar with the name clear of them, dragging an empty part of the navbar moves the window, Settings
  and About click rather than drag, the drawer folded to its header, and no console error or blocked
  request in devtools.
