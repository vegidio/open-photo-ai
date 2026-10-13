## Context

See proposal.md - Why. Five facts, checked in the code rather than assumed, shape this design:

- **The one-row limit is one line of code.** `Session::start` in `../rust-sak/src/tui/live.rs` builds its terminal with
  `TerminalOptions { viewport: Viewport::Inline(1) }`. Nothing else in the module assumes one row: the frame loop
  hands `frame.area()` to `LiveLine::render`, and `ProgressLine` and `Line<'static>` draw into whatever `Rect` they
  get.
- **The restore guard assumes one row.** `Session`'s `Drop` reads the viewport's **top** row
  (`get_frame().area().y`) and moves the cursor to `max(cursor.y, top)` before printing a newline. With one row,
  "below the top row" and "below the viewport" are the same place. With several rows they are not.
- **`ratatui` 0.30's `Viewport::Inline(n)` takes any height.** It scrolls the terminal up when there are fewer than
  `n` rows below the cursor, and clamps `n` to the terminal's height. No new crate or feature is needed.
- **The rows the CLI needs are known before the session starts.** Change 4 draws the bar, a blank row, and
  `min(images, 5)` list rows. The number of images is fixed once the inputs are expanded, which happens before
  anything is drawn.
- **`rust-sak` is at `26.10.6`**, pinned by `opai`, `cli`, `gui` and `perf`, with a clean working tree.

## Slices

CLI batch input is delivered as four changes. **This change is change 1.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-batch-1-live-rows` | In `rust-sak`: a `LiveLine` height, so a `Live` session draws more than one row; restoring below the whole viewport; a release. Here: every `rust-sak` pin moved to the new tag | - | **CURRENT** |
| 2 | `cli-batch-2-export-names` | In `opai`: export naming shared by both front ends. That covers the output format (named, or Preserve with camera RAW falling back to TIFF), prefix and suffix, claiming a free name (`_1` to `_999`) unless overwriting, and writing through a temporary file that is renamed over the target. Moved from `gui`'s `export/format.rs`, `export/destination.rs` and the frontend's `destinationFor`, and `gui` adopts it with no change in behaviour | - | PENDING |
| 3 | `cli-batch-3-inputs` | In `cli`: inputs that are any mix of files and directories, with `--recursive`; wildcards expanded on Windows (`wild`); duplicate inputs removed; the output directory skipped by the walk; `-o` read as a file (writable image extension, one input only) or a directory (created); the output tree mirroring each directory input, with named files flat; `--format`, `--prefix` and `--suffix` (only when `-o` is a directory); numbering by default and `--overwrite`, including in place; the pre-flight check (a named file that cannot be read refuses the run, a discovered one is skipped); a batch continuing past a failure with a non-zero exit; one `Opai` for the whole batch; the `cli-enhance` spec delta for inputs and outputs | 2 | PENDING |
| 4 | `cli-batch-4-list-view` | In `cli`: the chain bar with a five-row scrolling list under it, in UMD's colours (`🖼️ [n/N] Enhancing / Enhanced image <path>`, `🧨 … Failed image` in red), used for every run including `[1/1]`; the end summary; one plain line per image when stderr is not a terminal; the `cli-enhance` spec delta for progress | 1, 3 | PENDING |

Changes 1 and 2 are independent and may land in either order.

## Goals / Non-Goals

**Goals:**

- A `Live` session of any fixed height, with no change for existing one-row callers (MediaSim and `opai` today).
- The shell's prompt always lands below the whole drawing, on every exit path the one-row session already handles:
  finish, drop, error and panic.
- A short terminal clips the drawing instead of failing or corrupting the scrollback.

**Non-Goals:**

- Changing the height while a session is live. See Decisions.
- A list, stack or layout widget. Splitting the `Rect` and choosing colours is the caller's job, and ratatui's
  `Layout` already does it.
- Writing above the viewport (`insert_before`). `perf` has its own viewport for that.

## How the work divides

**`rust-sak` first, then the pins here.** Drawing more than one live row isn't specific to Open Photo AI, so it
belongs in `rust-sak`. The list it will carry in change 4 is: its wording, colours and the "last five" rule are this
CLI's look, so they stay in `crates/cli`. Nothing goes into `opai`, and `gui` only moves its pin.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| Live session, handle, Ctrl-C as a key, restore guard | `tui::Live`, `LiveHandle` | `tui` (exists) |
| Bar, spring, progress line | `tui::AnimatedProgress`, `ProgressLine` | `tui` (exists) |
| **A viewport taller than one row** | **nothing - this change adds it** | `tui` (extended) |

## Decisions

### The height is a `LiveLine` method, read once at start

```rust
pub trait LiveLine: Send + 'static {
    fn render(&mut self, area: Rect, buf: &mut Buffer);
    fn settled(&self) -> bool { true }
    fn snap(&mut self) {}
    /// How many rows the session reserves for this line. Read once, when the session starts.
    fn height(&self) -> u16 { 1 }
}
```

`Live::start` calls `line.height()` before the line moves into the shared mutex, and passes `max(height, 1)` to
`Viewport::Inline`. A height of 0 is treated as 1, because ratatui's inline viewport needs at least one row to anchor
the cursor.

The height goes on the line because the thing being drawn knows how many rows it needs. With the default, every
current implementation compiles unchanged and keeps one row.

**Alternative rejected: a `Live::start_with_height(target, line, height, on_interrupt)` constructor.** It works, but
it means the caller states the height in one place and draws to it in another, and the two can disagree. On the trait,
the composite that renders the rows also declares them.

**Alternative rejected: a `height` option on a builder, like `settle_cap`.** `settle_cap` changes `finish`, which
runs after start. The height is needed *by* `start`, before any builder method can run on the returned `Live`.

### The height is fixed for the session's life

`Viewport::Inline(n)` is fixed when the `Terminal` is built. Changing it while live would mean either rebuilding the
`Terminal` on the same rows (another cursor query mid-session, and rows the old viewport drew left in the scrollback),
or switching to `Viewport::Fixed` and managing the scroll by hand.

The CLI never needs to change it: the batch size is known before drawing starts, so change 4 asks for
`2 + min(images, 5)` rows. Until the list fills, rows under the last entry are drawn blank, which shows as ordinary
empty lines under the list.

**Alternative rejected: grow on demand.** It is the only way to avoid the blank rows, and it costs the cursor query
and re-anchoring above. Revisit if a caller cannot know its height in advance.

### The area handed to `render` is the clamped viewport

ratatui clamps an inline viewport to the terminal's height and recomputes it on resize. The frame loop already
passes `frame.area()`, not a size it computed itself, so a `LiveLine` asked for 7 rows in a 4-row window gets a 4-row
`Rect` and draws into that. `ProgressLine` already handles a narrow `Rect` by clipping. Change 4's composite must do
the same with a short one, which ratatui's `Layout` does.

`Session::start` also clamps the height to the terminal's rows itself, before building the viewport
(`viewport(height, rows)` in `live.rs`). ratatui clamps the drawn area, but it scrolls the terminal by the height it
was *asked* for, so a 7-row request in a 4-row window would push three blank lines into the scrollback. Clamping first
scrolls only by the rows there are.

### The guard restores below the viewport's bottom row

`Session`'s `Drop` changes from the viewport's top (`area().y`) to its bottom (`area().bottom() - 1`, as the row to be
below), keeping the existing `max` with the reported cursor row:

```
  before (one row)               after (n rows)
  row y   [ bar ]                row y     [ bar        ]
  ^ cursor -> y, then "\n"       row y+1   [            ]
                                 row y+2   [ list row 1 ]
                                 ...
                                 row y+n-1 [ list row k ]
                                 ^ cursor -> y+n-1, then "\n"
