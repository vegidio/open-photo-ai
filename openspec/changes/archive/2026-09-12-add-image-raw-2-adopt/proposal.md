## Why

`add-image-raw-1-rust-sak` put a RAW decoder into `rust-sak` behind its new `image-raw` feature and
moved this repository's pin onto the release that carries it. Nothing here calls it: `load` still
refuses a file whose extension names a camera RAW format with `ImageIoError::UnsupportedRaw`, before
its content is looked at, and `probe` still reports a RAW name as a codec failure because no
`ImageFormat` matches it.

That was the deliberate shape of the split — the dependency in one slice, the seam in the next — and
this is the next. Photographers are the audience, the formats their cameras write are the ones the
application cannot open, and the decoder that opens them is now a function call away.

## What Changes

Task estimate: **14**.

**Loading routes RAW to the RAW decoder.** The extension check stays first and stays the router, for
the reason it was written: most RAW formats are TIFF containers and carry plain TIFF magic, so
content routing would hand a NEF to the TIFF decoder and return an embedded preview as a success.
What changes is what sits behind the branch. A recognised RAW extension the backend covers is
decoded through `rust_sak::image::decode_raw_bytes` from the same buffer the identity is hashed over,
so the file is still read exactly once and `SourceImage`'s identity still equals `xxh3_file` of that
file.

**`UnsupportedRaw` is partially retired, not removed.** `rust-sak`'s `RawFormat` covers 26 of the 44
extensions `RAW_EXTENSIONS` publishes; the other 18 (`bay`, `bmq`, `cap`, `cine`, `cs1`, `dc2`,
`eip`, `gpr`, `ia`, `k25`, `kc2`, `mdc`, `ptx`, `pxn`, `rdc`, `sr2`, `srf`, `sti`) have no decoder in
any pure-Rust backend and stay refused — still before the file is read, so a RAW on a disconnected
drive names its format rather than an I/O failure. The partition is derived from
`RawFormat::from_extension`, never restated as a second hardcoded list that could drift from the
backend.

**Three extensions are added to `RAW_EXTENSIONS`:** `ari` (ARRI), `crm` (Canon Cinema RAW Light) and
`ori` (Olympus). The backend decodes all three and this application did not recognise them, so they
fell through to content routing — the exact misdecode the RAW check exists to prevent. The list goes
from 44 to 47 entries, 29 of them decodable.

**Probing gains a RAW form, mirroring `rust-sak`'s own split.** `probe`/`probe_blocking` are
unchanged and still return `ImageInfo` over the eight non-RAW formats. New `probe_raw`/
`probe_raw_blocking` return `RawImageInfo` — dimensions, sensor bit depth, camera make and model, the
`RawFormat`, and whether the container is a DNG. They are separate functions rather than one
dispatching `probe` because the two differ in more than their return type: `probe_file` reads a
header, `probe_raw_file` reads the whole file, and a single signature would hide a cost difference of
four orders of magnitude. See design.md — D1.

**`is_raw` becomes public.** With the split mirrored, a caller has to be able to ask which of the two
probes applies. It is `pub(crate)` today, so a front end outside this crate could not; a file listing
is the caller that needs it.

**A RAW decode that fails is a `Decode` failure.** A covered extension whose decode the backend
refuses — an unknown camera body, or a corrupt file, which `zenraw` reports as one error and cannot
tell apart — arrives as `ImageIoError::Decode` carrying `ImageError::Raw`, the same variant a corrupt
JPEG gets. No new error variant: the enum's cases are about what the application could not do, and
"could not decode this file" is already one of them. See design.md — D5.

**A minimal DNG fixture is built in this crate's test support.** `rust-sak` synthesises one for its
own tests, but it is `#[cfg(test)]`-internal and unreachable from here, so the builder — a few hundred
bytes of hand-written TIFF tags describing a tiny uncompressed Bayer CFA image — is ported into
`image/test_support.rs` beside the eight encoded fixtures. Real camera files are exercised through an
environment variable for manual verification, never in CI. See design.md — D7.

**Not a divergence but a known hole, deferred deliberately:** a developed RAW is 16-bit RGB, and two
of the eight encoders behind `save` refuse 16-bit input — GIF and WebP, which accept only 8-bit.
Loading a NEF and saving it as WebP therefore fails. The hole is not created here (a 16-bit PNG or
TIFF has always loaded as 16-bit) and nothing calls `save` yet, so nothing regresses; the fix is a
down-conversion in `rust-sak`'s encode dispatch, which is general-purpose and belongs there by the
project's own rule, and is slice 4 of the series. See design.md — Slices, `rust-sak` coverage
and Risks / Trade-offs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `image-io`: the requirement that camera RAW files are refused rather than misread is replaced. RAW
  files the backend covers are now decoded; the ones it does not are still refused by extension
  before the file is read. Loading a RAW reports the same identity and dimensions as any other
  format. Probing a RAW is a new operation of its own, reporting dimensions, sensor bit depth and
  camera make and model, and reading the whole file to do it.

## Impact

**Crates touched:** `opai` only — `image/raw.rs`, `image/load.rs`, `image/probe.rs`,
`image/error.rs`, `image/mod.rs`, `image/test_support.rs`, `lib.rs`, and `tests/public_api.rs`. No
`gui` or `cli` work and none is deferred into one: both front ends open files, so file opening lives
in `opai`, which is where slice 1 put it.

**`rust-sak`:** nothing added and nothing needed. Everything this slice calls —
`decode_raw_bytes`, `probe_raw_file`, `RawFormat`, `RawImageInfo`, `ImageError::Raw`,
`ImageError::NotRaw` — shipped in `26.9.3` and is already enabled through the `image-raw` feature on
the existing pin. `decode_raw_bytes` is taken rather than `decode_raw_file` precisely so the
one-read identity property holds; `probe_raw_file` is taken rather than `probe_raw_bytes` because the
file name is the only thing that distinguishes the TIFF-based formats, and the path is in hand.

**Third-party crates introduced:** none. No manifest change at all — the feature and the pin arrived
in the previous slice.

**Public API, additive except for one behaviour change:** `probe_raw`/`probe_raw_blocking`,
`is_raw`, and the `RawFormat`/`RawImageInfo` re-exports are new. `load`'s signature is unchanged, but
what it does with a RAW file is not: a caller matching `ImageIoError::UnsupportedRaw` to mean "any
RAW file" is now wrong for 29 of the 47 extensions. Nothing in this repository matches on it outside
its own tests and `tests/public_api.rs`, both updated here.

**Parity with the Go application:** this closes the gap. Go branches on the extension in
`LoadImage` and decodes through `raw-go`, and branches again in `ImageDimensions` to read RAW
metadata without demosaicing — the same two seams, filled the same way. Two differences remain, both
already recorded in slice 2's proposal and neither introduced here: camera coverage is `rawler`'s
300-plus rather than LibRaw's near-everything, and no mutex serialises RAW decodes because `zenraw`
is pure Rust with no global state where bundled LibRaw corrupts data when decoded concurrently. The
18 refused extensions are the visible edge of the first difference.

**Not in scope:** the 16-bit GIF/WebP encode gap (slice 4); `probe` dispatching to RAW on its own;
EXIF, XMP and ICC handling; `zenraw`'s development controls (exposure, white balance, demosaic
choice), which belong with the editing UI that would drive them; the file dialog's accepted-input
list, which belongs to the file-listing change that draws it; and any `gui` or `cli` code.
