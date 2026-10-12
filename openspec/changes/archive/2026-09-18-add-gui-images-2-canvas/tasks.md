## 1. `gui` - Rust

- [x] 1.1 Add the command that reports the formats the application opens, answering
      `opai::image::input_extensions()` and registered in `generate_handler![]` beside the two image
      commands, with a doc comment recording why the interface needs a list the picker already applies
      for itself (D2). Verify with a test that its answer is `input_extensions()`'s, and that
      `capabilities/default.json` is unchanged - it is this application's own command and gates
      nothing.

## 2. `gui` - frontend stores

- [x] 2.1 Add `stores/files.ts`: the open images in order, the index of the current one, one writer
      that adds a batch, and a `useCurrentFile` accessor (D3). Verify with tests covering a first open
      making the first file current, a second batch moving the current index to the first newly added
      file, a batch of files already open changing neither the list nor the index, and duplicates
      within one batch being collapsed.
- [x] 2.2 Add `stores/preview.ts`: the chosen comparison, defaulting to side by side and persisted
      under the Wails app's `app-storage` key so an upgrading user keeps their choice (D4). Verify with
      tests over the default, the setter, and what is written to storage - the shape
      `stores/enhancements.test.ts` already uses.

## 3. `gui` - frontend, the two routes in

- [x] 3.1 Add the extensions wrapper to `ipc/images.ts`, fetched once and held for the session (D2, R4).
      Verify with tests in the style of the existing `ipc/*.test.ts`: the command name, and that a
      second call does not invoke a second time.
- [x] 3.2 Add the hook that opens the picker - passing `dialogs.native.selectImage` and
      `dialogs.native.imagesFilter` already translated, adding what comes back to the file store - and
      wire it to the empty canvas's Browse images and the drawer's Add images. Verify with tests that
      both controls open the picker, that chosen files reach the store, and that a dismissal leaves the
      store untouched and raises no toast.
- [x] 3.3 Add the drag-and-drop listener on `onDragDropEvent`'s `drop` payload, hit-tested against the
      preview canvas the hook is handed a ref to: ignore a drop let go of outside it, and otherwise
      partition the paths by extension against 3.1's list, describe and add the supported ones, and
      raise `toasts.unsupportedFiles` naming the rest (D1, D2, D9). Verify with tests covering a mixed
      drop (supported files added, refused ones named in the toast), a drop of nothing supported (no
      describe call, toast raised), a drop of only supported files (no toast), a drop over the navbar
      and one over the drawer (nothing described, nothing said), the canvas's own edges, the units the
      position arrives in on a Retina Mac and on Windows (D9), and that the listener is removed on
      unmount.

## 4. `gui` - frontend, drawing the image

- [x] 4.1 Add the two pane labels to `en.json` and the other twelve locales (D8). Verify
      `i18n/parity.test.ts` passes and that no other key this slice uses is missing - the picker's two
      strings, `toasts.unsupportedFiles`, `navbar.dimensions.title`, `sidebar.zoomCropAlt` and
      `common.previewAlt` all already exist.
- [x] 4.2 Add the component that draws the current image in the three comparison modes: one pane for
      full, two for side by side, and the enhanced pane clipped over the original for split with the
      design's divider and grip drawn but not draggable; each pane an `<img>` at
      `renditionUrl(identity)` with no bound, letterboxed, labelled Original or Enhanced (D5, D6, D8).
      Verify with tests over each mode's panes and labels, and that both panes and all three modes
      request one and the same URL.
- [x] 4.3 Make `Preview` draw 4.2 in place of `PreviewEmpty` once an image is open. Verify with tests
      that the invitation is drawn with no image open and the image with one open.
- [x] 4.4 Bind the drawer header's comparison group to the preview store and make it available once an
      image is open, leaving the fold toggle, Select all and the zoom slider disabled (D3, R5). Verify
      with tests that choosing a mode changes what the canvas draws, that the group reports the stored
      mode on mount, and that the other three controls are still disabled with an image open.
- [x] 4.5 Turn `App.tsx`'s `hasFiles` into a read of the file store, keeping every control that takes
      `disabled` from it taking it (D3). Verify `App.test.tsx` still passes and add the case that an
      opened image flips the frame out of its empty state.

## 5. `gui` - frontend, the frame reports the image

- [x] 5.1 Add the navbar's current file: its name - the file's own, not its path - and the Dimensions
      block carrying `width x height`, both absent while no image is open. Verify with tests over an
      open image, no image, and a file several folders deep.
- [x] 5.2 Draw the sidebar miniature from the same URL the canvas uses, replacing the "No preview
      available" line while an image is open (D7). Verify with tests that the words are shown with no
      image and the image with one, and that its URL is the canvas's.

## 6. Verification

- [x] 6.1 Run the built application on macOS and confirm by screenshot: Browse images opens the picker
      and a chosen photograph is drawn; the navbar carries its name and dimensions and the sidebar its
      miniature; the three comparison modes each draw what the design draws; a drag-and-drop of several
      photographs onto the sidebar opens them; and a drop containing one non-image names it in a toast
      while the photographs still open. Use `~/Desktop/test`, whose seven files include a camera RAW,
      so the format the webview cannot decode is one of the ones confirmed on screen.
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy` and `cargo test` across the workspace, and
      `pnpm lint`, `pnpm typecheck` and `pnpm test` in `crates/gui`. Verify all six are clean.
