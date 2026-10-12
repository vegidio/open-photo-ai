## Why

Slice 1 put an arbitrary-angle `rotate` into `rust-sak`; slice 2 built the seam that carries a framing from
the window to the pixels — composed in `gui`, carried on the `opai://` URL and on the `enhance` command,
reflected in the navbar's dimensions, keyed into every cache on both sides. All of it works and none of it
can be reached: `stores/crop.ts` has a reader and no writer, and its own documentation says so in as many
words. The application ships a crop it is impossible to set.

This slice is the writer. It adds the crop control the design draws on the sidebar's miniature (screen 05)
and the near-fullscreen Crop/Rotate dialog behind it (screen 18) with every control that screen draws — the
aspect-ratio grid, the dimension fields and their swap, the fine-rotation slider with its quarter-turn and
flip actions, Reset, Cancel and Apply. It is the last slice of the series, and it is the one that makes the
previous two visible.

## What Changes

Task estimate: **15**, which is the ceiling exactly. The series is three slices (see design.md — Slices); this
proposal covers the third and last. Nothing in `opai`, `cli` or `rust-sak` is touched, and no `.rs` file in
`gui` is either: every seam the dialog writes into was built in slice 2 and none of them moves.
**This slice is entirely frontend.**

**In `gui` (frontend) — the dialog:**

- **`react-advanced-cropper` at 0.20.1**, the widget the Wails app uses and the one slice 1 measured its
  sign convention and its expanded-canvas formula against. It is the only new package, its peer range is
  `react >=16.8.0`, and its stylesheet has to be overridden into this project's Tailwind 4 theme the way the
  reference already overrides it — accepted at the time the decision was taken, and design.md D2 records what
  that costs.
- **A Crop/Rotate dialog**, near-fullscreen at `inset-8` over the canvas's own surface, topped by the
  existing `DialogTitleBar` and split into the cropper pane and a 256px settings column, exactly as screen 18
  draws it. The surface behind the cropper is the dotted one, drawn unconditionally whatever the preview
  canvas is set to draw. This was planned the other way round — `useCanvasSurface`'s branch, that helper
  having been written in `add-gui-canvas-background` with a comment naming this dialog as its second reader —
  and verifying the built dialog by hand settled it the other way: this is a work surface, where what is
  being judged is where an edge falls against the background behind it, and the animated alternative
  competes with the rectangle being placed. The helper's exported `DOTTED_SURFACE` is what the dialog reads,
  so the two regions still share one definition of the dotted surface; what they no longer share is the
  choice (design.md D9).
- **A controller hook holding the dialog's whole state**, so the components stay layout. The reference's
  `useCropController` is the model for this and its hard-won effect ordering — a ratio change that must reach
  the stencil before the box is reshaped, a swap that must wait for the cleared lock — is ported rather than
  re-derived, which was the deciding argument for taking the library at all.
- **The aspect-ratio grid**: ten options in two columns — Free, Square, 5:4, 4:5, 4:3, 3:4, 3:2, 2:3, 16:9,
  9:16 — each drawn as the design draws them, a circular well holding a rectangle in that ratio's own
  proportions, dashed for Free. The reference's Material crop glyphs are not carried over; the design
  replaces them, and a proportional box says what a ratio is in a way `MdCropLandscape` does not.
- **The dimension fields and the swap between them**, authoritative in **source** pixels while the widget
  works in the reduced space of a bounded rendition, with the scaling done at that one boundary.
- **The rotation row**: a quarter-turn button, a horizontal flip, a vertical flip, the fine-rotation slider
  from -90 to +90 with 15-degree marks, and Reset.
- **`crop.*` and `common.*` need no new strings for the most part** — `crop.title`, `crop.aspectRatio`,
  `crop.zoomHint`, `crop.ratio.free`, `crop.ratio.square`, `crop.width`, `crop.height`, `common.cancel`,
  `common.apply` and `common.reset` are already in all thirteen locale files, carried over with the
  catalogue. What is missing is the accessible naming the reference does without: the four icon-only controls
  and the miniature's crop button have no label in any of the thirteen. Five keys are added to all thirteen.

**In `gui` (frontend) — what the dialog writes:**

- **Apply writes `stores/crop.ts`**, which is what closes the seam slice 2 left open. The store, the URL, the
  enhance argument, the navbar, the miniature, the canvas and the run are all already wired to it, so this is
  one `setCrop` call and the application changes everywhere at once.
- **An unchanged framing clears the crop rather than storing an identity one**, matching the reference: a
  user who opens the dialog, looks, and applies is a user who has cancelled, and storing a framing that
  frames everything would re-key the picture and re-run the chain for the same pixels.
