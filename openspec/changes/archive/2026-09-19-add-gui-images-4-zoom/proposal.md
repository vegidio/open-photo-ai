## Why

The window can open photographs, switch between them and draw one of them - at exactly one size, the
size that fits. There is no way to look closer. The zoom slider the drawer draws is greyed out, the
split divider is painted at the middle of the canvas and does not move, and the sidebar's miniature is
a small copy of the same photograph with nothing on it to say which part of it the canvas is showing.
An application whose whole purpose is judging what a model did to an image cannot be judged at the one
scale at which every model looks fine.

This slice makes the canvas zoomable and pannable, and lights up the three controls that were left
drawn-but-dead for it.

## What Changes

- **Each open image is looked at at its own zoom and position.** Scale and offset are remembered per
  photograph, so switching to another image and back returns to the same crop of it rather than
  resetting to fit. A newly opened image starts fitted and centred.
- **The zoom slider becomes live**, from 1x to 8x as the reference's range, and the minus and plus
  icons either side of it become buttons stepping the current image by 0.5. Both stay unavailable
  while no image is open, which is what the frame already promises.
- **The mouse wheel and the trackpad pinch zoom the canvas**, anchored on the point under the pointer
  so that the detail being examined stays where it is rather than sliding out of the pane.
- **Dragging inside a pane pans the image**, and a photograph is never dragged away from the pane it
  is in: at 1x it stays centred, and above 1x it stops at its own edges.
- **Both panes move as one.** In side-by-side and in split, the original and the enhanced pane share
  one zoom and one position, because the comparison is only a comparison while the two show the same
  part of the photograph.
- **The split divider becomes draggable**, revealing more or less of the enhanced pane as it moves,
  and the two panes stay locked together underneath it.
- **The sidebar's miniature gains the viewport rectangle the design draws over it** - a white outline
  around the portion of the photograph the canvas is currently showing, which is how a user zoomed to
  8x knows where in the frame they are.
- **Switching images swaps the photograph in one step.** The canvas holds the image it is drawing
  until the next one's pixels can be painted, because it now sizes the `<img>` itself rather than
  leaving it to `object-contain` - without the hold, the outgoing photograph is drawn to the incoming
  one's shape for the length of a decode. The rest of the window moves at once.
