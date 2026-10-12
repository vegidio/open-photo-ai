## Why

Slice 1 put an arbitrary-angle `rotate` into `rust-sak` and pinned it here, and nothing calls it: no image
in this application can be flipped, turned or cropped, and nothing carries a crop from the window to the
pixels. This slice builds that seam — the composition, the crop on the `opai://` URL and on the enhance
command, and the cropped size in the window — so that slice 3's Crop/Rotate dialog is a screen
wired to something that already works, rather than a screen and its backing in one change.

The ordering is the series' own: the seam lands ahead of the dialog, so slice 3 never ships a control
that draws but does nothing.

## What Changes

Task estimate: **15**, which is the ceiling exactly. The series is three slices (see design.md -
Slices); this proposal covers the second. Nothing in this slice *sets* a crop — only slice 3 can — so
its own verification is against a crop injected in tests, and its user-visible effect on an uncropped
application is nil.

**`opai` is not touched.** An earlier draft of this slice put the crop there, inheriting a decision slice
1 recorded; design.md D2 records why that was reversed. Reading a file, writing one and running inference
is what the core library is for, and a framing a user dragged in a coordinate space one widget defines is
none of the three. The Go application draws the same line — every one of its crop sites is under
`cmd/gui` — and both callers here are in `gui` too. It is promoted to `opai` by the change that gives it
a second front end, not in anticipation of one.

**In `gui` (Rust):**

- **A `Crop` value and the composition that applies it**, in `crates/gui/src/images/crop.rs`: flip
  horizontally, flip vertically, rotate, then cut the rectangle — the reference's order, because the
  rectangle is expressed in the rotated image's coordinate space. It takes an `opai::Picture` and answers
  one, pairing the framed pixels with an **identity derived by hashing** the source's identity together
  with the crop, through `opai::Picture::new` — the constructor whose documentation says it exists for
  exactly this. That is what makes a cropped picture indistinguishable from any other picture to the run
  cache `opai::process` keys on.
- **The rotation angle crosses as an integer count of millidegrees**, not as a float. A crop is part of a
  cache key on both sides of the IPC boundary, and `f64` is neither `Eq` nor `Hash`; fixing the unit at
  1/1000 of a degree makes the crop a comparable, hashable value and removes float formatting from a URL.
- **`Crop::scaled`**, so a bounded picture can be framed in a reduced coordinate space rather than
  rotating 60 megapixels to fill a 144-pixel miniature.
- **`crates/gui/Cargo.toml` gains `rust-sak`**, at the tag `opai` and `perf` already pin, with two
  features: `image` for `rotate` and `crypto` for `xxh3_string`. A crate outside the core library pinning
  it narrowly is what `perf` already does. Nothing new is compiled — `opai` enables both features from
  the same source and tag — and the cost is a third pin that must move in lockstep with the other two.
- **Nothing from `rust-sak` is rebuilt.** The rotation is `rust_sak::image::rotate` from slice 1; the two
  flips and the rectangle cut are `DynamicImage`'s own `fliph`, `flipv` and `crop_imm`, which slice 1's
  D1 deliberately left in the `image` crate rather than wrapping (the methods rather than the
  `image::imageops` free functions of the same names, which would flatten a sixteen-bit source — D13); the identity hash is
  `rust_sak::crypto::xxh3_string`, the same function every derived identity in this project is folded
  with. What this slice adds is the composition and the identity, neither of which is general-purpose.
- **A request for an opened image's pixels may carry a crop**, alongside the bound it already carries.
  `Renditions` is keyed on what was asked for, so the crop joins the key with no invalidation to reason
  about — the property that key already has. A request with no crop is byte-for-byte the URL it is today.
- **A bounded rendition downscales before it rotates**, with the crop scaled by the same factor, so the
  sidebar's miniature costs a small warp rather than a full-resolution one.
- **The enhance command takes the crop** and applies it to the loaded picture before the chain runs, so
  the chain's input, the per-operation cache key and the result's identity are all the cropped picture's.

**In `gui` (frontend):**

- **A crop store keyed by identity**, forgotten when its image is closed, not persisted. It has no writer
  in this slice; the dialog in slice 3 is its writer.
