## Context

See proposal.md - Why for the motivation. What shapes the approach:

**What slice 1 left.** `rust_sak::image::rotate(&DynamicImage, f64) -> DynamicImage` turns a picture
clockwise for a positive angle onto the canvas `ceil(w|cos t| + h|sin t|)` by `ceil(w|sin t| + h|cos t|)`,
samples bilinearly with alpha-weighted neighbours, leaves uncovered area transparent, promotes to the
alpha-bearing sibling of the input's colour type and preserves bit depth. It is pinned here at
`rust-sak` 26.9.7 and has no caller.

**Where pixels reach the window.** `crates/gui/src/images/serve.rs` answers `opai://<identity>?size=<n>`:
it parses the request into `Asked { identity, bound }` — a type whose own doc comment already says a crop
is "left room for without being named yet" — resolves the identity to an admitted file or a held run
result, decodes, downscales through `bounded` (a `thumbnail` prefilter then Lanczos3), and encodes to
JPEG, or PNG where the picture carries alpha. `Renditions` is keyed on `Asked`, which is why it needs no
invalidation: the key covers everything that determines the pixels.

**Where an enhancement starts.** `crates/gui/src/enhance/run.rs`'s `enhance_with` judges the chain,
resolves the source identity to a path, writes the run slot, loads the file and calls `Opai::process`.
The result's identity is composed by `opai`'s `identity_after` from the *source picture's* identity, the
operations and the resolved depth — so whatever picture is handed in is what the whole chain and the
per-operation cache key off.

**What derives an identity today.** `inference/process.rs` folds `xxh3_string(&format!("{identity}|…"))`,
once per operation for a picture and once with a `DATA_DISCRIMINATOR` for a result that is not a picture,
with a test asserting the two derivations cannot collide. Both are `pub(crate)` and internal to a run.
What is public is `Picture::new`, whose own documentation says it "exists for the cases that cannot [read
the bytes and derive both], and is deliberately the single reviewed place where the invariant the rest of
the type protects could be broken" — a caller holding pixels it computed itself and the identity it
derived for them is exactly that case.

**Which crates take `rust-sak`.** `opai` and `perf` both pin it by git tag, `perf` with the single `fs`
feature. So a crate outside the core library depending on it directly, with a narrow feature set, is
established here rather than new — which is what makes D2's placement affordable.

