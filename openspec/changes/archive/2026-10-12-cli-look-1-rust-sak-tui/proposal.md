## Why

The `opai` terminal application draws its progress with `indicatif`: one plain `=>` bar per dependency, then one per
library call. The goal for the series is the look MediaSim's CLI already has — a single gradient bar that animates
smoothly, labelled with the current step, a `[n/N]` counter and a percentage, with coloured header and result lines.
`indicatif` cannot colour a bar cell by cell, so the gradient is out of reach without changing the drawing library.

MediaSim draws that bar with `ratatui` in a one-line inline viewport. That code is general-purpose: a second
first-party CLI now wants it, and copying it into `crates/cli` would leave two implementations to drift apart. By the
project's rule that a close-but-missing piece of infrastructure goes into `rust-sak` when another project could use
it, the bar moves into `rust-sak` first. This slice does that. The slices after it use it here.

## What Changes

Task estimate: **14**. The whole feature (the `rust-sak` widgets and live session, the library progress changes, and
the new terminal layout) is more than 15 tasks, so it is split into three slices (see design.md - Slices). This
proposal covers slice 1.

**In `../rust-sak` (a separate repository):**

- **A new `tui` Cargo feature**, off by default like every other feature. It adds `ratatui` 0.30 on its `crossterm`
  backend with default features off. That is the version MediaSim and this repository's `perf` crate already use.
- **The widgets, lifted from MediaSim's `crates/cli/src/progress/`:**
  - `GradientBar`: `█`/`░` cells filled through a `#5a56e0` to `#ee6ff8` gradient.
  - `Spring`: the critically damped animation that moves the bar towards its target.
  - `ProgressLine`: `<label>  [n/N]  <bar>  xx.x%  [ETA …]`. Unlike MediaSim's version it adds three things this
    application needs: a minimum label width, so the bar does not shift when the label changes; a percentage given
    directly, not derived from the counter; and an ETA column that is left out entirely when the caller does not want
    one.
  - The ETA text formatter, so MediaSim can move over without losing anything.
- **A live one-line session**, generalised from MediaSim's `show`/`Session`:
  - It draws a caller-supplied line in an inline viewport, in raw mode, at up to 60 frames per second, on a thread of
    its own.
  - It writes to **stdout or stderr**, as the caller chooses. MediaSim uses stdout; `opai` keeps stdout empty.
  - Producers update it through a cheap, `Send + Sync` handle, so updates can come from async code as well as from
    threads.
  - Ctrl-C reaches it as a key press while raw mode is on, so it is reported to the caller through a callback.
  - Finishing it lets an animation settle (up to a cap) and leaves the final frame in the scrollback, with the cursor
    on the line below.
  - The terminal is restored however the session ends: finish, drop, error or panic.
- **Docs and CI:** a module README in the style of the others, and the `tui` feature added to CI's per-feature
  isolation matrix.
- **A release:** a version bump and a new tag, done the way `add-image-raw-1-rust-sak` did it.

**In this repository:**

- `crates/opai/Cargo.toml`, `crates/gui/Cargo.toml` and `crates/perf/Cargo.toml` move their `rust-sak` pins to the
  new tag (`26.10.6`; see design.md - Version and pins).
- `crates/cli/Cargo.toml` gains a `rust-sak` dependency with only the `tui` feature, at the same tag, so the lock
  file holds a single `rust-sak`.
- Nothing calls the new API yet; slice 3 does. As in `add-init-0-rust-sak`, building here proves that the new API
  resolves, compiles and is public on all three platforms.

## Capabilities

### New Capabilities

None. This slice adds no behaviour observable from Open Photo AI: `opai` still draws its `indicatif` bars. What it
produces is a dependency that can draw the new bar, plus the pin that reaches it. `.openspec.yaml` sets
`skip_specs: true`, as `add-init-0-rust-sak` and `add-image-raw-1-rust-sak` did for the same reason.

### Modified Capabilities

None. The `cli-enhance` requirement "Progress is shown while the command works" changes in slice 3, where the change
can be checked against this application.

## Impact

**Crates touched:** `opai`, `cli`, `gui` and `perf`, in their manifests and the lock file only, with no source
changes. `gui` and `perf` only move their pin. No logic goes into a front end: the drawing code goes into `rust-sak`, and the front-end wiring is slice 3.

**`rust-sak`:** this slice *is* the `rust-sak` work. What it already provides and is **not** rebuilt: nothing for
terminals. Its modules cover hashing, HTTP, filesystem, image codecs, caching, telemetry, GitHub releases and
hardware probes, and none of them draws to a terminal. What this slice adds: the `tui` module.

**Third-party crates introduced (into `rust-sak`):** `ratatui` 0.30, with `crossterm` brought in through its backend
feature. Both are MIT. They are already in this repository's lock file through `perf`, so the workspace gains no new
crate.

**Later, outside this series:** MediaSim moving from its own `progress/` module to `rust_sak::tui`, in its own
repository. `perf`'s viewport is a different design (spinner, per-tile bar, `insert_before`) and is not changed.

**Not in scope:** colour palettes, `paint`/`bold` text helpers and `NO_COLOR` detection. They stay in each CLI, at the
repository owner's direction. The widgets take a `color: bool` and decide nothing about the environment. Also out of
scope: multi-line viewports, and any `opai` or `gui` source change.
