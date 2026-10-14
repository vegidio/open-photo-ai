## Why

The CLI progress redesign makes the `💾` dependency line read like UMD's query line:
`💾 Downloading dependency ONNX Runtime... ⣻ 34% Complete`, with a spinner that keeps moving while bytes arrive. This
crate has nothing that animates on its own clock. `rust_sak::tui` has a bar that eases with a spring, but no spinner.

A braille spinner is not specific to Open Photo AI: UMD and MediaSim both draw one, and any first-party CLI with a
wait of unknown length wants it. So by the project's rule it goes into `rust-sak`, not into `crates/cli`. This change
adds it first so change 3 can use it.

## What Changes

Task estimate: **9**. The whole redesign needs more than 15 tasks: the spinner, a `--processor` flag, and the new
header, bar and rows. It is split into three changes (see design.md - Slices). This proposal covers change 1.

**In `../rust-sak` (a separate repository):**

- **A `Spinner` widget in `tui`.** It draws the eight-frame braille "dot" spinner (`⣾ ⣷ ⣯ ⣟ ⡿ ⢿ ⣻ ⣽`) and moves one
  frame every 100 ms. It picks the frame from the time since it was created, not from how often it is drawn, so it
  turns at the same speed inside a 60 fps `Live` session and in any other caller.
- **A direction.** `Direction::Clockwise` is the default. `Direction::CounterClockwise` gives exactly the order of
  bubbles' `spinner.Dot`, the one UMD draws.
- **Styling like the rest of `tui`.** It has a foreground style and the module's `color` switch, and it can be drawn
  as a `Widget` or taken as a `Span` to embed in a caller's own `Line`.
- **Docs and an example:** a new row in the module README's API table and `examples/tui_spinner.rs`.
- **A release:** the version stays `26.10.6`, and the `26.10.6` tag is moved to the spinner commit (the repository
  owner chose this over releasing `26.10.7`).

**In this repository:**

- Every crate that pins `rust-sak` (`opai`, `cli`, `gui`, `perf`) keeps `tag = "26.10.6"`. Only `Cargo.lock` changes:
  it follows the moved tag to the new commit and still holds a single `rust-sak`.
- Nothing here calls the new API yet; change 3 does.

## Capabilities

### New Capabilities

None. This change adds no behaviour you can observe in Open Photo AI: the `💾` line still reads as it does today. It
produces a dependency that can draw a spinner, plus the pin that reaches it. `.openspec.yaml` sets `skip_specs: true`,
as `cli-batch-1-live-rows` did for the same reason.

### Modified Capabilities

None. The `cli-enhance` requirement "Progress is shown while the command works" changes in change 3.

## Impact

**Crates touched:** `opai`, `cli`, `gui` and `perf`, in their manifests and the lock file only, with no source
changes. No logic goes into a front end.

**`rust-sak`:** this change *is* the `rust-sak` work. What `tui` already provides and is **not** rebuilt: `Live`,
`LiveHandle`, `LiveLine` (including multi-row heights), `AnimatedProgress`, `ProgressLine`, `GradientBar`, `Spring`,
`format_eta` and the private `fg` helper. What is missing, and added here: a spinner.

**Third-party crates introduced:** none. The spinner is a frame table and an `Instant`.

**Not in scope:**

- The `💾` line itself: its wording, its colours (white and bold name, magenta spinner), and the `✓` it shows once a
  dependency is done. Those are this CLI's look and arrive in change 3.
- A spinner that is a `LiveLine` on its own. A spinner always sits inside a line of text, so callers embed it.
- Other spinner styles (line, pulse, moon...). Only the braille dot is needed. The frame table is private, so more
  can be added later without breaking anything.
