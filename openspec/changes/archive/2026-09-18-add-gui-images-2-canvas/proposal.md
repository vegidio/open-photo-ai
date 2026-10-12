## Why

The seam behind the empty canvas is finished - a picker, a drop route, an image record and a URI
scheme that serves an opened photograph's pixels at any size - and nothing in the window touches it.
Browse images is still inert, a photograph dragged onto the window still does nothing, and the canvas
still draws its invitation whatever the user does.

This slice is where the application first opens a photograph and shows it.

## What Changes

- **The two controls that add images work.** The empty canvas's Browse images and the drawer's Add
  images both open the platform's picker through slice 1's `open_images`; chosen files become the
  window's open images.
- **Dropping images on the preview canvas opens them.** A drop is a `tauri://drag-drop` event
  carrying paths and the position they were let go of at; the canvas is the region that accepts one,
  and the interface describes the files through slice 1's `describe_images`. Files in a format the
  application cannot open are named in a notice and not opened, while the rest are - which is the
  reference implementation's behaviour and needs the one thing it has that the frontend does not: the
  list of extensions the decoder accepts.
- **`gui` gains one command: the formats it opens.** `opai::image::input_extensions` already answers
  it for the picker's filter; this reports the same list to the interface so the drop route tests
  against the decoder's own list rather than a copy of it.
- **The window holds a set of open images and a current one.** A file store: what is open, which one
  the canvas is drawing, and the rule that opening files moves the current image to the first of the
  batch just added. Re-adding a file that is already open does not duplicate it.
- **The canvas draws the current image**, in all three comparison modes - the whole image, the two
  panes of side-by-side, and the split overlay - each pane labelled Original or Enhanced. The
  comparison control in the drawer header becomes live and its choice survives a restart.
- **The navbar reports the open image**: its file name, and a Dimensions block carrying the
  photograph's pixel size.
- **The sidebar draws a miniature of the current image** in place of "No preview available".
- No enhancement of any kind, no drawer thumbnails, no zoom. Both preview panes show the same
  photograph, because nothing in the application can yet produce a different one - which is exactly
  what the reference does with an empty enhancement stack.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-images`: gains the interface's side of opening a file - the two controls that open the picker,
  the drop route and what happens to files the application cannot open, the set of open images and
  which of them is current - and the application's ability to report the formats it opens, which the
  drop route needs.
- `gui-shell`: gains what the frame shows once an image is open - the canvas drawing it in the chosen
  comparison mode, the comparison control that chooses between the three, the navbar's file name and
  dimensions, and the sidebar's miniature. Nothing already recorded there changes: every requirement
  it carries today is about the frame with no image open, and each stays true word for word.

## Impact

**Crates: `gui` only.** `opai` and `cli` are untouched. The one Rust addition is a command reporting
`opai::image::input_extensions()`, which is a value `opai` already publishes and this crate already
uses for the picker's filter; everything else in this slice is the React frontend.

Nothing here is shared logic held in a front end. What a webview draws, which image it is drawing and
how it compares two of them is the Tauri application's own business and means nothing to the terminal
front end. The one thing that *would* be shared - the list of formats the application opens - is
already in `opai` and is read from there rather than restated.

**`rust-sak`: nothing is needed and nothing is missing.** This slice decodes nothing, hashes nothing
and writes no files; every byte it draws comes from the URI scheme slice 1 built, which reaches
`rust-sak` through `opai` as it already did.

**New dependencies: none.** The drop event is `@tauri-apps/api`'s `onDragDropEvent`, the store is
Zustand as the two existing stores are, the notice is the `sonner` toaster already mounted in
`providers.tsx`, and the split overlay is a `clip-path` rather than the reference's
`react-compare-slider`. The reference also uses `react-zoom-pan-pinch`; that is slice 4's question,
not this one's.

**Configuration:** none. `tauri.conf.json` already carries both platform forms of the scheme in
`img-src`, and `capabilities/default.json` is not touched - the drag-drop event is listened to
through `core:default`, which is already granted, and the new command is this application's own.

**i18n:** two new keys, for the Original and Enhanced pane labels, added to all thirteen locales.
Everything else this slice says to the user already exists in the catalogue, including the
unsupported-files notice and its plural forms.

**Deliberate divergences from the Go application**, each argued in design.md:

- **The panes are labelled.** The reference draws two unlabelled images side by side; with an empty
  enhancement stack they are identical and nothing says why. The design draws an Original and an
  Enhanced chip, and this follows the design.
- **A file the application cannot open is judged by its extension, as the reference judges it**, but
  the list is asked of the decoder rather than kept beside the interface.

**Estimated task count: 15.** At the limit rather than over it, and the one scope call that decides
that - whether the sidebar miniature belongs here or in slice 4 - is settled in design.md, Slices.
This is slice 2 of a four-slice series; design.md carries the whole list, and only this slice is
proposed here.
