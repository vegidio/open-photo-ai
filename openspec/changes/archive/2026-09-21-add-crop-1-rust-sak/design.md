## Context

See proposal.md - Why for the motivation. What shapes the approach:

`rust-sak`'s `image` module is a codec module: eight formats in, eight formats out, one
`DynamicImage` in the middle, laid out one public function per file (`decode_bytes.rs`,
`encode_file.rs`, `probe_file.rs`, ...) with `dispatch.rs`, `format.rs`, `info.rs`, `options.rs` and
`error.rs` behind them. It performs no pixel manipulation at all today.

The `image` crate it returns and accepts - which `opai` also depends on directly, because
`DynamicImage` is already in `opai`'s public API - covers two thirds of what a crop needs:
`imageops::flip_horizontal` / `flip_vertical`, `rotate90` / `rotate180` / `rotate270`, and
`crop_imm`. It has no rotation by an arbitrary angle, and none of the obvious neighbours do either
without adding a crate.

Downstream, three different consumers will want the rotated pixels, at three different sizes: the
canvas and sidebar miniature (a bounded rendition), the Crop/Rotate dialog's own preview (a bounded
rendition), and the input to an enhancement chain (full resolution). That spread is why the
primitive is a plain function over a `DynamicImage` rather than anything that knows about a file, a
cache or a crop.

## Goals / Non-Goals

**Goals:**

- One rotation primitive in `rust-sak`, general enough that a project with no crop feature would
  still want it.
- A sign convention and an expanded-canvas measurement that are stated, tested, and known to agree
  with the widget slice 3 will drive - before slice 3 is written.
- Bit depth preserved, so a developed 16-bit RAW is not flattened by being turned.
- No new third-party crate, in either repository.
- This repository left exactly as it was, save for one tag in the two manifests that pin it.

**Non-Goals:**

- Flip and crop wrappers. See D1.
- Composing flip, rotate and crop into one operation. That composition only means something once it
  produces an identity for the resulting pixels, and identity is `opai`'s vocabulary, not
  `rust-sak`'s. Slice 2.
- Any resampling quality knob (nearest, bicubic, Lanczos). Bilinear is what the reference uses and
  what a rotation of a photograph wants; a filter parameter can be added when something needs one.
- EXIF orientation. A separate concern from a rotation the user asked for, and not in this series.

## Slices

The crop feature, in order. Only the current slice is proposed; the rest are proposed when reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-crop-1-rust-sak` | `rotate` in `rust-sak`'s `image` module, released and pinned here | - | **current** |
| 2 | `add-crop-2-seam` | A crop reaching the pixels: composed in `opai`, carried in the `opai://` URL, applied to renditions and to the enhance input, reflected in the navbar dimensions | 1 | pending |
| 3 | `add-crop-3-dialog` | The crop button on the sidebar miniature and the whole Crop/Rotate dialog, every control screen 18 draws | 2 | pending |

Slice 2 sets no crop - nothing can, until slice 3 - and is verified against a crop injected in
tests. That ordering is deliberate: the seam lands ahead of the screen, so slice 3 never ships a
control that draws but does nothing.

### Decisions already taken for slices 2 and 3

Recorded here because this list is repeated into every slice's design.md and these are the choices
that shape them. They were settled with the repository owner during exploration.

- **The cropper widget is `react-advanced-cropper`**, the one the Wails app uses, accepting that its
  stylesheet has to be overridden into Tailwind 4 the way the reference already overrides it. The
  alternative was a hand-rolled stencil; the deciding factor was that `useCropController`'s
  edge cases port with the library and would otherwise be re-derived from nothing.
- **The crop is carried in the `opai://` URL**, and `Renditions` keys on identity, bound and crop.
  The alternative was a `set_crop` command holding it server-side, which would make the rendition
  key go stale and need invalidation - the one property `renditions.rs` is built to avoid.
- **The crop store is keyed by identity, not by path.** `stores/transform.ts` already made this
  call for the same species of state and argued it: the transform is keyed the way the pixels are.
  The reference keys its crop store by path.
- **The cropped picture's identity is derived by hashing, in `opai`.** The reference maintains two
  hand-synchronised token formats for this - Go's `CropCacheKey` emits `#c...`, TypeScript's
  `cropToken` emits `_c...` - and nothing keeps them in step. Hashing the crop into a real identity
  makes the cropped picture indistinguishable from any other to `serve.rs`'s parser and to the run
  cache, and deletes the TypeScript half outright.
