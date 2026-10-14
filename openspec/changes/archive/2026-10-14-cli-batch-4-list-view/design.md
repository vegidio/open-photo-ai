## Context

See proposal.md - Why. These facts from the code shape the design:

- **Every image draws its own bar session.** `progress::View` holds a `Slot<Live<AnimatedProgress>>`. `run::enhance_one`
  prints the header with `eprintln!`, calls `start_bar`, then `finish_bar`, and `run::run` prints each result line.
  Slice 3's D8 re-arms the slot after each image. Between images there is no live session, so a Ctrl-C there is a
  SIGINT, caught by `stop_on_ctrl_c`.
- **A `Live` session owns stderr.** Nothing else may write to it while the session is up. One session spanning the
  batch therefore means every per-image line has to be drawn inside it or printed after it.
- **`rust_sak::tui` at `26.10.6` already draws the layout.** `LiveLine::height` is read once at `Live::start`, and the
  `LiveLine` is given a smaller `Rect` when the terminal is shorter. `examples/tui_rows.rs` is a bar over a scrolling
  five-row list: a `Layout::vertical([Length(1), Length(1), Fill(1)])`, delegating `settled`/`snap` to the bar.
- **`AnimatedProgress`'s spring lives inside the value.** Replacing the value resets the spring to `0` at rest, so a
  new image's bar starts empty instead of easing back down from `100%`.
- **Failure text.** `run::failure_reason` prefixes `could not enhance <path>: ` unless the error already names the
  image (a decode error does). Slice 3 returns `CliError::Batch` whenever an image failed, so one failed image prints
  its own line and then `🧨 1 of 1 images failed`.
- **`unicode-width` is 0.2.2** in the lock file, via `ratatui-core`. `🖼️` (U+1F5BC + U+FE0F) is a presentation
  sequence that it counts as two columns. Until now `🖼️` was only ever printed with `eprintln!`. This is the first
  time it is drawn into a ratatui buffer.

## Slices

CLI batch input is delivered as four changes. **This change is change 4, the last.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-batch-1-live-rows` | In `rust-sak`: a `LiveLine` height, so a `Live` session draws more than one row; restoring below the whole viewport; a release (the `26.10.6` tag moved). Here: every `rust-sak` pin moved to it | - | **DONE** (archived 2026-10-13) |
| 2 | `cli-batch-2-export-names` | In `opai`: a public `export` module with the format choice (named, or Preserve with camera RAW falling back to TIFF), each format's extension and encoder settings, composing the name and the destination, and claiming a free name (`_1` to `_999`) or overwriting through a hidden file renamed over the target. `gui` adopts it with no change in behaviour | - | **DONE** (archived 2026-10-13) |
| 3 | `cli-batch-3-inputs` | In `cli`: inputs that are any mix of files and directories, with `--recursive`; wildcards on Windows (`wild`); deduplication; the output directory skipped by the walk; `-o` as a file or a directory; mirroring; `--format`, `--prefix`, `--suffix`; numbering by default and `--overwrite`; the pre-flight check; a batch continuing past a failure; one `Opai` for the batch; the `cli-enhance` delta for inputs and outputs | 2 | **DONE** (archived 2026-10-14) |
| 4 | `cli-batch-4-list-view` | In `cli`: one live session for the batch, with the chain bar over a five-row scrolling list in UMD's colours (`🖼️ [n/N] Enhancing / Enhanced image <path>`, `🧨 … Failed / Stopped image` in red), used for every run including `[1/1]`; the end summary, with the written file for one image and each failure's reason; one plain line per image when stderr is not a terminal; the `cli-enhance` delta for progress and exit lines | 1, 3 | **CURRENT** |

## Goals / Non-Goals

**Goals:**

- One live session from the first image to the last, so the list stays in place and a Ctrl-C reaches the run the same
  way wherever the batch is.
- All of the view's text, colours and padding as pure functions in `look.rs`, and the layout as a `LiveLine` that
  renders into a `Buffer`, so both are tested without a terminal.
- Memory that does not grow with the batch: the view keeps only the rows it can show.

**Non-Goals:**

- Changing what a single image's chain does, how it is planned, or how its bar is labelled and counted.
- A `rust-sak` list widget (see "How the work divides").
- Resizing the view while it is live. Its height is fixed at `2 + min(images, 5)` for the session.

## How the work divides

| Crate | Owns after this change |
|---|---|
| `opai` | Unchanged |
| `cli` | The view's layout, the rows' wording and colours, the summary, the plain lines, and which closing error the run returns |
| `gui` | Unchanged |
| `rust-sak` | Unchanged. The `tui` feature provides `Live`, `LiveLine` (with `height`), `LiveHandle` and `AnimatedProgress`, used as they are |

Everything here is presentation. The GUI shows its queue in React, and nothing in the list, the summary or the plain
lines is something it would call. The `rust-sak` gap, a list widget, is left open on purpose. Slice 1 and
`cli-look-1-rust-sak-tui` decided that wording and palettes belong to each CLI, and the list is ten lines of
`Layout` plus `Line::render` once its rows are built. If MediaSim or UMD grow the same view in Rust, a generic
"bar over the last *n* lines" `LiveLine` can move to `rust-sak` then.

No new third-party crate. `ratatui` (already a direct dependency) provides `Layout`, `Line`, `Span` and `Buffer`.

## Decisions

### D1. One session for the whole batch

`View::start_batch(total)` starts one `Live<BatchLine>`. The blank line under the dependency line is the session's own
top row, drawn only when `finish_install` froze a drawn dependency line, so neither a run with nothing to download nor
a terminal that refuses the session gets a stray blank line.
`View::finish_batch()` finishes it, leaving its last frame in the scrollback, and prints the blank line above the
summary. `run` calls them around the loop, so the session covers loading, enhancing and writing every image.

- **Ctrl-C** during the session calls `on_interrupt`, which cancels the token, as the bar's does today. A press while
  an image is loading or being written is noticed at the existing checks: after `enhance`, and at the top of the
  loop. `stop_on_ctrl_c` stays for plain mode.
- **The per-image slot re-arm from slice 3 (D8) goes.** `Slot` keeps `start`, `finish` and `close`. The batch slot is
  started once and closed by `finish_batch`.

**Alternative rejected: a session per image, each drawing the bar and the list.** Every finished session leaves its
frame in the scrollback, so the list would be printed again under itself for every image. That is the clutter this
slice removes.

### D2. `BatchLine`: the `LiveLine` the session draws

```rust
struct BatchLine {
    bar: Option<AnimatedProgress>,   // None: the bar's row is blank (no image yet, or nothing left to run)
    rows: VecDeque<Row>,             // at most LIST_ROWS, the most recent last
    total: usize,                    // N in [n/N]
    color: bool,
}
struct Row { index: usize, path: String, state: RowState }   // path: source.path.display(), as given or found
```

- `height()` is `2 + min(total, LIST_ROWS)`, with `LIST_ROWS = 5`, so a single image takes three rows, not seven, plus
  one blank row on top when a dependency line was drawn (D1).
- `render` splits the area as `tui_rows.rs` does: bar, blank, list. In a terminal shorter than the view, the list
  shows the most recent rows that fit. Each row is `look::row(...)`, a `Line<'static>`, rendered and clipped by
  ratatui at the terminal's width.
