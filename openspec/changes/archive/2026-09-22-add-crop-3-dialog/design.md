## Context

See proposal.md — Why for the motivation. What shapes the approach:

**What slices 1 and 2 left.** `rust_sak::image::rotate` turns a picture clockwise for a positive angle onto
the canvas `ceil(w|cos t| + h|sin t|)` by `ceil(w|sin t| + h|cos t|)`. `crates/gui/src/images/crop.rs` holds
`Crop` — two flips, an `i32` count of millidegrees positive clockwise, and a rectangle in the *rotated*
picture's coordinate space — and `cropped`, which flips, rotates, cuts and derives an identity by hashing.
`serve.rs` parses `crop=<left>,<top>,<width>,<height>,<millidegrees>,<flips>` off the rendition URL and
reduces the source before it turns it; `enhance/run.rs` applies the crop to the loaded picture before the
chain runs. On the frontend, `ipc/crop.ts` holds `CropInfo`, `cropQuery` and `framedDimensions`;
`stores/crop.ts` holds `Map<identity, CropInfo>` with `setCrop`, registered as a file owner, **with no
writer**; `useCurrentRendition`, `PreviewImage`, `NavbarDimensions` and `useEnhancementRun` all already read
it. Everything downstream of `setCrop` works today and is exercised by crops the tests inject.

**What the widget is.** `react-advanced-cropper` 0.20.1, the version the reference pins and the version
slice 1 measured against: it reports coordinates in the **rotated** picture's space with the origin at the
expanded canvas's top-left, its `rotateSize` is term-for-term the formula `rotate` implements, and its
integer clamp *floors* that extent where `rotate` rounds outward — so a rectangle taken from the widget is
always strictly inside the picture Rust produces. Its imperative surface is a ref: `getCoordinates`,
`setCoordinates`, `getState`, `rotateImage`, `flipImage`, plus `onChange` and `onReady`.

**What the surface has to be built from.** `components/ui/dialog.tsx` (Radix), `components/ui/dialog-title-bar.tsx`
— written in `add-gui-7-settings-dialog` with a comment naming crop as one of its four callers — and
`lib/canvas.tsx`'s `useCanvasSurface`, written in `add-gui-canvas-background` with a comment naming this
dialog as its second reader and explaining why the branch is there rather than at each call site. Neither
needs changing — though the dialog ended up taking that helper's exported value rather than its branch, for
the reason D9 gives.

**What the reference does.** `useCropController` owns the whole modal: it fetches `getImage(currentFile, 0)`
— the *uncropped* original at full resolution — seeds the cropper from any saved crop in `onReady`, defers
ratio reshapes and swaps to a later frame because a cleared `aspectRatio` has to reach the stencil before
the box is set, mirrors the stencil into the W/H fields on every change, and on Apply normalises the angle,
reads the flips off the widget's own transform state, compares against an identity framing and either stores
a `CropInfo` or removes the entry. `SidebarImage` puts the crop button at `bottom-3 right-3` over the
miniature and mounts the modal beside it.

**What the design draws.** Screen 18: a surface inset 32px on every side over the canvas's own dotted
background, a 48px title bar reading Crop/Rotate with a close box, the cropper pane with a dimmed overlay
and a thirds grid, a control row of three 32px icon buttons then a slider labelled -90°/0°/90° then a
112px-wide Reset, and a 256px column holding a two-column aspect-ratio grid, a rule, `w` and `h` fields with
a swap between them, the zoom hint, and Cancel and Apply. Each ratio is drawn as a circular well holding a
rectangle **in that ratio's own proportions**, dashed for Free, with the one in force ringed in the accent
colour. Screen 05 puts a 32px crop button over the miniature's lower trailing corner.

## Goals / Non-Goals

**Goals:**

- A dialog whose every control does what screen 18 draws it doing, with nothing inert.
- The widget's coordinate space converted to the photograph's own at exactly one boundary.
- A dialog that can be opened and dismissed, or opened and applied unchanged, having changed nothing.
- The reference's hard-won effect ordering carried over rather than rediscovered.
- The series closed: `stores/crop.ts` gains the writer its documentation promises.

**Non-Goals:**

- Anything in Rust. This slice adds no command, no URL grammar and no image work; if a task here reaches
  for a `.rs` file, something has been mis-scoped.
