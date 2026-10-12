## Slices

The full scaffold estimates at 15 tasks - exactly the ceiling, with no slack - so it is split in two.
The division is a delivery boundary, not just arithmetic: each slice ends with the project building,
testing and passing CI on its own.

| # | Slice | Delivers | Depends on | Status |
|---|-------|----------|------------|--------|
| 1 | `add-scaffold-1-workspace` | Cargo workspace (virtual manifest), `opai` library crate, `cli` binary crate calling into it, `rustfmt.toml`, `[workspace.lints]`, `LICENSE` (AGPL-3.0), `README.md`, `.gitignore`, `.editorconfig`, CI jobs for fmt/clippy/test | nothing | **current** |
| 2 | `add-scaffold-2-gui` | `gui` added to workspace members; Tauri 2 crate (`Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/`, icons); one Tauri command delegating to `opai`; pnpm + Vite + TypeScript + Biome + Vitest toolchain; `frontend/main.tsx` round-tripping an `invoke()`; CI frontend job | slice 1 | pending |

Done when slice 2 lands: `cargo run -p cli` and `pnpm tauri dev` both reach `opai`, and CI covers
both halves. Neither slice ports any UI or any behaviour from the Go application.

## Context

The repository contains only `.claude/` and `openspec/`. See proposal.md - Why.

Three things outside this repository shape the decisions below:

- **`../open-photo-ai`** - the Go + Wails original. Its `cmd/gui/frontend/` nests the React app
  inside the GUI command, and its `build/config.yml` holds the application identity.
- **`../intercue`** - a first-party Tauri 2 + React project. It is the source of the `rustfmt.toml`
  and `biome.json` this project adopts, and its layout is stock Tauri (frontend at the repository
  root, Rust in `src-tauri/`).
- **`../rust-sak`** - the first-party utility crate. Not consumed in this slice; see below.

The constraint that does the most work here is that this repository is a **three-crate workspace**,
which neither reference project is. intercue is a single Tauri crate; the Go project uses a Go
workspace with `cmd/` subdirectories. Neither layout transfers unaltered.

## Goals / Non-Goals

**Goals:**

- Establish the `opai` / `cli` / `gui` layering as the workspace's actual shape, so later changes
  inherit it rather than each arguing for it.
- Settle the repository layout - including where the frontend will live - once, now, while it costs
  nothing to move things.
- Adopt the first-party formatting and linting rules verbatim where they transfer, and record
  precisely where and why they could not.
- Leave `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` green, in CI and locally.

**Non-Goals:**

- Any application behaviour. Nothing here is user-observable; `.openspec.yaml` sets `skip_specs: true`.
- Choosing the CLI's argument parser, or deciding what the Rust `cli` ultimately does.
- Consuming `rust-sak`, `ort`, or any other dependency.
- Release, packaging, code signing or notarisation workflows.

## Decisions

### D1. Repository layout: the `gui` crate is also the frontend root

Chosen: **`gui/` holds `Cargo.toml`, `tauri.conf.json` and `package.json` side by side**, with Rust
in `gui/src/` and TypeScript in `gui/frontend/`. This slice creates none of it - the decision is
recorded here because slice 2 implements it and because it is cheap now and expensive later.

```
opai-rust/
+- Cargo.toml            virtual manifest, resolver = "3"
+- rustfmt.toml          verbatim from intercue
+- .gitignore  .editorconfig  LICENSE  README.md
+- .github/workflows/ci.yml
+- opai/                 SLICE 1 - core library, no UI knowledge
|  +- Cargo.toml
|  +- src/lib.rs
+- cli/                  SLICE 1 - terminal front end
|  +- Cargo.toml
|  +- src/main.rs
+- gui/                  SLICE 2 - Tauri 2 front end
   +- Cargo.toml  build.rs  tauri.conf.json
   +- capabilities/  icons/
   +- src/               RUST
   +- package.json  biome.json  vite.config.ts  tsconfig.json  index.html
   +- frontend/          TYPESCRIPT
   +- dist/              build output, gitignored
```

*Alternatives considered.*