- **The dialog works from a bounded rendition**, not the full-resolution original the reference
  fetches with `getImage(currentFile, 0)`, with coordinates scaled at the boundary and the typed
  width/height fields authoritative in source pixels. A large scan otherwise pays a full decode,
  encode, transfer and second decode to fill a widget under a thousand pixels tall.
- **A bounded rendition downscales before it rotates.** Rotating 60 megapixels to draw a 144-pixel
  miniature does the work and throws it away; the full-resolution warp is kept for the enhance
  input, where the pixels are the product.
- **Two of the reference's control behaviours are changed, and one is kept.** Picking an aspect
  ratio re-fits against the image bounds rather than inside the current box, so switching ratios is
  no longer monotonically destructive; the swap button selects the paired ratio (16:9 to 9:16) and
  keeps the lock rather than dropping to Free, the grid being built entirely in inverse pairs. The
  quarter-turn button still resets the fine rotation to zero, as the reference does.
- **The drawer's thumbnail strip stays uncropped.** This matches the reference, which is worth
  stating because it is easy to read as an oversight: `useThumbnail.ts` calls `getImage(file, size)`
  with no crop while `Preview/index.tsx` passes one, so the old app already shows an uncropped strip
  beside a cropped miniature.
- **Face detection and export take a crop in the reference and neither exists in this GUI yet.**
  Both must pick it up when they land. Recorded as a knowingly-open parity gap, not an oversight.

## Decisions

### D1 - Only `rotate` goes into `rust-sak`; flip and crop do not

The gap is rotation. `image::imageops::flip_horizontal`, `flip_vertical` and `crop_imm` already do
the other two, and `opai` depends on the `image` crate directly, so it can call them without a
wrapper. Adding `rust_sak::image::flip` and `rust_sak::image::crop` would be the project's
"do not reinvent what a dependency provides" rule applied backwards: a first-party wrapper whose
whole body is a call into a crate the caller already has.

*Alternative considered:* a single `rust_sak::image::transform(img, &Transform)` doing all three in
order. Rejected twice over - it would bake this application's flip-then-rotate-then-crop order into
a general-purpose toolkit, and the order only matters because the crop rectangle is expressed in the
rotated image's coordinate space, which is a fact about the cropper widget rather than about images.
The composition belongs in `opai`, next to the identity it produces.

### D2 - Positive is clockwise

The Go application's `ApplyCropInfo` negates the angle before calling `imaging.Rotate`, with a
comment explaining why: `imaging.Rotate` is counter-clockwise for positive angles while the frontend
cropper reports clockwise. That negation is a bug magnet sitting at the boundary between two
conventions.

Clockwise-positive matches CSS `transform: rotate()`, `react-advanced-cropper`'s reported rotation,
and the sign on the dialog's own slider label. Choosing it here means no call site ever negates, and
the number stored in a crop is the number the user saw.

*Alternative considered:* counter-clockwise-positive, matching `imaging.Rotate` and the mathematical
convention. Rejected: it optimises for resemblance to the Go source over correctness at every call
site, and this project's rule is that the user-visible result matches, not the code.

*Measured, not assumed (task 1.1).* A 240x120 image with a uniquely coloured marker in each corner
was rotated by +30 degrees in a scratch page driving `react-advanced-cropper` 0.20.1 - the version
the reference pins. The top-left marker landed at (63.5, 16.1) in the output; clockwise predicts
(64.4, 16.4) and counter-clockwise predicts (16.4, 124.4). The rendered image element's computed
transform was `matrix(1.547, 0.893, -0.893, 1.547, ...)`, a positive sine in `b` with y pointing
down, which is clockwise; and `advanced-cropper`'s own `rotatePoint` computes
`left' = dx cos - dy sin`, `top' = dx sin + dy cos`, clockwise in the same coordinates. The widget is
clockwise-positive, so `rotate` is too and no call site negates.

### D3 - The expanded canvas is the standard rotated bounding box

For a source of `w` by `h` turned by `t`:

```
    width  = w * |cos t| + h * |sin t|
    height = w * |sin t| + h * |cos t|
```