- Face detection and export picking up the crop — neither exists yet (proposal — Knowingly left open).
- A second way in to the dialog: no menu item, no keyboard shortcut. The design draws one control.
- Persisting a framing across restarts.

## Slices

The crop feature, in order. Only the current slice is proposed; the rest are proposed when reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-crop-1-rust-sak` | `rotate` in `rust-sak`'s `image` module, released and pinned here | - | done |
| 2 | `add-crop-2-seam` | A crop reaching the pixels: composed in `gui`, carried in the `opai://` URL, applied to renditions and to the enhance input, reflected in the navbar dimensions | 1 | done |
| 3 | `add-crop-3-dialog` | The crop button on the sidebar miniature and the whole Crop/Rotate dialog, every control screen 18 draws | 2 | **current** |

This is the last slice. The seam landed ahead of the screen, which is what lets this one ship a dialog whose
Apply is a single store write against a path that has been exercised since slice 2.

### Decisions carried in from the earlier slices

Settled with the repository owner during slice 1's exploration, repeated through slice 2, and **acted on
here** — which is what they were recorded for.

- **The cropper widget is `react-advanced-cropper`**, accepting that its stylesheet has to be overridden
  into Tailwind 4. *Acted on: D1, D2.*
- **The dialog works from a bounded rendition**, with coordinates scaled at the boundary and the typed
  width/height fields authoritative in source pixels. *Acted on: D3, D4.* The bound itself was left open
  until now and is settled here at 3072 (D3).
- **Picking an aspect ratio re-fits against the image bounds** rather than inside the current box.
  *Acted on: D6.*
- **The swap button selects the paired ratio and keeps the lock.** *Acted on: D7.*
- **The quarter-turn button still resets the fine rotation to zero**, as the reference does. *Acted on: D5.*
- **The crop store is keyed by identity**, and the crop is carried in the URL rather than held server-side.
  Both already built; this slice is the writer.
- **Face detection and export take a crop in the reference and neither exists in this GUI yet.** Still open,
  and now the only thing the series leaves behind.

## Decisions

### D1 — `react-advanced-cropper` 0.20.1, pinned to the version slice 1 measured

Exact, not a caret range. Slice 1's D2 and D3 chose the sign convention and pinned the expanded-canvas
formula *against this version's behaviour*, measured in a scratch page; a minor release that changed either
would put every rotated crop slightly off with nothing failing. The reference pins the same way.

Its peer range is `react >=16.8.0` and it ships an ESM bundle, so React 19 and Vite need nothing special.
It is the one package this slice adds.

*Alternative considered:* a hand-rolled stencil — a drag-and-resize box with eight handles, aspect-ratio
constraints, an image transform stack and wheel zoom over a rotated canvas. Rejected when the decision was
taken, and the reason has only got stronger now that `useCropController` has been read line by line: the
deferrals in D8 are not obvious, were clearly found the hard way, and port with the library.

### D2 — The stylesheet is imported into a cascade layer, not overridden with `!`

`@import "react-advanced-cropper/dist/style.css" layer(components);` in `frontend/style.css`.

Tailwind 4 emits into real `@layer` blocks, and unlayered CSS outranks every layered rule regardless of
specificity. A plain import therefore puts the widget's own styles *above* every Tailwind utility, which is
why the reference needs `bg-transparent!`, `p-1.5!`, `text-transparent!` and `opacity-100!` to say anything
about the widget at all. Importing into `components` puts them where Tailwind's own component layer sits,
below utilities, so ordinary classes win and the four `!` suffixes are not needed.

The fallback, if a rule turns out to need the escape hatch anyway, is the reference's: the `!` suffix on that
one class. What is *not* acceptable is copying the stylesheet into this repository to edit it, which would
make a dependency's internals this project's to maintain.

*Alternative considered:* the reference's approach verbatim. It works — it is shipping — but it spreads the
"this library's CSS outranks mine" fact across every call site as a suffix nobody can explain locally.

### D3 — The dialog works from a rendition bounded at 3072, and one factor converts

The dialog asks for `renditionFor(file, CROP_DIALOG_BOUND)` — **with no crop**, because the surface frames
the whole photograph (gui-crop: "The surface frames the whole photograph, not the framing already in force").

