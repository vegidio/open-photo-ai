## Why

Every screen after the empty canvas needs one thing the application cannot do yet: turn a file on
disk into pixels on screen. Screens 05, 08 and 08b draw a preview canvas, a strip of thumbnails and
a sidebar miniature, and all three are the same question asked at three sizes.

This slice answers it on the wire and draws nothing. It is the seam the three screen slices behind
it are built against, so each of those is a screen rather than a screen plus a transport.

## What Changes

- **`opai` gains a downscale.** `image::resize`, beside `load`, `save` and `probe`, producing a copy
  of a picture bounded by a longest-edge limit. It is the operation behind every thumbnail; nothing
  in the crate could produce one before.
- **The GUI can ask the user for image files.** A command that opens the platform's own file picker,
  filtered to the formats `opai::image` decodes, and a second command that describes paths the user
  supplied by any other route - a drag and drop, a future command line argument - so both routes
  produce the same records.
- **An image file becomes a record the interface can hold**: its path, its XXH3-64 identity, its
  dimensions, its extension and its size on disk. Read without decoding the file, so a folder of
  500 photographs lists at the cost of 500 headers rather than 500 decodes.
- **The GUI serves pixels to the webview over a custom URI scheme** rather than over the IPC
  channel. An opened image is addressed by its identity and a requested size; the handler decodes
  it through `opai::image::load`, bounds it, encodes it and streams the response. RAW, TIFF and HEIF
  reach the interface this way, and the webview cannot decode any of the three.
- **Only files the user has opened are reachable.** The scheme addresses an identity, never a path,
  resolved against the set of files the two commands above admitted.
- No UI. The empty canvas, its Browse images button and the drag-and-drop invitation are untouched
  and still inert; the slice behind this one wires them.

## Capabilities

### New Capabilities

- `gui-images`: how the application takes image files from the user, what it records about each one,
  and how an opened image's pixels reach the interface at a requested size.

### Modified Capabilities

None. `gui-shell` states that the empty canvas offers a control to browse for images and that the
controls acting on an image are unavailable while none is open. Both stay true: this slice adds no
control and opens no file on its own. The slice that wires the button is where those requirements
move.

## Impact

**Crates: `opai` and `gui`.** `cli` is untouched.

`opai` gets the downscale because it is image manipulation, which the architecture puts in the core
crate; the export queue and any future CLI preview want the same operation. Everything else is
`gui`, and correctly so: a file picker, a URI scheme and a webview's cache are all the Tauri
application's business and mean nothing to the terminal front end.

**`rust-sak`: `crypto` is used, `image` is used through `opai`, and nothing is missing.**
`crypto::xxh3_file` is the identity, exactly as `opai::image::load` already computes it. The
downscale is the one thing neither crate had, and it does **not** belong in `rust-sak`: that module
is a codec module - encode and decode across eight formats plus RAW - and the `image` crate it is
built on already offers `DynamicImage::resize` to every consumer unconditionally. Extending
`rust-sak` would wrap a method rather than fill a gap, so the operation goes in `opai` beside the
other three.

**New dependencies: one Rust crate, `tauri-plugin-dialog`, and no npm package.** The picker is opened
through the plugin's Rust API from a command of this crate's own, so the frontend never invokes the
plugin - the same judgement `src/logs.rs` already records for `tauri-plugin-opener`. Nothing new is
needed for the transport either: the webview loads a URL.

**Configuration:** `tauri.conf.json`'s `img-src` gains the custom scheme in both its platform forms.
`capabilities/default.json` is **not** touched: a capability gates a command the frontend invokes,
and the frontend invokes neither the dialog plugin nor the URI scheme.

**Deliberate divergence from the Go application.** It sends a preview as base64 over the IPC channel,
which the frontend turns into a Blob, an object URL and a 1.5 GB LRU cache that must revoke each
evicted URL or strand the decoded image for the life of the webview. Wails offers nothing else;
Tauri does. Serving the same bytes over a URI scheme removes the base64 inflation, the JSON channel,
the blobs and the revoke discipline, and lets the webview's own cache replace the LRU. The pixels the
user sees are identical.

**One further divergence, deliberately.** The Go application encodes every preview as JPEG, so a PNG
with transparency is previewed with its transparency composited away while exporting the same file
keeps it - the preview and the export disagree about the photograph. A rendition here is JPEG unless
the decoded picture carries an alpha channel, in which case it is PNG. One branch, and the
inconsistency is gone.

**Estimated task count: 13.** Within the limit. This is slice 1 of a four-slice series - see
design.md, which carries the whole list; only this slice is proposed here.
