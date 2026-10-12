## Context

See proposal.md - Why. What shapes the approach here is what already exists on each side of the seam.

`opai::image` decodes eight formats plus 29 camera RAW extensions, computes the XXH3-64 identity over
a file's whole bytes, and reads dimensions from a header without decoding. Every operation comes in
an `async` form explicitly built for this front end - the module's own words are *"the GUI wants the
asynchronous form: its runtime is shared with the window and the IPC plumbing, so decoding a
100-megapixel scan inline would stop the event loop."* What it has never had is a way to make a
picture smaller.

The GUI is a Tauri 2 application whose frontend is four always-present regions (`gui-shell`). Three
of them will draw the same photograph at three different sizes, and none can draw anything yet.

The reference implementation solves the transport with the only tool Wails offers - base64 over the
IPC channel - and then spends about 150 lines of `utils/image.ts` on the consequences: a Blob per
image, an object URL per Blob, a 1.5 GB LRU bounded by byte size, a `dispose` hook that must revoke
each evicted URL or strand its decoded image for the life of the webview, and an in-flight map so two
concurrent misses do not `set` twice and revoke a URL a caller is already rendering. None of that is
about photographs. It is about not having a way to serve bytes to a webview.

## Slices

The four slices that take the application from an empty canvas to an open, zoomable image. This is a
new series; the `add-gui-1-foundation` .. `add-gui-7-settings-dialog` series closed out on the
Settings pair and is not continued here.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-images-1-ipc` | `opai::image::resize`; the picker command and the describe-paths command; the image record; the identity-addressed rendition protocol and the opened-file registry behind it. No UI | - | **CURRENT** |
| 2 | `add-gui-images-2-canvas` | Browse images and drag-and-drop wired to slice 1; the file store; the canvas drawing the current image in all three preview modes; the navbar's filename and Dimensions; the sidebar miniature | 1 | pending |
| 3 | `add-gui-images-3-drawer` | Screens 08 and 08b - the thumbnail strip, its windowing and deferred decode, the per-item checkbox and name bar, Select all, the current-file outline, switching images, and the fold toggle made live | 2 | pending |
| 4 | `add-gui-images-4-zoom` | The per-image transform shared across panes, the drawer's zoom slider and the wheel/pinch zoom, split mode's draggable handle, and the viewport rectangle drawn over the sidebar miniature | 3 | pending |

**Slice 1 is this change.** Slice 2's budget is the one to watch: three preview modes, both opening
routes, the store, the navbar and the miniature may not fit 15 tasks, in which case the miniature
moves to slice 4, where the rectangle that belongs on it already lives. That is settled when slice 2
is proposed, not here.

**Crates.** `opai` for the downscale and the list of extensions the picker filters on; `gui` for
everything else. `cli` is untouched. The split is the architecture's own test - the downscale is
image manipulation, and an export queue or a terminal preview would want the same operation, while a
file picker and a webview's URL scheme mean nothing outside the Tauri application.

**rust-sak: `crypto` and `image` are used, both already enabled, and nothing is missing.** The
identity is `crypto::xxh3_file`, which `opai::image::load` already computes; every decode is
`rust_sak::image` through `opai::image`. The downscale is the one operation neither crate had, and
D2 below is why it does not belong in `rust-sak`. What `opai` had but did not *publish* is a second
matter, and D9 is that one.

**New crates: one**, `tauri-plugin-dialog`. **New npm dependencies: none** - see D4 and D1.

**Platforms.** Written for all three, validated on macOS, which is the development machine. Two
things here are platform-shaped and are called out where they arise: the rendition URL takes a
different form on Windows (D3), and the picker's threading rule is the class of bug this repository
has already been bitten by once (R1). Linux and Windows are untested.

**Execution providers: none are exercised.** Nothing in this slice builds a session or runs a model.

## Goals / Non-Goals

**Goals:**

- One way to get a photograph's pixels into the interface at any size, that works for every format
  `opai` decodes rather than the subset a webview happens to decode.
- A seam the three slices behind this one are built against, so each of those is a screen and not a
  screen plus a transport.
- Reach limited to the files the user actually opened.

**Non-Goals:**

- **Any UI.** The empty canvas, its Browse images button and the drag-and-drop invitation are left
  exactly as they are and still do nothing. Slice 2 wires them.
- **Closing a file.** Nothing is removed from the opened set in this slice; the overflow menu that
  closes an image is screen 09, and the registry's growth is addressed in R3.
- **Enhanced pixels.** Only the image as loaded. The enhancement pipeline is not reached.
- **Crop.** The reference's `GetImage` takes a crop and applies it when no bound was given. Crop is
  screen 18 and is not designed in here; the request shape leaves room for it without naming it.
- **A frontend image cache.** D6.

## Decisions

### D1. The pixels travel over a custom URI scheme, not over the IPC channel

The application registers an asynchronous URI scheme handler. The interface draws an image by
pointing at a URL; the handler decodes, bounds, encodes and streams the response.

*Alternative: base64 over IPC, as the reference does.* It inflates every payload by a third, routes
tens of megabytes through a JSON channel, and buys the whole Blob / object-URL / LRU / revoke
apparatus described in Context. Tauri has a mechanism Wails does not; porting the workaround for the
missing mechanism would be porting the wrong half of the reference.

*Alternative: Tauri's built-in `asset:` protocol.* It streams **the file's own bytes** and cannot do
either of the two things needed here. A TIFF, HEIF or camera RAW would reach the webview intact and
display as nothing, because no webview decodes them - and those are exactly the formats `opai` exists
to open. It also cannot produce a 100-pixel thumbnail, so the drawer would decode full-resolution
bitmaps to paint squares the size of a stamp. A scheme of our own is not a reimplementation of
`asset:`; it serves something `asset:` cannot address.

### D2. The downscale goes in `opai`, not in `rust-sak`

`image::resize` joins `load`, `save` and `probe`, in both forms the module offers, bounding the
longest edge and preserving the aspect ratio.

The project's standing rule is to extend `rust-sak` when one of its modules is close but incomplete,
and this is not that case. `rust-sak`'s `image` module is a **codec** module - encode and decode
across eight formats plus RAW, behind one uniform API. Downscaling is not a codec concern, and the
`image` crate the module is built on already offers `DynamicImage::resize` to every consumer
unconditionally, with no feature gate. Extending `rust-sak` would wrap a method that is already
reachable, not fill a gap. Nothing is duplicated either way: the filter choice and the
bound-the-longest-edge rule are this project's, and they are what `opai` adds.

Lanczos3, matching the reference's `imaging.Lanczos`.

The reference's resize takes the bound as a target rather than a cap, so a bound above the image's
longest edge would **enlarge** it. No caller ever does that - the frontend asks for 100 or for 0 -
so this is incidental rather than intended, and `opai::image::resize` returns the picture unchanged
instead. The spec states it as a requirement.

### D3. A rendition is addressed by identity and bound, and by nothing else

`opai://localhost/<identity>?size=<n>` on macOS and Linux; `http://opai.localhost/<identity>?size=<n>`
on Windows. The frontend never writes either form by hand - `convertFileSrc(identity, "opai")` from
`@tauri-apps/api` produces the platform's own, which is the one place that difference is allowed to
exist. `tauri.conf.json`'s `img-src` carries both forms, exactly as it already carries `asset:` and
`http://asset.localhost`.

