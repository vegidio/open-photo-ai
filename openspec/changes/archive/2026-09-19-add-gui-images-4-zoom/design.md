## Context

See proposal.md - Why. What shapes this slice is that the three controls it lights up were each left
drawn and dead by an earlier slice, with a comment saying what would wake them, and that the pixels
they magnify are already the right pixels.

`gui` has, and this slice adds nothing to: the `opai` URI scheme serving an admitted image's pixels,
and `renditionUrl(identity)` with no bound - which slice 2's D6 chose deliberately *for this slice*,
so that 8x is a magnification of the photograph rather than of a canvas-sized rendition. Nothing the
zoom itself needs is on the Rust side; what Rust did gain in this change is listed under Crates and
argued in proposal.md - Impact.

The frontend has four things waiting:

- `features/preview/PreviewImage.tsx` draws each pane as an `<img>` letterboxed by `object-contain`,
  and the split as a fixed `clip-path: inset(0 0 0 50%)` with the design's 2px rule and circular grip
  over the seam. Its own comment records why that is a clip rather than `react-compare-slider`: "the
  library moves its two children independently, and a divider that has to stay put across two
  transformed panes is not a thing it knows about."
- `features/drawer/DrawerHeader.tsx` holds `ZOOM_MIN = 1` and `ZOOM_MAX = 8` with a comment saying
  "they move up when the slice that wires the wheel zoom gives them a second reader", a `<Slider
  disabled value={[ZOOM_MIN]}>`, and a `Minus` and a `Plus` drawn as bare glyphs either side of it.
- `features/sidebar/Sidebar.tsx` draws the miniature from the canvas's own URL and says "The viewport
  rectangle the design draws over it needs the canvas's transform, which is a later slice."
- `stores/drawer.ts` holds `open` and deliberately not `zoom`, leaving where the zoom lands to "the
  slice that builds that transform" - this one. D1 answers it.

`hooks/useResizeObserver.ts` was ported from the reference in slice 3 and is what every measurement
below goes through.

The reference implementation does all of this in one component, `ZoomImage`, over
`react-zoom-pan-pinch`, with the per-image transform and the published viewport in its `useImageStore`
and the slider in `DrawerZoom`. Its comments are the record of what it measured - the pan write storm,
the stale-measurement bug after a sidebar toggle, the two panes fighting over one viewport slot - and
D2, D5, D6 and D7 are where this design keeps those conclusions.

## Slices

