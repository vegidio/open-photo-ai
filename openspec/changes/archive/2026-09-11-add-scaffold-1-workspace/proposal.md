## Why

The repository is empty apart from its OpenSpec configuration. Nothing can be proposed, implemented
or verified against a project that has no crates, no formatter, no linter and no CI - every later
change would otherwise carry its own share of setup, and the three-crate layering the architecture
depends on would be established incidentally rather than deliberately.

This is the first of two scaffolding slices. It stands up the Rust half: a workspace where `cli`
consumes `opai` and the whole toolchain passes. The GUI half follows in `add-scaffold-2-gui`.

## What Changes

Task estimate: **7**. The full scaffold (Rust workspace + Tauri crate + React toolchain + both CI
jobs) estimates at 15 - exactly the ceiling, with no slack - so it is split into two slices. This
proposal covers slice 1 only.

- **Cargo workspace** at the repository root, a virtual manifest with `resolver = "3"` and members
  `opai` and `cli`. `gui` joins the members list in slice 2.
- **`opai`** - the core library crate, created with a single public function returning the
  application version and a colocated test. It has **no dependencies**: nothing in this slice is
  fallible, so even `thiserror` waits for the first operation that can fail.
- **`cli`** - a binary crate that calls into `opai` and prints what it gets back. This is a linkage
  proof, not a feature; see the parity note below.
- **`rustfmt.toml`** - taken verbatim from https://github.com/vegidio/intercue: edition 2024,
  `max_width = 120`, and eight per-construct sub-widths set to 96 so chains and calls are not broken
  at rustfmt's default ~72.
- **`[workspace.lints]`** - `unsafe_op_in_unsafe_fn = "deny"` plus a small clippy set
  (`undocumented_unsafe_blocks`, `todo`, `unimplemented`, `dbg_macro`). No `rust-version` pin: an
  application makes no MSRV commitment.
- **`LICENSE`** (AGPL-3.0) and a **`README.md`** stub naming the three crates and the dev commands.
- **`.gitignore`** and **`.editorconfig`**, adapted from the Go project's (4-space, 120 columns,
  2-space YAML, tabs dropped with Go).
- **CI** (`.github/workflows/ci.yml`) - `cargo fmt --all --check`, `cargo clippy --all-targets
  --locked -- -D warnings`, and `cargo test`. The frontend job is added in slice 2.

## Capabilities

### New Capabilities

None. This slice creates no user-observable behaviour: it is project setup - build configuration,
licensing, formatting, linting and CI.

`.openspec.yaml` therefore sets `skip_specs: true`.

The one thing that might look specifiable - `cli` printing a version - is deliberately not written
as a requirement. The Go `cmd/cli` is, by its own opening comment, "a small end-to-end harness for
the colorization models... it goes through the public API only"; its flags are `-in`, `-out`, `-ep`,
`-precision` and `-quality`, and it has no version output at all. There is no inherited behaviour
here, and inventing a requirement to satisfy validation is exactly what this project's spec rule
forbids.

### Modified Capabilities

None. `openspec/specs/` is empty.

## Impact

**Crates touched:** `opai` (created), `cli` (created). `gui` is untouched and is not yet a workspace
member. No logic is placed in a front end: `cli` holds only its `main`, and the function it calls
lives in `opai` - which is the layering this slice exists to establish.

**`rust-sak`:** nothing is needed from it. This slice writes no infrastructure code - no hashing,
HTTP, downloads, extraction, temp files, config directories, image codecs, caching, telemetry or
hardware probes - so no `rust-sak` feature applies and none is enabled.

One finding to record now rather than discover later: **`rust-sak` is not published on crates.io.**
The registry returns `crate 'rust-sak' does not exist` for both `rust-sak` and `rust_sak`, the
GitHub repository has no releases, and the local checkout reads `version = "2.0.0"` while its README
instructs `cargo add rust-sak`. When `opai` first needs a module, the dependency will have to be a
git or path dependency, or `rust-sak` will have to be published first. A path dependency would break
CI and anyone cloning this repository alone, so this is worth settling before that change - not
during it.

**Dependencies added:** none, in either crate.

**Parity divergence:** `cli` does not reproduce the Go harness's behaviour in this slice, and its
placeholder output is not a claim about what the Rust `cli` will eventually do. The architecture
describes `cli` as "the terminal application - argument parsing, terminal output and progress
rendering", which is a broader thing than the Go model harness; that gap is real, intentional, and
belongs to a later change that decides the CLI's actual shape (including its argument parser, which
is not chosen here). No behavioural parity is claimed or lost by this slice.

**Not in scope:** `deny.toml` and dependency auditing; the `gui` crate and everything Tauri; the
entire frontend toolchain; release workflows and code signing; any choice of CLI argument parser.
