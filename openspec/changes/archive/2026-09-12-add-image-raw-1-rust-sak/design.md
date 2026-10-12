## Context

See proposal.md - Why. What shapes this design is that the destination module already exists and has
a settled shape, and that every candidate decoder is copyleft.

Four facts were checked rather than assumed, and all four constrain what follows:

- **`rust-sak`'s `image` module has a fixed vocabulary.** `ImageFormat` is both the decode source and
  the encode target; `ImageInfo` carries `format: ImageFormat`; every decoder returns
  `::image::DynamicImage`; `ImageError` wraps each codec's own error. One file per public function,
  with `dispatch.rs` behind them.
- **`rust-sak` is Apache-2.0 and published on crates.io**, at `26.9.1`, with no default features and
  MSRV 1.95. Its CI compiles every feature in isolation.
- **Every Rust RAW decoder is copyleft.** Verified on crates.io: `zenraw` 0.2.0 is
  `AGPL-3.0-only OR LicenseRef-Imazen-Commercial`; `rawler` 0.8.0 and `rawloader` 0.37.2 are both
  LGPL-2.1; LibRaw, which `raw-go` links, is LGPL-2.1 or CDDL. There is no permissively licensed
  option to choose instead.
- **`opai/src/image/raw.rs` already publishes the extension list** - 40 extensions, taken from the Go
  application's `SupportedRawExtensions` - and `load` refuses on it before any content routing. That
  list is what the next slice routes into a decoder; this slice does not touch it.

## Slices

RAW support is delivered as a series, and the series continues the one `add-image-io` opened.
**This change is slice 2.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-image-io` | Loading, writing and probing images over `rust-sak`; `SourceImage` and its identity; the RAW dispatch seam with nothing behind it | - | **done** (archived `2026-09-12-add-image-io`) |
| 2 | `add-image-raw-1-rust-sak` | RAW decoding in `rust-sak`'s `image` module behind a new `image-raw` feature: `zenraw` on its `rawler` backend, a decode-only `RawFormat`, a synthesised DNG fixture, and a `rust-sak` release. Here: the pin moved to that release with the feature enabled | 1 (for the seam it fills) | **CURRENT** |
| 3 | `add-image-raw-2-adopt` | Filling the seam in `opai` so a RAW file loads instead of being refused, and probing reports its dimensions; retiring `ImageIoError::UnsupportedRaw` for the formats the backend covers and keeping it for the formats it does not | 2 | PENDING |

Slice 1's design left open whether 2 and 3 stay apart or collapse, to be settled "when slice 2 is
reached and the `rust-sak` API is real rather than assumed". It is now real, and they stay apart: a
`rust-sak` release sits between them, and a single change would have to either verify against an
unreleased dependency or carry the release in its middle. Slice 3 is also larger than it looked -
retiring `UnsupportedRaw` is a partial retirement, because `RAW_EXTENSIONS` has 40 entries and the
backend does not cover all 40.

## Goals / Non-Goals

**Goals:**

- A ninth decoder family that a caller reaches the same way as the other eight, returning the same
  `DynamicImage`.
- Make the licence consequence of enabling it impossible to acquire by accident.
- A test suite that runs in CI on three platforms with no camera file, no network and no external
  binary.
- Leave the eight existing formats' behaviour, and every existing signature, untouched.

**Non-Goals:**

- Encoding RAW. Nothing encodes RAW; the format is an interchange of sensor readings.
- Matching LibRaw's camera list. See proposal.md - Parity divergence.
- Exposing `zenraw`'s development controls - exposure compensation, white-balance override, demosaic
  algorithm, output primaries, linear and camera-raw output modes. This slice decodes to a
  display-ready picture and nothing else; the editing controls belong with the editing UI that would
  drive them, and putting them in the seam now would fix their shape before anything consumes it.
- EXIF, XMP, ICC and Apple ProRAW container metadata. `zenraw` has features for all of them, and none
  is enabled - the module does not handle metadata for its other eight formats either.
- Any change to `opai`'s source, `load`, `probe` or `ImageIoError`.

## How the work divides

**`rust-sak`, then one manifest line in `opai`.** No `gui` or `cli` work, and none is deferred: this
slice produces no behaviour a front end could show. The decision that keeps it out of a front end was
already made in slice 1 - both front ends open files, so file opening lives in `opai`, and the codecs
under it live in `rust-sak`.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| Decode 8 non-RAW formats | `decode_bytes`, `decode_file` | `image` (already enabled) |
| The `DynamicImage` contract, `ImageError`, dispatch layout | the `image` module | `image` |
| **Decode RAW** | **nothing - this slice adds it** | **`image-raw` (new)** |

There is no third module and no local fork: the new functions live in `src/image/`, use its
`ImageError`, and return its `DynamicImage`. This is the "extend `rust-sak` rather than work around
it" path the project's rules require, and RAW qualifies on the stated test - a RAW decoder is
general-purpose, it belongs beside the other codecs, and another project would want it.

## Other crates introduced

`zenraw` (into `rust-sak`), with `rawler` underneath it. Both were weighed against the alternatives:

| Candidate | Why not chosen |
|---|---|
| **`zenraw` + `rawler`** - *chosen* | The only option that returns a **picture** rather than sensor readings. Decode, black/white-level normalisation, demosaic, white balance, camera→XYZ→sRGB matrix, crop and EXIF orientation are all in it, which is the whole of what `raw.Decode` gives the Go application through LibRaw. Pure safe Rust, `#![forbid(unsafe_code)]`, no C toolchain, no system libraries. |
| `rawler` alone | Far more mature - 0.8.0, August 2026, ~45k recent downloads, the active dnglab project, LGPL-2.1 rather than AGPL. But it returns CFA sensor data. Demosaicing, level normalisation and colour matrixing would have to be written in `rust-sak`, which is a slice of its own and is exactly the kind of image-science code a utility crate should be consuming rather than authoring. |
| `rawloader` alone | Same shape of gap as `rawler`, with narrower coverage (~200 cameras, Bayer only, no CR3, no X-Trans, no 10-bit lossless-JPEG DNG so no iPhone ProRAW). It is `zenraw`'s default backend and is deliberately turned off. |
| LibRaw bindings | Matches the Go application's camera coverage exactly, since that is what `raw-go` links. Costs a C toolchain and per-platform build work on macOS, Windows and Linux - the thing every other `rust-sak` codec avoids and the thing this project has no appetite for. |
| `darktable` backend | 900+ cameras, but it shells out to `darktable-cli`. A library cannot assume a user installed a photo editor. |