3072 on the longest edge, derived the way `THUMBNAIL_BOUND` was derived rather than picked: the cropper pane
is the window less a 32px dialog inset, 32px of padding, the 256px settings column and its own inset — about
1550 CSS pixels wide on a 1920-wide window — so 3072 covers it at 2x density. One fixed constant rather than
a bound derived from `devicePixelRatio`, for the reason `THUMBNAIL_BOUND` gives: the bound is in the URL, so
a bound that moved with the display would re-request the photograph when the window was dragged to another
monitor.

**One factor, computed once per opening:** `factor = source.longestEdge / rendition.longestEdge`, read from
the `<img>` the widget loaded rather than assumed from the bound, because Rust never enlarges — a photograph
under 3072 is served unchanged and the factor is exactly 1, which is the common case and is exact. Widget
coordinates multiply by it on the way out; typed source dimensions divide by it on the way in. Nothing else
in the dialog knows the rendition is reduced at all.

*Alternative considered:* the reference's full-resolution original. A 9504x6336 scan is a full decode, a
JPEG encode, a transfer and a second decode — several seconds and hundreds of megabytes of decoded pixels in
the webview — every time the dialog opens, to fill a widget a fifth of that size. **Its one real advantage is
that `factor` is 1 always, so the typed fields are exact**; D4 is what that costs and how it is contained.

### D4 — The typed dimensions are the photograph's pixels, and the field is authoritative while focused

Displayed width is `round(widgetWidth * factor)`; a typed value `v` sets the widget to `v / factor`. Where
the factor is 1 this is the identity. Where it is not, a typed value can come back one or two pixels away —
typing 2560 into a 9504-wide scan (factor 3.09) sets the widget to 828.5, which rounds to a stencil the
widget reports back as 2559 or 2561.

Contained the way the reference contains the same class of problem: the field holds its own input while it
is focused and mirrors the stencil only when it is not, so a user typing is never fought. On blur it snaps to
what the framing actually is. A one-pixel snap on a 2560-pixel edge is a two-thousandth of the width; the
alternative that removes it entirely is D3's rejected one.

The minimum is `MIN_CROP_SIZE = 16`, the reference's, **applied in the widget's own space** rather than in
source pixels: 16 reduced pixels is at least 16 source pixels because the factor is never below 1, so the
simple spelling is also the conservative one.

### D5 — Turning: one absolute angle, and the widget's transform state is the truth

The controller holds `baseRotation` (the quarter turns, a multiple of 90) and `fineRotation` (the slider,
-90 to +90) and drives the widget by the *delta* from its current rotation, normalised to the shortest path
so that 0 and 360 produce no movement and a reset never spins several times round. That is the reference's
`applyRotation`, carried over whole; the slider's own drag passes `transitions: false` so tracking is smooth.

The quarter-turn action snaps the total to the next multiple of 90 and zeroes the fine rotation — the
reference's behaviour, kept deliberately (proposal — Kept, though it reads like an oversight).

**The flips are read off the widget rather than mirrored in React state.** `flipImage` is a toggle and the
widget's `transforms.flip` is what it acted on; a second boolean beside it is a second source of truth that
`onReset` would have to keep in step. The reference does exactly this, and its `onReset` reads the flip state
back to decide what to undo.

On Apply the angle is normalised to `[0, 360)` and multiplied by 1000 into the millidegrees `Crop` wants
(D1 of slice 2). The slider steps in whole degrees, so this is always a multiple of 1000 — the unit buys
room the dialog does not currently spend, which is what slice 2 chose it for.

### D6 — A ratio re-fits against the photograph, not inside the current box

Choosing a ratio sets the box to the largest rectangle of that ratio that fits **the rotated photograph's
extent**, centred on the box's current centre, then lets the widget clamp it inside the bounds.

The reference fits inside `coords`, so each choice can only shrink the selection: 16:9 then 3:2 then 16:9
lands smaller than 16:9 did the first time, and nothing gets it back but a reset. That is monotonically
destructive and is the divergence gui-crop records.

**Against the rotated extent, not `imageSize`.** The widget's `imageSize` is the *unrotated* picture, while
its coordinates live in the rotated space — so `imageSize` under-sizes a turned photograph, which is a
latent quirk in the reference's own `onReset` (it resets a turned picture to less than the whole of it).
The extent comes from the same `rotateSize` slice 1 measured. Task 1.2 confirms both halves against the
pinned version rather than taking this paragraph's word for it.

