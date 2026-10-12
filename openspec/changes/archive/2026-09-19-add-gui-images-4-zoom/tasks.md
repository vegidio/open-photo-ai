## 1. The transform and its arithmetic

- [x] 1.1 Move `ZOOM_MIN` and `ZOOM_MAX` out of `features/drawer/DrawerHeader.tsx` into
  `lib/constants.ts` beside `DRAWER_HEIGHT`, and add `ZOOM_WHEEL_STEP = 0.05` and
  `ZOOM_BUTTON_STEP = 0.5` (D5, D11) - verify `pnpm typecheck` passes and the header's own comment
  about "a second reader" is replaced by one saying who the readers now are.
- [x] 1.2 Add `features/preview/geometry.ts` with the pure `fit`, `constrain` and `imageFraction`
  helpers of D3 and D6 - verify a new `geometry.test.ts` covers a landscape and a portrait
  photograph in a pane of the other shape, a photograph smaller than its pane (which is enlarged,
  not left at its own size), a zero-sized container, and a position clamped at each edge.
- [x] 1.3 Add `stores/transform.ts` - the identity-keyed `Map`, the `viewport`, their writers and the
  `useCurrentTransform()` accessor returning the fitted default (D1) - verify `stores/transform.test.ts`
  covers that two identities keep separate transforms, that an unknown identity reads as fitted, that
  every write replaces the map rather than mutating it, and that scale is clamped to `ZOOM_MIN`/`ZOOM_MAX`.

## 2. The canvas

- [x] 2.1 Rewrite `Pane` in `features/preview/PreviewImage.tsx` to drop `object-contain` and draw the
  `<img>` at its measured fitted size under a `translate(...) scale(...)` transform about a top-left
  origin, measuring itself with `useResizeObserver` (D3, D4) - verify the existing
  `PreviewImage.test.tsx` still passes and a new case asserts the transform style for a known
  transform and measured size.
- [x] 2.2 Add the drag-to-pan handlers to `Pane`: pointer capture on `pointerdown`, a constrained
  position on `pointermove` coalesced into one `requestAnimationFrame` store write, release on
  `pointerup`, and a flush of the pending write on unmount (D8) - verify a test drives a synthetic
  drag and asserts one store write per frame and that the last position survives unmount.
- [x] 2.3 Add the native non-passive `wheel` listener to `Pane`, stepping the scale by
  `ZOOM_WHEEL_STEP`, clamping it, and storing the anchor as a fraction of the displayed image
  alongside the position the anchoring resolves to (D5, D6) - verify a test asserts `preventDefault`
  was called, that the scale clamps at both ends, that the point under the pointer keeps its position
  across the zoom, that the stored position is the one drawn rather than the one zoomed from, and
  that a gesture away from a pane moves nothing.
- [x] 2.4 Publish the viewport from the enhanced pane only, as fractions of the displayed image (D7) -
  verify a test asserts the whole-image viewport at scale 1, a proportionally smaller one when
  magnified, and that the original pane writes nothing.
- [x] 2.5 Make the split divider draggable: its position becomes `PreviewImage` state driving both the
  divider's `left` and the enhanced pane's `clip-path`, with pointer capture on the divider alone and
  `role="slider"` plus arrow keys (D9) - verify a test asserts both values move together, that a drag
  on a pane does not move the divider, that the arrow keys change `aria-valuenow`, and that the
  divider stays where the user put it when another image becomes current.
- [x] 2.6 Hold the canvas on the photograph it is drawing until the next one's pixels can be painted,
  in `hooks/useSettledFile.ts`, so the `<img>`'s box and its contents change on one render (D12) -
  verify a test holds a decode open and asserts the canvas still draws the previous photograph, that
  a file with no rendition settles at once, and that the last image chosen wins a race between two
  decodes.

## 3. The drawer's zoom control

- [x] 3.1 Wire the zoom slider in `features/drawer/DrawerHeader.tsx` to the current image's transform -
  reading it as well as setting it, anchored on the pane centre - and give its thumb the
  `drawer.zoom` name through the vendored slider's `thumbLabel`, plus the value bubble the thumb
  carries under the pointer and through a drag (D11) - verify a test asserts the thumb reports the
  current image's scale, that it follows a change of current image, that the bubble prints the
  rounded value with no trailing zero, and that it stays silent while no image is open.
- [x] 3.2 Turn the minus and plus glyphs into real `Button`s stepping by `ZOOM_BUTTON_STEP` and
  holding at each end of the range (D11) - verify a test covers a step in each direction, a step that
  would pass an end, and that both stay disabled while no image is open.

## 4. The sidebar

- [x] 4.1 Restructure the miniature so the `<img>` element's box is the photograph's box, inside a
  `relative` wrapper that hugs it (D10) - verify the existing `Sidebar.test.tsx` still passes and a
  case asserts the wrapper is present only while an image is open.
- [x] 4.2 Draw the viewport rectangle over the miniature from the published viewport (D10) - verify a
  test asserts it surrounds the whole miniature at scale 1, that it is inset for a magnified image,
  and that it is absent while no image is open.

## 5. Catalogues and checks

- [x] 5.1 Add `drawer.zoom`, `drawer.zoomOut`, `drawer.zoomIn`, `drawer.zoomScale` and
  `preview.divider` to all thirteen locale files under `i18n/locales/`, not only `en.json` - verify
  `i18n/parity.test.ts` passes.
- [x] 5.2 Run `pnpm typecheck`, `pnpm test` and Biome scoped to the files this change touched (not
  `pnpm check`, which reformats the repo) - verify all three are clean.
- [x] 5.3 Launch the application, open several images, and check by hand the four things no test in
  `jsdom` can see: the wheel anchoring on the pointer, a pan stopping at the photograph's edges, both
  panes moving as one in side-by-side and split, and the sidebar rectangle tracking the canvas.