- **Applying a framing returns the canvas to fitted.** The zoom and pan are kept per image and are expressed
  against the photograph's shape, and a crop changes that shape — a pan of 0.6 across the old picture points
  somewhere unrelated in the new one. A **deliberate divergence**: the reference never touches its zoom on
  crop, which it gets away with because its zoom is one global value rather than a per-image one.

**In `gui` (frontend) — the way in:**

- **A crop button on the sidebar's miniature**, bottom-right, as screens 05 and 18 draw it and as the
  reference's `SidebarImage` places it. It is drawn with the miniature and absent without one, which is a
  **carve-out from the shell's standing rule** that controls acting on an image are disabled rather than
  hidden: the area it sits in is not a control strip but the miniature itself, and with no image that whole
  area is a sentence saying there is no preview. The mockup and the reference agree on this.

## Capabilities

### New Capabilities

- `gui-crop`: the Crop/Rotate dialog — how it is opened and dismissed, what photograph it edits, what each of
  its controls does, and what applying or cancelling leaves behind. A capability of its own rather than more
  of `gui-shell`, on the same grounds `gui-settings` is one: it is a surface with its own contract, opened
  from the frame rather than part of it.

### Modified Capabilities

- `gui-shell`: the sidebar's miniature gains the control that opens the Crop/Rotate dialog, and is the one
  image-acting control drawn only while there is an image.
- `gui-images`: an image whose framing changes is looked at fitted again, rather than at the magnification
  and position it carried under its previous shape.

## Impact

**Crates touched:** `gui`, and only its frontend — components, hooks, a store write, i18n keys and one
package. No Rust file in any crate changes, no Tauri command is added or altered, no URL grammar moves.
`opai` and `cli` are untouched.

**Why the logic in the front end is not logic that belongs in `opai`.** The question was answered in slice 2
(design.md D2 there) and this slice does not reopen it: what is added here is a dialog, its controls and the
state behind them, which is interface by construction. The one piece with any claim to being shared — what a
framing does to pixels — already exists in `crates/gui/src/images/crop.rs` and is not duplicated here. The
frontend composes a `CropInfo` and hands it over; it never touches a pixel.

**`rust-sak`:** nothing needed and nothing found wanting. This slice performs no hashing, no image
manipulation, no file access and no download; it produces seven numbers and stores them. The rotation
`rust-sak` gained in slice 1 is reached, as it has been since slice 2, through the rendition the dialog draws.

**Third-party crates introduced:** none. **Frontend packages introduced: one** — `react-advanced-cropper`
0.20.1, pinned to the version slice 1 measured. The alternative was a hand-rolled stencil: a drag-and-resize
box with eight handles, aspect-ratio constraints, an image transform stack and wheel-zoom over a rotated
canvas. The deciding factor, recorded when the decision was taken and unchanged since, is that the edge cases
in `useCropController` port with the library and would otherwise be re-derived from nothing.

**Parity divergences — five, each deliberate:**

- **Picking an aspect ratio re-fits against the image bounds, not inside the current box.** The reference
  fits the new ratio inside whatever box is already there, so switching ratios repeatedly only ever shrinks
  the selection and cannot grow it back. Settled during exploration and recorded in both earlier slices.
- **The swap button selects the paired ratio and keeps the lock.** The reference's swap transposes the box
  and drops to Free, which loses the constraint the user chose. The grid is built entirely in inverse pairs,
  so 16:9 has a 9:16 to swap to.
- **The ratio options are drawn as proportional boxes**, not as Material crop glyphs. The design's.
- **The dialog works from a bounded rendition at 3072px**, not the reference's `getImage(currentFile, 0)`.
  A 60-megapixel scan otherwise pays a full decode, encode, transfer and second decode to fill a widget
  under two thousand CSS pixels wide.
- **Applying a framing refits the canvas**, as above.

**Kept, though it reads like an oversight:** the quarter-turn button resets the fine rotation to zero, as the
reference does. A user who has leaned a picture 7 degrees and then turns it a quarter is squaring it up, not
asking for 97.

**Knowingly left open, carried forward rather than reopened:** face detection and export both take a crop in
the reference and neither exists in this application yet — the reference's `onApply` re-detects faces against
the new framing, and this one has no faces to re-detect. Both must pick the crop up when they land, along
with the fill-colour question slice 1 parked for export. Analytics is the same shape: the reference's
`onApply` emits `CropApplied` and `CropCancelled`, and this application has no analytics sink, so neither
event is raised.

**Not in scope:** persisting a framing across restarts, which matches the reference and the transform store;
cropping the drawer's strip, which stays uncropped by decision (slice 2, D12); and any second way in to the
dialog — the design draws one control and this adds one.