**Frontend at the repository root** (intercue's layout). Rejected: it puts `package.json`,
`vite.config.ts`, `index.html` and a JavaScript `src/` at the top of a workspace whose other two
crates never touch them, and it contradicts the architecture's own description of `gui` as the crate
that contains "Tauri commands and events, window and app state plumbing, and the React frontend".

**Frontend nested at `gui/frontend/` with its own `package.json`** (the Go project's layout).
Rejected on a concrete risk: with `tauri.conf.json` at `gui/` and `package.json` one level deeper,
the Tauri CLI has to walk *up* out of the frontend directory to find its configuration. That
behaviour could not be verified offline - no native CLI binary is installed and the registry sources
are not cached - and while `--config ../tauri.conf.json` would force it, designing around an
unverified assumption is a bad trade for a cosmetic gain. The chosen layout puts `package.json` and
`tauri.conf.json` in the *same* directory, which is exactly what intercue does at the root, so the
CLI finds its configuration in the working directory and the question never arises.

*The one cost, and why it is not a cost.* `gui/src/` is Rust, so the React source cannot use `src/`,
and both reference frontends alias `@/*` to `./src/*` - meaning every ported Wails import reads
`@/components/Foo`. Pointing `@/*` at `./frontend/*` in both `tsconfig.json` and `vite.config.ts`
makes the rename **invisible to ported code**: every such import transfers byte-identical. The
directory is named `frontend/` rather than `ui/` to keep the Go project's vocabulary, since `@/`
hides it from imports and readability is then the only criterion left.

### D2. Virtual workspace manifest with `resolver = "3"`

The root `Cargo.toml` is a virtual manifest - `[workspace]` with no `[package]` - so no crate is
privileged by living at the root. A virtual manifest does **not** inherit the resolver from a
member's edition, so `resolver = "3"` is set explicitly; without it Cargo falls back to resolver 1
and warns. `[workspace.package]` carries version, edition, authors, license and repository for
members to inherit.

Members are `["opai", "cli"]` in this slice; slice 2 appends `"gui"`.

### D3. `rustfmt.toml` verbatim, `[workspace.lints]` moderate, no MSRV

The `rustfmt.toml` is copied unaltered from intercue, comment included. Its substance is
`max_width = 120` plus eight per-construct sub-widths pinned to 96 - rustfmt's defaults cap those
around 60-70% of `max_width` (~72), which breaks chains and calls long before the 120 ceiling. The
file is identical across first-party Rust projects on purpose; diverging it would be the only reason
formatting could differ between them.

`[workspace.lints]` takes intercue's set: `unsafe_op_in_unsafe_fn = "deny"` under `[lints.rust]`, and
`undocumented_unsafe_blocks`, `todo`, `unimplemented`, `dbg_macro` as warnings under
`[lints.clippy]`. This is deliberately lighter than rust-sak's, which adds `missing_docs`,
`missing_debug_implementations`, `missing_errors_doc` and `missing_panics_doc` - appropriate for a
published library with a documented public API, friction for an application still being scaffolded.
The unsafe-related lints earn their place early regardless: `ort` and the platform probes will bring
FFI, and a denied `unsafe_op_in_unsafe_fn` from commit one is free, while retrofitting it is not.

No `rust-version` pin. An application makes no MSRV commitment, and the only MSRV in sight
(rust-sak's 1.95) arrives as a dependency constraint when that dependency arrives.

### D4. `opai` has no dependencies in this slice

Not even `thiserror`, which the conventions mandate for errors. Nothing in this slice is fallible, so
an error enum would be a speculative type with no variants in use. `thiserror` is added by the first
change that introduces an operation that can fail - which is also the change that knows what the
variants should be.

### D5. CI shape

One workflow, `.github/workflows/ci.yml`, on push to `main`, pull request, and `workflow_dispatch`,
with `permissions: contents: read` and a `concurrency` group cancelling superseded runs. Three jobs,
following rust-sak's structure since that is the first-party Rust CI:

- `fmt` - `cargo fmt --all --check`
- `clippy` - `cargo clippy --all-targets --locked -- -D warnings`, with `Swatinem/rust-cache`
- `test` - `cargo test --locked`

`--locked` requires `Cargo.lock` to be committed, which is correct for a workspace containing binary
crates. Jobs run on `ubuntu-latest` only in this slice: there is no platform-specific code yet, so a
three-OS matrix would burn CI minutes proving that a version string is the same everywhere. The
matrix belongs to the first change that introduces platform-conditional code.

rust-sak's per-feature matrix job is deliberately not copied - it exists because every `rust-sak`
feature must compile alone, and this workspace has no features.

### D6. Licensing and editor configuration

`LICENSE` is AGPL-3.0. This is **continuity, not a relicense**: `gh api repos/vegidio/open-photo-ai`
reports `AGPL-3.0` and the Go project's `LICENSE` opens with "GNU AFFERO GENERAL PUBLIC LICENSE
Version 3". `[workspace.package] license = "AGPL-3.0"` matches it.

`.editorconfig` is adapted from the Go project's: UTF-8, 4-space indent, 120 columns, trimmed
trailing whitespace, 2-space YAML. The `[*.go]` tab stanza is dropped and `[*.rs]` is explicit.
`insert_final_newline` is set to `true`, against the Go project's `false` - rustfmt emits a trailing
newline unconditionally, so leaving it `false` would put the editor and the formatter in permanent
disagreement.

### D7. pnpm supply-chain policy (recorded for slice 2)

`gui/pnpm-workspace.yaml` will carry intercue's `minimumReleaseAge: 1440` - no package version
younger than a day is installed, which is the window in which most hijacked releases are caught and
yanked - plus `onlyBuiltDependencies` for `esbuild` and `@tailwindcss/oxide`, both of which need
build scripts for the toolchain to work at all. Recorded here so slice 2 does not relitigate it.

## Division across crates

| Crate | This slice | Rule |
|-------|-----------|------|
| `opai` | Created. One public function returning the application version, with a colocated test. | Everything both front ends would need. |
| `cli` | Created. `main` only: calls `opai` and prints the result. | Argument parsing, terminal output, progress rendering - none of it yet. |
| `gui` | Untouched, not a workspace member. | Slice 2. |

No logic is placed in a front end. The single function `cli` calls lives in `opai` precisely because
`gui` will call the same one in slice 2 - which is the layering rule demonstrated rather than
asserted.

## `rust-sak`

**Features used: none.** This slice writes no infrastructure code - no hashing, HTTP, retries,
downloads, archive extraction, temp files, config directories, image codecs, caching, telemetry or
hardware probes - so no feature applies and none is enabled. No gap exists to fill, and no
third-party crate is introduced that could duplicate a `rust-sak` module, because no crate is
introduced at all.

## Other crates introduced

**None**, in either `opai` or `cli`. See D4.

## Model sessions, memory budgets, execution providers

Not touched. No ONNX Runtime, no `ort`, no sessions, no memory budgets, no execution providers, and
therefore no ownership or lifetime model to describe. The concurrency design the conventions call
for arrives with the model registry, in its own change.

## Platforms

Designed and to be verified on **macOS (arm64)**, the only machine available. **Windows and Linux
are untested locally**; CI covers Linux via `ubuntu-latest`. Nothing in this slice is
platform-conditional - a virtual manifest, a version string, a formatter configuration and three CI
jobs behave identically on all three targets - so the exposure is minimal and bounded to the CI
configuration itself.

**Execution providers: none exercised**, on any platform.

## Risks / Trade-offs

- **`rust-sak` is not on crates.io** → The registry returns `crate 'rust-sak' does not exist` for
  both `rust-sak` and `rust_sak`; the repository has no releases; the local checkout reads
  `version = "2.0.0"` while its README instructs `cargo add rust-sak`. No impact on this slice,
  which consumes nothing. Mitigation: settle it before the first change that needs a module, not
  during it - see Open Questions.

- **A path dependency on `../rust-sak` would break CI and fresh clones** → Not taken here.
  Recorded so the temptation is visible when it arrives.

- **`minimumReleaseAge: 1440` can block a same-day release that is actually needed** → An exclusion
  entry unblocks it. The Go project's `pnpm-workspace.yaml` carries a warning worth heeding: its
  exclusions had all drifted behind the installed versions and become dead entries, since an
  exclusion only means anything while it names the version being installed. Slice 2's concern.

- **`--locked` in CI fails on any uncommitted `Cargo.lock` drift** → Intended. That is the check
  working, not misfiring.

- **D1 is recorded but not implemented here**, so slice 2 could quietly drift from it → The layout
  diagram and the `@/*` alias rationale live in this document, and this Slices table is repeated in
  slice 2's `design.md` by the project's own rule. That is the mitigation.

## Open Questions

- **How will `opai` depend on `rust-sak`** - published crates.io release, git dependency pinned to a
  tag, or path? This is safely deferrable: it changes nothing about this slice's approach, artifacts
  or tasks, since nothing here depends on it. It must be answered by the first change that needs a
  `rust-sak` module, and answering it may mean publishing `rust-sak` 2.0.0 first - which is work in
  that repository, and by this project's rules needs its own tasks and explicit go-ahead.

- **What the Rust `cli` actually becomes** - the Go `cmd/cli` is a colorization-model harness, while
  the architecture describes `cli` as a full terminal application. Deferrable: this slice's `cli`
  claims neither. The change that answers it also picks the argument parser.
