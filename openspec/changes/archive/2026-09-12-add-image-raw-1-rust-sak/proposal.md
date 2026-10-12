## Why

`add-image-io` shipped loading, saving and probing for the eight formats `rust-sak` covers, and left
one branch deliberately empty: a file whose extension names a camera RAW format is refused with
`ImageIoError::UnsupportedRaw` rather than decoded. That is a known parity gap against the Go
application, which decodes RAW through LibRaw via `raw-go` and lists 40 RAW extensions as accepted
input. Photographers are the audience, and the formats their cameras write are the ones the
application cannot open.

The gap is in `rust-sak`, not here. Its `image` module is the crate's image-codec module and it
covers eight formats; a ninth family of decoders belongs beside them, by the project's own rule that
a close-but-incomplete `rust-sak` module is extended rather than worked around. This slice writes
that decoder and cuts the release; the slice after it fills the seam in `opai`.

## What Changes

Task estimate: **14**. RAW support end to end runs past the 15-task ceiling once the `rust-sak`
extension, its release and the adoption here are counted, so it is split into two slices (see
design.md - Slices). This proposal covers the `rust-sak` slice.

**In `../rust-sak` (a separate repository):**

- **A new `image-raw` Cargo feature** on the `image` module, off by default like every other feature,
  adding `zenraw` with its `rawler` backend. It is a separate feature rather than part of `image`
  because of what it costs: see the licence bullet below.
- **A decode-only `RawFormat` vocabulary.** RAW is read-only - the Go application says so in
  `utils/info.go` - so RAW does not join `ImageFormat`, which is the encode target as well as the
  decode source. `RawFormat` names the RAW families the backend decodes, maps extensions to them, and
  recognises RAW content.
- **`decode_raw_bytes` / `decode_raw_file`**, returning a `DynamicImage` exactly as the eight
  existing decoders do, so a caller sees one pixel type across all nine families.
- **`probe_raw_bytes` / `probe_raw_file`**, returning a new `RawImageInfo` - dimensions, bit depth,
  camera make and model, and the `RawFormat`. Separate from `ImageInfo` because `ImageInfo.format` is
  an `ImageFormat` and RAW has no member there, and because make and model are the two facts a
  photographer's file listing wants that no `ImageInfo` field carries.
- **New `ImageError` variants** for a RAW decode that failed and for bytes that are not RAW at all.
- **A minimal DNG fixture built in test code**, not a committed camera file: a few hundred bytes of
  hand-written TIFF tags describing a tiny uncompressed Bayer CFA image. It keeps the repository
  small and its fixtures' provenance unambiguous. Real camera files are exercised through an
  environment variable for manual verification, never in CI.
- **BREAKING for the crate's licensing story, and the reason this is its own feature.** Every Rust
  RAW decoder is copyleft: `zenraw` is AGPL-3.0-only (or a commercial Imazen licence), and both its
  backends, `rawler` and `rawloader`, are LGPL-2.1. `rust-sak` is Apache-2.0 and published on
  crates.io. Enabling `image-raw` therefore encumbers the resulting binary, and Apache-2.0 is
  FSF-incompatible with LGPL-2.1 in particular. The crate's own source stays Apache-2.0, and the
  encumbrance is opt-in: it is acquired by naming `image-raw`, never as a side effect of `image`.
  No notice is written into the manifest, the READMEs or the module docs - one was, and was removed
  at the repository owner's direction, leaving the dependency metadata (`cargo tree -f "{p} {l}"`,
  `cargo deny`) as what reports the licences. This was raised and the trade-off was accepted
  deliberately - see design.md - The licence decision.
- **A release.** Version bump and a new tag, the way `add-init-0-rust-sak` did it.

**In this repository:**

- `opai/Cargo.toml` moves its `rust-sak` pin to the new tag and enables `image-raw` alongside
  `image`, `crypto`, `fetch`, `fs` and `sysinfo`. Nothing calls it yet - that is the next slice - and
  the point is the same one `add-init-0-rust-sak` made: the build here proves the new API resolves,
  compiles and is public, on all three platforms, in the slice that owns the dependency rather than
  in the slice that writes the seam.
- `Cargo.lock` is updated and committed; CI already runs `--locked`.

## Capabilities

### New Capabilities

None. This slice creates no behaviour observable from Open Photo AI. Loading a NEF still fails with
`ImageIoError::UnsupportedRaw`, because `opai/src/image/load.rs` still refuses before it looks at
content and this slice does not touch it. What the slice produces is a dependency that can decode
RAW, plus the pin that reaches it.

`.openspec.yaml` therefore sets `skip_specs: true`, exactly as `add-init-0-rust-sak` did for the same
reason.

### Modified Capabilities

None. The `image-io` capability's requirements are unchanged: it says RAW is refused, and after this
slice RAW is still refused. `add-image-raw-2-adopt` is where that requirement changes, and where it
can be verified against this application rather than against `rust-sak`.

## Impact

**Crates touched:** `opai` (manifest and lock file only - one tag, one feature, no source change).
`gui` and `cli` are untouched, and no logic is placed in a front end because no logic is placed
anywhere in this repository by this slice.

**`rust-sak`:** this slice *is* the `rust-sak` work, so the usual "what did you find in it" answer is
inverted. What it already provides and is **not** rebuilt: the `ImageFormat`/`ImageInfo`/`ImageError`
vocabulary, the extension and magic-byte dispatch, the `DynamicImage` contract every decoder returns,
and the module's file-per-function layout - the new functions join all of it rather than sitting
beside a parallel set. What it does **not** provide, and this slice adds: any RAW decoder at all.

**Third-party crates introduced (into `rust-sak`, not here):** `zenraw` with `rawler`. Both were
checked against the alternatives rather than assumed - `rawler` alone, `rawloader` alone and LibRaw
bindings - see design.md - Other crates introduced. The transitive additions beyond those two
(`zenpixels`, `archmage`, `magetypes`, `enough`, `whereat`, `bytemuck`, `memchr`) are all MIT/Apache
or Unlicense; only `zenraw` and `rawler` are copyleft.

**Parity divergence - one, deliberate, and narrower than the Go application:**

- **Camera coverage.** `raw-go` statically links LibRaw, which reads essentially every camera ever
  made; `zenraw`'s `rawler` backend reads 300-plus. A camera in the gap is a file Open Photo AI Go
  opens and Open Photo AI Rust refuses. The trade is a pure-Rust decoder with no C toolchain, no
  system libraries and no platform build work on three targets - the same property `raw-go` was
  itself built to give the Go application, reached without the C. The refusal is honest and names the
  file, which is what `add-image-io` already established as the answer for a RAW file it cannot read.
- **Not a divergence, worth stating:** the Go application serialises every RAW decode behind a mutex
  because bundled LibRaw corrupts data when decoded concurrently. `zenraw` is `#![forbid(unsafe_code)]`
  pure Rust with no global state, so no such lock is needed and none is written. The user-visible
  result is the same picture, arrived at without a bottleneck.

**Not in scope:** any `opai` source code; removing `ImageIoError::UnsupportedRaw` or changing what
`load` does with a RAW file; RAW in `probe`; the `gui` file list; encoding RAW, which no library and
no camera does; EXIF, XMP and ICC handling; and the `darktable` backend, which shells out to an
external binary the application cannot assume is installed.
