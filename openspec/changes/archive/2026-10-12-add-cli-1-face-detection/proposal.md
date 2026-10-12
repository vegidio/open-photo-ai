## Why

The terminal front end is about to run face recovery (`opai photo.jpg --fr -o out.avif`). A face-recovery operation
restores the faces it is given and finds none itself. Today the step that finds them for a chain is only in the GUI
crate (`crates/gui/src/faces.rs::for_chain`). That step runs the detection, hands each recovery the faces it keeps,
and lets the rest of the chain run if the detection fails. The CLI needs exactly the same step, so under the
project's layering rule it belongs in `opai`, and it has to move before the CLI is written against it.

This change is slice 1 of the `add-cli` series. It delivers no CLI yet. It leaves the GUI running through the shared
step with no behaviour change.

## What Changes

- `opai` gains a public step that, for a chain carrying one or more face recoveries:
  - runs the shared face-recovery detection (`Detection::for_face_recovery`) once over the chain's input
  - hands each face recovery the faces a caller-supplied selection keeps
  - reports what was found
  - treats a failed detection as "restore nobody, run the rest", and a stopped detection as a stop of the chain
  - does nothing for a chain with no face recovery
- `opai` publishes the default selection, which is to keep the faces `Face::restorable` keeps. A caller with no
  per-face choices (the CLI) gets the same faces the GUI restores before a user has toggled any.
- The pure part (filling a chain from a detection outcome) is public in its own right. A front end can use it to
  test its own seams without a detector on disk.
- `gui`: `for_chain` and `ChainFaces` are removed from `crates/gui/src/faces.rs`. Enhance and export call the
  library step through their existing `Enhancer` seam, passing the user's `FaceChoice` as the selection.
  `FaceChoice`, `face_key`, `DetectedFace` and the picker's `detect_faces` command stay in the GUI, because they are
  the window's wire shapes and interaction.
- No change to what any user sees, and no breaking change to any existing public `opai` API.

Task estimate: about 9 tasks, well inside the 15-task budget. The whole CLI feature is estimated at 20 to 25 tasks,
which is why it is sliced. See design.md, "Slices".

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `face-recovery`: adds a requirement that the library can find the faces for a chain on the caller's behalf, with a
  default selection, failure tolerance and stop semantics. The existing requirement that a face-recovery *operation*
  finds none of its own is unchanged: the new step is a separate detection run that feeds the operation.

## Impact

- **Crates touched:** `opai` (new orchestration step under `inference/`, plus re-exports) and `gui` (`faces.rs`,
  `enhance/run.rs`, `export/run.rs` and their test fakes). `cli` is not touched in this slice. `perf` is not
  touched. Its `select::with_faces` has a different, intended policy: every face found, and a row blocked rather
  than run when the detection fails, because a benchmark must not time a restore of nobody. See design.md.
- **Logic in a front end:** this change moves logic *out of* a front end. What stays in `gui` is the window's
  per-face choice, keyed by the bounding-box string the frontend sends, which only the window has.
- **rust-sak:** nothing in it covers this, and nothing should. This is orchestration of this application's own
  models, not infrastructure.
- **Dependencies:** none added.
- **Parity:** no behavioural divergence from the Go and TypeScript implementation. The GUI keeps the behaviour its
  specs already describe (`gui-faces`: "A photograph with no faces, and a detection that fails"), and the selection
  default is the same `Face::restorable` rule the GUI and Autopilot already use.
