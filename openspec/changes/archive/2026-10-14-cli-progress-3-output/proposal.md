## Why

Today the CLI's live view shows the current image's bar and an `[n/N]` position in each row. That says which image
is running but not how far the whole batch has got, and nothing on the terminal says which processor or quality the
run uses. The redesign moves the output closer to UMD's: one bar for the whole batch, a percentage on each image's
row naming what it is doing, a spinner on the dependency line, and a `🧠` line naming the run's processor and quality.
Changes 1 (`tui::Spinner`) and 2 (`--processor`) are done, so this change can now draw the result.

## What Changes

Task estimate: **12**. The whole CLI progress redesign needs more than 15 tasks, so it is split into three changes
(see design.md - Slices). This proposal covers change 3, the last.

**In `crates/cli`, observable on the terminal (BREAKING for anyone parsing stderr):**

- **The dependency line** reads `💾 Downloading dependency <name>... <spinner> <n>% Complete`. The name and the
  percentage are white and bold, and the braille spinner (`rust_sak::tui::Spinner`) is magenta `#c792e9`. While the
  current dependency is at `100%`, a green `✓` replaces the spinner. As today, it is **one line, rewritten** when the
  next dependency starts (the repository owner chose this over one line per dependency).
- **A new `🧠` line** follows initialization and the processor check, in every mode, piped output included:
  `🧠 Running inference using the processor <Name>; quality <HD|SD>`, with the name green and the quality yellow. The
  name is what `-p` asked for, in `opai`'s spelling (`Auto`, `CPU`, `CoreML`, …). Under `auto` it reads `Auto`
  (the repository owner chose this over naming the provider `Auto` would try first). A blank line always follows it.
- **The bar covers the batch, not the current image.** It is always labelled `Enhancing`. Its `[n/N]` counts the images
  that have ended, failures included. It fills smoothly as `(ended + current image's progress) / N`, so it no longer
  empties between images, and it reaches `100.0%` only once the last image has ended. It ends with `ETA <time>`, the
  time the batch has left at its rate so far (`ETA --` until 1% has run).
- **Each row shows the image's own percentage and what it is doing**, instead of its position in the run:
  - `🖼️ [  0%] Preparing to enhance <path>` before the image's run has reported anything
  - `🖼️ [ 12%] Downloading <Family> for <path>`
  - `🖼️ [ 42%] Applying <Family> to <path>`
  - `🖼️ [100%] Enhancements applied to <path>` once written, also for an image with nothing to run
  - `🧨 [ 42%] Failed to enhance <path>` / `🧨 [ 42%] Stopped enhancing <path>`, red throughout

  The words are padded to the longest wording (`Downloading Face Recovery for`), so every path starts in the same
  column.

  The percentage is held at `99%` until the file is written. The percentage, `%` included, is blue, the family words and
  `Enhancements` are magenta, and the path is cyan and underlined.
- **Piped output** prints the `🧠` line and its blank line, then each image's final row (plus `: <reason>` for a
  failure) when it ends, then the summary. There is still no dependency line, bar or redraw.
- **Docs:** `docs/CLI_REFERENCE.md`'s "Terminal output", "What happens during a run" and "Stopping a run" sections are
  rewritten, and a breaking-change note is added.

**Unchanged:** the summary (`✨ …`, each failure's `🧨` line, the closing line), exit statuses, `NO_COLOR`, the
five-row list and its scrolling, Ctrl-C handling, and what goes to the log.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-enhance`: the requirement "Progress is shown while the command works" changes: the dependency line's wording,
  spinner and colours; the new `🧠` line in every mode; a batch-wide bar labelled `Enhancing`; per-image percentage
  rows; the new wording of the plain lines.

## Impact

**Crates touched:** `cli` only (`look.rs`, `progress.rs`, `run.rs`, module docs) and `docs/CLI_REFERENCE.md`. All of it
is terminal rendering, which the project's architecture puts in the front end. The data it draws comes from `opai`
unchanged: `Progress` for installs, `InferenceProgress` (`chain_fraction`, `stage`, `subject.family()`) for images,
`ExecutionProvider::as_str` for the processor's name. The GUI draws its own progress and needs none of this.

**`opai`:** unchanged. Showing `Auto` literally needs no resolution API.

**`rust-sak`:** `tui::Spinner` (added and released in change 1, tag `26.10.6`), and `Live`, `LiveLine` and
`AnimatedProgress`, which the CLI already uses. Nothing is missing, so `rust-sak` is not changed.

**Third-party crates introduced:** none.

**Breaking:** scripts that parsed `🖼️ [n/N] Enhanced image <path>` or `🧨 [n/N] Failed image <path>: <reason>` from
stderr see the new wording. The image's place in the run is no longer printed on its line. The log file still records
each path written.
