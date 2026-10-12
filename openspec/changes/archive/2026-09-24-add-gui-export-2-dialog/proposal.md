## Why

Slice 1 gave the window a way to write an enhanced photograph to disk, but a user still cannot reach it. The sidebar's
Export image button is drawn permanently disabled. Screens 15 to 15e design the dialog behind it: a queue of the
selected photographs, the settings that decide each file's name, folder, format and quality, and a batch that runs,
reports each file's stage and can be aborted. This slice builds that dialog, which closes the export feature.

## What Changes

This is **slice 2 of 2**, and it depends on `add-gui-export-1-seam`, which is archived. Fourteen tasks.

- **The sidebar's export button comes alive.** It is enabled while at least one open image is picked. It reads
  "Export image" for one picked image and "Export images" for several, and it opens the export dialog.
- **The export dialog**, drawn as screens 15 to 15e show it:
  - **The queue.** Every picked image gets one row, whatever its stack holds, grouped so photographs that need the
    same models are next to each other ("in pipeline order").
  - **What each row shows:**
    - a thumbnail
    - the name that will be written
    - the dimensions before and after
    - the source's size, plus the written size once the file is done
    - the type before and after
    - a stage pill: Queued, Analysing, Enhancing, Writing, Done or Failed
  - **Per-row extras:**
    - a bar of its own while the row is being worked on
    - a reveal button once its file is written
    - on a Failed pill, a red tooltip with the error, which clicking copies to the clipboard
  - **The title bar** gives a summary ("4 files ready · nothing written yet", "1 of 4 written · 2 remaining",
    "3 of 4 written, 1 failed"). One overall bar under it carries the whole batch.
  - **The settings column:**
    - a filename prefix and suffix
    - Save to: the original directory, or a folder the user browses for
    - Allow overwriting, with its normal or warning-coloured footnote
    - the format: Preserve or one of eight formats
    - the quality, shown only when every queued file resolves to the same lossy format
  - **The buttons:** Cancel / Save while idle. Abort, with Save disabled, while running. Close / Export again once the
    batch has ended. The close box and Escape are withheld while the batch runs.
- **Autopilot at export.** With Autopilot on, a picked photograph that has never had a list is analysed when its turn
  comes, and what it suggests is written into its stack before it is exported. That is the same rule and the same
  analysis the canvas uses. With Autopilot off, or where nothing is suggested, the photograph is exported as it is.
- **The export settings are remembered** across launches: the prefix, suffix, folder, overwriting and format. They
  start at the reference's defaults: no prefix or suffix, the original directory, overwriting off, and PNG. The
  quality draft is committed to Settings' per-format qualities when Save is pressed, as in the reference.
- **Two new commands in `gui`:**
  - `pick_directory` opens the platform's folder picker, for Browse….
  - `reveal_export` shows a written file in the file manager. It refuses any path that no export of this session
    wrote, just as `reveal_image` refuses a path it never admitted.
- **New i18n keys** for the stages, the summaries, the pipeline-order hint, the plural button label, the Failed
  tooltip and its copy confirmation, in all thirteen locales. The reference's table-header keys, which this design
  no longer draws, are removed.

**Deliberate divergences from the reference's behaviour:**

- **The batch continues past a failure.** The reference stops at the first file that fails. Screen 15e shows a failed
  row among finished ones, with the summary counting both, and a batch of forty photographs should not stop at the
  second.
- **Every picked photograph is queued.** The reference leaves out a photograph whose stack is empty, and one with no
  stack while Autopilot is off. Here those are exported as they are, framed but unenhanced, which is what the slice
  table recorded. A photograph the user picked and pressed Export for is one they expect a file for.
- **A photograph Autopilot suggests nothing for is exported as it is.** The reference skips it and leaves its row
  idle, so it silently produces no file.
- **Abort ends the batch.** It does not return the dialog to idle. Finished rows stay Done, and the buttons become
  Close / Export again, as after a finished batch. In the reference, pressing Save after an abort re-exported the
  files already written, and without overwriting that numbers them a second time.
- **Autopilot's suggestions are written into the stack.** The reference kept them local to the export. Here they land
  in the stack, as the slice table decided, so the canvas shows afterwards what was exported, and the photograph
  counts as having had a list.
- **Finer stages.** The reference shows Processing, Completed and Error. Here the row names what the backend
  reports: Analysing, Enhancing, Writing.
- **The Failed tooltip carries the actual error**, copyable. The reference showed a generic "Something went wrong...",
  plus a separate tag for a failed model download. A failed download is now just the message the library gives.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-export`: adds the requirements a user sees: the button, the queue, the settings, the batch's stages and
  summary, abort, the end state, the reveal and the failure tooltip.
- `gui-autopilot`: a photograph that is not current is analysed when an export reaches it. A failure in that analysis
  is reported on the export's row, not with the usual notice.

## Impact

**Crates: `gui` only.** `opai` and `cli` are not touched.

- **The logic stays in the front end.** Composing names, grouping the queue and deciding the batch's order and stages
  is presentation of one window's workflow. The terminal front end has no export queue, and the reference keeps all
  of it in its frontend too.
- **The two new commands are window plumbing.** One is a folder picker. The other is a reveal gated on this session's
  own writes.

**`rust-sak`: nothing needed.** The folder picker is `tauri-plugin-dialog`, already a dependency, through its Rust API
as `open_images` uses it. The reveal is `tauri-plugin-opener` through the existing `reveal.rs`.

**New crates: none. New npm packages: none.** Path composition is a small pure function that accepts either
separator, as `lib/paths.ts`'s `fileName` already does. `@tauri-apps/api/path` is asynchronous, and the queue names
are drawn synchronously.

**Code touched.**
- Rust:
  - `gui/src/export/`: the record of written paths and `reveal_export`
  - a `pick_directory` command
  - their registration in `lib.rs`
- Frontend:
  - `ipc/export.ts` gains two wrappers
  - a new `features/export/` for the dialog, its rows and its settings
  - a new persisted export-settings store
  - a new `lib/export.ts` for the pure rules
  - a batch runner
  - `hooks/useAutopilot.ts`'s `analyse`, made awaitable by the batch
  - `features/sidebar/Sidebar.tsx`
  - all thirteen locale files