**What the window knows about a photograph's size.** `ImageRecord` carries the file's own `width` and
`height`, read from the header without decoding. The canvas sizes its `<img>` box from those numbers
*before* the pixels arrive (`PreviewImage`'s `published`), and the navbar prints them. Both therefore
need the framing's dimensions from the crop itself, not from the picture that eventually loads.

**What the reference does.** `ApplyCropInfo` flips, rotates (negating the angle) and cuts; `GetImage`
applies the crop only when `size == 0`; `runInference` applies it and then appends `CropCacheKey` to the
image hash, whose format is duplicated by hand in TypeScript as `cropToken`; `NavbarDimensions` shows the
output size when an upscale is in the stack and the cropped size otherwise, with a popover comparing all
three.

## Goals / Non-Goals

**Goals:**

- One place that knows what a crop does to pixels and what identifies the result, placed where its
  callers are today and promoted when they move.
- A crop that travels in the request rather than being held anywhere, so nothing can go stale.
- A bounded cropped rendition that costs about what an uncropped one costs.
- The window sized and labelled from the framing, before its pixels arrive.
- Nothing in this slice able to *set* a crop, and nothing in the application behaving differently until
  something can.

**Non-Goals:**

- The dialog, the cropper widget, the crop button, and the aspect-ratio and quarter-turn controls —
  slice 3, whose decisions are already recorded below.
- Persisting a crop across restarts. The reference does not, and neither does the transform store this
  one is modelled on.
- Cropping the drawer's strip. See D12.
- Face detection and export taking a crop: neither exists in this application yet, and both are recorded
  as an open parity gap carried from slice 1.

## Slices

The crop feature, in order. Only the current slice is proposed; the rest are proposed when reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-crop-1-rust-sak` | `rotate` in `rust-sak`'s `image` module, released and pinned here | - | done |
| 2 | `add-crop-2-seam` | A crop reaching the pixels: composed in `gui`, carried in the `opai://` URL, applied to renditions and to the enhance input, reflected in the navbar dimensions | 1 | **current** |
| 3 | `add-crop-3-dialog` | The crop button on the sidebar miniature and the whole Crop/Rotate dialog, every control screen 18 draws | 2 | pending |

This slice sets no crop — nothing can, until slice 3 — and is verified against a crop injected in tests.
That ordering is deliberate: the seam lands ahead of the screen, so slice 3 never ships a control that
draws but does nothing.

### Decisions already taken for slice 3

Carried forward from slice 1's design, where they were settled with the repository owner. The ones this
slice acts on are marked; the rest are slice 3's and are repeated so they are not lost with the archive.

- **The cropper widget is `react-advanced-cropper`**, the one the Wails app uses, accepting that its
  stylesheet has to be overridden into Tailwind 4. Slice 3.
- **The crop is carried in the `opai://` URL**, and `Renditions` keys on identity, bound and crop — the
  alternative being a `set_crop` command holding it server-side, which would make the rendition key go
  stale. *Acted on here: D4.*
- **The crop store is keyed by identity, not by path**, as `stores/transform.ts` already argues for the
  same species of state. The reference keys its crop store by path. *Acted on here: D8.*
- **The cropped picture's identity is derived by hashing**, deleting the reference's two
  hand-synchronised token formats. *Acted on here: D3.* Slice 1 recorded this as "in `opai`"; that half
  was revisited when this slice was planned and the composition lives in `gui` instead — see D2, which
  records the argument and the evidence rather than leaving the reversal implicit.
- **The dialog works from a bounded rendition**, with coordinates scaled at the boundary and the typed
  width/height fields authoritative in source pixels. Slice 3, and D5 here is what makes a bounded
  rendition of a framing meaningful.
- **A bounded rendition downscales before it rotates.** *Acted on here: D5.*
- **Two of the reference's control behaviours change and one is kept** (aspect ratio re-fits against the
  image bounds; the swap button selects the paired ratio; the quarter-turn button still resets the fine
  rotation). Slice 3.
- **The drawer's thumbnail strip stays uncropped**, matching the reference. *Acted on here: D12.*
- **Face detection and export take a crop in the reference and neither exists in this GUI yet.** Both
  must pick it up when they land. Still open.

## Decisions

### D1 - What a crop is, and why its angle is an integer

`Crop`, in `crates/gui/src/images/crop.rs`, carries a horizontal flip, a vertical flip, a rotation and
the rectangle to cut — the reference's `CropInfo` fields, minus its "a zero value means no crop"
convention, which Rust spells as `Option<Crop>` instead.

**The rotation is an `i32` count of millidegrees, not an `f64` of degrees.** A crop is part of two cache
keys — `Renditions`' map key and the derived identity a run is given — and `f64` is neither `Eq` nor
`Hash`, because `NaN != NaN` and `0.0 == -0.0` while their bit patterns differ. Fixing the unit at 1/1000
of a degree makes `Crop` a plain `Copy + Eq + Hash` value: the map key is exact, the derived identity is
exact, and the URL carries an integer rather than whatever `f64::to_string` produced on the frontend.
`Crop::rotation_degrees() -> f64` is what `rotate` is called with. A thousandth of a degree over a
60-megapixel edge is a fifth of a pixel, so nothing the user can express is lost.

*Alternatives considered:* an `f64` field with a hand-written `Hash` over `to_bits` and a normalisation
of `NaN` and `-0.0` — more code, in the place where getting it wrong means two crops that look identical
keying differently; and passing the angle as a string, which moves the same problem into text.

The rectangle is `u32` and expressed in the rotated picture's coordinate space, as the widget reports it.
`Crop::new` refuses a zero-width or zero-height rectangle, so the type cannot describe nothing.

### D2 - The whole crop lives in `gui`, and `opai` is untouched

`crates/gui/src/images/crop.rs` holds the `Crop` value and `cropped(&Picture, Crop) -> Picture`, which
flips, rotates, cuts and names the result. Nothing is added to `opai` by this slice.