**Pinning.** `zenraw = { version = "0.2", default-features = false, features = ["rawler", "rawloader"] }`.

`rawloader` is here because `zenraw` 0.2.0 cannot compile without it when `rawler` is on - this was
found by building, not by reading. `rawler_backend.rs` routes every Bayer image through
`demosaic::demosaic_to_rgb_f32`, and that whole module is `#[cfg(feature = "rawloader")]` and typed
against `rawloader::CFA`. So `features = ["rawler"]` alone is a broken combination upstream, and
0.2.0 is the newest published version. Turning `rawloader` on is the only way to reach the `rawler`
backend at all.

What that costs is one more LGPL-2.1 crate in the tree, not a change of kind: `rawloader` sits in the
same class as `rawler`. What it does **not** cost is the decoder: `zenraw::decode` returns
`rawler_backend::decode` whenever `rawler` is enabled, so `rawler` remains the decoder and the
300-plus camera coverage this design chose is unchanged. `rawloader` is
linked for its `CFA` type and the demosaic code typed against it; its own decode entry point is never
reached.

`ultrahdr` stays off - it is a genuine default that nothing here needs. The `rawler` feature implies
`std` on its own. `zenraw` 0.2.0 depends on `rawler ^0.7.2`, not the newer 0.8.0 - noted so a later
`cargo update` that appears to hold `rawler` back is recognised as the pin it is rather than a
mistake.

**MSRV is unaffected:** `zenraw` requires 1.93 and `rawler` 0.7.2 requires 1.88, both below
`rust-sak`'s 1.95.

## Decisions

### The licence decision

**Accepted deliberately.** The conflict was raised before this design was written: `rust-sak` is
Apache-2.0 on crates.io, `zenraw` is AGPL-3.0-only, `rawler` is LGPL-2.1, and the FSF holds
Apache-2.0 incompatible with LGPL-2.1 specifically, over the patent-termination clause. The
alternatives were to put the decoder in `opai` instead - which is AGPL-3.0 and has no conflict at all
- or to split it into a separate copyleft sibling crate. Both were considered and neither was taken:
the decoder belongs beside the other eight codecs, and moving it out of `rust-sak` to dodge the
licence would be exactly the local workaround the project's rules forbid.

