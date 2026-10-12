## 1. The widget

- [x] 1.1 Add `react-advanced-cropper` at exactly `0.20.1` to `crates/gui/package.json` (design.md D1 — the
      version slice 1 measured its sign convention and canvas formula against, pinned exact rather than by
      caret), and import its stylesheet into `frontend/style.css` as
      `@import "react-advanced-cropper/dist/style.css" layer(components);` (D2). Add `CROP_DIALOG_BOUND = 3072`,
      `MIN_CROP_SIZE = 16` and `CROP_ZOOM_WHEEL_RATIO = 0.025` to `frontend/lib/constants.ts` with the
      arithmetic D3 gives for the first, beside the constants that already carry theirs. Verify `pnpm install`
      resolves 0.20.1 with no peer warning against React 19, that `pnpm build` emits the widget's CSS, and
      that a utility class on the cropper's own element wins over the library's rule without an `!` suffix -
      which is the whole of what the layering buys, and the four places the reference needs `!`
      (`bg-transparent`, `p-1.5`, the overlay's colour, the grid's opacity) are where to check it.
- [x] 1.2 Measure the pinned widget's imperative surface in a scratch vite page driven headlessly - no
      Playwright here, a `chrome --headless --dump-dom --virtual-time-budget` page as slice 1 used - and
      record the answers as comments in `features/crop/useCropController.ts` when it is written. Four
      questions, each load-bearing for a decision: whether `getState().imageSize` is the *unrotated*
      picture while `getCoordinates()` reports in the rotated space (D6 depends on it, and the reference's
      `onReset` looks wrong if it holds); what `rotateSize` answers for a known size and angle, against slice
      1's measurement; whether `getCoordinates({ round: true })` still reports in the rotated space when a
      flip is also applied (D4); and whether `transforms.flip` is the truth about the flips after
      `flipImage(true, false)` twice (D5). Verify each answer is written down with the number it came from,
      not paraphrased.

## 2. The controller

- [x] 2.1 Add `features/crop/useCropController.ts` with the source of pixels and the conversion: the
      uncropped rendition at `CROP_DIALOG_BOUND` through `renditionFor(file, CROP_DIALOG_BOUND)` - no crop,
      because the surface frames the whole photograph - loaded when the dialog opens for a file and cancelled
      if it closes first, and the single `factor = source.longestEdge / rendition.longestEdge` read from the
      loaded image rather than assumed from the bound (D3). Seed from a recorded framing on the widget's
      `onReady` - flips, turn and rectangle, the rectangle divided by the factor - and split the seeded angle
      into the quarter turns and the fine rotation the two controls hold. Verify tests cover the factor being
      exactly 1 for a photograph under the bound, the factor for one over it, a recorded framing seeding
      every one of its seven fields, and the dialog closing mid-load leaving no state behind.
- [x] 2.2 Add the turn and mirror handlers (D5): the fine rotation driving the widget by the shortest-path
      delta from its current angle with no transition while dragging, the quarter-turn action snapping to the
      next multiple of 90 and zeroing the fine rotation, and the two mirrors toggling through `flipImage`
      with the widget's own `transforms.flip` left as the only record of them. Verify tests cover +12 degrees
      reaching the widget as +12, a quarter turn after 7 degrees leaving the total at 90 and the slider at 0,
      four quarter turns returning to 0 rather than 360, and the same mirror twice leaving no mirror.
- [x] 2.3 Add the ratio and dimension handlers (D6, D7, D4): choosing a ratio reshaping the box to the
      largest rectangle of it that fits the *rotated* extent, centred on the current box and deferred a frame
      so the new lock reaches the stencil first, with the applied size pushed into the fields directly
      because the widget fires no change for a ratio-driven reshape; the displayed dimensions scaled to
      source pixels; a typed dimension divided back by the factor, clamped to the photograph and to
      `MIN_CROP_SIZE` in the widget's own space, with the other dimension following from any lock. Verify
      tests cover 16:9 then 3:2 then 16:9 landing at the same size both times (which is the divergence from
      the reference), a ratio chosen on a turned photograph measuring against the turned extent rather than
      `imageSize`, a typed width under a lock giving the height that lock implies, and a typed width larger
      than the photograph being brought inside it.
- [x] 2.4 Add the swap and reset handlers: swap transposing the box and selecting the *paired* ratio with the
      lock still on, deferred the same way for the same reason (D7), and reset returning the fine rotation to
      zero, undoing the quarter turns and the mirrors, releasing the lock and returning the box to the whole
      photograph, deferred behind the cleared lock (D8). Verify tests cover swapping under 16:9 selecting
      9:16 and keeping the lock, swapping under Free transposing and staying Free, reset from a turned,
      mirrored, locked and dragged-in state returning all four to their defaults, and reset followed by
      dismissal leaving a recorded framing untouched.
- [x] 2.5 Add `onApply` (D10): read the box, scale by the factor, normalise the angle to `[0, 360)` and
      multiply into millidegrees, read the flips off the widget, then either `forgetCrop(identity)` for a
      framing that changes nothing - no flips, no turn, the whole photograph - or `setCrop(identity, info)`
      for anything else; then `setTransform(identity, FITTED)` so the canvas is fitted to the new shape, and
      close. Verify tests cover a framing reaching the store with every field scaled to source pixels, an
      unchanged framing clearing a previously recorded one rather than storing an identity framing, a
      previously unframed image staying unframed, the transform being reset for the framed image and for no
      other, and cancel, Escape and the close box each writing nothing to either store.