- No keyboard zoom shortcuts, no double-click to zoom, and no crop button - see Impact.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-images`: gains how closely an open image is being looked at - that each one carries its own
  zoom and position, that a newly opened one is fitted, that the value is kept between 1x and 8x, and
  that returning to an image returns to the part of it that was being examined.
- `single-instance`: gains that a first launch asks for keyboard focus as well as for its window to
  be shown - one line in `window.rs`, unrelated to the zoom, recorded here because this is the change
  that carries it. See Impact.
- `gui-shell`: gains what the window does with that - the zoom control going live and what its slider
  and its two step buttons do, the wheel and pinch gesture over the canvas, dragging to pan, the two
  panes sharing one view, the split divider moving under the pointer, and the rectangle the sidebar's
  miniature draws over itself. Its existing requirement that the zoom control is unavailable with no
  image open is unchanged and stays true word for word.

## Impact

**Crates: `gui` only.** The zoom itself is the React frontend alone and crosses no IPC boundary: the
pixels being magnified are the ones slice 1's protocol already serves at the photograph's own size -
the canvas asks for no bound, deliberately, which is what makes an 8x zoom a magnification of real
pixels rather than of a canvas-sized rendition.

**Rust is edited all the same, for three things the zoom exposed rather than needed.** None of them
changes what any capability promises, and none was planned here:

- **A rendition cache** (`images::Renditions`, registered in `src/lib.rs`), so a thumbnail scrolling
  back into the drawer's strip is not a second decode of the same photograph. This application's
  cache rather than the webview's, because only WebView2 has one that sits in front of a custom
  scheme handler. Invisible to the specs: *An identity and a bound always denote the same pixels* is
  what makes it safe, and it is already recorded.
- **`resize` moves out of `opai` into `crates/gui/src/images.rs`**, the one crate that ever called it,
  and `gui` gains `image` as a runtime dependency with `default-features = false` so no codec is
  compiled on its account. Scaling a photograph to the size a window draws it at is a question about
  an interface rather than about reading a file, and the Go application draws the same line - its core
  library exports no resize either, and `cmd/gui` calls `imaging.Resize` itself. No requirement in
  `image-io` or anywhere else named the function; `tests/public_api.rs` did, and is updated.
- **`profile.dev` gains an optimisation level**, because at Cargo's default a 20-megapixel JPEG took
  4.35s to reach the drawer against 145ms released - roughly 30x - which meant a development build of
  this application was being compared against an optimised build of the one it replaces. Two entries:
  the dependencies carry the codecs and the resampler, and the workspace itself carries
  `rust_sak::image::encode_writer` monomorphised into `opai`.

`window.set_focus()` on reveal is a fourth, one line. It is the only one of the four with a user-visible
promise to make, so it gets a requirement - under `single-instance`, where raising a window already
lives, rather than under anything this slice is about.

Nothing in this slice is shared logic trapped in a front end. How far a webview has zoomed into a
photograph, where a divider sits and what rectangle a 144px panel draws over a miniature are facts
about a window; the terminal front end has no canvas.

**`rust-sak`: nothing is needed and nothing is missing.** This slice decodes nothing, resizes nothing
and reads no files.

**New npm packages: none.** The reference drives this with `react-zoom-pan-pinch`, and design.md, D2,
says why it is not adopted here: the reference disables the library's wheel, its double-click, its
padding, its velocity and its auto-alignment, then re-implements the wheel handler, the anchor
arithmetic and the bounds itself and drives the result imperatively through `setTransform`. What is
left that the library supplies is a pointer-drag handler and a wrapper `div`, and its own DOM would
have to be threaded through the `clip-path` split layout that slice 2 chose over `react-compare-slider`
for exactly this slice's sake.

**i18n: five new keys** - the zoom slider's accessible name and the two step buttons', the slider's
value bubble, and the split divider's - added to all thirteen locales. `sidebar.zoomCropAlt` and
`common.previewAlt` are already in the catalogue.

**Deliberate divergences from the Go application**, each argued in design.md:

- **One pane publishes the viewport, not both.** The reference lets each pane write the single
  viewport slot on every animation frame of a pan and then compares the value before storing it to
  undo the resulting re-render storm. Both panes are always the same size here, so the enhanced pane -
  the one present in all three comparisons - is the only writer, and the comparison it existed to
  justify is not needed.
- **A value bubble on the slider, which the design draws nowhere.** The reference's MUI slider shows
  `2.4x` while the thumb is held, and this does the same. The design's own plates draw no bubble;
  they also draw the zoom slider greyed out, so they say nothing about a control in use. This is the
  only slider in the application whose value is printed nowhere else, and the vendored shadcn slider
  takes the formatter as an opt-in prop, so Settings' four export-quality rows are untouched. D11
  carries the full argument, including why design.md's Non-Goals originally said the opposite.

**Kept as parity, and worth naming because it surprises:** any wheel event over the canvas zooms,
including a two-finger scroll, and each notch moves the scale by a fixed 0.05 whatever its magnitude.
That is the reference's `ZOOM_WHEEL_STEP` behaviour, and design.md R2 records it as observed rather
than assumed.

**Not in this slice: the crop button** the design draws in the corner of the sidebar's miniature. It
opens the near-fullscreen Crop / Rotate window of screen 18 - its own dialog, its own aspect-ratio
grid, its own rotation control and a crop the enhancement pipeline has to honour. It is its own change.
It is drawn nowhere today and stays undrawn, rather than being added as a disabled button, because
unlike the strip's three-dot glyph nothing in the miniature's layout depends on it being there.

**One new crate dependency**, and it belongs to the resize move above rather than to the zoom:
`gui` takes `image` at the version `opai` already pins, with `default-features = false`, so
`DynamicImage` is one type across the two crates and no codec is compiled on `gui`'s account. It was
already a dev dependency there for the fixtures `src/images.rs` builds.

**Estimated task count: 16**, against the 13 first written: the settled swap above and its tests were
not foreseen, and neither was the value bubble.

This is slice 4, the last, of the four-slice `add-gui-images-*` series; design.md carries the whole
list.