The reshape is deferred to the next animation frame, as the reference defers it, because on the render that
sets the ratio the new `aspectRatio` has not yet reached the stencil and the box would be re-constrained to
the previous one. The applied size is then pushed into the fields directly: the widget fires no `onChange`
for a ratio-driven reshape, so they would otherwise not update.

### D7 — Swap selects the inverse ratio and keeps the lock

The grid is ten options in five inverse pairs — Free and Square are their own inverses; 5:4/4:5, 4:3/3:4,
3:2/2:3, 16:9/9:16 are the other four. So the swap has somewhere to go: it transposes the box and selects
the paired key, leaving the lock on.

The reference transposes the box and drops to Free, which throws away the constraint the user chose and
leaves the grid showing Free next to a box that is exactly 9:16. The table is declared with each entry's
inverse, so the pairing is data rather than arithmetic on a float — `16/9` and `9/16` do not multiply to 1
in binary floating point, and a swap that derived its partner by inverting the number could not match a key.

Swapping under Free transposes the box and stays Free, which is the reference's behaviour for that case and
is what the user asked for.

### D8 — The controller owns the state; the components are layout

`features/crop/useCropController.ts` holds the rendition, the factor, the ratio, the two rotations, the
displayed dimensions and the deferred-work state, and hands the components a flat set of values and
handlers — the reference's arrangement, and for its stated reason: the effect ordering is the hard part and
it does not belong spread across five components.

Three deferrals are ported because each fixes a real ordering bug, and each is a `useEffect` on a piece of
pending state rather than work done inline:

1. **A ratio reshape** waits a frame for `aspectRatio` to reach the stencil (D6).
2. **A swap** waits for the *cleared* lock to reach the stencil before the transposed box is set — under D7
   the lock is not cleared but *changed*, and the same ordering applies for the same reason.
3. **A reset** waits for the cleared lock before the box is returned to the whole photograph.

The components are `CropRotate.tsx` (the surface), `ImageCropper.tsx` (the widget wrapper), `RotateControls.tsx`,
`AspectRatios.tsx` and `CropDimensions.tsx`, in `features/crop/` — flat files, as every other feature
directory in this frontend is, rather than the reference's directory-per-component with an `index.tsx`.

### D9 — What the dialog is made of, and what is not rebuilt

Radix `Dialog` and `DialogContent` with the near-fullscreen box the design draws — `inset-8` with the
centring transform overridden, the way `SettingsDialog` already overrides `max-w` and `sm:max-w`. The title
bar is `DialogTitleBar`, unchanged: it was put in `components/ui/` in `add-gui-7-settings-dialog` on the
explicit argument that crop, export, faces and About all draw exactly this bar.

**A click outside does not dismiss**, as the settings dialog refuses it and for the same reason: a stray
click is not an answer to a framing the user has been working on. Escape and the close box do dismiss, and
both go through Radix's `onOpenChange`, so there is one way out rather than two that could differ.

**The background behind the cropper is the dotted surface, unconditionally** - the mockup's, not
`useCanvasSurface`'s branch. This was the other way round when the slice was planned, on the argument that
`useCanvasSurface` was written with this dialog named as its second reader; verifying the built dialog by hand
settled it the other way. The canvas is a place to *look at* a photograph and the animated surface belongs
there; this is a work surface, where what is being judged is where an edge falls against the background behind
it, and a drifting field competes with the rectangle being placed. The helper's exported `DOTTED_SURFACE` is
what the dialog reads, so the two regions still share one definition of the dotted surface - what they no
longer share is the *choice*, which was the only thing the branch was ever protecting.

The trigger and the dialog are mounted together in a new `features/sidebar/SidebarMiniature.tsx` — the
miniature block lifted out of `Sidebar.tsx` with the button and the dialog beside it, which is what the
reference's `SidebarImage` is and is what keeps `Sidebar.tsx` from growing a `useState` that is nothing to do
with the column. The open state is a `useState` there; there is no draft to keep, so it needs no
`useSettingsDialog`-shaped hook.

### D10 — Apply writes one store, and two others follow from it

`onApply` reads the box, scales it by the factor, normalises the angle, reads the flips off the widget and
then does one of two things:

- **A framing that changes nothing** — no flips, zero rotation, and a rectangle covering the whole
  photograph — calls `forgetCrop(identity)`. The reference does the same, and slice 2's D3 gives the reason
  in the language of the seam: a no-op crop would mint a new identity for identical pixels and re-run a
  chain that had already been computed.
- **Anything else** calls `setCrop(identity, info)`.

Two consequences are not this slice's code at all, which is the point of the ordering:

- The canvas, the miniature, the navbar and its popover redraw, and the enhancement run restarts and
  releases the held result — all of it already wired in slice 2.
- **The canvas is returned to fitted**: `setTransform(identity, FITTED)`. This *is* one line here, because
  nothing else can know that a shape changed. `setTransform` rather than `forgetTransform`, which means "this
  photograph is gone" and is the file store's to call.

  **Only where the framing actually changed**, which is not decoration: `gui-images` refits an image *whose
  framing changes*, and `gui-crop` says applying a framing that changes nothing changes nothing about how the
  image is drawn. So the two branches above do not decide it — an unchanged framing applied to an image that
  carried one is a change, and one applied to an image that carried none is not. A user who was examining a
  detail at 4x, opened the dialog and applied without touching anything comes back to that detail.

Both branches then close the dialog. Cancel, Escape and the close box write nothing at all — there is no
draft to unwind, which is the same property `useSettingsDialog` spent a redesign to get.

### D11 — Constants, and where they live

`CROP_DIALOG_BOUND = 3072` (D3), `MIN_CROP_SIZE = 16` and `CROP_ZOOM_WHEEL_RATIO = 0.025` (the reference's
value for both) go in `lib/constants.ts` beside `THUMBNAIL_BOUND`, `ZOOM_WHEEL_STEP` and the rest — the file
that already holds every number two regions have to agree on, each with the arithmetic that produced it.

The ratio table is `features/crop/ratios.ts`, not `lib/`: it has one reader, and its shape (a key, a label or
a translation key, a value and an inverse key) is the aspect-ratio grid's own.

### D12 — i18n: five new keys, thirteen files

Already present in all thirteen locales, carried over with the catalogue: `crop.title`, `crop.aspectRatio`,
`crop.zoomHint`, `crop.ratio.free`, `crop.ratio.square`, `crop.width`, `crop.height`, `common.cancel`,
`common.apply`, `common.reset`, `common.close`. The numeric ratios are not strings in any language and are
not keyed, which is the reference's call and is correct.

Added: `crop.open`, `crop.rotate`, `crop.flipHorizontal`, `crop.flipVertical`, `crop.swap` — the accessible
names of the five icon-only controls. **The reference has none of them**: its `IconButton`s carry no label,
so a screen reader announces five unnamed buttons. That is an omission rather than a decision, and this
application has named every icon-only control it has shipped so far.

Added to every locale file, not only `en.json`, which the i18n parity test enforces. English text in the
twelve other files where no translation is supplied is what the catalogue already does for keys added since
it was carried over.

### D13 — What this is validated against

Platform-independent: no Rust, no session, no model, no execution provider. The one platform-shaped surface
the dialog touches is the rendition URL, built by `convertFileSrc` in the single place that owns that
difference, and it is the same call the canvas already makes.

Verified on macOS by hand — the suite cannot see any of it, because jsdom has no layout and the widget is
layout. Windows and Linux are covered by CI's build and test run, as every previous GUI slice has been. The
unit tests cover what is testable without layout: the ratio table's pairing, the scaling arithmetic, the
identity-framing comparison, the store write, and what the dialog renders.

## Risks / Trade-offs

- **A widget that will not load the photograph at all.** The cropper defaults `crossOrigin` to `true`, which
  sets `crossOrigin="anonymous"` on any image it decides is cross-origin - and `opai://localhost/...` is
  cross-origin to the page in both the dev server and the bundled app. `serve.rs` sends no
  `Access-Control-Allow-Origin`, so the load fails and the pane draws nothing, silently and only in the real
  webview. Found by hand, since jsdom loads no images at all. -> `crossOrigin` and `checkOrientation` are both
  turned off in `ImageCropper`, which makes the widget ask for the rendition exactly the way the canvas's own
  `<img>` has always asked for it. The second also removes a full extra transfer per opening, spent reading an
  EXIF tag off pixels Rust has already oriented.