- `settled`/`snap` delegate to the bar. With no bar, the line is settled.
- Pushing a sixth row pops the oldest. The rows' history is not needed: failures are kept by `run` (D5).

### D3. The view's API

```rust
View::start_batch(&self, total: usize)
View::start_image(&self, index: usize, path: &Path, planned: usize) // push an Enhancing row; fresh bar, or None if planned == 0
View::on_bar(&self) -> OnInference                                  // unchanged signature; updates `bar` inside BatchLine
View::end_image(&self, index: usize, path: &Path, end: &ImageEnd)   // set the row's state; print the plain line if not live
View::finish_batch(&self)
```

`ImageEnd` is `Enhanced`, `Failed(reason)` or `Stopped`. `look::RowState` is the same set plus `Enhancing`. A fresh
`AnimatedProgress` per image (label `Enhancing`, counter `[0/planned]`, `LABEL_WIDTH`, the look's colour) is what
resets the bar without it moving backwards (see Context).

**Whether the view is live** is decided once, by `start_batch`: interactive and the session started. Otherwise
`end_image` prints the plain line with `eprintln!`. Off a terminal it has no colour. On a terminal that refused the
session it is coloured as `look.color` says, so a refusal still shows every image's result.

### D4. The rows' text: `look.rs`

```rust
pub enum RowState { Enhancing, Enhanced, Failed, Stopped }
pub fn row(index: usize, total: usize, path: &str, state: RowState, color: bool) -> Line<'static>;
pub fn plain_row(index: usize, total: usize, path: &str, end: &ImageEnd, color: bool) -> String;
pub fn summary(written: &[PathBuf], output: &Path) -> Option<String>;  // painted by the caller as `success` is
```

- **Wording.** `🖼️ [n/N] Enhancing image <path>`, `Enhanced`, and `🧨 [n/N] Failed image <path>` or `Stopped`. `n`
  is right-aligned to `N`'s width with spaces (`[ 2/15]`), so the words do not shift when the count gains a digit.
  `Enhancing` and `Enhanced` differ in length, as they do in UMD and in `tui_rows.rs`. Padding them would put a gap
  in the sentence, so they are not padded.
- **Colours**, UMD's hex values from `cmd/cli/internal/charm/styles.go`: `BLUE #82aaff` on `n/N`, `MAGENTA #c792e9`
  on `image`, `CYAN #88ddff` with underline on the path. `🧨` rows are entirely `RED #f44336`, not bold, as UMD's
  `PrintError` is. The bold red stays for the summary's failure lines, as today. The existing `GREEN` goes with the
  header.
- **`plain_row`** has the same words as the final `row`. `Failed` adds `: <reason>`. A test asserts that the text of
  `row(...)` equals the uncoloured `plain_row(...)` without the reason, so the two cannot drift.
- **`summary`**: `None` when nothing was written. Exactly one written: the existing
  `✨ Image enhanced successfully; file saved at <file>` with the committed path. More:
  `✨ <k> images enhanced successfully; files saved under <output>`, with `-o` as given. Under a file `-o`, at most
  one image runs, so the plural form only arises for a directory.
- `header` is removed with its test.

### D5. The loop, the summary and the closing error: `run.rs` and `error.rs`

```
start_batch(sources.len())
for (i, source) in sources:
    if cancelled: stopped = true; break
    start_image(i+1, path, plan.operations.len())
    match enhance_one(...):                // no eprintln! inside it any more
        Ok(written)        → end_image(Enhanced); written.push(path)
        Err(Cancelled)     → end_image(Stopped); stopped = true; break
        Err(e)             → end_image(Failed(e.to_string())); failures.push(CliError::Image { path, source: e })
finish_batch()
print summary(written), if any
print each failure's line, if the view was live and sources.len() > 1
drop(opai)
return: stopped → Cancelled; failures.len() == 1 && sources.len() == 1 → that CliError::Image;
        failures non-empty → Batch { failed, total }; else Ok
```

- **`CliError::Image { path, source: Box<CliError> }`** replaces `failure_reason`. Its `Display` is the same rule:
  the source's message alone when it already names the image, otherwise `could not enhance <path>: <message>`. Its
  exit code is the source's, which is `1` for every per-image error. It is what the listed lines and a single
  image's closing line print, so the two read the same. The plain row's reason is the source's message alone, since
  the row already names the path.
- **The plan is computed before `start_image`**, because the bar's `[0/N]` needs `operations.len()`. `chain::plan` is
  pure and cheap, so this only reorders it.
- **The listed failure lines are printed only by a live view with more than one image.** In plain mode each image's
  line already carries its reason. With one image, the closing line is that failure.
- **Order on a stop:** the summary, then the listed failures before the stop, then `main` prints the stop line.
  `Cancelled` keeps precedence over failures for the exit status, as today.
- `tracing` records each failure in full (already) and each written path (already, in `enhance_one`'s `wrote …`).

**Alternative rejected: printing the failure lines as they happen, above the view.** `Live` has no "insert a line
above the viewport" operation (ratatui's `insert_before` would need `rust-sak` to expose its `Terminal`). That would
be a `rust-sak` change for a cosmetic gain. Collecting the failures and printing them at the end meets the spec.

### D6. Plain mode and its order

Off a terminal, `start_batch` and `finish_batch` print nothing, and each `end_image` prints one line. Then come the
summary's success line and `main`'s closing line. The blank lines that frame the view are printed only when the view
is live, so a log file holds exactly the lines the spec lists.

## Risks / Trade-offs

- **[`🖼️`'s width differs between `unicode-width` and the terminal.]** Ratatui counts it as two columns. A terminal
  that draws it as one would leave the rest of the row shifted by a column, and the diff-based redraw could leave a
  stray character at the row's end. → Checked by hand in Terminal.app, iTerm2 and VS Code's terminal during the manual
  pass. If one of them disagrees, the fix is local to `look::row` (for example, a single-codepoint emoji such as `📸`,
  as UMD uses), and the spec wording would change with it. Windows Terminal and conhost are **untested**.
- **[Early rows scroll away.]** Only the last five rows are in the final frame, so a long batch's earlier successes
  are not on screen afterwards. → By design. Their paths are in the log, failures are listed under the view, and the
  summary says how many were written.
- **[BREAKING plain output.]** Scripts that parsed `file saved at` per image lose it. → Marked in the proposal and in
  `CLI_REFERENCE.md`'s scripting section. A single image still ends with that line. The log records every written path.
- **[Load and write are not interruptible.]** A Ctrl-C during a large RAW decode is noticed only once the decode
  ends. → Same as slice 3. The session keeps drawing meanwhile, so the terminal does not look hung.
- **[A very narrow terminal clips the path.]** → Accepted. Ratatui clips at the right edge, and the plain lines and
  the log hold the full path.

## Validation

To be validated on **macOS** (Apple Silicon): `cargo test -p cli`, the workspace's `fmt`/`clippy`/`test`, and a manual
pass with real models on the CoreML provider: a single image (three-row view, numbered `out_1.png` named in the
summary), a batch of eight (scrolling, bar restarting), a failure that scrolls out, Ctrl-C mid-batch, `NO_COLOR=1`,
stderr piped to a file, and a terminal shorter than the view. **Linux** is covered by CI's Rust test job, with no
terminal, so only the pure text, `BatchLine::render` into a `Buffer`, and plain mode. **Windows** is built by CI, and
its terminal rendering (emoji width, raw mode under conhost) is **untested** unless checked by hand. Execution
providers do not matter here: each image runs exactly the chain it ran in slice 3.

## Migration Plan

There is no data migration. Users notice only the new terminal layout and the plain output (see Risks). To roll back,
revert the commit. `opai`, `gui` and `rust-sak` are untouched.