What that costs is real, and the mitigation is the shape of the feature rather than written
disclosure:

- The feature is **`image-raw`, not part of `image`**. Nobody acquires the encumbrance by enabling
  image support; they acquire it by asking for RAW by name.
- `rust-sak`'s own source stays Apache-2.0, which is accurate - the crate's code is Apache-2.0
  licensed. It is the *combination* that is encumbered, and only when the feature is on.
- `#[cfg(feature = "image-raw")]` guards every new item, so a build without it links nothing.

*Not done:* a licence notice anywhere in the crate. One was written - in the `Cargo.toml` feature
definition, both READMEs and the `image` module's rustdoc - and then **removed at the repository
owner's direction**, so nothing in `rust-sak`'s own documentation flags the encumbrance. What still
reports it is the dependency metadata: `cargo tree -f "{p} {l}" --features image-raw` names
`zenraw` AGPL-3.0-only and `rawler`/`rawloader` LGPL-2.1, and `cargo deny` reads the same fields.

*Also not done:* changing `rust-sak`'s declared `license` field, or adding a `licenses` section to
`deny.toml`. The first would misdescribe the crate's own code; the second would fail the build for
the feature this slice exists to add, and `deny.toml` documents that it deliberately runs only
`advisories` and `bans`.

### `RawFormat` is its own enum, not `ImageFormat` variants

`ImageFormat` answers two questions at once: what can be decoded, and what can be encoded to.
`ImageFormat::extension` feeds `encode_file`'s naming, `EncodeOptions` has a variant per member, and
`to_image_format` maps each to the `image` crate's. A RAW member would have to answer all of that
with "not applicable", and `EncodeOptions::default_for(ImageFormat::Nef)` would be a function call
with no honest return value.

A separate `RawFormat` says the true thing instead: these are decode-only. It carries
`from_extension`, `from_path`, `extension` and `from_magic`, mirroring `ImageFormat`'s shape so the
two read alike, and `is_raw_bytes` over `zenraw::is_raw_file` for content detection.

The Go application reached the same conclusion from the other side - `utils/info.go` keeps
`rawExtensions` as a list separate from `imageExtensions`, and its comment says "RAW is read-only: it
can be decoded but never written."

*Alternative rejected:* one `ImageFormat::Raw` variant covering all RAW. It collapses the distinction
a caller displaying a file listing needs, and it still cannot be encoded to.

### Probing returns `RawImageInfo`, not `ImageInfo`

`ImageInfo` is `Copy` and its `format` field is an `ImageFormat`, which has no RAW member. Widening
it would break the `Copy` derive the moment camera make and model are added, and those two are worth
adding: `zenraw::probe` returns them for free, and a photographer's file listing wants them where it
does not want a colour type. `RawImageInfo` carries `format: RawFormat`, `width`, `height`,
`bit_depth: Option<u8>`, `make`, `model` and `is_dng`.

`bit_depth` stays `Option` because `zenraw` reports it that way - it is estimated from the white
level and is not always derivable. Restating it as a `u8` with a made-up default would be the kind of
quiet lie `ImageInfo::bit_depth` exists to avoid.

**`format` is `Option<RawFormat>` too, which this design did not originally say.** It came out of
implementing `probe_raw_bytes`: that function has only the header to work from, and the header cannot
name a TIFF-based RAW. A NEF, an ARW, a CR2 and a PEF all open with the same four bytes, so
`RawFormat::from_magic` resolves only the containers that identify themselves - DNG, CR3, RAF, RW2 and
ORF - and there is nothing honest to put in the field for the rest. Both alternatives were worse:
making `probe_raw_bytes` fail on a NEF would refuse the most common RAW files there are, and filling
the field with a guess would be exactly the quiet lie the paragraph above rejects. `probe_raw_file`
always fills it in, because the extension is what distinguishes those formats and it has one.

*Alternative rejected:* returning `ImageInfo` with a fabricated `format`. There is nothing honest to
put in the field.

### Probing RAW reads the whole file; the other eight do not