- **The crop travels on the rendition URL and on the enhance call**, so the canvas and the sidebar's
  miniature draw the cropped photograph and a run enhances it.
- **The navbar reports the cropped size**, and gains the design's hover popover comparing the original,
  cropped and output dimensions — the block the navbar has drawn since `add-gui-1-foundation` with a
  comment saying the popover was left out because, with no crop and no scale, all three numbers were one.
  Both are now real. The keys it needs (`navbar.dimensions.original`, `.cropped`, `.output`) are already
  in all thirteen locale files, carried over with the catalogue.
- **A crop change re-runs the chain and releases the held result**, the way changing image does rather
  than the way changing an enhancement does: the pane's box changes shape in the same render its pixels
  do, so a result made from other pixels is never drawn to the new shape.

## Capabilities

### New Capabilities

None. Every behaviour here is an existing capability gaining a crop.

### Modified Capabilities

- `gui-images`: a request for an opened image's pixels may carry a crop; how a crop frames a photograph —
  the order, the direction, the clamping, the transparent corners and the bit depth; what an identity, a
  bound and a crop denote.
- `gui-enhance`: a run carries the crop of the image it runs on; the result identifies the cropped input;
  a crop change starts a new run and releases the previous result.
- `gui-shell`: the canvas and the sidebar's miniature draw the cropped photograph; the navbar reports the
  cropped size and compares the three sizes on hover.

## Impact

**Crates touched:** `gui` alone — the crop value and its composition, the rendition request, the enhance
input, and the frontend. `opai` and `cli` are untouched.

**Why the logic in the front end is not logic that belongs in `opai`.** The project's test is whether
both front ends would need it, and today neither the CLI nor the library does: a framing exists because
a window let someone drag one, in the coordinate space that widget defines, which is also what fixes the
flip-flip-rotate-cut order. What `opai` needs from this is a `Picture` whose identity describes its
pixels, and `Picture::new` is public and documented as the reviewed place a caller pairs the two — so
nothing is trapped here. `crop.rs` is a plain function over a `Picture`; the change that gives it a
second front end moves it into `opai` without rewriting it, which is this project's stated rule for when
a file's tier is re-decided.

**`rust-sak`:** nothing to extend, and this is the slice that spends what slice 1 added. `rotate` covers
the only gap the feature had; `crypto`'s `xxh3_string` covers the identity; the flips and the rectangle
cut are the `image` crate's and were deliberately not wrapped. The tag does not move and no feature is
added to it; what changes is that a third manifest — `gui`'s — now pins it, with `image` and `crypto`.

**Third-party crates introduced:** none, in either repository. No frontend package either — the cropper
widget is slice 3's.

**Parity divergences — four, each deliberate:**

- **The cropped picture gets a real identity, and the two hand-synchronised token formats go.** The
  reference maintains `CropCacheKey` in Go (emitting `#c…`) and `cropToken` in TypeScript (emitting
  `_c…`), appends one to the image hash and nothing keeps them in step. Here the crop is hashed into an
  identity in one place, and the TypeScript half does not exist.
- **A bounded rendition is cropped; the reference's is not.** `GetImage` applies the crop only when
  `size == 0`, so the old app's sidebar miniature is the *uncropped* photograph while its canvas is
  cropped. That is an artefact of cropping at full resolution being too expensive to do for a thumbnail,
  which downscaling first removes.
- **The drawer's thumbnail strip stays uncropped**, which *matches* the reference and is recorded because
  it reads as an oversight: `useThumbnail.ts` passes no crop while `Preview/index.tsx` passes one.
- **A crop change releases the enhanced result.** The reference keeps the previous result on screen. Here
  the canvas sizes its box from the photograph's own dimensions, so holding a result made from other
  pixels would draw it to the wrong aspect ratio until the re-run lands.

**Knowingly left open:** face detection and export both take a crop in the reference and neither exists
in this application yet; both must pick it up when they land. The `rotate` fill colour question slice 1
parked belongs to export for the same reason. Carried forward from slice 1 rather than reopened here.

**Not in scope:** the crop button on the sidebar miniature, the Crop/Rotate dialog and every control in
it, the cropper widget and its stylesheet — all slice 3. Persisting a crop across restarts, which
matches the reference and the transform store beside it.
