## Slices

The full scaffold estimated at 15 tasks - exactly the ceiling, with no slack - so it was split in
two. The division is a delivery boundary, not just arithmetic: each slice ends with the project
building, testing and passing CI on its own.

| # | Slice | Delivers | Depends on | Status |
|---|-------|----------|------------|--------|
| 1 | `add-scaffold-1-workspace` | Cargo workspace (virtual manifest), `opai` library crate, `cli` binary crate calling into it, `rustfmt.toml`, `[workspace.lints]`, `LICENSE` (AGPL-3.0), `README.md`, `.gitignore`, `.editorconfig`, CI jobs for fmt/clippy/test | nothing | **done** |
| 2 | `add-scaffold-2-gui` | `gui` added to workspace members; workspace version moved to CalVer; Tauri 2 crate (`Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/`, icons); one Tauri command delegating to `opai`; pnpm + Vite + TypeScript + React + Biome + Vitest toolchain; `frontend/main.tsx` round-tripping an `invoke()`; CI frontend job and Tauri's Linux system deps | slice 1 | **current** |

This is the last slice. When it lands, `cargo run -p cli` and `pnpm tauri dev` both reach the same
`opai` function, and CI covers both halves. Neither slice ports any UI or any behaviour from the Go
application; the scaffold is finished and feature work starts on top of it.

## Context

See proposal.md - Why. The repository today is slice 1's output: a virtual manifest with members
`opai` and `cli`, `opai` exposing a single `version()` with no dependencies, `cli` a `main` that
prints it, and a three-job `ubuntu-latest` CI.

Three things outside this repository shape the decisions below:

- **`../intercue`** - the first-party Tauri 2 + React project, and the source of most of what
  follows. Its `rustfmt.toml` is already in this repository verbatim; its `biome.json`,
  `tsconfig.json`, `pnpm-workspace.yaml` and CI shape are adapted here. Its dependency versions were
  checked against the registry rather than trusted: Tauri 2.11.x, `tauri-build` 2.6.3, Vite 8,
  Vitest 5, Biome 2.5.12, TypeScript 7 and React 19 are all current as of September 2026.
- **`../open-photo-ai`** - the Go + Wails original. `cmd/gui/build/config.yml` holds the application
  identity and `cmd/gui/main.go` the window options; both are read directly below rather than
  guessed at.
- **`../rust-sak`** - the first-party utility crate. Not consumed; see below.

The constraint that does the most work is the one slice 1 already named: this is a **three-crate
workspace**, which intercue is not. intercue is a single Tauri crate with its frontend at the
repository root and its Rust in `src-tauri/`. That layout cannot transfer, and slice 1's D1 already
decided what replaces it.

## Goals / Non-Goals

**Goals:**

- Implement slice 1's D1 layout exactly, so the decision recorded there is not quietly redesigned.
- Demonstrate the layering rather than assert it: `cli` and `gui` calling the *same* `opai` function.
- Give the repository one version number, with a test that keeps it one.
- Leave `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `biome ci`, `tsc --noEmit`
  and `vitest run` all green, locally and in CI.
- Carry the Go app's window identity and geometry where they are configuration, and record precisely
  what was deferred where they are not.

**Non-Goals:**

- Any application behaviour, and any part of the Go app's UI. `.openspec.yaml` sets
  `skip_specs: true`.
- MUI, Tailwind, Zustand and i18next - see D8.
- Consuming `rust-sak`, `ort`, or any inference dependency.
- Release, packaging, code signing or notarisation. `tauri.conf.json` will carry a `bundle` block
  because a Tauri config needs one, but no workflow builds a bundle in this slice.
- Deciding `gui`'s module structure beyond what one command needs.

## Decisions

### D1. The layout is slice 1's D1, implemented unchanged

```
opai-rust/
+- Cargo.toml            members = ["opai", "cli", "gui"]; version = "26.9.0"
+- opai/  cli/           unchanged by this slice
+- gui/
   +- Cargo.toml  build.rs  tauri.conf.json
   +- capabilities/default.json
   +- icons/
   +- src/               RUST     - main.rs, lib.rs
   +- package.json  biome.json  vite.config.ts  tsconfig.json  pnpm-workspace.yaml  index.html
   +- frontend/          TYPESCRIPT - main.tsx, App.tsx, ipc/
   +- dist/              build output, gitignored
