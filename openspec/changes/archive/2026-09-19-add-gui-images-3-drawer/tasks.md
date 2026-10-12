## 1. `gui` - frontend stores

- [x] 1.1 Add `stores/drawer.ts`: `open`, a setter and a toggle, not persisted (D1), with the zoom
      deliberately absent and a comment saying why. Verify with tests over the folded default and both
      writers, in the shape `stores/preview.test.ts` already uses.
- [x] 1.2 Give `stores/files.ts` a `setCurrentIndex` and the selection: `selectedPaths`,
      `toggleSelected`, `selectAll`, `unselectAll`, each replacing the `Set` rather than mutating it,
      plus the rule that a batch opened into an empty window picks its first file (D2). Verify with
      tests covering picking and unpicking one file, that picking does not touch the current image,
      select-all and unselect-all over a list, the first-batch pick, that a later batch leaves the
      selection alone, and that each writer produces a new `Set` so subscribers see the change.

## 2. `gui` - frontend, the strip

- [x] 2.1 Add `@tanstack/react-virtual` to `crates/gui/package.json` (D3) and add
      `hooks/useResizeObserver.ts` measuring an element and re-measuring on resize, guarded for an
      environment with no `ResizeObserver` (D9). Verify `pnpm install` succeeds and with tests that the
      hook measures on mount, re-measures on a resize and does not throw without the global.
- [x] 2.2 Add `features/drawer/DrawerItem.tsx`: a wrapper holding a `<button>` over the thumbnail
      surface and its name bar, with the checkbox as a sibling overlaid on the corner; the image at
      `renditionUrl(identity, 384)`, decorative and `loading="lazy"`; the accent outline and inverted
      name bar when it is the current file; the three-dot glyph as a non-interactive icon; memoized
      (D4, D5, D6, D10). Verify with tests over the button's accessible name being the file name,
      clicking it reporting the item's index, the checkbox picking and unpicking without changing the
      current image, the current item's marking, the requested URL carrying the 384 bound, and a file
      with no identity rendering the surface with its name bar intact.
- [x] 2.3 Add `features/drawer/DrawerBody.tsx`: the horizontal strip, windowed with a virtualizer over
      the open files with an overscan of 4, two spacers standing in for the unmounted items, the item
      size measured by 2.1, and one stable click handler writing `setCurrentIndex` (D3, D9). Verify
      with tests that a short list mounts every item and no spacer, that a long list mounts a bounded
      subset with both spacers present and a total scrolling extent covering all of it, and that
      clicking an item makes it the current image.

## 3. `gui` - frontend, the drawer's controls

- [x] 3.1 Add `drawer.showImages`, `drawer.hideImages` and `drawer.selectImage` to `en.json` and the
      other twelve locales (D11). Verify `i18n/parity.test.ts` passes and that the keys 2.2 and 3.2
      use resolve - `drawer.selectAll`, `drawer.unselectAll` and `common.previewAlt` are already in
      the catalogue.
- [x] 3.2 Make `DrawerHeader`'s fold toggle live - reading and toggling the drawer store, labelled
      from 3.1, still disabled while no image is open - and its Select all live, with the three states
      and the label that becomes Unselect all once everything is picked (D1, D11). Verify with tests
      over the toggle's icon and label in both states, that it stays disabled with no image open, and
      over Select all's none/some/all reporting plus what operating it does in each.
- [x] 3.3 Make `Drawer` read the drawer store instead of `const open = false`, draw `DrawerBody` in
      place of the empty rectangle, and fold on a click outside itself (D1). Verify with tests that
      the body is translated out of view when folded and in place when unfolded, that a click on the
      canvas folds it, and that a click on one of its own thumbnails does not.
- [x] 3.4 Add the effect that unfolds the drawer whenever the number of open images changes to more
      than one, in `Drawer` (D7). Verify with tests that one image leaves it folded, a second unfolds
      it, a batch added after the user folded it unfolds it again, and a batch that adds nothing new
      leaves it folded.
- [x] 3.5 Make the toaster's bottom offset follow the drawer, clearing the unfolded body and the
      folded header respectively (D8). Verify with tests over the offset in both states, and that the
      two heights come from `lib/constants.ts` rather than being written out again. **Added during
      implementation, agreed with the user:** lift the canvas's two pane labels by the drawer's body
      height as well, which is the design's own `chipBottom` and what `PreviewImage.tsx`'s comment
      leaves to "the slice that unfolds the drawer". Same rule, same constant, same reason as the
      toaster - the drawer is drawn over the canvas, so a chip that did not move would be behind the
      strip. Verify with tests over the offset in both states and in every comparison mode.

## 4. Verification

- [x] 4.1 Run the built application on macOS and confirm by screenshot: opening a second image unfolds
      the drawer by itself; the strip draws one thumbnail per file with its name; clicking a thumbnail
      moves the outline, the inverted name bar, the canvas, the navbar's name and dimensions and the
      sidebar's miniature together; the checkboxes and Select all show all three states; a notice
      raised with the drawer open clears the strip; and the fold toggle folds and unfolds. Use
      `~/Desktop/test`, whose seven files include a camera RAW, so the strip is confirmed drawing a
      format the webview cannot decode on its own.

      **Confirmed.** All seven behaviours above were seen and captured against `~/Desktop/test`.

      The undecodable format could not be reached on that run - the folder now holds seven JPEGs and
      no camera RAW, and none exists elsewhere on the machine - so it was left open and has since
      been **verified by hand against a format the webview cannot decode on its own**. That closes
      the last of the eight. It is the behaviour slice 1 built and this slice only asks for at a
      different bound: the strip requests the same URL the canvas does, and the protocol is what
      decodes it either way.
- [x] 4.2 Confirm a large batch on macOS by screenshot and by the webview's own devtools: open several
      hundred images, scroll the strip end to end, and check that the mounted thumbnails stay a bounded
      handful, that scrolling reaches the last file, and that requests for a thumbnail's pixels appear
      as it comes into view rather than all at once (D3, D5).

      **Confirmed** with 407 images open (the seven above plus 400 byte-distinct copies): 17
      `drawer-item` elements mounted, a 48,567px scroll extent covering all 407, `photo-400.jpg` -
      the last file - reached by scrolling, and 17 `size=384` requests in the Network panel at rest,
      rising to 58 after visiting the middle and the end. Not 407, and not all at once.
- [x] 4.3 Run `pnpm lint`, `pnpm typecheck` and `pnpm test` in `crates/gui` and verify all three are
      clean. No Rust changes in this slice, so `cargo fmt --check`, `cargo clippy` and `cargo test` are
      run once to confirm the workspace is untouched rather than as part of the work.

      All six clean: lint over 129 files, `tsc --noEmit`, 411 tests in 40 files, and `cargo fmt
      --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` all passing with
      no Rust file edited.