## 3. The surface

- [x] 3.1 Add `features/crop/ImageCropper.tsx` wrapping the widget: the default box covering the whole
      photograph, the thirds grid, the **undrawn** surround — the reference's `text-transparent`, not the
      mockup's dim, because the widget spreads it to the nearest clipping ancestor and any visible colour
      washes the whole pane (design.md — Risks) — the aspect ratio passed through, the minimum size, and
      wheel and pinch zoom at `CROP_ZOOM_WHEEL_RATIO`, with the photograph given a clipping parent of its
      own so a magnified one holds no pixels outside the boundary the box can be dragged to. Verify by eye
      that the grid draws permanently and the surround is undimmed with the layered stylesheet and no `!`
      suffix (D2), and by test that it renders and forwards its ref.
- [x] 3.2 Add `features/crop/ratios.ts` - the ten options in five inverse pairs, each with its key, its
      label or translation key, its value and its inverse's key (D7) - and `features/crop/AspectRatios.tsx`
      drawing them two to a row as screen 18 draws them: a circular well holding a rectangle **in that
      ratio's own proportions**, dashed for Free, the one in force ringed in the accent colour. Verify tests
      cover every option's inverse being present in the table and being the option whose value is its
      reciprocal, the grid marking exactly one option, and Free and Square being the only two whose labels
      come from the catalogue.
- [x] 3.3 Add `features/crop/RotateControls.tsx` - the quarter-turn, horizontal-mirror and vertical-mirror
      buttons, the -90 to +90 slider marked every 15 degrees, and Reset - and `features/crop/CropDimensions.tsx`
      - the `w` and `h` fields with the swap between them, each field holding its own input while focused and
      mirroring the framing when it is not (D4). Verify tests cover the slider's range, its marks and its
      degree-suffixed label, each button reaching its handler, a field not being clobbered mid-typing by an
      incoming framing, and an emptied field falling back to the minimum on blur.
- [x] 3.4 Add `features/crop/CropRotate.tsx`: the near-fullscreen Radix dialog inset 32px with the centring
      transform overridden, the existing `DialogTitleBar` reading `crop.title`, the dotted surface behind the
      cropper pane unconditionally — the helper's exported `DOTTED_SURFACE` rather than `useCanvasSurface`'s
      branch, which is a work surface taking a value rather than a choice (D9) — the 256px settings column
      holding the ratio grid, a rule, the dimension row, the zoom hint and Cancel and Apply, and the rotation
      row centred under the cropper. Verify tests cover the dialog rendering every control screen 18 draws,
      the title bar naming the dialog accessibly, a click outside leaving it open while Escape and the close
      box dismiss it, and the surface staying dotted for a user whose canvas is the particle field, with no
      particle field drawn in the dialog.

## 4. The way in

- [x] 4.1 Lift the sidebar's miniature into `features/sidebar/SidebarMiniature.tsx` with its viewport
      rectangle unchanged, add the crop button over its lower trailing corner as screens 05 and 18 draw it,
      and mount `CropRotate` beside it on a local open state (D9). The button is drawn with the miniature and
      absent without one, which is the carve-out the gui-shell delta records. Verify tests cover the button
      appearing with an image and not without, operating it opening the dialog, `Sidebar.tsx` being unchanged
      in what it draws, and the viewport rectangle still tracking the canvas.
- [x] 4.2 Add the five accessible names the reference does without - `crop.open`, `crop.rotate`,
      `crop.flipHorizontal`, `crop.flipVertical`, `crop.swap` (D12) - to **all thirteen** locale files, and
      wire each to its control. Verify the i18n parity test passes, that no key this slice needs was already
      present under another name, and that every icon-only control in the dialog and on the miniature is
      reachable by its accessible name in the tests above.

## 5. Checks

- [x] 5.1 Run the frontend lint, typecheck and test scripts scoped to the files this slice touched - biome
      scoped rather than repository-wide, because `pnpm check` reformats files this slice never opened. No
      Rust changes in this slice, so there is nothing for `cargo fmt`, `cargo clippy` or `cargo test` to
      cover; verify that by confirming `git status` names no `.rs` file. Verify `openspec validate --changes
      add-crop-3-dialog --strict` passes.
- [x] 5.2 Verify the framing by hand on macOS, which is the only coverage the central behaviour gets because
      jsdom has no layout: open a photograph, open Crop/Rotate from the miniature and confirm the whole
      photograph is shown; drag a rectangle in, apply, and confirm the canvas, the miniature, the navbar's
      dimensions and its hover popover all agree and that the canvas came back fitted; re-open and confirm
      the framing is shown on the whole photograph and can be widened again; turn the photograph 12 degrees,
      frame a rectangle that includes a rotated corner and confirm the canvas draws that corner transparent
      exactly where the dialog drew it - this is the check that the widget and Rust agree about the rotated
      canvas, which is what slice 1 pinned and what would fail here if it were wrong; lock 16:9, swap, and
      confirm 9:16 is selected and locked; magnify until the photograph fills the pane and drag an edge
      outwards to confirm the box reaches the photograph's own edge with no strip left outside what it can
      cover — the check the clip in 3.1 exists for, and the only one there is, since the widget renders no
      background at all in jsdom; run an enhancement over a framing and confirm it enhances the framed
      pixels; apply an unchanged framing and confirm nothing re-runs and that the canvas is left at the
      magnification it was on; and confirm a large scan opens
      the dialog in about the time a canvas rendition takes rather than several seconds.
