## Context

See proposal.md - Why. These facts were checked in the code and shape the design:

- **The dependency line is a static `Line<'static>`** (`look::dependency_line`), drawn by a `Live<Line<'static>>` in
  `progress::Install`. `Live` redraws at up to 60 fps, but a `Line` has no clock, so a spinner inside one would freeze
  on the frame it was built with.
- **The batch view is `BatchLine`** (`progress.rs`): an optional `gap` row (a blank row under a drawn dependency line),
  the current image's `AnimatedProgress`, a blank row, and up to five `Row { index, path, state }`. `start_image`
  replaces the bar with a fresh one for each image. `bar_report` moves it from `InferenceProgress::chain_fraction`,
  and sets its label and counter from `look::label` and `look::counter`.
- **Plain mode keeps no progress.** `bar_report` returns at once when no session is live, and `end_line` builds the
  plain line from `index`, `total`, `path` and `ImageEnd` only.
- **`InferenceProgress` has everything a row needs.** `chain_fraction` never decreases over one run and covers the
  detection too (via `Handover`). `stage` tells `Installing` apart from `Running`/`Cached`, and `subject.family()`
  names the family. A detection reports as `Family::Detection`, which `look::family_label` already reads as
  `Face Recovery`.
- **`rust_sak::tui::Spinner`** (change 1, tag `26.10.6`) has public `direction`, `style` and `color` fields, and a
  `span()` that reads its own clock. `AnimatedProgress` zero-pads `n` in its counter, as UMD does.
- **`run` knows the processor and quality** (`args.processor`, `args.quality`) and checks the processor right after
  `finish_install`, before `start_batch`.

## Slices

