## Why

The library can name models and install runtimes, but it cannot yet open an image or write one out.
Every remaining slice - inference, the per-operation cache, Autopilot, the export queue - needs a
decoded image, the identity that keys the cache, and a way to write the result back to disk. Nothing
downstream can start until this exists.

## What Changes

- A new `opai` module providing the three operations both front ends need: load an image from disk,
  write one out, and read an image's dimensions without decoding it.
- A `SourceImage` value carrying the file it came from, its decoded pixels, and the XXH3-64 hash of
  its bytes - the identity the per-operation cache will key on.
- Each operation is offered twice: an async form for the GUI, whose Tauri runtime must not be
  blocked by a decode, and a blocking form for the CLI and tests, which have no runtime to protect.
- A dedicated error type for image IO, separate from `InitError`.
- The crate's internal `spawn_blocking` helper is generalised over its error type, which today is
  fixed to `InitError`.

Deliberate behavioural divergences from the Go implementation:

- **Writing takes an explicit destination.** Go's `SaveImage` writes to `ImageData.FilePath`, so
  every call site builds a throwaway `ImageData` carrying the *output* path and an empty hash. Here
  the destination is a parameter, and `SourceImage.path` always means where the pixels came from.
- **Dimensions are answerable from a loaded image.** Go's `ImageData` cannot report its own size;
  callers reach into `Pixels`. This is worth the divergence because the value is derived from the
  pixels rather than stored, so it cannot go stale.
- **Probing reports more than dimensions.** Go's `ImageDimensions` returns `[]int{width, height}`;
  `rust-sak` already returns format, colour type and bit depth alongside them, and bit depth is what
  distinguishes 10/12-bit AVIF and HEIF from 16-bit.
- **Encoder settings are per-format.** Go passes one `quality int` (0-100) and maps it onto each
  encoder's own scale. `rust-sak` models this as `EncodeOptions`, one variant per format. Callers
  that pass nothing get `rust-sak`'s defaults, which are not necessarily the per-format defaults the
  Wails GUI ships; the GUI slice supplies those explicitly when it lands.

Known parity gap, left open deliberately: **RAW files are not decodable in this slice.** The Go app
decodes them through `raw-go`. This slice puts the dispatch seam in place - RAW is detected by
extension before any other routing - but no decoder behind it. See the series in `design.md`.

## Capabilities

### New Capabilities

- `image-io`: reading an image file into memory together with the identity that keys the cache,
  writing an image to disk in a caller-chosen format, and reading an image's dimensions without
  decoding its pixels.

### Modified Capabilities

None. No existing requirement changes; generalising `spawn_blocking` over its error type is an
internal refactor with no observable behaviour change.

## Impact

**Crates touched: `opai` only.** No `gui` or `cli` code changes - both are consumers of this API and
neither has an image path yet. Nothing here belongs in a front end: the GUI's file list, the CLI's
input handling and the export queue all need the same three operations.

**What `rust-sak` already provides, checked first:**

- `image` - `decode_bytes` (magic-byte routing), `encode_writer` (explicit format), `probe_file`
  (streams the header only), `ImageFormat`, `ImageInfo` and `EncodeOptions`. Covers all eight
  formats the Go `types.ImageFormat` enumerates. Nothing to build.
- `crypto` - `xxh3_bytes`, matching the hash Go's `LoadImage` and `Xxh3File` produce. Already
  enabled for dependency verification.
- **Not covered: RAW.** Deliberately out of this slice rather than worked around locally.

**Dependencies added:** the `image` feature on `rust-sak` (joining `crypto`, `fetch`, `fs`,
`sysinfo`), and the `image` crate, which `DynamicImage` in the public API makes unavoidable.

**Existing code touched:** `task.rs` (`spawn_blocking` generalised over its error) and its four call
sites in `deps` and `runtime`; `lib.rs` for the new module and its public exports.

**Estimated task count: 11**, within the 15 limit. This is the first slice of a series; the RAW
slices are proposed when they are reached.
