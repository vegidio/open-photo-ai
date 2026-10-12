## 1. Workspace and crate manifest

- [x] 1.1 Make `gui` a resolvable workspace member: add `"gui"` to `members` in the root
      `Cargo.toml`, change `[workspace.package] version` from `0.1.0` to `26.9.0` (design D2), and
      create `gui/Cargo.toml` per design D4 - package named `gui` inheriting the workspace version,
      edition, authors, license and repository plus `lints.workspace = true`; a `[lib]` named
      `gui_lib`; `tauri-build` 2 as a build-dependency; `tauri` 2, `serde` (with `derive`),
      `thiserror` 2 and `opai = { path = "../opai" }` as dependencies - alongside `gui/build.rs`
      calling `tauri_build::build()`. The member list and the manifest land together because neither
      verifies without the other. Verify `cargo metadata --format-version 1` succeeds and reports
      all three members at version `26.9.0`, and that `cargo run -p cli` now prints `26.9.0`.

## 2. Tauri configuration

- [x] 2.1 Create `gui/tauri.conf.json` per design D3, D6 and D7: identifier `io.vinicius.opai`,
      productName and mainBinaryName `Open Photo AI`, version `26.9.0`; `build` with
      `beforeDevCommand: "pnpm dev"`, `devUrl: "http://localhost:1420"`, `beforeBuildCommand: "pnpm
      build"`, `frontendDist: "../dist"` (relative to `gui/`, so `gui/dist`); a `main` window titled
      `Open Photo AI` at 1280x720 with matching `minWidth`/`minHeight` and `maximized: true`; the
      `csp`/`devCsp` pair from D6; and a `bundle` block pointing at `icons/`. Also create
      `gui/capabilities/default.json` granting only `core:default` to the `main` window. Verify both
      files parse as JSON (`python3 -m json.tool`) and that every value D7's table marks deferred is
      absent.

- [x] 2.2 Generate `gui/icons/` per design D11. Prepare a 1024x1024 RGBA source from
      `../open-photo-ai/cmd/gui/build/appicon.png` (907x907 RGBA - upscale it; note
      `appicon-macos.png` is 1024x1024 but RGB with no alpha and cannot be used directly), then run
      `pnpm tauri icon <source>` from `gui/` once task 4.1 has installed the CLI. Verify
      `icon.icns`, `icon.ico`, `32x32.png`, `128x128.png`, `128x128@2x.png` and the Windows
      `Square*Logo.png` set all exist under `gui/icons/` and are committed.

## 3. `gui` crate: Rust

- [x] 3.1 Write `gui/src/lib.rs` and `gui/src/main.rs` per design D4 and D5: `main.rs` carries the
      `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` attribute and calls
      `gui_lib::run()`; `lib.rs` holds `run()` building the Tauri app and a single
      `#[tauri::command]` whose entire body returns `opai::version()`, registered via
      `tauri::generate_handler![]`. Verify `cargo build -p gui` succeeds.

- [x] 3.2 Add the drift guard from design D2 as a `#[cfg(test)]` module in `gui/src/lib.rs`: parse
      `tauri.conf.json` as data and assert its `version` equals `env!("CARGO_PKG_VERSION")` and its
      `identifier` equals `io.vinicius.opai`. Verify `cargo test -p gui` passes, then verify the
      guard actually guards - temporarily change one of the two values, confirm the test fails, and
      revert.

## 4. Frontend toolchain

- [x] 4.1 Create `gui/package.json` (private, `type: "module"`, `packageManager` pinned to the
      installed pnpm 12.3.4, and scripts `dev`, `build`, `preview`, `tauri`, `lint`, `format`,
      `check`, `typecheck`, `test`) and `gui/pnpm-workspace.yaml` per design D9 with
      `minimumReleaseAge: 1440` and `allowBuilds` - **not** `onlyBuiltDependencies`, which pnpm 11
      removed - starting from `esbuild` and completed from what pnpm reports blocking on the first
      install. Dependencies per proposal Impact, versions tracking `../intercue`. Add `node_modules`,
      `dist`, `gen/schemas` and `coverage` to `.gitignore`. Verify `pnpm install` from `gui/` exits 0
      and `pnpm-lock.yaml` is written and tracked by git.

