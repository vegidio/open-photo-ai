## Why

Crop is one of the domain concepts this rewrite is required to preserve, and the design draws it in
two places: the crop button on the sidebar miniature (screen 05) and the near-fullscreen Crop/Rotate
dialog (screen 18). The dialog's centrepiece is fine rotation - a slider from -90 to 90 degrees with
15-degree marks - so the picture the user frames is a picture rotated by an arbitrary angle, not a
quarter turn.

Nothing in this stack can produce that picture. The `image` crate flips (`imageops::flip_horizontal`,
`flip_vertical`), turns by quarter turns (`rotate90`/`180`/`270`) and crops (`crop_imm`), and stops
there: rotation by 12 degrees, with the canvas expanded to hold the corners and the uncovered area
left transparent, does not exist in it. `rust-sak`'s `image` module is this project's image module
and it does not have one either.

By the project's own rule, a close-but-incomplete `rust-sak` module is extended rather than worked
around, and an expanding arbitrary-angle rotation is as general-purpose as a decoder: it is the
missing third of the flip/rotate/crop trio the `image` crate already covers two of. This slice adds
it there and cuts the release; the two slices after it build the crop seam and the dialog.

## What Changes

Task estimate: **10**. The crop feature end to end - the rotation primitive, the seam that carries a
crop from the window to the pixels, and the dialog with every control screen 18 draws - runs well
past the 15-task ceiling, so it is split into three slices (see design.md - Slices). This proposal
covers the first.

**In `../rust-sak` (a separate repository):**

- **One new function on the existing `image` module: `rotate`.** It takes a `DynamicImage` and an
  angle in degrees and returns the rotated picture on a canvas expanded to contain it, sampled
  bilinearly, with the area no source pixel covers left fully transparent. It joins the module's
  existing file-per-function layout as `src/image/rotate.rs`.
- **Nothing for flip and nothing for crop.** Both are already in the `image` crate, which this
  module returns and takes `DynamicImage` from and which `opai` already depends on directly.
  Wrapping `flip_horizontal` and `crop_imm` in `rust-sak` would be the "do not reinvent what a
  dependency provides" rule applied backwards. Only the gap goes in.
- **A stated sign convention: positive is clockwise.** The Go application's `imaging.Rotate` is
  counter-clockwise for positive angles, which is why `ApplyCropInfo` negates the angle it was
  given before calling it. Clockwise-positive matches CSS `transform: rotate()`, the cropper widget
  the dialog will use, and every angle the user will ever see, so the negation is designed out
  rather than carried over. It is documented on the function, because a rotation whose sign is
  ambiguous is a rotation applied backwards somewhere.
- **A stated output colour type: the alpha-bearing sibling of the input.** Rotation necessarily
  produces pixels no source pixel covers, so an opaque output cannot represent the result. `Rgb8`
  rotates to `Rgba8`, `Rgb16` to `Rgba16`, and an input that already carries alpha keeps its type.
  Bit depth is preserved - a developed RAW is 16-bit and must not be flattened by being turned.
- **No new dependency, in `rust-sak` or here.** The alternative considered and rejected was
  `imageproc`, whose `warp_into` would do this in ten lines; see design.md - Other crates
  considered.
- **A release.** Version bump to the CalVer number for the release month and a new tag, the way
  `add-init-0-rust-sak` and `add-image-raw-1-rust-sak` did it, gated on the repository owner's
  explicit go-ahead.

**In this repository:**

- `crates/opai/Cargo.toml` moves its `rust-sak` pin to the new tag. No feature is added: `rotate`
  lives behind the `image` feature this crate already enables. `crates/perf/Cargo.toml` moves with
  it: it pins the same git dependency by tag, and two tags of one git source resolve to two
  `rust-sak` entries in the lock file, so the pin is bumped in lockstep or not at all. `perf` gains
  nothing from the new function - it takes `rust-sak` for `fs` alone - and its bump is bookkeeping
  rather than adoption.
- `Cargo.lock` is updated and committed; CI already runs `--locked`.
- No source change anywhere. The point is the one the earlier `rust-sak` slices made: the build here
  proves the new function resolves, compiles and is public on all three platforms, in the slice that
  owns the dependency rather than in the slice that writes the seam.

**One piece of verification, ahead of its slice.** Which direction is positive, and what the
expanded canvas measures, are the two facts that have to agree between `rust-sak`'s rotation and the
cropper widget in slice 3 - and a disagreement on either would surface two slices from now as every
rotated crop landing offset. Both are pinned by test here, and the widget's own conventions are
measured now rather than assumed, because the API's sign is chosen from the answer.

## Capabilities

### New Capabilities

None. This slice creates no behaviour observable from Open Photo AI. No image can be rotated, no
crop can be set, and nothing in `opai`, `gui` or `cli` calls the new function - what the slice
produces is a dependency that can turn a picture, plus the pin that reaches it.

`.openspec.yaml` therefore sets `skip_specs: true`, exactly as `add-init-0-rust-sak` and
`add-image-raw-1-rust-sak` did for the same reason.

### Modified Capabilities

None. `image-io`, `gui-images` and `gui-enhance` all describe what this application does with a
photograph, and after this slice it does exactly what it did before. `add-crop-2-seam` is where
those requirements change, and where they can be verified against this application rather than
against `rust-sak`.

## Impact

**Crates touched:** `opai` and `perf` (manifests and the lock file only - one tag, no feature
change, no source change). Both pin `rust-sak` by git tag and a single git source resolves to a
single version, so the tag moves in both or the lock file grows a second copy; only `opai` calls
anything behind it. `gui` and `cli` are untouched, and no logic is placed in a front end because no
logic is placed anywhere in this repository by this slice.

**`rust-sak`:** this slice *is* the `rust-sak` work, so the usual "what did you find in it" answer
is inverted. What it already provides and is **not** rebuilt: the `image` module's `DynamicImage`
contract, its `ImageError` vocabulary, its file-per-function layout and its per-feature CI matrix -
`rotate` joins all of it rather than sitting beside a parallel set. What it does **not** provide,
and this slice adds: rotation by any angle other than a quarter turn.

**Third-party crates introduced:** none, in either repository. That is the notable outcome rather
than an absence - see design.md.

**Parity divergence - one, and it is internal rather than behavioural:**

- **The sign of a positive angle.** The Go application rotates counter-clockwise and negates at the
  call site; this rotates clockwise and does not. The picture a user sees for a given slider
  position is identical. Recorded because it is the single fact most likely to be mis-ported by
  someone reading `ApplyCropInfo` and translating it literally.

**Not in scope:** flip and crop, which the `image` crate already does; composing flip, rotate and
crop into one operation, which is `opai`'s job in slice 2 because it is the crop's identity that
makes the composition meaningful; any `opai`, `gui` or `cli` source code; the rendition URL; the
crop store; the dialog and every control in it; and the crop's effect on face detection and export,
neither of which exists in this GUI yet.