`size` absent or `0` means the image's own dimensions, matching the reference's convention.

*Alternative: put the path in the URL,* which is effectively what the reference does by passing a
path to `GetImage`. It would make **every image file on the machine** readable by anything running in
the webview, re-encoded and handed back. Tauri's own `asset:` protocol carries a configurable scope
for precisely this reason. Addressing by identity costs one map and removes the question.

### D4. The opened-file registry is the scope, and the two commands are its only writers

A map from identity to path lives in Tauri's managed state. `open_images` (the picker) and
`describe_images` (paths from anywhere else) are the only things that write to it, and the protocol
handler is the only thing that reads it. An identity that is not in the map is refused as unknown,
which is a different answer from an identity that is in it and whose file has since gone away - the
spec requires the two to be distinguishable, because one is a bug in the interface and the other is a
photograph on an ejected drive.

Two commands rather than one because the two routes genuinely differ in shape: the picker is a call
that returns records, while a drag-and-drop arrives as a `tauri://drag-drop` window event carrying
paths, with nothing to return a value to. Both funnel into the same describe-and-admit path, so the
records are identical, which the spec requires.

The picker is opened through the dialog plugin's **Rust** API from a command of this crate's own,
not by the frontend invoking the plugin's command. That is the same judgement `crates/gui/src/logs.rs`
already records for `tauri-plugin-opener`, reached by a different route: here the command has work to
do that the plugin's own cannot - building the filter list from `opai`, and returning records rather
than paths. It also means **no `@tauri-apps/plugin-dialog` npm package and no capability entry**,
because the frontend never invokes the plugin and there is nothing for a capability to gate.

