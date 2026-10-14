## Context

See proposal.md - Why. Four facts, checked in the code rather than assumed, shape this design:

- **UMD's spinner turns counter-clockwise.** UMD (`cmd/cli/internal/charm/spinner.go`) uses bubbles' `spinner.Dot`,
  frames `⣾ ⣽ ⣻ ⢿ ⡿ ⣟ ⣯ ⣷`, at 10 frames a second, styled pink. Each frame is a full 2x4 braille cell with one dot
  missing, and the gap goes down the left column and up the right one:

  ```
   frame:  ⣾    ⣽    ⣻    ⢿    ⡿    ⣟    ⣯    ⣷
   gap:    TL   ML   L3   BL   BR   R3   MR   TR      (down the left, up the right = counter-clockwise)
  ```

- **A `Live` session redraws every frame whether or not anything changed.** Its loop in `src/tui/live.rs` calls
  `terminal.draw` at up to 60 fps and only waits on `event::poll` between frames. A widget that reads the clock in
  `render` therefore animates without any producer calling `update`.
- **`tui` widgets share one colour convention.** `ProgressLine` and `AnimatedProgress` take a `color: bool`, and
  `tui::fg` gives a plain style when it is off, so `NO_COLOR` is honoured by the caller passing `false`.
- **`rust-sak` is at `26.10.6`** (`26db855`, the multi-row commit), pinned by `opai`, `cli`, `gui` and `perf`, with a
  clean working tree.

## Slices

The CLI progress redesign is delivered as three changes. **This change is change 1.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-progress-1-spinner` | In `rust-sak`: a `tui::Spinner` (braille dot, 100 ms per frame, clockwise by default with a `Direction` to reverse it), README row, example, the `26.10.6` tag moved to the new commit. Here: `Cargo.lock` follows the moved tag; the pins stay at `26.10.6` | - | **CURRENT** |
| 2 | `cli-progress-2-processor` | In `cli`: `--processor/-p` taking `auto` (default), `cpu`, `coreml`, `cuda`, `tensorrt`, `webgpu`. Values the OS can never offer are left out of the parser and `--help` at compile time (`coreml` only on macOS, `cuda` and `tensorrt` never on macOS). After initialization the choice is checked against the machine's supported providers, the same check the GUI uses, and an unsupported one is refused with a `🧨` line naming those that are available. The choice is handed to face detection and processing instead of the fixed `Auto`. `cli-enhance` spec delta and `CLI_REFERENCE.md` | - | PENDING |
| 3 | `cli-progress-3-output` | In `cli`: `💾 Downloading dependency <name>... <spinner> <n>% Complete` (name and percentage white and bold, spinner magenta `#c792e9`, a green `✓` in its place once done); `🧠 Running inference using the processor <Name>; quality <HD/SD>` (name green, quality yellow) under it, with the blank line always under that; the bar labelled `Enhancing`, `[n/N]` counting finished images (failures included), filling smoothly as `(done + current image's chain fraction) / N`; rows `🖼️ [ 42%] Applying <Family> in the image <path>`, `Downloading the model for <Family>`, held at 99% until written, `[100%] All enhancements applied` (also for an empty chain), `🧨 [ 42%] Failed to enhance` / `Stopped enhancing` in red, with family words and `enhancements` in magenta; the same rows printed one per image when stderr is not a terminal; `cli-enhance` spec delta and `CLI_REFERENCE.md` | 1, 2 | PENDING |

Changes 1 and 2 are independent and may land in either order.

## Goals / Non-Goals

**Goals:**

- A spinner whose speed does not depend on the caller's frame rate, and which can be tested without sleeping.
- Clockwise by default, with UMD's direction one field away.
- Usable both as a `Widget` and as a `Span` inside a caller's `Line`, since the `💾` line is a `Line` of spans.

**Non-Goals:**

- Being a `LiveLine`. A spinner has no text of its own and no settled state. The line that holds it decides both.
- A done state such as `✓`. What replaces the spinner once the work ends is the caller's look (change 3), and a
  caller swaps the `Span` for its own.
- More spinner styles, or a configurable speed. Only one is needed, and the private frame table leaves room for both.

## How the work divides

**`rust-sak` first, then the pins here.** A spinner is not specific to Open Photo AI, so it belongs in `rust-sak`.
Its colour in this app (magenta), the text around it and the `✓` are this CLI's look, so they stay in `crates/cli`
for change 3. Nothing goes into `opai`, and `gui` only moves its pin.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| A session that redraws on its own clock | `tui::Live` | `tui` (exists) |
| Colour switch for `NO_COLOR` | `tui::fg` convention, `color: bool` | `tui` (exists) |
| **A spinner** | **nothing - this change adds it** | `tui` (extended) |