`zenraw::probe` takes `&[u8]`, so `probe_raw_file` reads the file and hands over all of it.
`probe_file` for the native formats streams a `BufReader` and stops after the header.

This is a real asymmetry and it is the backend's, not a choice made here. It could be hidden by
reading a bounded prefix and retrying whole, the way `probe_file` already does for AVIF, HEIF and
WebP - but RAW metadata is spread across IFD chains whose offsets routinely point deep into a 40 MB
file, so the prefix would miss far more often than it hit and the retry would be the normal path.
Reading once is cheaper than reading twice most of the time. It is documented on the function so a
caller listing a directory of RAWs knows what it is paying.

### Decoding produces `ImageRgb16` through `OutputMode::Develop`

`zenraw`'s default `OutputMode::Develop` runs the full pipeline - levels, demosaic, white balance,
colour matrix, tone curve, sRGB gamma, crop, orientation - and the `rawler` backend emits
`PixelDescriptor::RGB16_SRGB`. That maps to `DynamicImage::ImageRgb16` with no conversion, and it is
the mode that matches what `raw.Decode` gives the Go application.

Keeping 16 bits rather than narrowing to 8 is the point: these files exist because the sensor
recorded more than 8 bits, this is an enhancement application, and `DynamicImage` already carries
`ImageRgb16` so nothing downstream needs widening to accept it.

**The conversion must respect stride.** `zenpixels::PixelBuffer` has a `stride` separate from its
width, and the buffer may be SIMD-aligned, so `as_strided_bytes` is not a packed image. The
conversion checks `is_contiguous`/`into_contiguous_pixels` first and falls back to copying row by row
through `row(y)`, rather than assuming the fast case and silently skewing the picture when it does
not hold.

`RawDecodeConfig::default()` is used as-is apart from `max_pixels`, which stays at its 200-megapixel
default - above any camera and below a decompression bomb. Cancellation is `&enough::Unstoppable`:
`rust-sak`'s `image` module is entirely synchronous, and threading a cancellation token through one
of nine decoders would be an inconsistency for no caller's benefit.

### The fixture is built, not committed

A test needs a real RAW file, and there are two ways to get one.

A camera file from a public corpus - raw.pixls.us dedicates its samples to CC0 - is genuinely
representative, and is 10 to 30 MB. `rust-sak` has no `include` key in its manifest, so everything in
the repository ships in the published crate, and a multi-megabyte binary in a utility crate for one
test is not a reasonable thing to make every consumer download.

So the fixture is a function: a few hundred bytes of hand-written TIFF tags describing a tiny
uncompressed Bayer CFA DNG - `PhotometricInterpretation` 32803, `Compression` 1, `BitsPerSample` 16,
`CFAPattern`, `CFARepeatPatternDim`, `DNGVersion`, `BlackLevel`, `WhiteLevel`, `ColorMatrix1`,
`CalibrationIlluminant1`, `AsShotNeutral`, and strip offsets over a generated gradient. `rawler`'s
DNG decoder was read to confirm this is enough: it takes each of those from the IFD and falls back to
a default where a tag is absent, so a minimal file is a supported file rather than a lucky one.

Building it also makes the test *say* something - the pixel values are known, so the assertion can be
"a red-ish CFA pattern develops to a red-ish picture" rather than "it did not error".

Real camera files are still worth running against, so a test reads a directory named by
`ZENRAW_SAMPLES_DIR` and decodes everything in it, skipping silently when the variable is unset. That
is for a developer with a folder of NEFs, and never for CI.

*Alternative rejected:* generating the DNG in a `build.rs`. It would run for every consumer of the
crate, including those who never enable the feature.

### Module layout

The module's existing shape is one file per public function plus shared files, so:
`src/image/raw_format.rs` (`RawFormat`), `src/image/raw_info.rs` (`RawImageInfo`),
`src/image/decode_raw_bytes.rs`, `src/image/decode_raw_file.rs`, `src/image/probe_raw_bytes.rs`,
`src/image/probe_raw_file.rs`, and `src/image/raw_dispatch.rs` for the `PixelBuffer` conversion the
four share. Every one is `#[cfg(feature = "image-raw")]`, including its `mod` declaration and its
`pub use`. Tests go in `src/image/tests.rs` beside the existing ones, behind the same gate.

## Risks / Trade-offs