rounded outward, with the source's centre mapped to the destination's centre. This is what
`imaging.Rotate` produces and what `react-advanced-cropper` measures its coordinates against, so a
crop rectangle taken from the widget lands on the same pixels the backend produced.

This is the single most load-bearing fact in the series. A disagreement of one pixel between the
widget's idea of the rotated canvas and `rotate`'s would put every rotated crop slightly off, and it
would surface in slice 3 - two changes after the code that caused it. So the formula is pinned by
test here, and the widget's own behaviour is measured in this slice rather than assumed, because the
answer is also what fixes D2's sign.

*Measured, not assumed (task 1.1).* `advanced-cropper`'s `rotateSize` is
`abs(w cos t) + abs(h sin t)` by `abs(w sin t) + abs(h cos t)` - the same formula, term for term.
For 240x120 at +30 degrees it returns 267.846 x 223.923, and pushing the stencil out as far as it
will go clamps it to `{left: 0, top: 0, width: 267, height: 223}`: the widget reports its
coordinates against the expanded canvas, with the origin at that canvas's top-left corner.

One refinement follows from the same measurement. The widget's extent is the exact real-valued box
and its integer clamp *floors* it (267.846 to 267), while `rotate` rounds *outward* (to 268). The
Rust canvas is therefore at most one pixel larger per axis than the space the widget measured in, so
a crop rectangle taken from the widget is always strictly inside the rotated image - never past its
edge. That is the safe direction for the discrepancy to fall, and it is why the rounding stays
outward: rounding inward would let a maximal stencil ask for a column of pixels that does not exist.
Slice 2 clamps the crop rectangle to the rotated bounds regardless, for the same reason it would
clamp any value crossing a process boundary.

### D4 - The output promotes to the alpha-bearing sibling, and keeps its bit depth

Rotating by anything but a quarter turn produces destination pixels no source pixel covers. An
opaque output cannot represent them, so the result carries alpha: `Rgb8` to `Rgba8`, `Rgb16` to
`Rgba16`, `Luma8` to `LumaA8`, `Luma16` to `LumaA16`, `Rgb32F` to `Rgba32F`; an input that already
has alpha keeps its own type. That is every variant `DynamicImage` has, written out rather than
sampled, because a caller reading three rows of a five-row matrix has to guess at the other two -
and a variant `DynamicImage` gains later falls to `Rgba8`, which loses nothing an enumerated arm
would have kept.

Bit depth is preserved rather than normalised to eight bits. A developed RAW is `Rgb16` in this
project, and the enhance path is the caller that will hand one in - flattening it inside a rotation
would lose half the depth the RAW decoder exists to provide.

*Alternative considered:* always return `Rgba8`, as `imaging.Rotate` returns `NRGBA`. Simpler, one
code path, and wrong for 16-bit sources.

### D5 - Bilinear sampling, inverse-mapped, with f32 intermediates

Each destination pixel is mapped back through the inverse rotation into source coordinates and
sampled bilinearly from the four surrounding pixels; a destination whose source coordinate falls
outside the image is left fully transparent. Weights and accumulation are `f32` regardless of the
input's depth, so a 16-bit source does not overflow or quantise in the intermediate.

This is the same sampling the reference gets from `imaging.Rotate`, and `opai` already has the
idiom: `models/face_recovery/composite.rs` inverse-maps and reads bilinearly for its own reasons.

**The four neighbours are weighted by their own alpha**, accumulated premultiplied and divided back
out at the end, so a neighbour that is transparent contributes its coverage but not its colour.
Blending a transparent *black* neighbour in directly is what gives a naively rotated photograph a
dark fringe along its new edges - a straight bilinear average would pull the outermost half-pixel
towards zero on every channel. The edge this produces instead blends out to transparent with the
colour it had. Pinned by test, because the fringe is invisible in a synthetic image and obvious in a
photograph.

**The exact quarter turns do not take this path at all.** After the angle is taken modulo 360, `0`,
`90`, `180` and `270` return the source clone or the `image` crate's own `rotate90`/`180`/`270`,
which are memcpy-shaped rather than resamples. The sampler very nearly agrees with them already -
but `cos 90` is `6.1e-17` rather than zero in binary floating point, so it blends a sliver of the
neighbouring pixel about `1e-14` wide. Rounding to an integer subpixel hides that; an `f32` subpixel
keeps it, and so would every pixel of a rotated `Rgba32F` picture. Taking the exact path makes a
quarter turn exact by construction rather than by luck, which is also what lets the tests in task
3.2 assert equality rather than a tolerance.

