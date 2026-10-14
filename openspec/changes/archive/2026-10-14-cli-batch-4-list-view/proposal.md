## Why

Slice 3 made `opai` take many inputs, but its terminal output is still the single-image layout repeated per image:
a header, a bar, a result line, then the next header. A batch of forty images fills the scrollback with 160 lines,
and an early failure gets buried. The series planned a compact view from the start: the current image's chain bar,
with a five-row list under it that scrolls to the most recent image, in UMD's CLI look. Slice 1 gave `rust-sak` a
live session taller than one row to draw it. This slice draws it, adds an end summary, and gives the non-terminal
mode one line per image.

## What Changes

Task estimate: **11**. This change is slice 4, the last of the four-slice `cli-batch` series (see design.md -
Slices).

**In `cli`:**

- **One live view for the whole batch, used for every run, including `[1/1]`.** After initialization, a single
  `rust_sak::tui::Live` session draws:
  - the current image's chain bar, unchanged (label, `[n/N]` step counter, gradient, percentage);
  - a blank row;
  - the list: one row per image started so far, showing the most recent five. The session is `2 + min(images, 5)`
    rows tall, so a single image does not leave four empty rows behind.
- **The rows, in UMD's colours.** `🖼️ [2/5] Enhancing image photos/b.jpg` while an image runs, and
  `🖼️ [2/5] Enhanced image photos/b.jpg` once it is written. On a colour terminal the `n/N` is blue, `image` is
  magenta and the path is cyan and underlined. A failed image's row becomes `🧨 [3/5] Failed image photos/c.jpg`, and
  a stopped one `🧨 [3/5] Stopped image photos/c.jpg`, both entirely in red.
- **The bar restarts for each image**, from `Enhancing [0/N]`, without animating back down. An image with no
  enhancement left to run (an upscale skipped as unnecessary) leaves the bar row blank while it runs.
- **The header line goes.** `🖼️ Applying enhancements to the image <path>` and the per-image result lines are
  replaced by the list.
- **An end summary, printed below the frozen view:**
  - When at least one image was written: `✨ Image enhanced successfully; file saved at <file>` when exactly one was
    written (so a numbered name such as `out_1.png` stays visible), or
    `✨ <k> images enhanced successfully; files saved under <output>` for more.
  - Then one `🧨 <reason>` line per failed image, because a failure can have scrolled out of the five-row list.
  - Then the run's closing failure line, as today: `🧨 <failed> of <total> images failed` for a batch of more than
    one image, or `🧨 stopped: nothing more was written` after a Ctrl-C.
- **A single-image failure ends with its own line.** With one image, the image's own `🧨` reason is the closing line,
  and the redundant `🧨 1 of 1 images failed` that slice 3 prints goes away.
- **BREAKING - the plain (non-terminal) output.** When stderr is not a terminal, each image prints one uncoloured line
  when it ends, with its final row's words: `🖼️ [2/5] Enhanced image photos/b.jpg`, or
  `🧨 [3/5] Failed image photos/c.jpg: <reason>`. The summary follows. The header lines and
  `✨ Image enhanced successfully; file saved at …` per image are gone, so a script that read the written path from
  each success line must use the log instead, or the summary for a single image.
- **A terminal that refuses the live session** (raw mode or cursor position unavailable) gets the plain lines,
  coloured as the terminal allows, so per-image results are never lost.
- **Ctrl-C anywhere in the batch is the session's key press.** The session covers loading and writing too, not just
  the bar, so a Ctrl-C between two images' bars stops the run through the same path.
- **Docs.** `docs/CLI_REFERENCE.md` ("Terminal output", "Stopping a run", "Scripting") and the module docs in
  `main.rs`, `run.rs` and `progress.rs`.

**Not in `opai`, `gui` or `rust-sak`:** none changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-enhance`: "Progress is shown while the command works" is rewritten for the bar-over-list view, the summary and
  the one-line-per-image plain mode. "Exit status says how the run ended" says how a batch's failures are reported:
  the listed reasons, the count line for more than one image, and the image's own line for a single image.

## Impact

**Crates touched:** `cli` only (`look.rs`, `progress.rs`, `run.rs`, `error.rs` docs, `main.rs` docs). This is all
terminal rendering: the row wording, colours, the view's layout and the summary are the CLI's vocabulary, and the GUI
has its own queue UI. Nothing here is logic both front ends would need, so nothing moves to `opai`.

**`rust-sak`:** `tui::Live` with `LiveLine::height` (slice 1, tag `26.10.6`) draws the multi-row session, and
`AnimatedProgress` draws the bar inside it, as `examples/tui_rows.rs` does. The list itself is not a `rust-sak`
widget. Slice 1 and `cli-look-1-rust-sak-tui` decided that palettes and wording stay in each CLI. Nothing is added to
`rust-sak`, and the pin does not move.

**Third-party crates introduced:** none.

**Not in scope:**

- Running images concurrently, or a whole-batch progress bar. The bar is the current image's chain.
- An ETA, for the image or the batch.
- Changing the dependency line, the bar's labels and counter, or anything about naming and inputs (slices 2 and 3).