- **`zenraw` is very young.** 0.2.0, first published March 2026, 457 downloads in total, and its
  README carries "Developed with Claude (Anthropic). Not all code manually reviewed." → It is the
  only crate that does this job, so the alternative is not a safer crate but writing a demosaic and
  colour pipeline in `rust-sak`. The exposure is bounded: it sits behind an off-by-default feature,
  behind `rust-sak`'s own functions, so replacing it later changes one module and no signature. The
  synthesised fixture plus the `ZENRAW_SAMPLES_DIR` suite is what checks it rather than trusting it.
  Its youth already showed: `features = ["rawler"]` without `rawloader` does not compile (see
  **Pinning** above), a combination its own CI evidently does not build.
- **`zenraw`'s README is already out of date against its own 0.2.0 source.** The README documents
  `apply_gamma` and `with_gamma()` and says linear f32 is the default; the source has `OutputMode`
  and defaults to `Develop`. → Implement against the source and the docs.rs API, not the README, and
  pin `0.2` rather than a range that could move under the code.
- **An AGPL-3.0 dependency in an Apache-2.0 crate is a trap for a downstream user who does not read
  feature documentation.** → An off-by-default feature and a name that has to be typed deliberately;
  `cargo tree -f "{p} {l}"` and `cargo deny` report the licences to anyone who looks. **This risk is
  larger than the design first set it:** the written notice that mitigated it was removed at the
  repository owner's direction, so a downstream user who reads only `rust-sak`'s documentation is
  not told. It cannot be reduced further without moving the decoder out of `rust-sak`, which was
  considered and declined.
- **A camera the `rawler` backend does not know is a file the application refuses.** → It fails with
  a message naming the file and the format, never with a wrong picture, and slice 3 keeps
  `UnsupportedRaw` for exactly this case. The backend can be widened later without a signature
  changing.
- **A hand-built DNG could exercise only the path a hand-built DNG takes.** → Accepted: it proves the
  seam, the conversion and the error mapping, which is what a fixture in a codec-wrapper module is
  for. Real coverage is `ZENRAW_SAMPLES_DIR` and the application itself.
- **Compile time and binary size both grow.** `rawler` is a large crate with a decoder per camera
  family. → It compiles only when the feature is on; `rust-sak`'s CI already builds each feature in
  isolation, so the cost is visible rather than smeared across the default build.

## Platforms and validation

No platform-specific code: `zenraw` and `rawler` are pure Rust with no build scripts fetching
binaries and no system libraries, so unlike the `image` feature's AVIF/HEIF/WebP codecs there is
nothing platform-sensitive to go wrong. `rust-sak`'s CI must still run the new feature on macOS,
Windows and Linux, and `opai`'s CI proves the pin resolves on all three.

No execution providers are involved; this slice runs no inference.

Nothing is validated yet - this design is written ahead of implementation, and the API described here
was read from `zenraw` 0.2.0's published source rather than compiled against. The tasks require
`cargo fmt`, `cargo clippy` and both test suites to pass before the change is called complete.

## Migration Plan

1. Land the `image-raw` feature in `../rust-sak` on its default branch, with tests and the full gate
   green, including the per-feature isolation build.
2. **Confirm with the repository owner before pushing** - `rust-sak` is a shared, published crate and
   the project's rules require an explicit go-ahead. How the licence consequence is documented, if at
   all, is part of what is being confirmed rather than a detail settled afterwards. It was confirmed
   as *not at all*: see **The licence decision**.
3. Bump the version and cut the tag.
4. Move the pin here, enable `image-raw`, update `Cargo.lock`, and verify a cold `cargo build
   --locked` and CI.

Rollback is the pin: reverting `opai/Cargo.toml` and `Cargo.lock` returns this repository to the
previous `rust-sak` tag, because no source file here uses the new API in this slice. The tag itself is
immutable and harmless if unreferenced.

## Open Questions

- ~~The version number for the `rust-sak` release.~~ **Answered: `26.9.3`.** CalVer `YY.M.MICRO` in
  September 2026 put the next number at `26.9.2`, and that is what was cut first - with the licence
  notice still in it. The notice was then removed at the repository owner's direction (see **The
  licence decision**), so a second release followed and `26.9.3` is the tag `opai` pins. `26.9.2` is
  left in place rather than moved, because a tag is immutable and an unreferenced one is harmless.