**A picker failure cannot be told from a dismissal, and the spec no longer asks for it.** An earlier
draft of this design required the two to be distinguishable, and the plugin cannot support it:
`blocking_pick_files` returns `Option<Vec<FilePath>>`, so a dialog that never opened and a user who
pressed Cancel arrive as the same `None`. There is no error channel to read, and inventing the
distinction would mean reporting a failure on every dismissal or a dismissal on every failure - one
of which cries wolf and the other of which is the thing the requirement was trying to prevent.

Both are therefore an empty selection, and the log line says which two things it could have been.
Nothing is lost at the interface: the user has added no images either way, and there is no branch a
window would take on the difference. What the requirement was protecting against - a failure shown to
the user as a successful choice of nothing - is not reachable, because an empty selection is not
shown to the user as anything at all.

This is the one place the slice is narrower than the spec it was proposed with, and the spec moved
with it rather than the code being bent to a requirement the platform will not answer.

### D5. The filter list is derived in `opai`, never written down twice

The picker filters on what `opai::image` can actually open: the eight `ImageFormat` extensions and
the decodable subset of the RAW extensions. `opai::image` gains one function that answers it.

`crates/opai/src/image/raw.rs` already states the rule this follows, about asking `rust-sak` which
RAW formats decode rather than keeping a constant: *"A second list would be a copy of someone else's
truth: a backend that gained a format would go on being refused by a stale list."* The new function
is derived the same way - from `RAW_EXTENSIONS` filtered by the existing `decodable` check, and from
an exhaustive match over `ImageFormat`, so a ninth format is a compile error rather than a silent
omission from the picker.

### D6. The webview's cache replaces the LRU

Renditions are served with a long-lived immutable cache directive. This is not optimism: the identity
covers every byte of the file and the bound is in the URL, so a given URL cannot come to mean
different pixels. That is the whole justification for the reference's cache key `${file.Hash}_${size}`,
stated to the webview instead of enforced by hand.

What this deletes: the LRU, its byte-size accounting, its `dispose`/revoke discipline, the in-flight
map that exists only to stop two misses revoking each other's URL, and `createImageData`'s
`fetch(data:) -> Blob -> createObjectURL` round trip. An `<img src>` replaces all of it.

### D7. A rendition is always decoded and re-encoded, never passed through

A JPEG requested at no bound could in principle be streamed byte for byte. It is not.

The canvas must show the pixels the enhancement pipeline will operate on, and `opai::image::load` is
what defines those: it routes on content rather than extension, so a JPEG named `.png` decodes as the
JPEG it is; it develops camera RAW through demosaic, white balance, the camera's colour matrix and
tone curve; and it produces 16 bits per channel for RAW. A passthrough would make the preview the
webview's interpretation of the file, and the two would disagree - visibly, for RAW, and confusingly,
for a misnamed file.

The rendition is JPEG at quality 90, matching the reference's preview encode, **except** when the
decoded picture carries an alpha channel, where it is PNG. That exception is a deliberate divergence:
the reference encodes JPEG unconditionally, so a transparent PNG previews with its transparency
composited away while exporting the same file keeps it. One branch on the colour type removes an
inconsistency the user can see.

### D8. Describing is concurrent, and tolerant per file

Hashing reads every byte and is what dominates a large selection, so files are described concurrently
on blocking threads, as the reference does with a worker per CPU. A file whose header or size cannot
be read is still admitted with that fact unknown rather than failing the batch - the spec's reasoning
is that a selection is a batch the user assembled, and the reference reaches the same conclusion by
logging a warning and carrying on.

Records are ordered by path, matching the reference, so the same selection lists the same way twice.

### D9. `opai` publishes two operations it already performed, rather than `gui` reaching past it

`image::identity` and `image::encode`, each in both forms, beside the other five.

