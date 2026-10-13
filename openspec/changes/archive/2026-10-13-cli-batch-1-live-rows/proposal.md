## Why

The `opai` command takes one photograph and draws one live line: the gradient bar from `rust_sak::tui`. The batch
series lets it take many files, wildcards and directories. In a batch, the terminal shows the current image's chain bar
with a list of up to five images under it. The list scrolls to the most recent image and follows UMD's CLI look:
`🖼️ [2/3] Enhancing image ~/Desktop/b.jpg`.

`rust_sak::tui::Live` cannot draw that. Its session hard-codes `Viewport::Inline(1)`, so it draws exactly one row.
Being able to draw more than one live row is general-purpose: any CLI with a bar and a status list needs it, and UMD
and MediaSim are first-party examples. So by the project's rule this goes into `rust-sak`, not into `crates/cli`.
This change does that first, so later changes can use it.

## What Changes

Task estimate: **10**. The whole feature is more than 15 tasks: the multi-row session, export naming moved into
`opai`, batch inputs and output rules in the CLI, and the list view. It is split into four changes (see design.md -
Slices). This proposal covers change 1.

**In `../rust-sak` (a separate repository):**

- **A live session taller than one row.** `LiveLine` gains `fn height(&self) -> u16`, defaulting to `1`. `Live::start`
  reads it once and reserves that many rows in its inline viewport. Every existing `LiveLine` (`AnimatedProgress`,
  `Line<'static>`) keeps a height of 1 and draws exactly as before. The change is additive, not **BREAKING**.
- **The terminal is restored below the whole viewport, not below its first row.** Today the guard puts the cursor
  under the viewport's top row. With more than one row, the shell prompt would land on top of the drawing.
- **A height clamped to the terminal.** A viewport taller than the terminal is drawn at the terminal's height, and the
  `LiveLine` is given the smaller `Rect`, so a short window clips the drawing instead of failing.
- **Docs and an example:** the module README's API table and a multi-row example (a bar over a scrolling list).
- **A release:** the existing `26.10.6` tag is moved to the new commit and the version stays `26.10.6` (see
  design.md - The release reuses the `26.10.6` tag).

**In this repository:**

- Every crate that pins `rust-sak` (`opai`, `cli`, `gui`, `perf`) keeps `tag = "26.10.6"`, and `Cargo.lock` moves to
  the retagged commit, so the lock file still holds a single `rust-sak`.
- Nothing here calls the new API yet; change 4 does. Building here proves the new API resolves and compiles.

## Capabilities

### New Capabilities

None. This change adds no behaviour you can observe in Open Photo AI: `opai` still draws its one-line bar. It
produces a dependency that can draw more rows, plus the pin that reaches it. `.openspec.yaml` sets `skip_specs: true`,
as `cli-look-1-rust-sak-tui` did for the same reason.

### Modified Capabilities

None. The `cli-enhance` requirement "Progress is shown while the command works" changes in change 4, where the new
layout can be checked against this application.

## Impact

**Crates touched:** `opai`, `cli`, `gui` and `perf`, in their manifests and the lock file only, with no source
changes. No logic goes into a front end.

**`rust-sak`:** this change *is* the `rust-sak` work. What `rust-sak` already provides and is **not** rebuilt: the
whole `tui` module (`Live`, `LiveHandle`, `LiveLine`, `AnimatedProgress`, `ProgressLine`, `GradientBar`, `Spring`,
`format_eta`) and its private `StreamBackend`. What is missing, and added here: a viewport taller than one row.

**Third-party crates introduced:** none. `ratatui`'s `Viewport::Inline(n)` already supports any height.

**Not in scope:**

- Growing or shrinking the viewport while a session is live. The height is fixed for the session's life (see
  design.md).
- A list widget. The scrolling five-row list, its UMD colours and its wording are the CLI's, in change 4, because
  palettes and text helpers stay in each CLI, as `cli-look-1-rust-sak-tui` decided.
- Any `opai`, `gui` or `cli` source change.
