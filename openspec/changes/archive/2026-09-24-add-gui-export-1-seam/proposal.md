## Why

The sidebar's Export image button is drawn and permanently disabled, because nothing behind it can write a file. The
window can run a chain over an open photograph and draw the result, but it has no way to run that chain again at
full depth and put the result on disk. Screens 15 to 15e need that door, and they also need to know which stage each
file has reached. Today the only progress the application reports drives the canvas chip.

## What Changes

This is **slice 1 of 2**. It delivers the backend seam and its frontend IPC wrapper, and changes nothing a user can
see. The button, the dialog, the queue and Autopilot at export are `add-gui-export-2-dialog`, proposed when reached.

- **An `export` command** in `gui`. It resolves an opened image and applies the framing in force. It runs the chain
  the window sends at the source's own depth, then writes the result to the destination the window names, in the
  format and quality it names. It answers the path actually written and the number of bytes. An empty chain is a
  request, not a mistake: it writes the framed photograph as it is.
- **Exporting never disturbs the canvas.** An export does not go through the enhancement slot, so it neither stops
  the run the window is drawing nor drops the result it holds.
- **Numbering instead of overwriting.** When overwriting is off and the destination is taken, the file is written as
  `name_1.ext`, `name_2.ext` and so on. The name is claimed atomically, so two exports that resolve to the same name
  can never both win it. An export that fails or is stopped leaves no file behind.
- **A `cancel_export` command** that stops an export by the window's own name for it. A stop that arrives before its
  export still stops it.
- **An `export:progress` event**, separate from `enhance:progress`. It carries the same fetch and run reporting,
  followed by a `writing` stage, so the queue can say what each file is doing without moving the canvas chip.
- **The cancellation table moves out of `autopilot.rs`** into a shared module, because export is its second caller.
  Autopilot's behaviour does not change.
- **`frontend/ipc/export.ts`**: the wire types, plus `exportImage`, `cancelExport` and `onExportProgress`. It mints
  the run name before the invoke, as `enhance` and `suggest` do.

**Deliberate divergences from the reference's behaviour:**

- **Real progress per file.** The reference reports 10% before inference and 90% after it, and sends the inference's
  own progress to the canvas event instead. Here the export's fetch and run fractions go to the export's own event,
  which is what lets slice 2 show Analysing / Enhancing / Writing honestly.
- **A stopped or failed export leaves nothing.** The reference releases its placeholder only when the write fails. A
  cancellation between claiming the name and writing is not possible there because it checks before claiming. Here
  the claim is released on every path that does not end in a written file.
- **Running out of numbers is refused.** After `_999` the reference falls back to that last name and lets the write
  overwrite it or fail. Here the export is refused with a write error naming the destination, so overwriting stays
  something only the user's switch can cause.
- **The result keeps the source's depth.** The reference is 8-bit throughout. Here a 16-bit source written to a
  format that holds 16 bits keeps them. The window's own run stays 8-bit, as `enhance/run.rs` already records.

## Capabilities

### New Capabilities

- `gui-export`: how the window asks for an open photograph to be enhanced and written to a file, where the file goes
  and under what name, how the export reports its stages, and how an export is stopped or refused.

### Modified Capabilities

None. `image-io` already specifies writing to a destination in a caller-chosen format with per-format settings, and
nothing below the window changes. `gui-autopilot`'s requirements do not change when its table moves.

## Impact

**Crates: `gui` only.** `opai` is not edited. `Opai::process`, `OutputDepth::Source`, `image::save` and the
re-exported `ImageFormat` and `EncodeOptions` are already public. `cli` is untouched.

The numbering stays in `gui`, which reverses what exploration assumed. It has one caller, the export command. The
terminal front end is a version banner today. The reference keeps the same logic in `cmd/gui/services`, not in its
core library. When a CLI export gains overwrite protection, that change promotes it.

**`rust-sak`: `image` through `opai` for encoding (`EncodeOptions::default_for` with the quality set on top), and
nothing missing for this slice.** `fs` has temp-file helpers but no "claim a free name next to this one" helper, and
the claim is one `OpenOptions::create_new` loop, so it is local. One parity gap is recorded rather than filled:
the reference writes TIFF with Deflate compression, and rust-sak's `EncodeOptions::Tiff` takes no compression
setting. Exported TIFFs are therefore larger than the reference's. Filling that belongs in `rust-sak` and is left for
its own change.

**New crates: none. New npm packages: none.**

**Code touched.** In Rust: a new `gui/src/export/` module (the command, the destination claim, the progress wire), a
new shared cancellation-table module lifted from `gui/src/autopilot.rs`, and the command registration in
`gui/src/lib.rs`. On the frontend: a new `frontend/ipc/export.ts`.

**i18n: no new keys.** Nothing in this slice is shown to a user.

**Task count: twelve.** The whole feature is over one change's size and falls into two halves that each leave the
project working. The split is in design.md, under Slices.