### D6 - Transparent fill, with no fill-colour parameter

The uncovered area is fully transparent. No parameter, because nothing in this project wants another
colour: the crop that follows the rotation normally excludes the corners entirely, and where the
user deliberately drags the stencil over them, transparent is the honest answer.

*Alternative considered:* a `fill: Rgba<u16>` parameter, which `imaging.Rotate` takes and the Go
application passes `color.Transparent` to. Deferred rather than rejected - see Open Questions, where
export is the caller that might one day need it.

### D7 - No new crate; `imageproc` was the alternative

`imageproc::geometric_transformations::warp_into` with an affine projection would do D3 and D5 in
about ten lines, into a destination buffer sized by the caller. It was rejected on three counts:
`rotate_about_center`, the obvious entry point, keeps the source's dimensions rather than expanding,
so the expansion is hand-written either way; `imageproc` is a large crate to take for one function,
and `rust-sak`'s features are deliberately narrow; and its warp is generic over `Pixel` in a way
that makes D4's per-colour-type promotion awkward rather than free.

The hand-written version is on the order of fifty lines of arithmetic with no dependency, in a crate
whose whole purpose is to be the place such things live.

### D8 - `rust-sak` features used

`image`, which `opai` already enables. `rotate` needs the `image` crate and nothing else - no codec,
no `crypto`, no `fs` - so it goes behind the existing feature rather than gaining one of its own,
and `rust-sak`'s per-feature CI matrix already builds `image` in isolation.

### D9 - Release shape

Version bump to the CalVer number for the release month and a new tag, as `add-init-0-rust-sak` and
`add-image-raw-1-rust-sak` did. Consumers pin by tag, so a push is a release: the repository owner's
explicit go-ahead is a task of its own, before anything is pushed.

The pin bump lands in the same slice as the release rather than in slice 2, so that a cold
`cargo build --locked` in this repository proves the new function resolves, compiles and is public
on all three platforms while the dependency is still the thing being worked on.

## Risks / Trade-offs

- **The widget and the backend disagree about the rotated canvas or the sign.** → The formula and the
  sign are pinned by test in this slice, and `react-advanced-cropper`'s own conventions are measured
  here rather than in slice 3, because D2's answer depends on the measurement. If the widget turns
  out to report something other than D3, this slice's API changes while it is cheap to change.

- **A rotation is expensive at full resolution.** A 60-megapixel source is 60 million inverse maps
  and 240 million samples. → Slice 2's bounded renditions downscale first, so only the enhance
  input - where the pixels are the product - pays the full cost, and `Renditions` caches the
  result per size.

- **Transparent corners reach a model that expects three channels.** An enhancement chain fed a
  rotated picture may receive `Rgba8` where it expected `Rgb8`. → Not this slice's problem, but it
  is slice 2's and is named here so it is not discovered there: the composition in `opai` crops
  after it rotates, and a crop inside the rotated content carries no transparent pixels at all. The
  case that does is a stencil deliberately dragged over a corner.

- **A `rust-sak` tag can be deleted or moved after it is consumed**, and a stale one may still
  resolve from a local cargo cache while failing for everyone else. → The adoption task requires a
  *cold* `--locked` build, and `gh api repos/vegidio/rust-sak/tags` is checked after the tag is cut.

- **Fifty lines of hand-written arithmetic is fifty lines that can be subtly wrong**, in a way a
  smoke test passes and a photograph reveals. → Tests pin the bounds formula, the sign, the identity
  case (0 degrees returns the source unchanged in every respect but colour type), the quarter-turn
  cases against the `image` crate's own exact `rotate90`/`180`/`270`, and round-tripping by +t then
  -t returning the original content.

## Migration Plan

Nothing to migrate. The new function has no callers in this repository at the end of the slice, so
the rollback is reverting one line in `crates/opai/Cargo.toml` and the lock file.

## Open Questions

- **Does `rotate` eventually need a fill colour?** Export writes JPEG, which carries no alpha, so a
  rotated-and-exported picture with transparent corners has to resolve them to something. That is a
  question for the slice that builds the export queue, and answering it later costs one optional
  parameter with a transparent default - it changes no signature in use and no task here.