## Decisions

### The frame comes from elapsed time, not from a render counter

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Clockwise,
    CounterClockwise,
}

#[derive(Debug, Clone)]
pub struct Spinner {
    pub direction: Direction,
    /// The glyph's style when `color` is on. Defaults to the gradient's end colour, `#ee6ff8`.
    pub style: Style,
    pub color: bool,
    started: Instant,
}

impl Spinner {
    pub fn new() -> Self;                                      // started now, clockwise, coloured
    pub fn frame_at(&self, elapsed: Duration) -> &'static str; // pure: elapsed / 100 ms, modulo 8, in `direction`
    pub fn glyph(&self) -> &'static str;                       // frame_at(self.started.elapsed())
    pub fn span(&self) -> Span<'static>;                       // glyph() in `style`, or plain without colour
}

impl Widget for &Spinner { /* span() into the area's first cell */ }
```

The frame is `(elapsed / 100 ms) % 8`. `Live` draws at up to 60 fps, and a caller outside a session might draw at 2.
If the frame advanced once per render, the spinner would turn at whatever rate the caller happened to draw. Reading
the clock keeps it at 10 frames a second, which is bubbles' `spinner.Dot` rate, everywhere.

`frame_at` is a pure function of the elapsed time, so the order, the wrap-around and the direction can all be tested
without a clock or a sleep. `glyph` and `span` are the thin, clock-reading layer on top.

**Alternative rejected: a `tick()` the caller calls each frame.** That is how bubbles works, but bubbles has a
message loop that schedules the ticks at a fixed rate. `Live` has no such hook, and calling `tick` from `render` brings
back the speed-depends-on-frame-rate problem.

### One frame table; the direction reads it forwards or backwards

The table is stored once, clockwise: `⣾ ⣷ ⣯ ⣟ ⡿ ⢿ ⣻ ⣽` (the gap goes TL, TR, down the right, BR, BL, up the left).
`Direction::Clockwise` reads frame `i`. `Direction::CounterClockwise` reads frame `(8 - i) % 8`, which starts on the
same `⣾` and gives `⣾ ⣽ ⣻ ⢿ ⡿ ⣟ ⣯ ⣷`, exactly bubbles' `spinner.Dot` order.

With one table, the two directions cannot disagree about which glyphs exist. A test pins both sequences in full, so
the counter-clockwise one is checked against UMD's order rather than just against "the reverse of the other".

**Alternative rejected: a `reverse: bool`.** It says the same thing less clearly at the call site
(`reverse: true` reverses from what?), and an enum leaves room for nothing else being added by mistake.

### Coloured like the rest of `tui`, embeddable as a `Span`

`style` is applied only when `color` is on, the same convention as `fg` (which takes a `Color`, so `span()` applies the
whole `Style` itself), so a caller honouring `NO_COLOR` passes `false` and gets a bare glyph. The default foreground is
the gradient bar's end colour (`#ee6ff8`), so an unconfigured spinner matches the module's look. Change 3 sets the CLI's
magenta.

`span()` is the main entry point: the CLI's `💾` line is a `ratatui::text::Line` of spans, and the spinner is one
span among the name and the percentage. `Widget` is there for callers that lay out a cell themselves.

## Risks / Trade-offs

- **[Braille glyphs render at different widths or in fallback fonts on some terminals, notably the legacy Windows
  console.]** → The glyphs are single-width in Unicode, and UMD has shipped this spinner on Windows Terminal without
  reports. The legacy console host is **untested**. A caller that cares can avoid drawing the spinner where it
  already avoids colour.
- **[A caller that redraws rarely sees the spinner skip frames.]** → This is intended: it keeps its speed and drops
  frames instead of slowing down. Inside `Live` at 60 fps it never skips.
- **[Platforms.]** → The unit tests are pure and run in CI on macOS, Windows and Linux. The example is checked by hand
  on macOS only. Windows Terminal and a Linux terminal stay **untested** until change 3's manual checks. No execution
  provider is involved.

## Migration Plan

Additive. Existing callers compile and behave unchanged. The repository owner chose to move the `26.10.6` tag to the
spinner commit (`9c22b71`) instead of releasing `26.10.7`, so the version stays `26.10.6`. Here, the four manifests keep
`tag = "26.10.6"`, and only the two `rust-sak` source lines in `Cargo.lock` move from `26db855` to `9c22b71`, so the
lock file still holds one `rust-sak` and `gui`'s `image::DynamicImage` stays one type. To roll back, revert those two
lines in `Cargo.lock` to `26db855`; that commit still exists, so a locked build can still fetch it. Nothing here calls
the spinner until change 3.