```

`package.json` and `tauri.conf.json` sit in the same directory, which is the property slice 1 chose
this layout for: the Tauri CLI finds its configuration in the working directory and never has to
walk up out of the frontend root. `pnpm` commands run from `gui/`.

`@/*` aliases to `./frontend/*` in **both** `tsconfig.json` and `vite.config.ts` - Vite does not read
`compilerOptions.paths`, so omitting the second breaks the build while `tsc` still passes. The alias
is what makes `gui/src/` being Rust invisible to ported code: every `@/components/Foo` import from
the Wails frontend transfers byte-identical.

### D2. One version, CalVer, pinned by a test

`[workspace.package] version` moves from slice 1's `0.1.0` to **`26.9.0`**, and every crate inherits
it. `tauri.conf.json` carries the same literal string.

CalVer because the Go app ships CalVer (`build/config.yml`: `version: "25.12.0"`) and this is the
same product continuing, not a new one. `26.9.0` rather than `25.12.0` because it is the current
YY.M.MICRO and stays ahead of what the Go app has already shipped, so a release can never go
backwards.

*Why the string is duplicated rather than derived.* Tauri's `version` field accepts a semver string
or a path to a **`package.json`**; with the field omitted it falls back to the crate's `Cargo.toml`.
Neither helps here. `gui/Cargo.toml` says `version.workspace = true`, and the fallback path reads
that manifest directly rather than through `cargo metadata`, so workspace inheritance is not
something it can be relied on to resolve. Pointing it at `gui/package.json` would just move the
duplicate into a file nobody thinks of as the version's home.

So the value is written twice and a test makes that safe: `gui`'s Rust tests parse
`tauri.conf.json` as data and assert its `version` equals `env!("CARGO_PKG_VERSION")` and its
`identifier` equals the value decided in D3. This is intercue's own pattern - its CI comments
describe the Rust tests as "cross-boundary drift guards... the *only* thing tying files in two
languages together" - and it is the right shape here for the same reason: nothing else in the
toolchain notices when one of the two is edited alone.

*Alternative considered.* `build.rs` reading `tauri.conf.json` and failing the build on a mismatch.
Rejected: it fails at build time in a script whose job is `tauri_build::build()`, and a failing test
names the two values in its assertion message where a panicking build script does not.

### D3. Application identity: the Go app's own

`identifier: "io.vinicius.opai"`, `productName: "Open Photo AI"`, taken from
`../open-photo-ai/cmd/gui/build/config.yml`. The rewrite is the same product, so it keeps the same
reverse-DNS identity rather than acquiring a second one that would have to be migrated away from at
release.

*The known cost.* On a machine with both applications installed, the two bundles share an identifier,
which is what macOS uses to decide which app a document or a URL scheme belongs to. This is a
development-time nuisance and nothing more: the Go app derives its config and cache directories from
an **app name passed to `opai.Initialize`**, not from the bundle identifier (see
`../open-photo-ai/init.go`), so neither app can read or corrupt the other's data. If the bundle
ambiguity becomes annoying while both are installed, the fix is a dev-only identifier suffix, not a
different release identity.

*Alternative considered.* `io.vinicius.opai-rust`. Rejected: it makes the rename a release-blocking
task later in exchange for convenience now, and `-rust` is an implementation detail leaking into a
user-visible identifier.

### D4. `gui`'s Cargo layout: lib plus thin bin, package named `gui`

`gui/Cargo.toml` declares a `[lib]` named `gui_lib` alongside the default binary. The `_lib` suffix
is stock Tauri and avoids a lib/bin target name clash, which is mainly a Windows problem.
`src/main.rs` is a `windows_subsystem` attribute and a call to `gui_lib::run()`; everything else
lives in `src/lib.rs`. This is intercue's split, and it exists so the crate's contents are reachable
from `#[cfg(test)]` - a binary-only crate cannot host the D2 drift guard.

The package is named `gui`, not `opai-gui`, matching `cli` from slice 1. The bundled application is
named by `productName` and `mainBinaryName` in `tauri.conf.json`, so the Cargo package name is never
user-visible and consistency across the three crates is the only criterion left.

### D5. The command, and why it is the only one

One `#[tauri::command]` returning `opai::version()`. Its body is a call and nothing else.

The point is not the version string. It is that `cli/src/main.rs` and this command call the **same**
`opai` function, which is the layering rule demonstrated rather than asserted - and the first time in
this project it can be. Two front ends over one library is exactly what `opai` exists for; a second
placeholder command would prove nothing the first does not.

Registered through `tauri::generate_handler![]` in `run()`. The frontend reaches it through a typed
wrapper in `frontend/ipc/` rather than calling `invoke()` from a component, so the command name is
written once on the TypeScript side and the Vitest test has something to test.

### D6. Security configuration: closed by default

**Capabilities.** `capabilities/default.json` grants `core:default` to the `main` window and nothing
more. No plugins are installed in this slice, so there is nothing else to grant - and the file is
worth writing now rather than later precisely so the first plugin arrives as a visible addition to a
minimal list instead of into an absent file.

**CSP.** A `default-src 'self'` policy with `connect-src 'self' ipc: http://ipc.localhost` (the IPC
origins Tauri 2 needs), `img-src 'self' data: asset: http://asset.localhost`, `object-src 'none'`,
`base-uri 'self'` and `frame-ancestors 'none'`. `style-src` allows `'unsafe-inline'`, which is not a
concession to convenience: Vite injects styles inline in development, and MUI's Emotion runtime -
arriving with the first UI change - emits `<style>` elements at runtime and cannot work without it.

A separate `devCsp` widens `script-src` and `connect-src` for the Vite dev server and its HMR
socket, so the shipping policy is not loosened to make `pnpm tauri dev` work. This app makes no
outbound requests yet; when the model and runtime downloads arrive, they will be made **from Rust**,
where the CSP does not apply and the URL allowlist can be code rather than configuration.

### D7. Window configuration: geometry now, behaviour later

From `../open-photo-ai/cmd/gui/main.go`: `title: "Open Photo AI"`, `width`/`height` and
`minWidth`/`minHeight` of `1280x720`, and `maximized: true` for the Wails
`WindowStateMaximised` start state. All four are `tauri.conf.json` keys, cost nothing, and would be
a gratuitous parity gap if skipped.

Four things from the same block are **deferred**, and each needs code or a plugin rather than a
config key:

| Wails | Tauri equivalent | Why not now |
|-------|------------------|-------------|
| `TitleBar: MacTitleBarHidden` + `InvisibleTitleBarHeight: 50` | `titleBarStyle: "Overlay"` + `hiddenTitle`, plus a CSS drag region | The 50px region is a frontend layout element. Without the real UI there is nothing to attach it to, and a window with a hidden title bar and no drag region cannot be moved. |
| `Backdrop: MacBackdropTranslucent` | `transparent: true` + `macos-private-api` | Requires the frontend to paint its own background; a transparent window over a placeholder is an unreadable window. |
| `EnableFileDrop: true` | `dragDropEnabled` + a listener | The listener is the feature. Enabling the event with nothing to receive it is not parity. |
| `SingleInstance` | `tauri-plugin-single-instance` | A dependency and a capability grant whose whole purpose is to focus a window the user cares about. |

Listing them here is the mitigation for the risk that a deferral becomes a regression nobody
remembers choosing.

### D8. Frontend toolchain only - no UI libraries

React, TypeScript, Vite, Biome and Vitest. Not MUI, Tailwind, Zustand or i18next, though all four are
in the project's stack and all four will arrive.

The reason is that those four are configured against components, and there are none. MUI needs a
theme and a `StyledEngineProvider` layer order - both Go `main.tsx` and intercue `main.tsx` carry
non-obvious, hard-won versions of exactly that. Tailwind needs its layer registered in the same
place. i18next needs a locale catalog, and this project's own task rule says keys go into *every*
locale file - a rule with nothing to apply to yet. Writing any of it now against a placeholder means
writing it twice.

What *is* set up is the part that is genuinely infrastructure and genuinely painful to retrofit: the
`@/*` alias, `biome.json`'s formatting and import-sorting rules, `tsconfig.json`'s strictness flags,
and a Vitest configuration that shares the alias with the build.

**`biome.json`** is intercue's, minus what does not apply: no `useSortedClasses` (a Tailwind rule)
and no React-component-definition rule in `nursery` until there are components worth constraining.
The formatter settings - 4-space, 120 columns, double quotes, always semicolons, trailing commas -
match `.editorconfig` and `rustfmt.toml`, so the two languages agree about width and indent. Import
sorting keeps React first; the MUI group is added with MUI.

**`tsconfig.json`** is intercue's strictness set: `strict`, `noUnusedLocals`, `noUnusedParameters`,
`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitReturns`,
`verbatimModuleSyntax`, `noUncheckedSideEffectImports`. Every one of these is cheap to adopt on an
empty codebase and expensive on a populated one, which is the argument for adopting them exactly now.
Note that the Go project's `tsconfig.json` has a *weaker* set - no `exactOptionalPropertyTypes`, no
`verbatimModuleSyntax` - so some ported code will need adjusting. That is a known, bounded cost paid
by the change that ports it, and the alternative is weakening this file to match the weakest input.

**`vite.config.ts`** is deliberately close to empty: the React plugin, the `@/*` alias, a
`strictPort` 1420 dev server, `clearScreen: false` so Rust errors stay visible, `ignored:
["**/src/**"]` on the watcher so Cargo's rebuild loop is not double-triggered, and the Vitest
`test` block. **No React Compiler.** intercue runs it through a separate Babel plugin and documents
at length that the inline `react({ babel })` form most guides show is silently a no-op under
`@vitejs/plugin-react` v6 - a trap worth knowing about, and worth walking into deliberately with real
components to compile rather than pre-emptively with none.

`vitest/config` provides `defineConfig` rather than `vite`, so the tests inherit the alias and the
React plugin instead of needing a second config that can drift from this one.

### D9. pnpm supply-chain policy - correcting slice 1's D7

Slice 1's D7 said `gui/pnpm-workspace.yaml` would carry `minimumReleaseAge: 1440` plus
`onlyBuiltDependencies` for `esbuild` and `@tailwindcss/oxide`. Two corrections, both checked rather
than assumed:

1. **`onlyBuiltDependencies` no longer exists.** pnpm 11 removed it along with
   `neverBuiltDependencies` and `ignoredBuiltDependencies`, replacing all of them with
   **`allowBuilds`**, a map of package name to boolean. intercue's current `pnpm-workspace.yaml`
   already uses the new spelling; slice 1's D7 was written from the Go project's older file. The
   local pnpm is 12.3.4, so `allowBuilds` is the only correct form.
2. **`@tailwindcss/oxide` is not needed**, because D8 does not install Tailwind. It joins the list
   with Tailwind.

`minimumReleaseAge: 1440` stands unchanged - no package version younger than a day is installed,
which is the window in which most hijacked releases are caught and yanked. The `allowBuilds` list
starts at `esbuild` and is **finished from what pnpm actually reports** during the first install,
rather than from a list copied out of another project: the set depends on the exact dependency tree,
and Vite 8 bundles Rolldown where Vite 7 bundled esbuild.

`packageManager` pins the installed pnpm (12.3.4) rather than intercue's 11.24.0, so Corepack
resolves to the version this slice is actually verified against.

**No `minimumReleaseAgeExclude` entries.** The Go project's file carries five, and intercue's
comment records what happened to its own: they "had all drifted behind the installed versions, which
made them dead entries - an exclusion only means anything while it names the version being
installed". An exclusion is added when the cooldown actually blocks something, naming the version it
blocks, and not before.

### D10. CI: a frontend job, and the system deps the Rust jobs now need

`.github/workflows/ci.yml` keeps its shape - same triggers, same `permissions: contents: read`, same
concurrency group - and gains a `frontend` job: pnpm via `pnpm/action-setup`, Node LTS with pnpm
caching, `pnpm install --frozen-lockfile`, then `biome ci .`, `tsc --noEmit` and `vitest run`.

`biome ci` rather than `biome check`: same rules, but it never writes and it formats its output for
the annotations panel.

The `clippy` and `test` jobs gain a Linux system-dependency step. This is **not optional**: adding
`gui` to the workspace members means `cargo clippy --all-targets` and `cargo test` compile it, and
`tauri` links against webkit2gtk. On a bare `ubuntu-latest` both jobs fail at the link step, long
after clippy has reported the tree fine. The packages are Tauri's own documented Debian list -
`libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libxdo-dev`, `libssl-dev`,
`libayatana-appindicator3-dev`, `librsvg2-dev`.

Still `ubuntu-latest` only, and slice 1's reasoning still holds: nothing here is
platform-conditional. The three-OS matrix belongs to the first change that writes `cfg(target_os)`
code - which, given `ort` and the hardware probes, is not far off.

**No bundle build in CI.** `tauri build` would produce an artifact nothing consumes, on a job that
takes minutes. Packaging gets its own change.

*The `frontendDist` question, answered by precedent.* `frontendDist` points at `../dist`, which is
gitignored - so the `clippy` and `test` jobs compile `gui` against a directory that does not exist on
a fresh checkout, and `tauri::generate_context!` embeds the frontend at compile time. The reason this
is expected to work is not theory: **intercue's CI does exactly this today** - its `rust` job runs
`cargo clippy --all-targets` and `cargo test` with `dist` gitignored, untracked, and never built by
that job, and it is the gate a release tag has to pass. Tauri 2 treats the missing directory as empty
for anything short of `tauri build`. If it nevertheless fails, the fix is one line - `mkdir -p
gui/dist` ahead of the cargo steps - not a frontend build in the Rust jobs. Confirm which during the
verification task rather than pre-emptively adding a step that may be dead.

**Answered, in task 6.3: no step is needed.** With `gui/dist/` deleted and `gui/src/lib.rs` touched
to force `generate_context!` to re-run against the missing directory, both `cargo clippy
--all-targets` and `cargo test` exit 0 on tauri 2.11.5. The precedent held, and the `mkdir -p
gui/dist` fallback was **not** added to either job - it would have been a dead step.

The icons are a different matter and are *not* optional: `generate_context!` reads
`icons/32x32.png` at compile time and fails the build outright when it is absent. That is why D11's
generated set is committed rather than produced by a build step, and it is why task 2.2 has to land
before task 3.1 can build - an ordering neither task states.

### D11. Icons

`tauri icon` generates the full set - `icons/`, `icon.icns`, `icon.ico`, the Windows Square*Logo
PNGs - from one source image, and the source is the Go app's `cmd/gui/build/appicon.png` so the
artwork does not diverge.

**Neither Go source file is directly usable**, which is worth knowing before the task is attempted
rather than during it. `tauri icon` wants a 1024x1024 RGBA PNG. `appicon.png` is 907x907 RGBA - too
small - and `appicon-macos.png` is 1024x1024 but **RGB, with no alpha channel**. The source has to be
prepared: upscale `appicon.png` to 1024x1024, or add an alpha channel to `appicon-macos.png`. The
former preserves the transparency the icon actually needs and is the one to try first.

Generated icons are committed, as in both reference projects. They are build inputs, and `tauri
build` fails without them.

## Division across crates

| Crate | This slice | Rule |
|-------|-----------|------|
| `opai` | **Untouched.** No source change, no new dependency. | Everything both front ends would need. |
| `cli` | **Untouched.** | Argument parsing, terminal output, progress rendering - still none of it. |
| `gui` | Created: Tauri configuration, `run()`, one command, the drift guard, and the frontend. | Tauri commands and events, window and app state plumbing, and the React frontend. |

No logic is placed in a front end. `gui`'s only non-plumbing line is a call to `opai::version()` -
the same function `cli/src/main.rs` calls, which is the entire point of D5.

That `opai` needs no change to serve a second front end is the layering working. Any future slice
that cannot add a front-end feature without editing `opai`'s *interface* is fine; one that needs to
move logic *out* of a front end has already gone wrong.

## `rust-sak`

**Features used: none.** This slice writes no infrastructure code - no hashing, HTTP, retries,
downloads, archive extraction, temp files, config directories, image codecs, caching, telemetry or
hardware probes - so no feature applies and none is enabled. No gap exists to fill, and therefore no
question of extending `rust-sak` versus writing something local: there is nothing to write.

No third-party crate introduced here duplicates a `rust-sak` module. The Rust dependencies are
`tauri`, `tauri-build`, `serde` and `thiserror`; `rust-sak` provides no application framework, no
build script helper, and no serialisation or error-derive layer. `thiserror` is what `rust-sak` and
the other first-party crates use, so adopting it is following the convention rather than competing
with it.

Slice 1's open finding stands and is **still unresolved**: `rust-sak` is not published on crates.io -
the registry returns `crate 'rust-sak' does not exist` for both spellings, the repository has no
releases, and the local checkout reads `version = "2.0.0"` while its README instructs `cargo add
rust-sak`. Nothing in this slice forces the question, so it passes intact to the first change that
needs a module.

## Other crates introduced

All in `gui`; none in `opai` or `cli`.

| Crate | Why, and over what |
|-------|--------------------|
| `tauri` 2 | The framework the project's stack mandates. No alternative was weighed. |
| `tauri-build` 2 | Build-dependency. `tauri_build::build()` generates the Tauri context, the capability JSON schemas and the platform resources; a Tauri app does not compile without it. Not a choice. |
| `serde` (`derive`) | A `#[tauri::command]`'s return value crosses the IPC boundary as JSON. `tauri` re-exports serde but does not provide the derive macro, so it is declared directly. Tauri's IPC is built on serde; there is no alternative to weigh. |
| `thiserror` 2 | The project's error convention, matching `rust-sak` and the other first-party crates. Chosen over `anyhow` for the same reason those crates do: a library boundary wants named variants a caller can match on, and `gui`'s command returns cross the IPC boundary where an opaque error is useless to the frontend. Over a hand-written `impl Error`, because the derive is the whole value. |

On the frontend side, every package is either mandated by the stack (React, TypeScript, Vite, Biome,
Vitest) or required to use it (`@tauri-apps/api` for `invoke`, `@tauri-apps/cli` for `tauri dev`,
`@vitejs/plugin-react`, `jsdom` and the Testing Library packages for component tests, the `@types/*`
for the above). Versions track `../intercue` and were verified current against the registry rather
than taken on trust - see Context.

## Model sessions, memory budgets, execution providers

Not touched. No ONNX Runtime, no `ort`, no sessions, no memory budgets, no execution providers, and
therefore no ownership or lifetime model to describe.

The one thing worth recording for the change that does bring them: `gui`'s command in D5 is
stateless, so this slice establishes no precedent for Tauri state management. The model registry will
arrive as a `tauri::State` holding something owned by `opai`, and the ownership design belongs to
that change, not to a decision accidentally set here.

## Platforms

Designed and to be verified on **macOS (arm64)**, the only machine available. `pnpm tauri dev` is
verifiable there; `cargo build -p gui`, `cargo clippy` and `cargo test` are verifiable there.

**Windows and Linux are untested locally.** CI covers Linux via `ubuntu-latest` once D10's system
dependencies land - and that job is doing real work now, not ceremony: it is the only thing that
compiles `gui` against webkit2gtk rather than WKWebView. **Windows is compiled by nothing** in this
slice. The exposure is bounded and known: the `_lib` suffix in D4 and the `windows_subsystem`
attribute in `main.rs` both exist specifically for Windows, and both are stock Tauri boilerplate
verified by every Tauri project rather than by this one.

**Execution providers: none exercised**, on any platform.

## Risks / Trade-offs

- **`minimumReleaseAge: 1440` can block an install outright** → A version younger than a day is not
  installed, and with a fresh lockfile every package resolves to its newest matching version. If
  something published in the last 24 hours is picked, install fails. Mitigation: pin that one
  package a patch back, or add a `minimumReleaseAgeExclude` entry *naming the exact version* - and
  remove it once the version ages out, which is the discipline both reference projects' comments
  record failing at.

- **The `allowBuilds` list cannot be written correctly in advance** → It depends on the resolved
  tree, and Vite 8 bundles Rolldown where earlier versions bundled esbuild. Mitigation: D9 makes the
  first install the source of truth. pnpm reports the packages whose build scripts it blocked; that
  report finishes the list.

- **`tauri.conf.json` and `Cargo.toml` carry the same version string** → The drift guard in D2 is
  the mitigation, and it is a test rather than a convention, so it fails in CI rather than at
  release.

- **The identifier is shared with the installed Go app** (D3) → macOS bundle ambiguity while both
  are installed. Bounded: config and cache directories come from an app name, not the identifier, so
  no data is at risk. Mitigation if it bites: a dev-only identifier suffix.

- **`gui` in the workspace breaks the existing CI jobs until D10 lands** → `cargo clippy
  --all-targets` and `cargo test` compile `gui`, and `ubuntu-latest` has no webkit2gtk. Mitigation:
  the system-dependency step and the member-list addition are in the same slice, and the final
  verification task runs the whole gate.

- **The Rust CI jobs compile `gui` with no `gui/dist/` present** → `frontendDist` is gitignored and
  the Rust jobs do not build the frontend. Evidence says this is fine (intercue's release gate does
  it), but it is precedent rather than a run of *this* configuration. Mitigation: the verification
  task checks it explicitly, and the fallback is one `mkdir -p gui/dist` step - cheap either way.

- **`tsconfig.json` is stricter than the Go project's** (D8) → Ported frontend code will not compile
  unchanged; `exactOptionalPropertyTypes` and `verbatimModuleSyntax` in particular will need import
  and optional-property edits. Accepted deliberately: the cost is paid per-file by the change that
  ports each file, and the alternative is permanently weakening this file to the weakest input.

- **Four Wails window behaviours are deferred** (D7) → A later change could ship a window that looks
  wrong against the original without anyone noticing the omission was a choice. Mitigation: the D7
  table names all four with their Tauri equivalents, and the proposal repeats them under parity
  divergence.

- **Neither Go icon source is usable as-is** (D11) → Found now rather than mid-task. Mitigation: the
  preparation step is part of the icon task, with the upscale named as the approach to try first.

- **Windows compiles nowhere in this slice** → Accepted. Nothing here is Windows-conditional beyond
  two pieces of stock Tauri boilerplate, and a Windows CI runner to prove a version string is the
  same everywhere is the same trade slice 1 declined for all three platforms.

## Open Questions

- **How will `opai` depend on `rust-sak`** - published crates.io release, git dependency pinned to a
  tag, or path? Unchanged from slice 1 and still safely deferrable: nothing in this slice consumes
  it, so the answer changes nothing here. It must be answered by the first change that needs a
  `rust-sak` module, and answering it may mean publishing `rust-sak` 2.0.0 first - work in that
  repository, which by this project's rules needs its own tasks and explicit go-ahead.

- **When the three-OS CI matrix arrives.** Deferrable because nothing in this slice is
  platform-conditional. It is owed by the first change that writes `cfg(target_os)` code, which the
  execution providers and hardware probes make likely to be soon.