`opai` reads and writes image files and runs inference. A crop is neither: it is a framing the user
dragged, in a coordinate space one widget defines, ordered flip-flip-rotate-cut *because* that widget
reports its rectangle against the rotated picture. What `opai` needs is a `Picture` whose identity
honestly describes its pixels, and `Picture::new` is public and documented as the reviewed place a
caller pairs the two. The seam is already the right shape; nothing has to move to use it.

The evidence runs the same way. In the Go application every crop site is under `cmd/gui` —
`image_service.go`, `face_service.go` and `utils/operations.go` — and the core library never sees one.
Both callers here are in `gui` too: `serve.rs`, which answers what the window draws, and
`enhance/run.rs`, which prepares what a run is given. This project's own rule is that a file's tier is
decided by its callers and re-decided whenever one is added, so the crop is promoted to `opai` by the
change that gives it a second front end — a CLI that crops, or an export queue that lands in the
library — and not before, on the strength of expecting one.

**This reverses the "in `opai`" half of a decision slice 1 recorded.** The argument that put it there was
that deriving an identity means hashing through `rust_sak::crypto`, which `gui` did not depend on. That
is a dependency question, not a placement one, and it is answered by `gui` pinning `rust-sak` the way
`perf` already does (D13). The other half of that argument — that the reference's two hand-synchronised
token formats should become one derived identity — survives the move untouched, because the number of
producers goes from two to one either way.

*Alternative considered:* keeping a `derive_identity(source, tag)` in `opai` so no front end hashes. It
leaves `opai` carrying a function that exists only for the crop, which is the same objection in a smaller
package.

What was never in question is *how large a picture the window wants*: `bounded` already lives beside the
rendition handler on exactly that argument, and D5's "reduce first, then warp" is a cost decision about
drawing.

### D3 - The derived identity, and the crop that changes nothing

`xxh3_string(&format!("{identity}|crop|{rotation}|{flip_h}{flip_v}|{left}|{top}|{width}|{height}"))`,
in the same shape `identity_after` folds an operation and `identity_of_data` folds a non-picture result —
the same hash function, reached from `gui`'s own `rust-sak` pin (D13), and paired with the pixels through
`Picture::new`.

The `crop` discriminator is what keeps this derivation clear of `opai`'s two. It cannot be pinned by a
test the way `identity_of_data`'s is, because those two functions are `pub(crate)` and a `gui` test
cannot call them; what is asserted instead is the property that matters from this side — a cropped
picture's identity differs from its source's, two crops differ from each other, and the same crop derives
the same value every time. The risk that a crop fold collides with an operation fold is covered by the
discriminator being a word no `cache_tag` emits, stated here because a test cannot say it.

**A crop that changes nothing answers the source picture unchanged, identity included** — no flip, zero
rotation, and a rectangle covering the whole picture. This mirrors `process`'s own "an empty chain
returns the source unchanged, identity included", and it is not a micro-optimisation: slice 3's dialog
can be opened and dismissed having changed nothing, and without this rule that would mint a new identity
for identical pixels and re-run the entire chain that had already been computed.

*Alternative considered:* the reference's approach — apply the crop, then append a token to the source's
hash. Rejected in slice 1 and stated again because its cost is visible here: the token is a second
description of the framing that every consumer has to know to strip or append, which is why it exists
twice in the reference and drifts.

### D4 - The crop travels in the request, as one compact query parameter

`opai://<identity>?size=<n>&crop=<left>,<top>,<width>,<height>,<millidegrees>,<flips>`, where `<flips>`
is one of `-`, `h`, `v`, `hv`. Absent `crop`, the URL is byte-for-byte what it is today, so every
existing rendition, cached response and test is untouched.

Parsed as strictly as `size` already is: a `crop` that does not parse in full refuses the request rather
than being read as "no crop". `serve.rs`'s existing argument applies unchanged — a malformed URL must not
silently serve a full-resolution photograph where a thumbnail was expected, and it must not serve the
whole photograph where a framing was asked for.

`Asked` gains `crop: Option<Crop>`, and because `Renditions` is keyed on `Asked`, the crop joins the
cache key with no further work and no invalidation: the key still covers everything that determines the
pixels.

