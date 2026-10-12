## Why

Autopilot is the one control on the empty shell, and it does nothing: the switch is drawn, persisted and live, and
the Enhancements settings page records which families it may suggest, but no path in the window asks `opai` for an
analysis. `opai` already analyses a picture through `Opai::suggest`, narrowed to a set of families, with a face
signal that goes through the same detection and run store `detect_faces` uses. What is missing is a door to it from
the window, and a way to withdraw the question when the photograph it was about is closed.

## What Changes

This is **slice 1 of 2**. It delivers the backend seam and its frontend IPC wrapper, and changes nothing a user can
see. Running the analysis when a photograph is selected and applying what comes back is `add-gui-autopilot-2-run`,
proposed when reached.

- **A `suggest` command** in `gui`: resolves an opened image, applies the framing in force, and runs
  `Opai::suggest` over it, narrowed to the families the window names. It answers the suggestions in the library's
  family order, on a wire tagged by family the way an operation is, with the upscale's scale beside it.
- **A `cancel_suggest` command** that stops an analysis by the window's own name for it. Several analyses may be in
  flight at once (one per photograph the user has selected and moved off), so this is a table of runs, not the
  enhancement slot. A stop that arrives before its own analysis still stops it.
- **A stopped analysis is its own refusal**, distinct from a failed one, so the window can stay silent for a
  photograph that was closed and report one that genuinely failed.
- **`frontend/ipc/autopilot.ts`**: the wire types and `suggest`/`cancelSuggest`, minting the run name before the
  invoke as `enhance` and `detectFaces` do.

**Deliberate divergences from the reference's behaviour:**

- **An analysis can be stopped.** The reference's `SuggestEnhancements` returns a cancellable promise, but nothing
  cancels it: a photograph closed mid-analysis goes on being analysed. Here, closing a photograph (slice 2) withdraws
  the question, so it stops holding a detector session and CPU for an answer nobody will read.
- **The analysis sees the photograph as it is framed.** The reference analyses the file on disk. Analysing the framed
  picture means the face signal's detection is stored under the same identity `detect_faces` asks for, so the faces
  slice 2 needs to size afterwards are served from the run store rather than detected a second time. On a photograph
  nobody has cropped, the two are the same picture.

## Capabilities

### New Capabilities

- `gui-autopilot`: how the window asks for a photograph to be analysed, what comes back across the boundary, which
  photograph and framing the analysis sees, and how an analysis is stopped.

### Modified Capabilities

None. `autopilot` already specifies the analysis itself: narrowing, ordering, incompleteness, cancellation, and the
face signal sharing its detection with a direct one. No requirement below the window changes.

## Impact

**Crates: `gui` only.** `opai` is not edited: `Opai::suggest`, `Suggestion`, `Family` and `Scale` are already
public, and `Family` and `Scale` already serialize. `cli` is untouched. Nothing here is logic the terminal could
want: which open photograph a window analyses, and under what name it stops one, are properties of a window. The
suggestion wire type is `gui`'s own because `Suggestion` has no `Serialize`, and giving it one would fix a wire shape
in the library for one front end.

**`rust-sak`: nothing used directly, and nothing missing.** The detector's transfer reaches `fetch` and its result
reaches `memo`, both through `opai`. The framed picture's identity reaches `crypto` through
`gui/src/images/crop.rs`, which already exists.

**New crates: none. New npm packages: none.**

**Code touched.** In Rust: a new `gui/src/autopilot.rs` and the command registration in `gui/src/lib.rs`. On the
frontend: a new `frontend/ipc/autopilot.ts`. `load_framed`, `Opened` and `Processor` are reused as they are.

**i18n: no new keys.** Nothing in this slice is shown to a user.

**Task count: eight.** The whole feature is over one change's comfortable size and falls into two halves that each
leave the project working. The split is in design.md, under Slices.