The four slices that take the application from an empty canvas to an open, zoomable image. This is a
new series; the `add-gui-1-foundation` .. `add-gui-7-settings-dialog` series closed out on the Settings
pair and is not continued here.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-images-1-ipc` | `opai::image::resize`; the picker command and the describe-paths command; the image record; the identity-addressed rendition protocol and the opened-file registry behind it. No UI | - | **DONE** |
| 2 | `add-gui-images-2-canvas` | Browse images and drag-and-drop wired to slice 1; the file store; the canvas drawing the current image in all three preview modes; the navbar's filename and Dimensions; the sidebar miniature | 1 | **DONE** |
| 3 | `add-gui-images-3-drawer` | Screens 08 and 08b - the thumbnail strip, its windowing and deferred decode, the per-item checkbox and name bar, Select all, the current-file outline, switching images, and the fold toggle made live | 2 | **DONE** |
| 4 | `add-gui-images-4-zoom` | The per-image transform shared across panes, the drawer's zoom slider and the wheel/pinch zoom, split mode's draggable handle, and the viewport rectangle drawn over the sidebar miniature | 3 | **CURRENT** |

**Slice 4 is this change, and it closes the series.** It answers the question slice 3's design left
open - where the zoom lives - in D1.

**Crates.** `gui`, and within it the React frontend for everything this slice is *about*: see
proposal.md - Impact for why none of that is shared logic the terminal front end could want. Rust is
edited too, for three things that are not the zoom and are listed in proposal.md - Impact rather than
designed here: the rendition cache behind the protocol handler, the downscale moving out of `opai`
into the one crate that calls it, and the dev profile that makes a development build's decode times
comparable to the Go application's.

**rust-sak: nothing used directly, nothing missing.** This slice decodes, resizes and hashes nothing.
The photograph being magnified is the response the protocol handler slice 1 built already served,
which reaches `rust-sak` through `opai` exactly as it did.

**New crates: none. New npm packages: none** - D2.

**Platforms.** Written for all three, validated on macOS. Two things are platform-shaped and both are
named below: how a webview reports a trackpad pinch (D5, R2) and whether `setPointerCapture` behaves
identically across WebKit, WebView2 and WebKitGTK (R4).

**Execution providers: none are exercised.** Nothing here builds a session or runs a model.

## Goals / Non-Goals

**Goals:**

- One arithmetic for "where is this photograph drawn", used by the pane that draws it, the bounds that
  hold it in, the anchor the wheel zooms about and the rectangle the sidebar draws - so the four
  cannot come to disagree, which is the bug the reference's own comment records paying for once.
- A pan that costs one store write per frame rather than one per pointer event, from the first
  version - the reference tuned this after shipping it, and its measurement is the reason.
- Every control the design draws around the canvas made live, so that this slice leaves nothing in the
  frame drawn-and-dead for a later one.

**Non-Goals:**

- ~~**A zoom readout.**~~ **Reversed during implementation, and D11 now carries it.** The argument
  here was that the design draws no bubble and that adding one means extending the vendored
  `components/ui/slider.tsx` that Settings' four export-quality rows also render. Both halves are
  true and neither turned out to decide it: the extension is an opt-in prop those four rows do not
  pass, and the zoom is the one slider in the application that prints its value nowhere else - a
  thumb somewhere along a 1-to-8 track, where the export rows each have a value span beside them.
- **Keyboard zoom shortcuts and double-click to zoom.** The reference has neither - it passes
  `doubleClick: { disabled: true }` explicitly - and the design draws no shortcut anywhere.
- **The crop button** the design draws in the miniature's corner (proposal.md, Impact). It opens
  screen 18, which is its own change.
- **Anything about the enhanced pane being genuinely different from the original.** Both panes still
  point at one URL, as slice 2 left them; this slice makes them move together, which is what makes
  that difference legible when a pipeline finally produces one.
- **Removing a transform when an image is closed.** Nothing closes an image yet - slice 3 recorded the
  same thing about the selection - so the map only ever grows within a session, and D1 sizes it.

## Decisions

### D1. The transform lives in its own store, keyed by identity, holding the map and the viewport

`stores/transform.ts`: `transforms: Map<string, ImageTransform>` and `viewport?: ImageViewport`, with
`setTransform(identity, transform)`, `setViewport(viewport)` and a `useCurrentTransform()` accessor
returning the fitted default for an image that has none.

**Its own store, not the drawer's.** `stores/drawer.ts` holds `open` and its comment already refused
the reference's pairing of `open` with `zoom`: the two sit in one header, which is the whole of what
they have in common. The zoom is a property of how a photograph is being looked at and outlives the
drawer being folded; whether a strip is showing is not a property of any photograph.

**Not the file store either.** That store holds what is open, which one is current and which are
picked - facts about the *set* of files. A transform is a fact about one photograph, written sixty
times a second during a drag, and putting it there would re-run every drawer item's selector on every
frame of a pan.

**Keyed by identity, where slice 3 keyed the selection by path.** The two are keyed differently on
purpose, and `ipc/images.ts` already says which way this one goes: identity is "the key everything
about this photograph hangs off: the rendition URL, and later the per-image transform". The argument
that made the selection path-keyed - identity is optional, so a file whose bytes could not be read
would have no key - does not apply here, because such a file has no rendition URL either and therefore
cannot be drawn at all, let alone magnified. The transform is keyed the same way the pixels are.

That does mean two copies of one photograph in different folders share one view. They are two rows in
the strip and two independently pickable files, and one thing to look at; the reference collapses them
the same way, keying on `file.Hash`.

**The map is replaced on write, not mutated.** Slice 3's R3 is the standing rule here and the reason
`immer` is absent where the reference has it. The cost is one `new Map(...)` per frame of a pan: a
few hundred entries copied sixty times a second, which is thousands of pointer-sized copies per second
and is not the expensive part of anything. The alternative - mutating in place and relying on each
subscriber's selector reading `transforms.get(id)` and seeing a fresh *value* object - happens to work
and is exactly the reasoning slice 3 called "the quietest bug this slice can ship". It is not worth
saving a Map copy to have two rules about mutation in one codebase.

### D2. No `react-zoom-pan-pinch`: the transform is a CSS transform and two pointer handlers

The reference's `ZoomImage` mounts `TransformWrapper` and then turns almost all of it off -
`wheel: { disabled: true }`, `doubleClick: { disabled: true }`, `disablePadding`,
`panning: { velocityDisabled: true }`, `autoAlignment: { animationTime: 0 }` - and re-implements the
wheel listener, the anchor arithmetic and the bounds itself, driving the result imperatively through
`ref.setTransform(...)` from an effect. What it still gets from the library is a pointer-drag handler
and two wrapper `div`s.

Against that: its DOM would have to be threaded through the `clip-path` layout slice 2 chose over
`react-compare-slider`, the imperative `setTransform` bridge is a second source of truth that the
effect exists to keep in step with the store, and D3's arithmetic is the same twenty lines whether a
library holds it or not.

*Alternative: adopt it and use it properly* - let it own the wheel, the bounds and the state. It is the
honest version of taking the dependency, and it is not available: `limitToBounds` centres a small image
but does not know that a *sibling* pane must be constrained identically, the wheel anchor has to be
expressed as a fraction of the image so the sibling can apply it to its own container (D6), and the
library's state is per-instance where this slice's state is per-photograph.

*Alternative: `panzoom` or a smaller equivalent.* Same trade at a smaller size: what is actually needed
is `translate` plus `scale` and a clamp.

### D3. One function computes where the photograph is drawn, and everything reads it

A pure `fit(container, image)` and a pure `constrain(position, scaledSize, containerSize)`, in
`features/preview/geometry.ts` beside the pane that uses them, with their own tests.

```
fit       = min(containerW / imageW, containerH / imageH)       -> the drawn size at scale 1
constrain = scaled <= container ? (container - scaled) / 2      -> centred
                                : clamp(position, container - scaled, 0)