- [x] 4.2 Create `gui/biome.json` per design D8: intercue's configuration minus `useSortedClasses`
      and the React `nursery` rules, with 4-space indent, 120 columns, double quotes, always
      semicolons, trailing commas, and import sorting keeping React first. Exclude `src`, `dist`,
      `icons`, `capabilities` and `pnpm-lock.yaml` from its file set. Verify `pnpm exec biome ci .`
      exits 0 from `gui/`.

- [x] 4.3 Create `gui/tsconfig.json` and `gui/vite.config.ts` per design D8: the full strictness set
      (`strict`, `noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`,
      `exactOptionalPropertyTypes`, `noImplicitReturns`, `verbatimModuleSyntax`,
      `noUncheckedSideEffectImports`), `@/*` mapped to `./frontend/*` in **both** files, the React
      plugin, `strictPort: true` on port 1420, `clearScreen: false`, the watcher ignoring
      `**/src/**`, and a Vitest `test` block using `jsdom` and `defineConfig` from `vitest/config`.
      Verify `pnpm exec tsc --noEmit` exits 0.

## 5. Frontend

- [x] 5.1 Create `gui/index.html` (titled `Open Photo AI`, a `#root` div, and a module script
      pointing at `/frontend/main.tsx`) and `gui/frontend/main.tsx` plus `gui/frontend/App.tsx`:
      `main.tsx` mounts `App` into `#root`, and `App` calls the ipc wrapper from 5.2 and renders the
      version it returns. Verify `pnpm build` exits 0 and writes `gui/dist/index.html`.

- [x] 5.2 Create `gui/frontend/ipc/` with a typed wrapper around `invoke()` for the command from
      task 3.1, so the command name is written once (design D5), and a colocated Vitest test that
      mocks `@tauri-apps/api/core` and asserts the wrapper invokes that exact name and returns its
      result. Verify `pnpm test` passes and the test fails if the command name string is changed on
      the TypeScript side alone.

- [x] 5.3 Run the application end to end: `pnpm tauri dev` from `gui/`. Verify a window titled `Open
      Photo AI` opens maximised, cannot be resized below 1280x720, and displays `26.9.0` - the value
      having travelled from `opai::version()` through the Tauri command and `invoke()` into the DOM.
      This is the linkage proof the whole slice exists for; `cargo run -p cli` printing the same
      string from the same function is the other half.

## 6. CI and documentation

- [x] 6.1 Extend `.github/workflows/ci.yml` per design D10: add a `frontend` job
      (`pnpm/action-setup`, `actions/setup-node` with `cache: pnpm`, `pnpm install
      --frozen-lockfile`, then `biome ci .`, `tsc --noEmit` and `vitest run`, all with
      `working-directory: gui`), and add Tauri's Debian package list -
      `libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev
      libayatana-appindicator3-dev librsvg2-dev` - as a step in the `clippy` and `test` jobs.
      Verify with `actionlint` if available, otherwise that the workflow parses as valid YAML and
      each `run` matches a command already passing locally.

- [x] 6.2 Update `README.md`: `gui` is no longer "not present yet", add `pnpm tauri dev` and the
      frontend scripts to the developer commands, and note that pnpm commands run from `gui/`.
      Verify by reading it back - a newcomer should be able to build and run both front ends from it
      alone, with no step that is only in this change's artifacts.

- [x] 6.3 Run the full gate and settle the design's one unverified assumption. Rust:
      `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`,
      `cargo test --locked`. Frontend, from `gui/`: `pnpm exec biome ci .`, `pnpm exec tsc --noEmit`,
      `pnpm test`. Then check the `frontendDist` question in design D10 - delete `gui/dist/` and
      re-run `cargo clippy --all-targets` and `cargo test`; if either fails, add a `mkdir -p
      gui/dist` step to those two CI jobs and record it in the design. Verify all six commands exit
      0, `Cargo.lock` and `gui/pnpm-lock.yaml` are both tracked by git, and `git status --porcelain`
      is empty after a build.