```

With `n = 1`, `bottom() - 1 == y`, so a one-row session restores exactly where it does today. The `max` with the
cursor row is kept, so a panic message printed past the viewport still pushes the prompt below it.

### The release reuses the `26.10.6` tag

The plan was a new `26.10.7`. Instead, the `26.10.6` tag was moved from `a533247` to the commit that adds multi-row
sessions (`26db855`), and the crate's version stays `26.10.6`. The change is additive, so the commit the tag used to
name is a strict subset of what it names now.

The cost is that the tag is no longer immutable: two lock files that both say `tag=26.10.6` can hold different
commits. A consumer with `a533247` locked keeps building, because that commit is still reachable as the new one's
parent, and it picks up the multi-row code the next time it runs `cargo update -p rust-sak`. Here, the manifests keep
`tag = "26.10.6"` and only the commit hash in `Cargo.lock` moves.

All four crates still pin the same tag, for the reason `cli-look-1-rust-sak-tui` recorded: two tags compile the crate
twice, and `gui`'s `image::DynamicImage` becomes two unrelated types. Later `rust-sak` releases go back to a new CalVer
tag each.

## Risks / Trade-offs

- **[An inline viewport near the bottom of the window scrolls the terminal up by its height when it starts.]** → This
  is ratatui's documented inline behaviour, and the reason the height is capped at `2 + 5` in change 4. Any
  scrollback that moves is above the cursor and is not overwritten.
- **[A resize that shrinks the window below the height while live.]** → ratatui recomputes the inline area on the
  next `draw`. The drawing is clipped, and the scrollback above may hold a partly drawn frame. This is the same class
  of artefact the one-row session already has on a resize. It is accepted and not engineered around.
- **[Emoji width (`🖼️` with its variation selector, `🧨`) differs between terminals.]** → This is not this change's
  concern, since this change draws no emoji. Change 4 decides how its rows measure them.
- **[Windows and Linux terminals are untested for multi-row inline viewports.]** → CI builds `tui` alone and runs the
  `TestBackend` tests on macOS, Windows and Linux. The live multi-row example is checked by hand on macOS in this
  change. Windows Terminal and a Linux terminal remain **untested** until change 4's manual checks.

## Migration Plan

Additive. Existing callers compile and behave unchanged, so moving the pins needs no source edit. Because the tag was
reused, there is no older tag to roll back to: to roll back, put `Cargo.lock`'s `rust-sak` and `o11y-macros` entries
back on `#a53324709f21516dba823590c9851d9bcdec2618` and build with `--locked`. Nothing here calls `height()` until
change 4.
