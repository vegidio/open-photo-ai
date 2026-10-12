## 1. The support question, first

- [x] 1.1 Verify in the running application - not in jsdom, not from a support table - that a shape
      animating `offset-distance` along `offset-path: rect(0 auto auto 0 round Npx)` actually travels in
      this WKWebView. If it does not, switch to the rotating conic-gradient fallback of design.md D3
      before anything below is built, and record which route was taken in design.md.

## 2. The token

- [x] 2.1 Add `--activity: #22d3ee` beside `--success-bright` in `frontend/style.css` and map it into the
      `@theme inline` block as `--color-activity`, with a comment recording that it is the analysing
      spinner's second arc and that the spinner is not built yet. Verify a `text-activity` utility
      resolves in a scratch render, then remove the scratch.

## 3. The beam

- [x] 3.1 Add `components/ui/border-beam.tsx`: MagicUI's mask-and-`offset-path` structure with the
      `motion.div` replaced by a CSS-animated element per D2, written to project convention - `type`
      alias, arrow function, optional-property syntax, named React imports. Keep `size`, `duration`,
      `delay`, `colorFrom`, `colorTo`, `reverse`, `initialOffset`, `borderWidth` and `className`; drop
      `transition`. Verify by test that it renders `pointer-events-none`, that `delay` emits a negative
      `animation-delay`, and that `reverse` inverts the keyframe direction.

## 4. The hover

- [x] 4.1 Change `hooks/useDroppedImages.ts` to return whether a drag is currently over the target:
      recompute from the position on both `enter` and `over`, clear on `leave` and `drop`, reusing the
      existing `positionScale` and `isOver` rather than adding a second hit-test (D1, D4). Verify by test
      that a drag entering the window over the navbar and moving onto the canvas reports true - the case
      an `enter`-only implementation misses - that leaving reports false, and that `drop` still opens
      exactly what it opened before.

## 5. The drop region

- [x] 5.1 Draw the region in `features/preview/Preview.tsx` while the hook reports a drag over the canvas:
      its own border, carried by four beams alone with no track under them, alternating between
      `--success-bright` and `--activity`, each a quarter of a cycle apart (D5). The region is its own
      always-mounted element, bottomed at the drawer's body while the drawer is open, and it is what the
      hook hit-tests (D7). Verify by test that the beams appear and disappear with the hook's answer,
      that they render with an image open as well as on the empty canvas, that nothing is drawn under
      them, and that the region is `aria-hidden` with no text.
- [x] 5.2 Verify by test that neither the region nor any of the four beams intercepts pointer events - the failure
      this change could introduce is a drop that silently stops working, and a wheel-zoom or drag-to-pan
      that stops reaching the canvas.

## 6. Verification

- [x] 6.1 Drag a file over the running application and confirm by eye: nothing over the navbar, the
      sidebar or the drawer; four beams travelling the region's own border, with nothing drawn under them,
      once the pointer crosses onto the canvas; it clearing on leaving and on dropping; the same over an
      already-open photograph; and the mark appearing unchanged for a file the application cannot open,
      which is then named in a notice on drop.
- [x] 6.2 Run `pnpm typecheck`, `pnpm test` and `pnpm exec biome ci .` - the last of these rather than
      `pnpm lint`, which runs the lint rules only: the formatter is what CI runs and what catches a file
      this change left without a trailing newline. Scope any Biome *write* to the files this change
      touched, so the diff does not swell with unrelated reformatting.