- **The vendored widget's odd defaults are load-bearing, and reasoning about them from the mockup rather
  than from the reference costs two rounds.** The reference styles the stencil `overlayClassName:
  'text-transparent!'` - the surround is not dimmed at all - and that looks like an omission beside a mockup
  that draws `box-shadow: 0 0 0 9999px rgba(9,9,11,.55)`. It is not. The widget's surround is a 1000px
  shadow spreading to the nearest clipping ancestor, so any visible colour covers the whole pane rather than
  the photograph; with the rectangle covering the whole photograph on opening, the only thing it dims is the
  dotted surface, and the dialog reads as a black panel laid over it. Both attempts to keep the dim and
  confine it made things worse - clipping at the boundary left 447px of washed surface a side for a portrait
  photograph, and shaping the widget's box to the photograph drew it smaller than its pane and cut the drag
  handles, which overhang the rectangle's edge and need the `p-1.5` ring. -> `ImageCropper` now carries the
  reference's four stencil and geometry values exactly, with a comment saying what each one prevents, and
  the gui-crop delta records that the surround is undrawn rather than dimmed. The standing lesson is the one
  already written down for Go ports and applies to a vendored frontend component word for word: read the
  reference first, and when a measurement says it is wrong, suspect the port.

- **A widget's behaviour is assumed rather than measured, and the assumption is wrong.** D6 depends on
  `imageSize` being the unrotated picture; D5 depends on `transforms.flip` being the truth about flips; D4
  depends on `getCoordinates({ round: true })` reporting in the rotated space under a flip as well as under a
  rotation. → Task 1.2 measures all three against the pinned version in a scratch page, the way slice 1
  measured the sign and the canvas formula, before the controller is written. This is the project's
  established answer to a frontend library it cannot drive from a test.

- **jsdom cannot see the thing this slice is.** No layout means no drag, no stencil, no rotation, no zoom —
  the tests can assert that the dialog renders its controls and that the arithmetic and the store write are
  right, and nothing about whether the framing lands where the user put it. → The manual verification task is
  not a formality here; it is the only coverage the central behaviour gets, and it is written out as
  specific checks rather than "try the dialog". The measured facts from task 1.2 are recorded in the
  controller's comments so the next reader does not have to re-measure.

- **The typed dimension fields snap by a pixel on a large photograph.** → D4. Contained to the blur, absent
  entirely for any photograph under 3072, and the alternative is a full-resolution decode per opening.

- **The stylesheet layering may not hold.** If `layer(components)` does not sort the way D2 expects — a
  bundler that hoists the import, a rule of the widget's that carries its own `!important` — the widget will
  be styled wrong in a way no test sees. → The four overrides the reference needs are exactly the four places
  to look, they are checked by eye in the same task that adds the import, and the fallback is one `!` suffix
  per rule that needs it.

- **A dialog drawn over the canvas keeps the canvas drawing.** The preview underneath still holds its
  rendition and, with particles chosen, still runs its animation while the dialog is open. → The particle
  field is already documented as costing nothing while unwatched (gui-shell), and the dialog's own surface
  reuses that same component. Nothing here makes it worse; it is noted because a near-fullscreen dialog is
  the first surface that could have.

- **Apply refits the canvas, which a user who was examining a detail may not want.** → It is the honest
  answer for a photograph that changed shape, and the specs record the divergence from the reference
  explicitly rather than leaving it to be discovered. A user who was at 4x on a detail that survived the crop
  is two scroll notches from it.

- **The last slice of a series is where anything the series got wrong surfaces.** A disagreement between the
  widget's idea of the rotated canvas and Rust's would show up here as every rotated framing landing slightly
  off — two changes after the code that caused it, which is exactly what slice 1 warned about. → Slice 1
  measured and pinned both the sign and the formula for this reason, and slice 2 clamps totally, so the
  failure mode is at worst a one-pixel edge rather than an offset framing. The manual verification includes a
  rotated framing checked against the canvas it produces.

## Migration Plan

Nothing to migrate: no stored state, no URL grammar, no command signature. Reverting is removing
`features/crop/`, the miniature's button, the five i18n keys, three constants, one stylesheet import and one
package — and the application returns to a crop that works and cannot be set, which is exactly where slice 2
left it.

## Open Questions

None. The two that were open when this slice was planned — the bound the dialog works from, and what happens
to a magnified canvas when a framing is applied — were settled with the repository owner and are D3 and D10.