*Alternative considered:* JSON in the query, which would let one encoder serve both doors (D8's "one type,
two spellings"). Rejected: it is unreadable in a log line, percent-encoding it makes the cache key
sensitive to how the encoder orders fields, and the positional form is six numbers whose parser is
twenty lines and refuses everything else.

### D5 - A bounded rendition reduces first, and the crop is scaled to match

For a request carrying both a bound and a crop, the order is: reduce the *source* so the framing will
land near the bound, scale the crop by the reduction actually achieved, then flip, rotate and cut.

The factor is `bound / max(crop.width, crop.height)`, capped at 1 so nothing is ever enlarged. The source
is reduced through the existing `bounded` — one downscale implementation, not two — and the crop is then
scaled by the factor the reduction *actually achieved* (`new_width / old_width`), not by the factor asked
for, so rounding in the resize cannot put the rectangle a pixel off the framing.

This is what makes a cropped miniature affordable: rotating 60 megapixels to draw a 144-pixel thumbnail
does the work and throws it away. An unbounded request — what the canvas asks for — takes the factor 1
path and is therefore exact: its dimensions are the crop rectangle's, which is what lets the window size
its box from the crop before the pixels arrive (D9).

**The cost is that a bounded framing may differ from `crop.width * factor` by a pixel**, absorbed by the
clamp in D6. The reference sidesteps this by not cropping bounded renditions at all, which is the parity
divergence the proposal records.

### D6 - Clamping is total: there is no "crop out of bounds" error

The rectangle is brought inside the rotated picture's bounds — origin clamped into the picture, extent
clamped to what remains, with a floor of one pixel on each edge — and nothing fails. Slice 1's D3 already
established that the Rust canvas is at most one pixel *larger* per axis than the space the widget
measures in, so a rectangle from the widget is always strictly inside it; what clamping guards against is
the process boundary and D5's rounding, not the user.

*Alternative considered:* a `CropError` arm, refusing a rectangle that does not intersect. It would add a
failure mode to `opai`, a `Refusal` variant with its own status code in `serve.rs`, and an `EnhanceError`
arm — three boundaries widened for a case no interface can produce, and whose honest answer ("here is the
nearest pixel that exists") is cheaper than the error.

### D7 - The crop is applied where the enhance seam already resolves the source

`enhance_with`'s ordered contract gains one step: judge the chain, resolve the source, write the slot,
**load, then crop**, then run. The crop is applied to the loaded `Picture` before `Opai::process` sees it,
so every downstream consequence follows for free — the chain runs over the framed pixels, each step's
cache key is folded from the framed picture's identity, and the result's identity is composed from it.
Nothing in `opai::process` learns about crops.

It is applied after the slot is written, not before, for the reason the existing order gives: the slot is
written before the decode so a displaced run stops immediately rather than after slow work, and a
full-resolution warp is slow work.

*Alternative considered:* a `crop` field on `ProcessOptions`, applied inside `process`. Rejected: it would
put a second way of composing an identity inside the function that already composes one, and `process`'s
input would stop being the picture its caller handed it.

### D8 - One crop value, two spellings, each pinned on both sides

The frontend holds `CropInfo` in `ipc/crop.ts` beside the function that spells it for a URL; `ipc/images.ts`
imports both, and `ipc/enhance.ts` sends the same value as a command argument, where serde decodes it into
the same `Crop`. Two encodings of one value is the cost of having two doors; each is pinned by a
test on both sides, the way this project already pins every command name twice and the scheme three times.

`stores/crop.ts` holds `Map<identity, CropInfo>`, keyed by identity for the reason `stores/transform.ts`
states at length — the framing is keyed the way the pixels are, and a file whose bytes could not be read
has no rendition to frame. It registers as a file owner so closing an image forgets its crop, and it is
not persisted. **It has no writer in this slice.** That is the slice boundary working as intended, and
the store's own documentation says so, so nobody reads it as an oversight.

### D9 - The window sizes and labels from the crop, not from the served picture

`PreviewImage` hands its pane a `published` size that the `<img>` box is laid out from *before* the
rendition loads — that is the whole point of `useSettledFile`. So the framing's dimensions have to come
from the crop's own rectangle (`crop.width` x `crop.height`), not from the picture that eventually
arrives. One helper answers "the dimensions of this record under this crop", and the canvas, the navbar
and its popover all read it.

This is exactly what the reference's `cropDimensions` does, and it is only correct because D5 leaves the
unbounded rendition exact.

### D10 - A crop change releases the held enhanced result

The result held by `useEnhancementRun` travels beside the source identity it was made from, which is what
makes "a result belongs to the photograph it was made from" a property of the value rather than a race.
The crop joins it: a render whose crop differs from the held result's does not match, so the enhanced
pane falls back to the newly framed source and a run over the new framing starts.

That is the *image-change* behaviour rather than the *enhancement-change* behaviour, deliberately, and it
diverges from the reference. Changing an enhancement keeps the previous result on screen because the
shape of what is drawn does not change; changing a crop changes the pane's box in the same render, so
holding the previous result would draw it to the wrong proportions until the re-run landed.

The crop also joins the run effect's dependencies, so changing it stops the run in flight and starts one
over the new framing — the same rule the stack, the image and the processor already follow.

### D11 - The navbar shows the effective size, and compares all three on hover

The block shows the enhanced result's dimensions when the stack enlarges the image, and otherwise the
framing's, falling back to the file's — the reference's own rule, read off `NavbarDimensions`. Resting on
it shows Original, Cropped (only when a crop is set) and Output, which is what the design draws and what
the navbar's existing comment says was left out because all three were one number. Both halves of that
reason are now gone: a crop makes original and cropped differ, and the upscale stack makes output differ.

The three keys — `navbar.dimensions.original`, `.cropped`, `.output` — are **already present in all
thirteen locale files**, carried over with the catalogue, so this needs no new strings. The popover is
built from the project's existing `components/ui/popover.tsx` primitive rather than a new one.

### D12 - The drawer's strip stays uncropped

`DrawerItem` builds its thumbnail URL with a bound and no crop, and this slice leaves it that way. The
strip answers "which photographs are open"; the reference behaves the same way (`useThumbnail.ts` passes
no crop while `Preview/index.tsx` passes one). Stated in the spec as well as here, because a cropped
miniature beside an uncropped strip reads as a bug otherwise.

### D13 - Crates and `rust-sak` features

**No third-party crate, in either repository, and no new frontend package.** The four pieces are already
written: `rust_sak::image::rotate` behind its `image` feature, `rust_sak::crypto`'s `xxh3_string` behind
its `crypto` feature, and `DynamicImage`'s own `fliph` / `flipv` / `crop_imm` from the `image` crate
`gui` already depends on. Slice 1's D1 decided the flips and the cut are not to be wrapped in `rust-sak`;
nothing here revisits that.

**The methods on `DynamicImage`, not the `image::imageops` free functions of the same names.** The free
functions take a `GenericImageView`, which sees a `DynamicImage` as `Rgba<u8>` — so they would flatten a
developed sixteen-bit RAW on the way through, and the gui-images requirement that a framing keeps the
depth of the photograph it was taken from would be broken by the very call that frames it. The methods
dispatch on the variant and keep it. Stated because the two spellings read as interchangeable and only
one of them is correct here.

**`crates/gui/Cargo.toml` gains `rust-sak`**, at the tag `opai` and `perf` already pin, with exactly two
features: `image` for `rotate` and `crypto` for `xxh3_string`. Neither compiles anything new — features
union across the graph and `opai` already enables both, from the same git source and tag, so the lock file
still holds one `rust-sak` entry.

**The cost is a third pin to move in lockstep.** Two tags of one git source resolve to two entries in the
lock file, which is the property crop-1's adoption task already had to respect for `opai` and `perf`; it
now covers three manifests. Stated here because a future `rust-sak` release that bumps two of the three is
a lock file with two copies of the crate and `DynamicImage` as two unrelated types.

The `rust-sak` tag itself does not move and gains no feature. Nothing in this slice is general-purpose
enough to belong there: a composition ordered by one widget's coordinate convention, and an identity
derived in this project's own scheme, are both this application's.

### D14 - What this is validated against

Platform-independent and provider-independent: no session, no model, no execution provider and no memory
budget is touched, so the ownership and lifetime questions those raise do not arise here. The one
platform-shaped surface is the rendition URL, which differs by platform (`opai://localhost/…` versus
`http://opai.localhost/…`) and is built by `convertFileSrc` in the single place that already owns that
difference; the path and query are identical on both, which is what the existing parser depends on.

Verified on macOS by the suite and by hand with an injected crop; Windows and Linux are covered by CI's
build and test run, as every previous GUI slice has been.

## Risks / Trade-offs

- **A crop store with no writer reads as dead code**, and a later reader may "clean it up". → Its own
  documentation states that slice 3 is its writer, the spec deltas describe the behaviour it serves, and
  the tests exercise it through an injected crop, so removing it fails the suite rather than passing it.

- **Two spellings of one crop can drift** — the query form and the command's JSON. → A round-trip test on
  each side, and the frontend builds both from one `CropInfo` value in one module. This is the same
  arrangement, and the same mitigation, as the command names written twice and the scheme written three
  times.

- **The bounded framing can be a pixel off the exact one.** → D5 scales by the achieved reduction rather
  than the requested one, D6 clamps what is left, and the unbounded rendition the canvas draws is exact.
  A thumbnail a pixel narrow is invisible; the framing the user works in is not affected.

- **A full-resolution warp on the enhance path is expensive** — a 60-megapixel source is 240 million
  samples before the first model runs. → It is paid once per run rather than per draw, it is the path
  where the pixels are the product, and the run cache keys on the framed identity so a repeated chain
  over the same framing pays nothing at all. The renditions the window draws never take it (D5).

- **A cropped picture carries alpha** where the source did not, because rotation exposes corners, and an
  enhancement chain may be handed `Rgba8` where it expected `Rgb8`. → Named in slice 1 as this slice's
  problem. A rectangle inside the rotated content carries no transparent pixel, which is every crop a
  widget stencil can produce; a deliberately over-dragged corner is the case that does, and the pipelines
  already accept an alpha-bearing `DynamicImage` because a PNG with transparency has always been able to
  reach them. Asserted by a test rather than assumed.

- **The navbar now reads the enhancement stack** to report an output size, which is a second region
  depending on the stack's shape. → It reads the same `useFileEnhancements` selector the sidebar does,
  and the one number it derives (the upscale factor) is computed in `lib/enhancements.ts` beside the
  other derivations of that shape rather than inline in the navbar.

- **A `gui` crate that hashes and warps is a front end doing image work**, which is the shape the project
  warns about — logic trapped inside an interface. → The test the project actually states is "would both
  front ends need it", and today neither the CLI nor anything in `opai` does: the framing exists because
  a window let the user drag one. The mitigation is the promotion rule in D2, applied when a second
  caller appears rather than in anticipation of one, and a `crop.rs` that is a plain function over a
  `Picture` — nothing about it would have to be rewritten to move it into `opai` later.

- **The crop's identity derivation can no longer be tested against `opai`'s two.** `identity_after` and
  `identity_of_data` are `pub(crate)`, so a `gui` test cannot assert the three cannot collide the way
  `opai`'s own test asserts it for two. → The discriminator is a word no operation's `cache_tag` emits,
  and what a `gui` test can assert — a cropped picture differing from its source, two crops differing,
  one crop deriving stably — is asserted. This is a real loss of verification against the previous plan
  and is recorded rather than glossed.

## Migration Plan

Nothing to migrate. Until slice 3 ships a writer for the crop store, every URL, every command argument
and every cache key is exactly what it is today, so the slice can be reverted by removing the crop field
from three seams with no stored state to unwind.

## Open Questions

- **Should the sidebar's miniature keep sharing the drawer's bound?** It does today, which makes the
  miniature free because the strip has already produced that rendition. With a crop it cannot share, since
  the strip is uncropped (D12) — a cropped miniature is its own rendition at the same bound. The bound
  itself stays as it is; whether the miniature wants a different one is a question about how it is drawn,
  answerable in slice 3 when the dialog makes cropping ordinary, and it changes nothing here.