The CLI progress redesign is delivered as three changes. **This change is change 3.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-progress-1-spinner` | In `rust-sak`: a `tui::Spinner` (braille dot, 100 ms per frame, clockwise by default with a `Direction` to reverse it), README row, example, the `26.10.6` tag moved to the new commit. Here: `Cargo.lock` follows the moved tag; the pins stay at `26.10.6` | - | **DONE** (archived 2026-10-14) |
| 2 | `cli-progress-2-processor` | In `cli`: `--processor/-p` taking `auto` (default), `cpu`, `coreml`, `cuda`, `tensorrt`, `webgpu`, with the OS's impossible values left out of the parser, the choice checked against the machine after initialization (exit 2 when refused), and handed to detection and processing. `cli-enhance` spec delta and `CLI_REFERENCE.md` | - | **DONE** (archived 2026-10-14) |
| 3 | `cli-progress-3-output` | In `cli`: the `💾 Downloading dependency <name>... <spinner> <n>% Complete` line (single line, rewritten per dependency; `✓` at 100%); `🧠 Running inference using the processor <Name>; quality <HD/SD>` in every mode, `Auto` shown literally, a blank line under it; a batch-wide bar labelled `Enhancing` with `[ended/N]`, filling as `(ended + current row's fraction) / N`; per-image percentage rows (`Preparing to enhance`, `Downloading <family> for`, `Applying <family> to`, `Enhancements applied to`, `Failed to enhance`, `Stopped enhancing`, padded so the paths line up), held at 99% until written; the same final rows in plain mode; `cli-enhance` spec delta and `CLI_REFERENCE.md` | 1, 2 | **CURRENT** |

This is the last slice. Nothing in the series remains after it.

## Goals / Non-Goals

**Goals:**

- One record per image, `ImageProgress`, that both the live row and the plain line are drawn from, so the two modes
  cannot disagree about an image's percentage or wording.
- The batch bar's position is a pure function of how many images ended and the current row's fraction, so it can be
  tested without a terminal.
- All wording and colour stay in `look.rs` as pure functions, as today.

**Non-Goals:**

- Naming the provider `Auto` resolves to, or showing a CPU fallback on the terminal. The log keeps both.
- One line per dependency. The repository owner chose the single, rewritten line.
- Any change to the summary, the exit statuses or Ctrl-C handling.

## How the work divides

**All in `cli`.** Everything added is how the terminal draws data that `opai` already reports: `Progress` for
installs, `InferenceProgress` for images, `ExecutionProvider::as_str` for the processor's spelling. The GUI draws its
own progress from the same reports and needs none of this. `opai` is not changed. Showing `Auto` literally means
`opai`'s crate-private resolution (`providers::options::resolve_chain`) does not need to be made public.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| A spinner that turns on its own clock | `tui::Spinner` | `tui` (added in change 1) |
| A session that redraws every frame, multi-row | `tui::Live`, `tui::LiveLine` | `tui` (exists) |
| A gradient bar that eases, with a zero-padded `[n/N]` | `tui::AnimatedProgress` | `tui` (exists) |

Nothing is missing, so `rust-sak` is not extended. No third-party crate is introduced. `ratatui`, `crossterm` (via
ratatui) and `clap` are already `cli` dependencies.

## Decisions

### The dependency line becomes a `LiveLine` that holds a `Spinner`

```rust
// progress.rs
struct DependencyLine {
    name: String,         // Dependency::as_str(), owned
    fraction: f64,
    spinner: Spinner,     // started once, with the line; MAGENTA, `color` from the look
    color: bool,
}

impl LiveLine for DependencyLine {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let mark = (whole_percent(self.fraction) == 100).then(|| look::done_mark(self.color))
            .unwrap_or_else(|| self.spinner.span());
        look::dependency_line(self.name, self.fraction, mark, self.color).render(area, buf);
    }
}

// look.rs
pub fn dependency_line(name: &str, fraction: f64, mark: Span<'static>, color: bool) -> Line<'static>;
pub fn done_mark(color: bool) -> Span<'static>;   // "✓", GREEN (#00c202) when coloured
```

`install_report` updates `name` and `fraction` through the `LiveHandle`, and the session's own redraws turn the
spinner. Reading the spinner's span in `render`, not in the report, is what keeps it turning while a slow download
reports nothing. The spinner is started once, with the line, so it does not jump back to its first frame when the next
dependency starts.

`look::dependency_line` stays pure: the caller passes the mark span, so the tests pin the exact text with a fixed
glyph. The `✓` shows whenever the line reads `100`, so a finished dependency shows it briefly before the next one
starts, and the last one keeps it once `finish_install` freezes the line. A Ctrl-C mid-download freezes the line on
whatever spinner frame it was drawing. That is what was on screen, and the failure line under it says it stopped.

Colours: the name and the percentage are bold white (`Color::White` + `BOLD`, as the percentage is today). The spinner
uses `look::MAGENTA` (`#c792e9`, UMD's magenta, already the row's `image` colour). The `✓` uses a new `look::GREEN`
(`#00c202`, UMD's green). The spinner keeps `Direction::Clockwise`, the default chosen in change 1.

**Alternative rejected: keep `Live<Line>` and push a new `Line` from a ticker task.** That adds a task and a timer to
do what `Live`'s frame loop already does, and its speed would depend on the ticker's rate. Change 1 made the spinner
read the clock precisely so that it can be drawn inside `render`.

**Alternative rejected: one line per dependency.** It shows each `✓`, but the repository owner chose to keep the
single rewritten line.

### The processor line is printed by `run`, between the check and the batch

```text
initialize -> finish_install -> check_processor -> eprintln!(🧠 line) -> eprintln!() -> stop_on_ctrl_c -> start_batch
```

```rust
// look.rs
pub fn processor_line(provider: ExecutionProvider, quality: Quality, color: bool) -> String;
// "🧠 Running inference using the processor " + provider.as_str() in GREEN + "; quality " + "HD"/"SD" in YELLOW
```

It is an ordinary `eprintln!` because no session holds stderr at that point. The dependency session is closed, and
the batch's has not started. It is printed in both modes, which is what the repository owner chose for piped output.
Printing it after `check_processor` means a refused processor ends with only the `🧨` line.

The blank line under it is always printed, so `BatchLine`'s `gap` row and `Install::drawn` go away. They existed to
put a blank row under a drawn dependency line without leaving a stray blank line when the terminal refused the
batch's session. With the processor line always between the two, that case no longer exists. The view's height becomes
`2 + min(N, 5)`.

`provider.as_str()` already spells `Auto`, `CPU`, `CoreML`, `CUDA`, `TensorRT` and `WebGPU`, so the CLI adds no
spelling of its own. `Quality` gets a `label()` (`"HD"`/`"SD"`) beside its `ValueEnum` derive.

### One `ImageProgress` per image, kept in both modes

```rust
// look.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity { Preparing, Downloading(Family), Applying(Family) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState { Working(Activity), Applied, Failed, Stopped }

pub fn row(percent: u8, path: &str, state: RowState, color: bool) -> Line<'static>;
pub fn plain_row(percent: u8, path: &str, end: &ImageEnd, color: bool) -> String;
pub fn image_percent(fraction: f64, written: bool) -> u8;   // floored; capped at 99 unless written

// progress.rs
struct ImageProgress { index: usize, path: String, fraction: f64, state: RowState }
```

`View` keeps the current image's `ImageProgress` in its own mutex, outside any session. `start_image` creates it
(`fraction 0`, `Working(Preparing)`). The inference callback updates it on every report, live or not:

| `report.stage` | activity |
|---|---|
| `Installing(_)` | `Downloading(report.subject.family())` |
| `Running`, `Cached` | `Applying(report.subject.family())` |

`fraction` takes `max(fraction, report.chain_fraction)`, so the row never moves backwards even if a report comes out
of order. `end_image` turns the state into `Applied`, `Failed` or `Stopped` and keeps the fraction. When a session is
live, the same update is mirrored into the matching `BatchLine` row. When none is, `end_image` prints
`plain_row(image_percent(fraction, written), …)`. This is how a piped failure line carries its percentage, which today
it could not.

The row's spans are `emoji + " ["`, the percentage and its `%` right-aligned to four columns (blue), `"] "`, then the
wording, whose family name or `Enhancements` is its own magenta span, then spaces padding the wording to
`look::WORDS_WIDTH` plus one, then the path (cyan, underlined). `WORDS_WIDTH` is the longest wording, `Downloading Face
Recovery for`, so every path starts in the same column, in the live list and the plain lines alike (the repository
owner asked for rows of the same length during implementation). The wordings are short so the padding stays small:
`Preparing to enhance`, `Downloading <family> for`, `Applying <family> to`, `Enhancements applied to`, `Failed to
enhance`, `Stopped enhancing`. A `🧨` row is red throughout, as today. The family words come from the existing `family_label` (the GUI's chip words: `Colorize`,
`Light Adjust`, …), so the bar's old vocabulary and the rows' new one stay the same list.

`index` stays in `ImageProgress` only to find the row to update. It is no longer printed.

**Alternative rejected: keep the position `[n/N]` on the rows and add the percentage beside it.** The slice moves the
count to the bar (`[ended/N]`), and the rows follow UMD's `[ 42%]`. Two counters per row would crowd the line.

**Alternative rejected: show `Applying <first family>` before any report.** Before the first report the image is still
being decoded, and an image with an empty chain has no family at all. One word, `Preparing`, covers both.

### The batch bar is created once and fed `(ended + current) / N`

```rust
struct BatchLine {
    bar: AnimatedProgress,      // label "Enhancing", counter Some((ended, total)), created in start_batch
    rows: VecDeque<Row>,
    total: usize,
    color: bool,
}

pub fn batch_fraction(ended: usize, current: Option<f64>, total: usize) -> f64;
// (ended + current.unwrap_or(0)) / total, where `current` is the current row's fraction capped below 1 until written
```

`View` counts `ended` (incremented by `end_image` for `Applied` and `Failed`; a `Stopped` image ends the run, so it does
not matter whether it is counted) and recomputes the bar on every inference report and every `end_image`. The current
image's contribution is the same capped value its row shows, `image_percent`'s cap applied to the fraction
(`min(fraction, 0.99)` until written). So the bar cannot reach `100.0%` before the last image has ended, and the bar
and the row cannot disagree.

`start_image` no longer touches the bar. Since an image's contribution starts at `0` exactly where the previous
image's `1` was added to `ended`, the bar never moves backwards between images. `AnimatedProgress` eases toward each
new value, which is the "fills smoothly" of the slice.

`look::label`, `look::counter`, `LABEL_WIDTH`, `ENHANCING`/`DOWNLOADING` as bar labels, and `BatchLine::gap` are
removed. `family_label` stays because the rows use it.

**Alternative rejected: drive the bar by image count only (`ended / N`).** That is UMD's bar, but UMD's items are many
and fast. A batch of two large images would sit at `0.0%` for minutes and then jump.

### The bar estimates the time left, in the CLI

The repository owner asked for an ETA on the batch's bar during implementation. `rust_sak::tui::AnimatedProgress`
already draws one (`eta: Option<Option<Duration>>`, formatted by `format_eta`: `ETA --`, `ETA 4.2s`, `ETA 1m5s`), but
it has no estimator, so `BatchLine` computes it:

```rust
fn time_left(elapsed: Duration, fraction: f64) -> Option<Duration>;
// None (`ETA --`) below 1% of the batch; zero at 1; otherwise elapsed × (1 − fraction) / fraction
```

`elapsed` is measured from `BatchLine::new`, and the estimate is recomputed in `render`, every frame, from the bar's
target fraction, so it counts down while a long tile loop reports nothing. It is a straight-line rate: the first
image's model load or download makes the early estimate high, and it settles as the batch goes on. The repository owner
chose to keep the estimator in the CLI rather than add one to `rust-sak`.

### Ownership and lifetime

No model session, memory budget or execution provider is touched. The `ExecutionProvider` printed is the `Copy` value
`run` already holds. The `Spinner` lives inside the dependency line's session and is dropped with it at
`finish_install`. `ImageProgress` lives in `View`'s `Arc<Inner>` behind a mutex, as `Install` and `Batch` do, because
the inference callback runs on the library's threads.

## Risks / Trade-offs

- **[Scripts parsing the old per-image lines break.]** → Marked BREAKING in the proposal. `CLI_REFERENCE.md` gets a
  note beside the earlier "The output changed" entry. The log file still records each path written, which is the
  documented way to read results.
- **[A row's family words read a little oddly, e.g. `Applying Colorize`.]** → They are the GUI's chip words, kept so
  the GUI and the CLI name an enhancement the same way. Changing the vocabulary is a separate decision.
- **[`Auto` tells the user nothing about the hardware.]** → The repository owner's choice. The log records what each
  image ran on, and `-p` can name the provider.
- **[A slow model download inside a long batch barely moves the batch bar.]** → The row shows the download with its
  own percentage, so the user still sees progress.
- **[Platforms.]** → The new wording, colours, `batch_fraction`, `image_percent` and the plain lines are pure functions
  with unit tests that run in CI on macOS, Windows and Linux. The live view, the spinner and the colours are checked by
  hand on macOS only. Windows Terminal and Linux terminals are **untested** by hand, and the legacy Windows console is
  untested for the braille glyphs, as change 1 recorded. No execution provider is involved, but the manual check runs
  on CoreML and CPU (`-p cpu`) only.

## Migration Plan

No stored state, settings or dependency changes. To roll back, revert the change. The `rust-sak` pin stays at
`26.10.6`.
