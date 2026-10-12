## Context

See `proposal.md` - Why. What shapes the approach here is that `rust-sak`'s `image` and `crypto`
modules already cover every codec and the hash, so almost every decision below is about the seam
between them and this crate rather than about image handling itself.

Three existing facts constrain the design:

- `task::spawn_blocking` is the crate's only route off the async runtime, and its closure is
  `FnOnce() -> T + Send + 'static`. Nothing borrowed can cross it.
- `task::spawn_blocking` returns `Result<T, InitError>` and owns `InitError::Cancelled`. It is
  currently unusable by any other error type.
- The Go original, `../open-photo-ai/utils/image.go`, is the behavioural source of truth, and its
  call sites carry more information than its function signatures do.

## Slices

This feature is delivered as a series. **This change is slice 1.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-image-io` | Loading, writing and probing images over `rust-sak`; `SourceImage` and its identity; the RAW dispatch seam with nothing behind it | - | **CURRENT** |
| 2 | `add-image-raw-1-rust-sak` | RAW decoding in `rust-sak`'s `image` module behind a new `image-raw` feature: `zenraw`, a decode-only `RawFormat`, small committed DNG fixtures, and a `rust-sak` release | 1 (for the seam it fills) | PENDING |
| 3 | `add-image-raw-2-adopt` | Repointing this crate at that release, enabling `image-raw`, and filling the seam so RAW loads instead of being refused | 2 | PENDING |

Slices 2 and 3 are not proposed yet. Whether they stay two changes or collapse into one is decided
when slice 2 is reached and the `rust-sak` API is real rather than assumed - the reason for holding
them apart is that a `rust-sak` release sits between them, which is awkward to verify inside a single
change.

## Goals / Non-Goals

**Goals:**

- One description of "an image the application is working on" that every later slice can build on.
- An identity that a file listing and a decoded image agree on, without decoding twice.
- Make the stale-identity bug the Go type documents but cannot prevent hard to write here.
- Keep the async runtime free of decode and encode work, without making the CLI or tests carry one.

**Non-Goals:**

- RAW decoding. Slice 2.
- Any caching. The identity is designed to be a cache key; the cache itself is a later change.
- Any front-end work. No `gui` or `cli` code changes.
- Choosing the per-format encoder defaults the Wails GUI ships. Those arrive with the export UI.
- Colour management, EXIF orientation, ICC profiles. Neither the Go original nor `rust-sak` handles
  them today, so this slice does not introduce a difference either way.

## How the work divides

**`opai` only.** The GUI's file list, the CLI's input handling and the export queue all need these
same three operations, so all of it belongs in the core crate; nothing here is front-end specific.
`gui` and `cli` are untouched by this slice.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| Decode 8 formats | `decode_bytes` (magic-byte routing) | `image` (new) |
| Encode 8 formats | `encode_writer` (explicit format) | `image` (new) |
| Header-only metadata | `probe_file` | `image` (new) |
| Format vocabulary and settings | `ImageFormat`, `ImageInfo`, `EncodeOptions` | `image` (new) |
| Identity hash | `xxh3_bytes` | `crypto` (already enabled) |

`image` joins `crypto`, `fetch`, `fs` and `sysinfo` on the `rust-sak` dependency.

**The one gap is RAW**, and it is being filled by extending `rust-sak` rather than by local code -
that is slice 2. It qualifies under the project's own test: a RAW decoder is general-purpose, it
belongs beside the other codecs in the same module, and another project would want it.

## Other crates introduced

Only `image` (the crate). It is not a choice: `rust-sak`'s decoders return `image::DynamicImage`, so
it already appears in the signatures this crate consumes and in its own public API. Pinned to the
version `rust-sak` builds against, with `default-features = false` - `rust-sak` owns which codecs are
compiled, and enabling them again here would duplicate the codec set it deliberately routes around
for AVIF, HEIF and WebP.

## Decisions

### The destination is a parameter, not a field

`save` takes the destination path from the caller. `SourceImage.path` means only where the pixels
came from.

Go's `SaveImage` writes to `ImageData.FilePath`, which forces every call site to build a throwaway
`ImageData` carrying the output path and an empty hash - `cmd/gui/services/image_service.go:210` and
`cmd/cli/main.go:114` both do exactly that. The type ends up describing two different things, and
the identity field is a lie in half its uses. Taking the destination as a parameter removes both
problems, and makes "write the result somewhere else" the default rather than "overwrite the
source".

*Alternative rejected:* deriving the destination from the source path with a suffix. The export queue
must be able to write to a user-chosen directory, so the caller has to supply it.

### The requested format governs the bytes; the destination's name is never consulted

`save` uses `encode_writer` with an explicit format into a file it opens itself, rather than
`encode_file`, which would pick the format from the destination's extension and silently ignore the
format argument.

The Go path was traced to settle this. `cmd/gui/frontend/src/utils/export.ts` builds the output name
from the chosen format via `resolveExt`, then derives the format it passes to the backend *back* from
that extension with `getImageFormat(ext)`. The two arguments cannot disagree in the real caller, so
guarding against a mismatch would defend a state the application cannot reach. What is left is
whether the library sniffs a string or is told; being told is the smaller contract.

`EncodeOptions` carries its own format, and `encode_writer` already refuses options that target a
different one - but that refusal comes too late to be the whole answer. `encode_writer` is handed a
writer, so by the time it looks at the options `File::create` has already truncated whatever was at
the destination: a user re-exporting over a previous export with a mis-paired setting would lose that
export *and* get an error. The spec requires that nothing is written, so `save` makes the same check
itself, before it opens the destination, and reports `rust-sak`'s own `ImageError::FormatMismatch` so
that a caller sees one refusal rather than two spellings of it.

Duplicating a check `rust-sak` also makes is the point rather than an oversight: `rust-sak`'s protects
the encode, this one protects the file.

### Loading reads the file once

`std::fs::read`, then `xxh3_bytes` over the buffer and `decode_bytes` over the same buffer.

The identity must cover every byte of the file, including trailing bytes a decoder stops before -
Go achieves this with a `TeeReader` into an `io::Pipe` feeding the hasher plus a drain loop, purely
to avoid reading a large RAW twice. Holding the encoded bytes briefly gets the same single-pass
property with none of the machinery, and the buffer is small next to the `DynamicImage` that is
about to exist anyway.

It also settles format detection: with all the bytes in hand, magic-byte routing via `decode_bytes`
is free, and it matches Go's `image.Decode` rather than trusting the extension.

*Alternative rejected:* `decode_file` + `xxh3_file`. Two full reads, and extension-based routing.

### Probing streams; loading does not. The asymmetry is deliberate

`probe` calls `probe_file`, which for the native formats hands a `BufReader<File>` to the decoder and
reads only the header - its own comment notes a multi-hundred-megabyte TIFF costs a few kilobytes -
and for AVIF, HEIF and WebP reads a bounded 64 KiB prefix, re-reading whole only if that was not
enough.

That means probing routes on the extension while loading routes on magic bytes. The justification is
cost, not accident: loading already holds every byte because the identity requires it, so
content-based routing is free there; probing exists precisely to avoid holding those bytes, so it
takes the cheaper signal. The one caller this mirrors - Go's `CreateFileTypes` - only ever probes
paths that already passed an extension filter, and tolerates a probe failure with a warning.

`rust-sak` could close the asymmetry with a streaming magic-byte probe. It is not worth it here:
TIFF's IFD offset may point at the end of the file, which makes "sniff a prefix" awkward for exactly
the formats that matter, and the observable cost today is a mis-named file losing a dimensions
column while still loading and exporting correctly.

### Pixels are held behind an `Arc`

`SourceImage` holds `Arc<DynamicImage>`.

This is what makes the async forms possible at all: `spawn_blocking` requires `'static`, so the
async `save` cannot forward a borrow of the caller's image, and a deep clone of a decoded image -
hundreds of megabytes after an upscale - is not an option. An `Arc` clone is a refcount bump.

