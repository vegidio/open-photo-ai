## Why

`add-faces-1-seam` shipped face recovery running over **every** face the detector finds, and named
what it left: *"Choosing which faces to restore - screens 13 and 13b - is `add-faces-2-select`,
proposed when reached."* This is that slice, and it closes the series.

Restoring every face is the right default and the wrong ceiling. A group photograph has faces the
user did not ask about - a stranger in the background, a face too small to matter, a face the model
makes worse - and today the only way to leave one alone is to crop it out of the photograph. The
reference has had a per-face toggle since its own face recovery landed; screens 13 and 13b draw it;
and every piece it needs on this side of the boundary was built by slice 1.

## What Changes

This is **slice 2 of 2**, and the last of the series. It delivers choosing which of the detected
faces a recovery restores.

- **A deselection in `stores/faces.ts`**, held per photograph beside the faces already there.
- **A face starts chosen or skipped according to its size.** A bounding box covering more than
  512x512 pixels - the tile the recovery models actually run at - starts skipped, because a face
  larger than the tile is reduced into it, restored and enlarged back, which on an already sharp face
  can only soften it. Everything below that starts chosen. The rule decides only what a face starts
  as; turning one on or off by hand is untouched by it.
- **Screen 13's Faces block** in the Face Recovery options panel: a rule, the *Faces* heading, the
  centred two-line help, and a button reading `Select faces (1)` that carries the chosen count.
- **Screen 13b's Select faces dialog**: the framed photograph fitted to its own aspect, a box per
  detected face - yellow for chosen, grey for skipped - a footer hint, and an Apply that carries
  `Apply to 1 of 2 faces`.
- **Apply commits and dismissal discards.** The dialog edits a working copy; Apply writes it, and
  the close box, Escape and a click outside leave the selection as it was.
- **The row's info line reports the ratio**: `Athens, 1/2 Faces, HD` while some are skipped, and
  `Athens, 2 Faces, HD` while none are - which is the `faces_partial` catalogue key already carried
  in all thirteen locales.
- **A committed change re-runs the preview**, once per Apply rather than once per box clicked, and
  the run carries only the chosen faces.
- **One movement of the progress bar across the detection and the run it was for.** Slice 1 left the
  two drawn as they arrived - the detection filled the bar, the chain emptied it and filled it again,
  which reads as the enhancement having been applied twice. Exercising this slice's chooser is what
  made that unmissable, because a chooser is a reason to re-run a face recovery over and over.

**Deliberate divergences from the reference's behaviour**, both argued in design.md:

- **An explicit Apply, where the reference commits on close.** The reference's `FaceToggle` commits
  its working copy from `handleClose`, so every way out of the dialog is a commit and there is no way
  to change one's mind. The mockup draws a footer with an Apply button carrying the count, which only
  means something if dismissal is the other answer. Screen 13b is the newer statement and it is the
  one this follows.
- **A deselection is stored as the `Face` value rather than as an index into the detected array.**
  The consequence is visible: a framing change re-detects, the new faces are at new coordinates, and
  every face in the re-framed photograph is chosen. The reference keeps indices, so its deselection
  survives a crop by position - which is the wrong answer when the crop removed the face that index
  named. Settled with the repository owner during slice 1's exploration and recorded in its archived
  design.

**No requirement changes below the window, and no Rust at all.** `enhance`'s `face_recovery` arm
already carries the faces the window hands it, `face-recovery` already requires that a result stored
for one selection is not served for another, and the rendition protocol already serves a bounded
*framed* picture - the sidebar's miniature has asked for one since `add-crop-3-dialog`. What is
missing is entirely in the frontend.

## Capabilities

### Modified Capabilities

- `gui-faces`: the faces found in a photograph can now be chosen among - where that choice is made,
  what commits it and what discards it, what a framing change does to it, and where the boxes are
  drawn.
- `gui-enhance`: a face-recovery run restores the chosen faces rather than every found one, and its
  row reports the ratio.

`gui-faces` also gains the indicator rule the bullet above describes, and its requirement about
detecting only where the enhancements need it gains the case where the answer lands for a stack that
no longer does: the answer is kept and the run in flight is left alone.

### New Capabilities

None. Choosing among faces is what `gui-faces` is about, and slice 1 created it.

## Impact

**Crates: `gui` only, and its frontend only.** No Rust file is edited. The window already has the
command, the wire arm and the store; what this adds is a selection over the answer they produce,
which is a property of a window and of nothing else. `opai` is untouched and needs nothing: it
neither knows nor should know which of a photograph's faces a user is currently interested in.
`cli` is untouched - it names its own operations and passes its own faces.

**`rust-sak`: nothing used, and nothing missing.** No Rust is written here at all, so no download,
hash, extraction, temp file, codec, cache or hardware probe is either.

**New crates: none. New npm packages: none.** The dialog is `components/ui/dialog.tsx` and
`DialogTitleBar`, both already carrying Crop/Rotate and Settings; the boxes are absolutely
positioned buttons, as the reference's are.

**Code touched.** All under `crates/gui/frontend`: `stores/faces.ts` (a deselection beside the
faces, and the record of which faces have been decided that the size rule reads), a new `lib/faces.ts`
holding a face's key, the faces a run restores and the size rule itself, a new `features/faces/`
holding `SelectFacesDialog.tsx`, `FaceBoxes.tsx` and `useFaceSelection.ts`, and edits to
`features/enhancements/FaceRecoveryOptions.tsx`, `features/enhancements/EnhancementRow.tsx`,
`hooks/useEnhancementRun.ts` and `lib/constants.ts`.
The indicator adds two more: `features/preview/ProgressBar.tsx`, which is given the figure rather
than reading it off the report it happens to hold, and `features/preview/PreviewImage.tsx`, which
passes it down.

**i18n: two new keys**, in all thirteen locales. The footer the mockup draws is the one part of
these screens the reference has no dialog chrome for, so the catalogue carried no string for it:
`faces.hint` and `faces.apply`. Everything else is already there - `faces.selectFaces`,
`faces.toggleFace`, `enhancements.faceSelector.title/help/button` and
`enhancements.faces_partial_one/other` were carried over with the catalogue in `add-gui-1-foundation`
and are present and translated in all thirteen files.

**Task count: thirteen.** The series' second and last slice fits in one change.
