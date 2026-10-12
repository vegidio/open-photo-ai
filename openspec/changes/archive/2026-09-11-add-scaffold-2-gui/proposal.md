## Why

Slice 1 stood up the Rust half of the scaffold: a workspace where `cli` consumes `opai`, with
formatting, linting and CI green. The `gui` crate it names does not exist. Until it does, the
architecture's central claim - that both front ends reach the same logic in `opai` - is asserted by
one front end rather than demonstrated by two, and every later GUI change would have to bring the
entire Tauri and frontend toolchain with it as a prerequisite.

This is the second and final scaffolding slice. It stands up the GUI half, and with it the whole
scaffold is done.

## What Changes

Task estimate: **14**, inside the 15 ceiling with a slot of slack. Slice 1 estimated the full
scaffold at 15 - exactly the ceiling - and split it; this is the second half of that split.

- **`gui` joins the workspace** as a third member, per slice 1's design D2.
- **The workspace version moves to CalVer `26.9.0`.** Slice 1 scaffolded `0.1.0`; the Go app ships
  CalVer (`25.12.0`), and `tauri.conf.json`, `opai::version()` and `cargo` must not disagree about
  what version this is. One value in `[workspace.package]`, inherited everywhere, pinned by a test.
- **`gui` - the Tauri 2 crate**: `Cargo.toml` (lib + bin split), `build.rs` calling
  `tauri_build::build()`, `tauri.conf.json`, `capabilities/default.json`, and generated `icons/`.
- **Application identity**: identifier `io.vinicius.opai`, productName `Open Photo AI` - the Go
  app's own, so the rewrite is the same product rather than a second one.
- **One Tauri command delegating to `opai`.** `gui` gets a single `#[tauri::command]` that returns
  `opai::version()`. It is the linkage proof `cli`'s `main` already is on the terminal side, and the
  reason it exists is that `cli` and `gui` now call the *same* `opai` function.
- **A drift guard** in `gui`'s Rust tests: `tauri.conf.json`'s `version` and `identifier` are read
  as data and pinned against `CARGO_PKG_VERSION` and the value decided here. The bundle identity and
  the crate cannot silently diverge.
- **Frontend toolchain** at `gui/`: pnpm (with `allowBuilds` and the `minimumReleaseAge` cooldown),
  Vite 8, TypeScript 7, React 19, Biome 2.5, Vitest 5. `gui/frontend/main.tsx` mounts a placeholder
  that `invoke()`s the command above and renders what comes back, with a Vitest test over the
  `invoke` wrapper.
- **CI** gains a frontend job (Biome, `tsc --noEmit`, Vitest) and the Linux system packages Tauri
  needs, without which `cargo clippy` and `cargo test` stop compiling on `ubuntu-latest`.
- **`README.md`** is updated: `gui` is no longer "not present yet", and `pnpm tauri dev` joins the
  developer commands.

**Not in scope**, deliberately: MUI, Tailwind, Zustand and i18next. Slice 1's plan is the toolchain
only, and those libraries are chosen well against real components - a theme, a layer order and a
locale catalog written now against a placeholder would be rewritten by the first UI change. The
first GUI feature change installs them.

Also out of scope: `deny.toml` and dependency auditing, release/packaging/code-signing workflows,
any port of the Go app's UI, and the `rust-sak` dependency question slice 1 left open (nothing here
needs it - see below).

## Capabilities

### New Capabilities

None. Like slice 1, this is project setup - a build target, a bundle configuration, a toolchain and
CI. `.openspec.yaml` sets `skip_specs: true`.

A window does open and print a version, which is the one thing here that looks specifiable. It is
not written as a requirement, for the same reason slice 1 declined to specify `cli` printing a
version: **there is no inherited behaviour to capture.** The Go app's window opens onto the photo
editor, not onto a version string; nothing in `../open-photo-ai` corresponds to what this placeholder
shows. Writing a requirement for it would mean deleting that requirement in the first change that
renders the real UI, and inventing a requirement to satisfy validation is what this project's spec
rule forbids.

### Modified Capabilities

None. `openspec/specs/` is empty.

## Impact

**Crates touched:** `gui` (created), root workspace manifest (member list, version). `opai` and
`cli` are **untouched** - no source change, no new dependency. That they need none is the point:
`gui` reaches `opai::version()`, the same function `cli` already calls, through the same public API.

**Logic in a front end:** none. `gui`'s Rust is its `main`, a `run()` that builds the Tauri app, and
one command whose whole body is a call into `opai`. Nothing that both front ends would need is
placed there.

**`rust-sak`:** nothing is needed from it, and nothing is taken. This slice writes no infrastructure
code - no hashing, HTTP, downloads, extraction, temp files, config directories, image codecs,
caching, telemetry or hardware probes - so no feature applies and none is enabled. Slice 1's finding
stands unchanged and still unresolved: **`rust-sak` is not published on crates.io**, so the first
change that needs a module must settle whether it is a git dependency, a path dependency, or a
release that has to be cut first. This slice does not force that question.

**Dependencies added:**

- `gui` Rust: `tauri` 2, `tauri-build` 2 (build-dependency), `serde` (derive), `thiserror` - the
  last two because a `#[tauri::command]` returns serialisable values and the project's error
  convention is `thiserror`. `opai` by path.
- `gui` frontend: `react`, `react-dom`, `@tauri-apps/api` as dependencies; `@tauri-apps/cli`,
  `vite`, `@vitejs/plugin-react`, `typescript`, `@biomejs/biome`, `vitest`, `jsdom`, the Testing
  Library packages and the `@types/*` needed by the above. Versions track `../intercue`, the
  first-party Tauri 2 project, which is current: Tauri 2.11.x, Vite 8, Vitest 5, Biome 2.5,
  TypeScript 7, React 19.

**CI:** `.github/workflows/ci.yml` gains a `frontend` job and the Linux `-dev` packages Tauri links
against. This is not optional housekeeping - adding `gui` to the workspace makes `cargo clippy
--all-targets` and `cargo test` compile it, and on a bare `ubuntu-latest` that fails at
`webkit2gtk`. The existing jobs would break without it.

**Parity divergence:** the window carries the Go app's title and geometry - `Open Photo AI`, 1280x720
minimum, maximised on start - because those are configuration and cost nothing to get right now.
Four window behaviours are **deliberately deferred**, each needing code or a plugin rather than a
config key, and each belonging with the UI it serves:

- the macOS hidden title bar with its 50px invisible drag region,
- the translucent macOS backdrop,
- file drag-and-drop onto the window,
- single-instance activation (a second launch focuses the existing window).

None is lost, and none is a claim that Tauri cannot do it. They are listed here so the gap is picked
up by a later change instead of being discovered as a regression against the Wails app.

The placeholder frontend is likewise not a claim about what the GUI will look like. It renders one
string and will be deleted whole by the first change that ports the real UI.