It is the right shape independently of that. Pixels behind an `Arc` cannot be mutated in place
without `Arc::make_mut`, so the failure the Go type warns about in its own doc comment - altering
`Pixels` and leaving `Hash` describing the original - becomes hard to write rather than merely
discouraged. And the per-operation cache this identity exists to key will hand the same image to
several callers, which is `Arc` by construction; choosing it now avoids changing a public type later.

*Alternative rejected:* `tokio::task::block_in_place`, which carries no `'static` bound. It panics on
a current-thread runtime, and this crate builds one in its own tests.

### `SourceImage` keeps its fields private

Path, pixels and identity are private, with accessors, and `dimensions()` is derived from the pixels
rather than stored. Any construction that substitutes pixels goes through a constructor that takes
the matching identity. A stored dimensions field would be a second source of truth for a fact the
pixels already carry - the same class of defect as a stale identity, which this design otherwise
spends effort preventing.

### Both a blocking and an async form of each operation

The blocking form is the implementation; the async form is a `spawn_blocking` wrapper over it. The
async form keeps the plain name (`load`, `save`, `probe`) to match `Opai::initialize`, and the
blocking form takes the `_blocking` suffix.

A sync-only API means every Tauri command has to remember to wrap the call, and forgetting freezes
the window while every test still passes. An async-only API forces a runtime on the CLI and on unit
tests. Offering both costs three lines per operation.

