## Why

Face Recovery is the second enhancement family this application can offer, and the only one that
cannot be built from the catalogue alone: a face-recovery operation carries the faces it is to
restore, and nothing in this window has ever asked for them. `add-gui-enhance-2-sidebar` named the
gap when it closed - *"for face recovery a detection run the window has no wiring for"* - and left
it as the reason the add menu offers one family.

Everything on the far side of the gap is already built and specified. `opai` publishes
`Detection::newyork`, runs it through `Opai::execute` on the analysis path, and stores its result
under the image's identity plus the operation. `FaceRecovery::athens` and `::santorini` take the
faces as a parameter, and `Faces` already folds an order-sensitive signature of the selected
bounding boxes into the run cache tag. What is missing is entirely in `gui`: a command that runs
the detector, a wire arm that carries the faces back, and the frontend wiring between them.

## What Changes

This is **slice 1 of 2**. It delivers face recovery running over **every** face the detector finds.
Choosing which faces to restore - screens 13 and 13b - is `add-faces-2-select`, proposed when
reached.

- **A `detect_faces` command** in `gui`: resolves an opened image, applies the framing in force,
  and runs `Detection::newyork` at FP32 through `Opai::execute`, answering the faces in the framed
  photograph's own coordinate space.
- **A `face_recovery` arm on the wire operation**, carrying the codename, the precision and the
  faces. The fidelity is **not** on the wire: the window fixes it at `Fidelity::MAXIMUM` (1.0), so
  a field it always sets to one value would be surface describing a control that does not exist.
- **Face Recovery joins the add menu** - the second member of `RUNNABLE` - and its rows report
  `Athens, 2 Faces, HD` off the detected count.
- **Detection is reported as Face Recovery**: the command emits through the existing progress event
  under the `face_recovery` family, so the first-use transfer of the detector is drawn by the chip
  already on screen rather than freezing the window silently.
- **Faces are detected against the framing, and re-detected when it changes** - a crop or a rotation
  moves every face, so the previous answer describes a photograph nobody is looking at.

**Deliberate divergences from the reference's behaviour**, both argued in design.md:

- **Detection runs at FP32 always.** The reference ties the detector's precision to the recovery
  model's tier, so an HD recovery is fed by the FP32 graph and an SD one by the FP16 graph.
  `face-detection` already requires that the two find the same faces in the same order, differing
  only sub-pixel - so the tier buys nothing and costs a second detector build on disk and a second
  cache entry per photograph.
- **Detection reports progress.** The reference's `DetectFaces` is silent, which was affordable
  when its models shipped with the application. Here they are fetched on first use.

**No requirement changes below the window.** `face-detection`, `face-recovery`, `data-inference`
and `run-cache` already specify everything this slice relies on, including both of the caching
properties this change was asked for:

- a face-recovery result stored for one selection is not served for another (`face-recovery`), and
- a repeated analysis of the same image is served without transferring or opening the model
  (`data-inference`), which - because `gui`'s `cropped()` derives a framed picture's identity from
  the crop's own fields - makes a photograph turned to 90°, then to 50°, then back to 90° reuse the
  detection made the first time.

## Capabilities

### New Capabilities

- `gui-faces`: what the window does about faces - when it asks for them, what they are expressed
  in, what a framing change does to them, what it shows while the detector is being fetched, and
  what a failed or empty detection means for the enhancement that needed it.

### Modified Capabilities

- `gui-enhance`: Face Recovery becomes an enhancement the stack can carry - offered by the add
  menu, described by a row's info line, and carried into a run with its faces.

## Impact

**Crates: `gui` only.** `opai` is not edited and needs nothing: it already publishes the detector,
the analysis path, the face vocabulary and both recovery constructors. `cli` is untouched. Nothing
here is logic the terminal front end could want - which faces of an open photograph a window is
about to restore is a property of a window, and the CLI names its own operations directly.

**`rust-sak`: nothing used directly, and nothing missing.** No download, hash, extraction, temp
file, codec, cache or hardware probe is written here. The detector's transfer reaches `fetch`, its
result reaches `memo`, and the framed picture's identity reaches `crypto` - all three through
`opai` and through `gui/src/images/crop.rs`, which already exists.

**New crates: none. New npm packages: none.**

**Code touched.** In Rust: `gui/src/enhance/operation.rs` (one arm), a new `gui/src/faces.rs`, and
the command registration in `gui/src/lib.rs`. On the frontend: a new `ipc/faces.ts` and
`stores/faces.ts`, a new `features/enhancements/FaceRecoveryOptions.tsx`, and edits to
`lib/enhancements.ts`, `features/enhancements/EnhancementRow.tsx` and
`hooks/useEnhancementRun.ts` - which grows the detection step rather than a second hook being
written beside it, for the reason design.md D4 gives.

**i18n: no new keys.** `enhancements.infoFaces`, `enhancements.faces_one/other`,
`enhancements.faceRecovery.*`, `preview.progress.faceRecovery` and `errors.faceDetectFailed` are
already in all thirteen locales, carried over with the catalogue in `add-gui-1-foundation`.

**Task count: thirteen.** The whole feature is over the limit; the split and what slice 2 carries are
in design.md - Slices.