Neither is new behaviour. `opai::image::load` already hashes a file's bytes with `crypto::xxh3_file`,
and `opai::image::save` and the run cache already encode through `rust_sak::image::encode_writer`.
What did not exist is a way to ask for either **without** the thing it is bundled into, and this slice
needs exactly that twice:

- **The describe path needs the identity without the decode.** The spec requires dimensions from a
  header *"so that describing a large selection costs a header per file"*, and getting the identity by
  loading instead would develop all 500 camera RAW files in a folder to fill in a listing. The hash
  still reads every byte — it has to, to be the same value a loaded picture carries — but skips the
  decode, which is the expensive half. `opai::image`'s own documentation has promised this reading
  since it was written: *"a file listing hashes paths without decoding them, and has to recognise a
  loaded image as the same photograph."*
- **The rendition handler needs the encode without the file.** There is no destination; the bytes go
  into an HTTP response.

*Alternative: `gui` depends on `rust-sak` directly.* Rejected by `opai/src/image/mod.rs`'s own
recorded reasoning for re-exporting `rust-sak`'s vocabulary — *"which is not a matter of taste: it is
pinned by git tag and published to no registry"*. A second manifest pinning it by tag is a second
place a revision bump has to be remembered.

*Alternative: `gui` encodes through the `image` crate itself.* It would compile and it would break the
files this slice exists for. A developed RAW is 16 bits per channel and the `image` crate's JPEG
encoder refuses that colour type; `rust-sak`'s encode dispatch narrows it instead. Every RAW preview
would fail, which is the one format a webview could never have drawn on its own.

*Alternative: compute the identity in the frontend.* The reference does **not** do this — Go's
`CreateFileTypes` hashes in the backend and the Wails frontend only ever reads `file.Hash`. It would
also be the wrong side of the boundary here in a way it is not there: the identity is this slice's
*capability key*, so a window that computed it could claim any identity it liked.

**Task budget.** This is one task, 1.3, which keeps the change at the 15 the project allows. It was
not in the original list because this design assumed both operations were already reachable.

## Risks / Trade-offs

**R1. The picker's threading rule is the bug class this repository has already hit.** The dialog
plugin's blocking picker must not be called from the main thread, while the native picker itself must
be opened on it; an `async` command is run on a runtime worker and is the combination that works, and
a synchronous command would deadlock. This is the same shape as the `tauri-plugin-opener` failure
already documented in `crates/gui/Cargo.toml` and `src/logs.rs`, where the wrong thread aborts the
process on macOS. → Implement the command as `async`, and verify the picker opens and cancels on
macOS before the slice is called done. Windows and Linux stay unverified, as R4 says.

**R2. The webview's cache is outside our control.** Having deleted a bounded LRU, nothing here bounds
what the webview retains. → Thumbnails are small and the full-resolution renditions are few - one per
image actually displayed - so the exposure is far below the 1.5 GB the reference budgeted for. If it
proves otherwise, the directive is one line and can be narrowed without touching the interface.

**R3. The registry only grows.** Nothing removes an entry, so a long session that opens and closes
many files accumulates path entries. → A path and an identity are tens of bytes; a thousand files is
noise. Closing an image is screen 09 and the natural place for the removal, and slice 3 or 4 will
have the file store that knows when it happens.

**R4. Two platform-shaped things are untested.** The rendition URL differs on Windows, and the
picker's threading is enforced differently on each platform. → The URL form is produced by
`convertFileSrc` rather than by us, and the CSP carries both forms, so the failure mode is a visible
broken image rather than a wrong one. Both stay unverified off macOS and are recorded as such.

**R5. A re-encode is not free.** D7 spends a decode and an encode where a JPEG passthrough would
spend neither. → It is what buys RAW, TIFF and HEIF working at all, and correct pixels for a misnamed
file. The work happens on a blocking thread and the result is cached by the webview, so it is paid
once per image and never on the window's event loop.

## Open Questions

**What bound the drawer and the miniature actually request.** The reference asks for 100 for a drawer
thumbnail; the design draws the strip at 128px tall with square items, and the sidebar panel at
144px. Whether those want 100, 128, 144 or a device-pixel-ratio multiple of one of them is a question
for the slice that draws them, and changes nothing here - the bound is a number in a URL and the
protocol treats every value the same way.