```

The pane draws the `<img>` at the fitted size, positioned at the pane's top-left, and moves it with
`transform: translate(Xpx, Ypx) scale(s)` about a `top left` origin - which is the coordinate system
the two functions above are written in and the reference's as well. `object-contain` goes away: it
centres the photograph by a rule the pane cannot read back, and every number below is derived from
knowing exactly where the photograph's top-left corner is.

**`fit` does not clamp to 1, so a small photograph is enlarged to fill the pane.** This settles
something slice 2 left incidental: `max-w-full max-h-full object-contain` never enlarges, so a 200px
photograph was drawn 200px wide in a 1000px canvas. The design's own plate arithmetic is
`Math.min(availW / f.w, PANE_H / f.h)` with no clamp, the reference's `useResizeObserver` callback is
the same expression, and it is what makes the slider mean something: 1x is "fitted" for every
photograph, rather than a magnification whose relationship to the pane depends on a number the user
cannot see. A user who wants the 200px photograph at its own size is the case this loses, and it is
not a case the reference, the design or the spec has ever offered.

*Alternative: keep `object-contain` and apply the transform over it.* The browser's centring then
becomes an unmeasured offset inside every calculation - the bounds, the wheel anchor and the published
viewport would each need it added back, computed from the same numbers, in three places.

### D4. Each pane measures itself, and re-measures on every resize

`useResizeObserver` (slice 3's port) on the pane's own element, storing the fitted size in that pane's
state.

Measured on every resize rather than once per image, which is the reference's own correction and the
comment it left: "measuring once left all three working off stale values after a window resize or a
sidebar toggle". The three are the wheel anchor, the bounds and the published viewport, and all three
are wrong together, which is what makes the bug hard to see - the image stays where it is and only
stops obeying its edges.

`jsdom` has no `ResizeObserver`, and the hook already guards for that, so a pane in a test measures
once at mount and then never - which is the behaviour the tests want anyway.

### D5. The wheel listener is native and non-passive, and it is per pane

`container.addEventListener("wheel", onWheel, { passive: false })` in an effect, because React's
synthetic `onWheel` is registered passive and `preventDefault()` on it is a silent no-op. This is the
reference's comment and it is still true.

**Per pane rather than once on the canvas**, because the handler needs the pane's own rectangle to turn
a client coordinate into a point of the photograph, and in side-by-side the pointer is over one
particular half.

**Every wheel event zooms, at a fixed 0.05 per event.** This is parity, and it surprises: a two-finger
scroll on a trackpad zooms rather than scrolling, and a fast flick moves the scale exactly as far per
event as a slow one. It is the reference's `ZOOM_WHEEL_STEP`, verified in its source rather than
assumed, and the same behaviour the design's crop dialog describes in words - "You can use the mouse
wheel or the trackpad pinch to zoom in and out the image". R2 is what the alternative would cost.

### D6. The wheel's anchor is a fraction of the photograph, not a pixel offset

The handler stores `anchor: { x, y }` as the position within the *displayed image* as a fraction in
[0, 1], clamped so a pointer in the letterbox margin resolves to the nearest edge; the pane applies it
as `screen = position + anchor * prevScaledSize`, then `newPosition = screen - anchor * newScaledSize`,
then constrains.

Fractions rather than pixels because the sibling pane has to apply the same anchor to its own
container. In side-by-side the two panes are the same size, so pixels would work today - and they stop
working the moment the panes differ, which is one layout change away. The reference chose fractions for
exactly this reason and says so.

An absent anchor means the centre of the pane, which is what the slider and the step buttons produce:
they are not pointed at any part of the photograph, so the thing to hold still is the middle. That is
one branch, not two code paths - `anchor ?? centreOfPane` - and it is what the spec means by "about the
middle of the pane".

**The handler stores the position the anchoring resolves to, not the one the photograph is standing
at.** Both, in other words: the anchor for a sibling pane to apply to its own container, and the
resolved position for the store to hand back. This was got wrong first and is worth the sentence.
Storing the pre-zoom position draws correctly - the pane resolves the anchor as it paints, so what is
on screen is right either way - but the store is what a photograph is handed back from when it
becomes current again, and one trailing the screen by a notch returns the user to a crop
`anchor * fitted * ZOOM_WHEEL_STEP` away from the one they left. That is 43px of a 1000px pane at the
edge of a photograph: small enough to survive every test that seeded the store directly, and large
enough to see. The handler runs the same `resolve` the pane does, off the same three values, so the
position stored and the position drawn are one number computed once rather than two that agree.

### D7. The enhanced pane publishes the viewport; the original pane does not

One writer for `viewport`, and it is the enhanced pane because it is the only one drawn in all three
comparisons.

The reference has both panes write it on every frame of a pan, and then compares the value before
storing it to undo the damage - its `sameViewport` exists because "two `ZoomImage` panes share one
transform key and both recompute this single slot on every animation frame of a pan, each writing a
fresh object". Both panes are always the same size here - in side-by-side each takes half, and in split
the enhanced pane is `inset-0.5` over an original that fills the same box - so the second writer is
producing a value identical to the first's. Removing the writer removes the reason for the comparison.

*Alternative: compute it in `PreviewImage` instead of in a pane.* It needs the measured pane size and
the fitted size, which are the pane's own state; lifting them up means the parent re-rendering on every
pane resize to hold numbers only the pane uses.

### D8. Panning is pointer capture plus one store write per animation frame

`pointerdown` on the pane calls `setPointerCapture` and records the pointer's position and the
transform at that moment; `pointermove` computes the new position, constrains it, and coalesces the
store write into a `requestAnimationFrame`; `pointerup` releases. The pending write is flushed on
unmount rather than cancelled.

The coalescing is the reference's, and so is the reason - without it every pointer frame re-renders
both panes, re-runs the transform effect and republishes the viewport, "a sustained ~60Hz cascade
through the tree for the length of a drag". The flush on unmount is its own later correction, and its
comment is the record of the gap: "the coalesced write is the only one that ever reaches the store, so
dropping it loses up to a frame of panning". Both arrive here in the first version.

`setPointerCapture` rather than document-level listeners, because a drag that leaves the pane - and a
drag that leaves the *window* - must keep delivering moves to the element that started it, and the
capture is also what makes the release land in the right place. It is the same reasoning that put the
drawer's click-away on `pointerdown` rather than `click`: where the press went down is the honest
answer to what the gesture is about.

### D9. The split divider is a `role="slider"`, dragged by the divider alone

Its position is a percentage in `PreviewImage`'s own state, driving both the divider's `left` and the
enhanced pane's `clip-path: inset(0 0 0 X%)` - the two numbers slice 2 wrote as a matched pair of
literals, which is exactly why they become one variable now.

**The divider is the only draggable thing**, which is the reference's `onlyHandleDraggable={true}`, and
here it is not a preference: the panes themselves are the pan surface from D8, so a drag started on a
pane and a drag started on the divider have to mean different things.

**State in `PreviewImage`, not in a store.** Where the divider sits is not a property of a photograph -
the spec says so - and it is not a standing preference either: the reference does not persist it, and
`stores/preview.ts` writes into the Wails app's own `app-storage` key, which held exactly one field.
Adding a second would be this application writing a key the application it is replacing does not read.
Switching images leaves the divider alone because nothing remounts `PreviewImage`; switching
comparisons away and back resets it to the middle, which is incidental in the reference for the same
reason and is not worth a mechanism to preserve.

**`role="slider"` with `aria-valuenow` and arrow keys**, rather than a `<div>` with a pointer handler
and an `aria-label` that names nothing. The reference's handle is reachable by no route but the mouse;
this codebase has taken the other answer at every turn - slice 3's D6 made a thumbnail a real button
rather than suppress two lint rules, and `components/ui/slider.tsx` grew a `thumbLabel` because Radix
puts the role on the thumb. A keydown handler moving the divider by 2% is five lines and makes the
third comparison operable.

### D10. The sidebar's miniature is restructured so a percentage means something

The `<img>` loses `h-full` and keeps `max-h-full max-w-full`, so the element's own box is the
photograph's box, and it is wrapped in a `relative` element that hugs it; the rectangle is an absolutely
positioned child with `left/top/width/height` in percent, straight from the published viewport.

Today's `h-full max-w-full object-contain` makes the element 144px tall and up to 256px wide *whatever
the photograph's shape*, with the browser letterboxing inside it. A 3000x1000 photograph is drawn
256x85 inside a 256x144 element, so a rectangle positioned in percentages of the element would be
stretched over dead space by 40% of the panel's height. The reference's miniature is
`max-h-36 max-w-full` for this reason, and the design's own markup puts the rectangle in a box that
hugs the image.

The design draws the rectangle at a fixed inset on every screen, including the ones at 1x. That is a
static mockup rather than a rule: the spec says the outline surrounds the whole miniature when the
photograph is drawn whole, which is the same answer the arithmetic gives and the same one the reference
renders.

### D11. Five new catalogue keys, and the slider carries a value bubble

`drawer.zoom` for the slider's thumb - which is `thumbLabel`, not `aria-label`, because Radix puts
`role="slider"` on the thumb and `components/ui/slider.tsx` grew that prop for precisely this - plus
`drawer.zoomOut` and `drawer.zoomIn` for the two step buttons, `drawer.zoomScale` for the bubble
below, and `preview.divider` for D9's handle.

**The bubble, which the Non-Goals above ruled out and this reverses.** The vendored slider grows a
`formatValue` prop and prints the value over the thumb while the pointer rests on it and for the
length of a drag. What changed the answer is that the cost named in the Non-Goal is not paid by
anyone who does not ask: `formatValue` is opt-in, and Settings' four export-quality rows do not pass
it and render exactly as before. What is left is the case for it, which stands on its own - this is
the only slider in the application whose value is printed nowhere else. The export rows each have a
value span beside them; the zoom has a thumb somewhere along a 1-to-8 track, and a user dragging it
has no way to land on 2x deliberately.

Rounded to the slider's own step and then trimmed through `Number`, so a scale that reaches the
control as `3.4500000000000006` reads `3.5x` and a whole magnification reads `4x` rather than `4.0x`.
That trim is a deliberate departure from the reference, which formats every value with `toFixed(1)`.

The minus and plus become real `Button`s where the design draws bare glyphs. That is the same call the
fold toggle got in slice 3: the mockup draws *every* control as a styled `<span>`, including the fold
toggle we shipped as a ghost button, so "the mockup drew no button chrome" is not evidence about
interactivity. The reference makes both live, stepping by 0.5.

`sidebar.zoomCropAlt` and `common.previewAlt` are already in the catalogue and are unchanged.

### D12. The canvas draws the image whose pixels are ready, not the one that is current

`hooks/useSettledFile.ts`: the current record, held back until `new Image().decode()` resolves for its
rendition, and everything in `PreviewImage` keyed off that one record rather than off the store.

This is D3's bill arriving. Dropping `object-contain` means the `<img>`'s box is a number this code
computes - `fitInside` against the pane, from the *record's* dimensions - and the record changes the
instant the strip is clicked, while the element goes on showing the previous photograph until the new
rendition has decoded. Between those two moments the old photograph is drawn into the new one's box:
a landscape squeezed into a portrait for as long as the decode takes, which on a first visit to a file
is long enough to read as the image distorting before it changes. `object-contain` never had this
problem, because the browser was doing the fitting against the pixels rather than against the record.

Holding the record back makes the two halves one event: the source, the fitted size and the identity
the transform is stored under all change on the same render.

**Only the canvas waits.** The navbar, the sidebar and the strip's own outline key off
`useCurrentFile` and move at once, because they are the acknowledgement that the click landed - a
strip that took a beat to mark what had been chosen would be a worse trade than the one this fixes.

**It mounts settled**, on whatever is current: there is no photograph on screen at that point for the
swap to be atomic with, and waiting would only delay the first image. `decode()` is absent in jsdom
and in any webview old enough to lack it, and the feature test there settles immediately - which is
exactly the behaviour this replaced.

## Risks / Trade-offs

**R1. The canvas draws the photograph at its own dimensions, and now magnifies it to 8x.** A 100 MP
photograph at 8x is a composited layer the size of a small wall. → The `<img>` is scaled by a CSS
transform rather than resized, so the compositor holds one decoded bitmap and a matrix; what grows is
the rasterised tile area within the pane, which is bounded by the pane. This is the arrangement the
reference ships and the reason slice 2's D6 accepted the full-size rendition in the first place. Its
R2 - a bounded rendition if the full-size one proves too much - remains the escape hatch and remains
unexercised.

**R2. Every wheel event zooms, so a trackpad two-finger scroll zooms.** A user expecting to scroll
gets a magnification, and 0.05 per event means a hard flick barely moves. → Parity, deliberately, and
named in proposal.md. The alternative is to read `event.ctrlKey` - which is how browsers report a
trackpad pinch - and treat a plain scroll as a pan, which is better behaviour and a divergence from the
reference that nobody has asked for. It is one branch in one handler if it is ever wanted, and the
spec's scenarios do not distinguish the two gestures.

**R3. The `<img>` is now sized and positioned by measured numbers rather than by CSS.** A pane that
measures 0 - mounted hidden, or measured before layout - draws a photograph of zero size. → `fit`
returns zero for a zero container and the pane renders the `<img>` with no explicit size until it has a
measurement, which is what it does today. The one environment where this is permanent is `jsdom`,
where D4's guard means no observer ever fires; the tests therefore assert the transform the store holds
and the arithmetic in `geometry.ts`, not pixel sizes read back from a layout that does not exist.

**R4. `setPointerCapture` across three webviews.** WebKit, WebView2 and WebKitGTK are three
implementations of one specification, and pointer capture is the part of it most likely to differ,
particularly for a drag that leaves the window. → The failure is a drag that stops following the
pointer, not a wrong transform: every write is constrained before it is stored, so a lost pointer
leaves the image somewhere legal. Validated on macOS only, as every GUI slice in this series has
recorded.

**R5. Two panes, one transform, and a store write from whichever one the pointer is in.** A pan in the
original pane and a simultaneous wheel in the enhanced one would interleave. → They cannot: there is
one pointer, the pan holds its capture for the length of the drag, and both handlers read the store's
current value rather than a value captured at mount.

**R6. The map of transforms only grows.** Nothing closes an image, so a session that opens five
hundred files and magnifies each one holds five hundred entries, and each pan copies the map. → Each
entry is four numbers; five hundred of them is a few tens of kilobytes, and the copy is five hundred
pointer writes against a frame budget of sixteen milliseconds. The slice that closes an image deletes
the entry, as the reference's `removeImageTransform` does - it is one line in the same store, and it is
named here so it is not forgotten.