*Alternative rejected:* a `blocking` submodule in the style of `reqwest`. Justified for a large
surface; three functions do not need it.

### `spawn_blocking` is generalised over its error

It becomes `spawn_blocking<T, E, F>(work: F) -> Result<T, E> where E: From<Cancelled>`, with
`Cancelled` a new marker error in `task`.

`InitError::Cancelled` stays a unit variant and gains a hand-written
`impl From<task::Cancelled> for InitError`, rather than becoming a tuple variant carrying the marker.
That keeps `InitError`'s public shape and its existing `matches!(err, InitError::Cancelled)` tests
untouched, and confines the change to `task.rs` plus type annotations at the four existing call
sites (`lib.rs:249`, `install.rs:62`, `install.rs:185`, `install.rs:207`).

### Camera RAW is detected by extension, ahead of everything else

`load` checks the extension against a RAW list before any content-based routing, and reports an
unsupported format.

This is not a placeholder: without it, RAW silently decodes *wrongly*. Most RAW files carry plain
TIFF magic (`II*\0` / `MM\0*`), so `decode_bytes` would route a NEF or ARW to the TIFF decoder and
return an embedded preview or corrupt pixels rather than failing. Go documents exactly this and
branches on the extension for the same reason. An honest error now is strictly better than a wrong
image, and slice 3 fills the same branch with a decoder.

### Module layout

A directory, `crates/opai/src/image/`, following `rust-sak`'s own `src/image/` rather than inventing
a different shape: `mod.rs` (module docs, re-exports), `source.rs` (`SourceImage`), `load.rs`,
`save.rs`, `probe.rs`, `raw.rs` (extension detection only), `error.rs`. Tests live beside the code
they cover.

Naming the module `image` shadows the `image` crate inside this crate, which is resolved the same way
`rust-sak` resolves it - `use ::image::DynamicImage` with a leading `::`. Following the sibling crate
is worth more than avoiding the disambiguation.

## Risks / Trade-offs

- **Tiny AVIF frames hang the encoder.** `rust-sak`'s `image` README records that the AVIF encoder
  still hangs on sub-16px frames - a hang, not an error. → No AVIF fixture in the test suite may be
  smaller than 16x16, and the round-trip tests use a size comfortably above it. Noted in the tasks so
  it is not rediscovered by a hanging CI job.
- **AVIF encodes serialise against each other.** The module funnels them through an internal lock
  because SVT-AV1 keeps per-encode global state. → Correctness is unaffected; it bounds export-queue
  throughput for AVIF only, and belongs in the export slice's design rather than here.
- **First build needs network access.** The `avif`, `heif` and `webp` crates fetch prebuilt static
  codec binaries on first build. → Already true for any consumer of this feature; CI either allows
  the fetch or sets `AVIF_BINARIES_DIR` / `HEIF_BINARIES_DIR` / `WEBP_BINARIES_DIR`. It is new to
  *this* repository, because `image` is a feature this change turns on for the first time.
- **Holding encoded bytes in memory during load.** A 60 MB RAW is 60 MB briefly resident. → Bounded
  by one file at a time per call, and small beside the decoded image. The alternative costs a second
  full read of every file.
- **The identity is only as good as its construction site.** Privacy and `Arc` make the stale case
  hard, not impossible - a caller can still construct a `SourceImage` with a mismatched identity
  through the constructor that accepts one. → That constructor is the single reviewed place where
  the invariant can be broken, which is the point; it is documented as such.

## Platforms and validation

No platform-specific code: every path goes through `rust-sak` and `std::fs`. The change is intended
to build and pass on macOS, Windows and Linux, and CI must exercise all three because the codec
binary fetch is the only platform-sensitive part.

No execution providers are involved - this slice runs no inference.

Nothing is validated yet; this design is written ahead of implementation. The tasks require
`cargo fmt`, `cargo clippy` and the test suite to pass before the change is called complete.
