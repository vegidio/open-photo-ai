## Why

A developed camera RAW is 16 bits per channel, and three of the eight encoders behind `save` refuse
it: GIF and WebP reject every 16-bit colour type, and GIF additionally rejects grayscale. Slice 3
landed RAW loading and recorded the refusal in the spec as a known limitation, so today a
photographer who opens a NEF can export it as JPEG or PNG but is told their WebP export is an
"unsupported colour type". The Go application has no such gap — its encoders take the `image.Image`
interface and convert whatever they are handed — so this is a parity regression the Rust port
introduced by using concrete pixel buffers, and it is the last piece of the RAW series.

Measuring the encoders rather than only the reported case turned up three more refusals on the same
seam, all reachable from an ordinary file the user already loaded:

| Written as | Refuses today |
|---|---|
| GIF | grayscale (`L8`, `La8`), every 16-bit type, every 32-float type |
| WebP | every 16-bit type, every 32-float type |
| TIFF | grayscale-with-alpha (`La8`, `La16`) |
| PNG, AVIF, HEIF | every 32-float type |
| BMP, JPEG | nothing — the `image` crate's own encoders already convert |

An 8-bit grayscale PNG exported as GIF fails today for the same reason a developed RAW does, and by
the same one-line mechanism. All of it is fixed together.

## What Changes

- **`rust-sak`'s encode dispatch gains a compatibility step.** Before an image reaches a codec, it is
  converted to the widest colour type that codec accepts: precision is preserved where the target can
  hold it (a 32-float image written as PNG becomes 16-bit, not 8-bit) and narrowed only as far as the
  encoder forces. BMP and JPEG are untouched — they already convert inside the `image` crate.
- **A picture is never refused for its colour type again.** All eight formats accept all ten
  `DynamicImage` variants. The only colour-type errors that remain are the ones no conversion can fix,
  and there are none left in this API's surface.
- **`rust-sak` is released** (26.9.4) and `opai`'s git-tag pin moves to it.
- **`opai` records the closed gap.** The `image-io` spec's "Bit depth of a developed RAW" requirement
  stops saying GIF and WebP refuse 16-bit input and says every format writes it; the test that pins
  the current refusal (`a_developed_raw_is_sixteen_bit_and_two_of_the_eight_encoders_refuse_it`)
  becomes a test that all eight write it; the module documentation's "known limitation" section goes.
- No change to `save`'s signature, to `EncodeOptions`, to any error type, or to what a caller writes.
  A caller that succeeded before succeeds identically — the conversion only runs where the encode
  would otherwise have failed.

**Not breaking.** The behaviour change is refusal → success. Nothing that worked stops working, and
no front end calls `save` yet, so no user can observe the before state.

## `rust-sak` first

The infrastructure question is settled before anything else, as the project requires: `rust-sak`'s
`image` module already owns every encoder this touches, so nothing new is built and no third-party
crate is introduced. The one thing it does **not** have is a per-format colour-type conversion —
the `image` crate keeps its own (`dynimage_conversion_8bit`) `pub(crate)`, so it cannot be borrowed
and roughly twenty lines of match arms are written in `rust-sak` instead.

The fix belongs in `rust-sak` rather than in `opai` by the project's own test: it is general-purpose,
every consumer encoding a 16-bit picture hits it, and working around it locally would mean `opai`
converting pixels before handing them to an API whose whole point is to accept a `DynamicImage`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `image-io`: "Bit depth of a developed RAW" currently requires GIF and WebP to refuse a 16-bit
  picture and names itself a known limitation. It is replaced by a requirement that writing converts
  a picture to whatever the target encoder accepts, for colour layout as well as sample depth, so
  every supported format writes every picture the system can load.

## Impact

- **Crates touched: `opai` only** (a dependency-pin bump, a spec-pinning test, and documentation).
  No `gui` or `cli` work and none deferred into one: encoding is shared logic and lives in `opai`,
  as settled in slice 1.
- **Another repository: `rust-sak`** — `src/image/dispatch.rs`, its tests, and its `image` module
  README, followed by a release. This is explicit work with its own tasks, and the release is not
  pushed without the go-ahead, since `rust-sak` is a published crate.
- **Dependencies:** `rust-sak`'s git tag moves 26.9.3 → 26.9.4 in `crates/opai/Cargo.toml`. Nothing
  is added or removed on either side.
- **Parity:** this closes a divergence rather than creating one. The Go application converts
  implicitly because Go's encoders take an interface; after this change the Rust port converts
  explicitly and reaches the same outcome.
- **Estimated size:** 12 tasks — within the 15-task limit, so this is one change and the last slice
  of the series.
