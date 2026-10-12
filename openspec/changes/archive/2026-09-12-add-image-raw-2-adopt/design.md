## Context

See proposal.md — Why. The state this change starts from, all of it in `crates/opai/src/image/`:

- `raw.rs` holds `RAW_EXTENSIONS` (44 entries, the Go application's published list), `extension_of`
  and `is_raw`, all `pub(crate)`. Its module doc says outright that it exists ahead of any decoder
  and that "a later change puts a decoder behind the same branch". This is that change.
- `load.rs` checks the extension first and returns `ImageIoError::UnsupportedRaw` before opening the
  file; otherwise it reads the file once, hashes the buffer for the identity and decodes from the
  same buffer.
- `probe.rs` calls `rust_sak::image::probe_file`, which routes on the extension, so a RAW name comes
  back as `ImageError::UnknownExtension` mapped to `ImageIoError::Codec`.
- `test_support.rs` generates its eight fixtures by encoding one synthetic picture through the codec
  each test is about to exercise; nothing is committed as a binary.

The dependency side is already done. `crates/opai/Cargo.toml` pins `rust-sak` at tag `26.9.3` with
`image-raw` enabled, so `decode_raw_bytes`, `decode_raw_file`, `probe_raw_bytes`, `probe_raw_file`,
`RawFormat`, `RawImageInfo`, `is_raw_bytes`, `ImageError::Raw` and `ImageError::NotRaw` are all
resolvable and compiled today. **This change adds no dependency and touches no manifest.**

## Slices

RAW support is delivered as a series, continuing the one `add-image-io` opened.
**This change is slice 3.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-image-io` | Loading, writing and probing images over `rust-sak`; `SourceImage` and its identity; the RAW dispatch seam with nothing behind it | - | **done** (archived `2026-09-12-add-image-io`) |
| 2 | `add-image-raw-1-rust-sak` | RAW decoding in `rust-sak`'s `image` module behind a new `image-raw` feature: `zenraw` on its `rawler` backend, a decode-only `RawFormat`, a synthesised DNG fixture, and a `rust-sak` release. Here: the pin moved to that release with the feature enabled | 1 (for the seam it fills) | **done** (archived `2026-09-12-add-image-raw-1-rust-sak`) |
| 3 | `add-image-raw-2-adopt` | Filling the seam in `opai`: a RAW file loads instead of being refused, a RAW file can be described without being decoded, `UnsupportedRaw` retires for the 29 extensions the backend covers and stays for the 18 it does not | 2 | **CURRENT** |
| 4 | `add-image-encode-depth` | A 16-bit picture written to an encoder that accepts only 8 bits is converted down rather than refused. In `rust-sak`'s encode dispatch, because it is general-purpose and affects every consumer, plus a release and the pin here | 3 (which makes 16-bit the normal case) | PENDING |

Slice 4 is new to this table and is the one piece of RAW support this slice deliberately leaves
undone; the `rust-sak` coverage section below and Risks / Trade-offs carry the reasoning and the
numbers. It is a slice rather than a task here for the same reason slices 2 and 3 are apart: the fix
belongs in another repository and needs a release between the change and its verification.

## Goals / Non-Goals

**Goals:**

- A RAW file loads the way every other format loads — same type, same identity, same dimensions
  contract — so nothing downstream has to know which family a picture came from.
- Keep every property the extension-first check was written to protect: a NEF is never handed to the
  TIFF decoder, a genuine TIFF is never taken for a NEF, and a RAW the application cannot decode is
  refused by name before the file is read.
- The file is still read exactly once, so the identity keeps covering every byte without a second
  pass over a 60 MB file.
- One source of truth for which RAW formats can be decoded: the backend's own answer, never a second
  hardcoded list here.
- The happy path is exercised in CI on three platforms with no camera file, no network and no
  external binary.

**Non-Goals:**

- A unified description type over both format families. See D1.
- Making `probe` dispatch to RAW on its own. See D6.
- Any change to the eight existing formats' behaviour, to `save`, or to `SourceImage`.
- A public accepted-input extension list for a file dialog. The Go counterpart
  (`SupportedInputExtensions`) returns the union of image and RAW extensions, which is a question for
  the change that draws the dialog; nothing draws one yet.

## How the work divides

**`opai` only.** No `gui` or `cli` work, and none is deferred into one: both front ends open files,
so file opening lives in `opai`. That was settled in slice 1 and nothing here reopens it.

## `rust-sak` coverage

| Need | Covered by | Feature |
|---|---|---|
| Decode a RAW file to a `DynamicImage` | `decode_raw_bytes` | `image-raw` (already enabled) |
| RAW metadata without decoding | `probe_raw_file` | `image-raw` |
| Which RAW formats are decodable | `RawFormat::from_extension` | `image-raw` |
| The metadata shape | `RawImageInfo` | `image-raw` |
| What the RAW decoder refused | `ImageError::Raw`, `ImageError::NotRaw` | `image-raw` |
| Decode the eight non-RAW formats, probe them, encode them | unchanged from slice 1 | `image` |

**Nothing is missing, and nothing new is added to `rust-sak`.** One gap exists and is not this
slice's to fill: `rust-sak`'s encode dispatch refuses 16-bit input for GIF and WebP rather than
converting it down. JPEG and BMP convert (the `image` crate's own encoders override the conversion
hook), PNG, TIFF, AVIF and HEIF take 16 bits natively, and GIF and WebP answer
`ImageError::Unsupported(Color(Rgb16))`. It is general-purpose — every `rust-sak` consumer encoding a
16-bit PNG hits it — so by the project's own rule it is fixed in `rust-sak` rather than worked around
here, which makes it slice 4.

**No third-party crate is introduced.** The RAW backend (`zenraw` on `rawler`) was chosen in slice 2
against `rawler` alone, `rawloader` alone and LibRaw bindings; that reasoning lives in that slice's
archived design and is not repeated.

## Decisions

### D1 — Probing mirrors `rust-sak`'s split rather than hiding it

`probe`/`probe_blocking` keep their signature and keep returning `ImageInfo`. New
`probe_raw`/`probe_raw_blocking` return `RawImageInfo`. A caller picks between them with the
now-public `is_raw`.

The two are not two spellings of one operation:

| | `probe` | `probe_raw` |
|---|---|---|
| Covers | BMP, GIF, JPEG, PNG, TIFF, AVIF, HEIF, WebP | 29 RAW extensions |
| Reads | the header; a bounded prefix for AVIF, HEIF, WebP | **the whole file** |
| Reports | format, width, height, colour type, bit depth | format (optional), width, height, sensor bit depth (optional), make, model, is-DNG |
| Format type | `ImageFormat` — also the encode target | `RawFormat` — decode only |

*Alternatives considered.* A single `probe` returning an enum or a flattened struct over both: one
call for a file listing, but it breaks the existing signature, invents an `opai` vocabulary on top of
`rust-sak`'s two honest types, and — the reason it was rejected — puts a kilobyte read and a 40 MB
read behind one name, with nothing in the signature telling the caller which they are paying for.
Deferring RAW probing to a fourth slice: rejected because slice 2's plan named it as part of this
slice and because `probe_raw` is two functions and an error mapping, not a slice's worth of work.

*Parity note.* The Go application does unify, but only at `{width, height}`: `ImageDimensions`
branches on the extension internally and returns two integers either way, discarding the make, model
and bit depth that `RawImageInfo` carries. A listing-facing view that narrows both types to what a
file list actually shows is a reasonable thing to build — for the change that builds the file list,
which is the caller that knows what it needs.

### D2 — Loading decodes from bytes, not from the path

`decode_raw_bytes(&bytes)` rather than `decode_raw_file(path)`, even though the path is in hand and
the file-taking form exists.

The identity is the reason. `load_blocking` reads the file once and derives both the XXH3-64 and the
pixels from that one buffer, which is what makes the identity cover every byte — including trailing
bytes a decoder stops before — while equalling `xxh3_file` of the same file. `decode_raw_file` reads
the file itself, so taking it would mean either reading a 60 MB RAW twice or giving up the property.
The Go application went to a `TeeReader` into a pipe with a drain loop to avoid that same second
pass; here holding the buffer is enough, and it is already how the other eight formats work.

The shape this gives `load_blocking`:

```
extension = extension_of(path) if is_raw(path)
if extension is Some and RawFormat::from_extension(extension) is None:
    return UnsupportedRaw          # before the file is opened
bytes    = read(path)?             # one read, as before
identity = xxh3_bytes(&bytes)      # over every byte, as before
pixels   = if extension.is_some() { decode_raw_bytes(&bytes) } else { decode_bytes(&bytes) }
```

The RAW branch is the only new line, and the refusal moves from "every RAW extension" to "every RAW
extension the backend does not cover", which is what keeps the pre-read refusal for the formats that
still cannot be opened.

### D3 — Probing takes the path, not the bytes

The mirror image of D2, for a different reason: `probe_raw_bytes` leaves `RawImageInfo::format` as
`None` for every TIFF-based format, because a NEF, an ARW, a CR2 and a PEF open with the same four
bytes and the header cannot separate them. `probe_raw_file` fills the format in from the extension.
Probing exists to describe a file, so the field that names the format is the one it cannot be allowed
to drop, and the path is what supplies it. Both forms read the whole file, so nothing is paid for it.

### D4 — Decodability is asked of the backend; recognition stays local

Two questions, two owners:

- **Is this a RAW file?** `opai`'s own `RAW_EXTENSIONS`, and the answer is deliberately a superset of
  what can be decoded. A RAW format with no decoder is still a RAW format and must never reach the
  TIFF decoder, so dropping the 18 uncovered extensions from the list would reintroduce the exact
  misdecode the check exists to prevent.
- **Can it be decoded?** `RawFormat::from_extension(extension).is_some()`, asked of `rust-sak` at the
  point of use. Never a second constant here: a backend that gains a format would then be silently
  refused by a stale list, which is the failure mode a hardcoded copy always has.

`ari`, `crm` and `ori` join `RAW_EXTENSIONS` because the backend decodes all three and this
application did not recognise them, so they were falling through to content routing. That takes the
list to 47, of which 29 decode:

```
recognised   47   (44 from the Go list + ari, crm, ori)
decodable    29   ari arw crw cr2 cr3 crm dcr dcs dng erf fff iiq kdc mef mos
                  mrw nef nrw orf ori pef qtk raf raw rw2 rwl srw 3fr x3f
refused      18   bay bmq cap cine cs1 dc2 eip gpr ia k25 kc2 mdc ptx pxn
                  rdc sr2 srf sti
```

The 18 are the visible edge of slice 2's camera-coverage divergence, and the refusal names the
extension so a user with a folder of `.gpr` files is told which format is missing rather than that
their file is broken.

### D5 — A failed RAW decode is a `Decode` failure

The mapping from `rust-sak`'s errors:

| `rust-sak` | `ImageIoError` | Why |
|---|---|---|
| `ImageError::Raw(..)` | `Decode { path, source }` | The file was read and could not be decoded — the same thing a corrupt JPEG is, reported the same way. The codec's own words stay reachable through the error chain. |
| `ImageError::NotRaw` | `Decode { path, source }` | A `.nef` holding something that is not RAW. Also "read it, could not decode it". |
| `ImageError::Io(..)` | `Read { path, source }` | Kept apart from a decode failure exactly as `probe` already keeps it apart: a missing file and a corrupt file are two different things to tell a user. |
| `ImageError::UnknownExtension` | `Codec { path, source }` | Only reachable by calling `probe_raw` on a path that is not RAW at all — a caller's mistake, passed through as the codec's refusal rather than dressed up as an unsupported RAW format. |

*No new variant.* `zenraw` reports an unknown camera body and a corrupt file as one `RawError` and
cannot separate them, so a `RawDecode` variant saying "this camera may not be supported" would be
guessing on the user's behalf half the time. `UnsupportedRaw` was the other candidate and is worse:
it would tell a photographer with one corrupt NEF that NEF is unsupported, while the next NEF in the
same folder opens fine.

### D6 — `probe` on a RAW name stays a `Codec` failure

A mis-routed `probe("DSC_0001.nef")` keeps returning `Codec { source: UnknownExtension }`, which is
what it returns today and what the existing "a name that names no supported format is refused"
requirement already covers. A dedicated "use the RAW form" variant was considered and rejected: it
would widen a user-facing error enum to describe a programming mistake, and `is_raw` is public
precisely so the mistake is avoidable rather than diagnosable. It is pinned by a test so the
behaviour is chosen rather than inherited.

`rust_sak::image::is_raw_bytes` is **not** re-exported. It answers `true` for a plain TIFF — it is a
header check, and the RAW formats are TIFF containers — so it cannot be used to route between the
RAW and TIFF decoders. Keeping it off this crate's surface stops it being reached for; `is_raw` on a
path is the routing question, and it is the one exposed.

### D7 — The DNG fixture is ported, not shared

`rust-sak` synthesises a minimal DNG for its own tests — a few hundred bytes of hand-written TIFF
tags describing a tiny uncompressed Bayer CFA image with the tags `rawler`'s DNG decoder reads
(`DNGVersion`, `CFAPattern`, `CFARepeatPatternDim`, `BlackLevel`, `WhiteLevel`, `ColorMatrix1`,
`CalibrationIlluminant1`, `AsShotNeutral`, strip offsets) over a generated gradient. It lives in
`#[cfg(test)] mod raw` and is unreachable from here, so the builder is ported into
`image/test_support.rs` beside the eight encoded fixtures.

This keeps the slice inside one repository and gives CI a real RAW decode on all three platforms.
The alternatives both cost more than the duplication: a `test-support` feature on `rust-sak`
publishing the builder means another release inside this slice, which is the boundary slices 2 and 3
were split along; and an environment-variable-only test against real camera files leaves the happy
path unexercised in CI, which is where a broken seam would actually be caught.

What makes the duplication tolerable is that the fixture encodes a *file format*, not an API: DNG 1.x
tag layout does not move when `rust-sak` releases. The samples-directory test comes across as well,
so a local folder of real camera files can be run through this crate's own `load` and `probe_raw`.

### D8 — What becomes public, and why each piece has to

```
opai::image::is_raw                     # so a caller can choose between probe and probe_raw
opai::image::{probe_raw, probe_raw_blocking}
opai::image::{RawFormat, RawImageInfo}  # re-exported from rust-sak
```

`RawFormat` and `RawImageInfo` are re-exported for the reason `ImageInfo` and `ImageFormat` already
are: a caller reads a `RawImageInfo` back from this crate's own function, and making it depend on
`rust-sak` to spell the return type would put a git-tag-pinned, unpublished crate in its manifest.
`RawFormat` comes with it because it is a field of `RawImageInfo`.

`RAW_EXTENSIONS` stays `pub(crate)`. The list a file dialog wants is the union of image and RAW
extensions, which is the dialog's question to ask.

## Risks / Trade-offs

- **A camera in the coverage gap, or one of the 18 refused formats** → The refusal names the
  extension, so the message is "this application cannot open `.gpr` files" rather than "this file is
  broken". Slice 2 accepted this trade deliberately in exchange for a pure-Rust decoder; the table in
  D4 is the exact list, which is more than the Go application can state about its own LibRaw build.
- **A 16-bit RAW cannot be exported as GIF or WebP** → Documented in the spec as a known limitation
  and fixed in slice 4. Nothing regresses: a 16-bit PNG or TIFF has always loaded as 16-bit, and no
  front end calls `save` yet, so no user can reach it before slice 4 lands.
- **`probe_raw` reads the whole file** → A directory of 40 MB RAWs costs 40 MB of reads each, where
  the same listing over JPEGs costs kilobytes. It is the backend's constraint rather than a choice
  here (RAW metadata lives at offsets deep into the file), it is documented on the function, and the
  Go application is not obviously cheaper — `raw.DecodeConfig` takes a reader and may stop earlier,
  which is a performance divergence and not a behavioural one. A file listing that finds it too
  expensive can show RAW dimensions lazily, which is the listing's decision to make.
- **Two copies of the DNG fixture builder** → Tolerable because it encodes a file format rather than
  an API (D7). If `rawler` ever tightens what it accepts, both copies fail the same way and the
  `rust-sak` pin is a tag, so it moves only on a deliberate bump.
- **Three existing tests assume RAW is always refused** → `load.rs`'s TIFF-bytes-as-`.nef` test and
  its missing-`.cr3` test both use extensions that are now decodable, and `tests/public_api.rs`
  expects `UnsupportedRaw` from a non-existent `.nef`. All three move to an uncovered extension, and
  the covered-extension cases they used to make are re-made as decode assertions. Catching these is
  a task, not a hope: they fail loudly, since a decodable extension now reaches the file system.
- **A picture that decodes but is wrong** is the one failure a test of "it did not error" would miss
  → the fixture's sensor values are known, so the assertion is that a red-dominant CFA develops to a
  red-dominant picture, and that transposing the channel order moves the dominant channel. Both come
  across from `rust-sak` with the builder.

## Platforms and providers

Pure Rust with no platform-specific code: `zenraw` is `#![forbid(unsafe_code)]` and pulls in no C
toolchain or system library, which is why slice 2 chose it. The same tests run on macOS, Windows and
Linux in CI, and the pin already compiles on all three from slice 2's own verification. Nothing here
touches model sessions, memory budgets or execution providers, so there is no ownership or lifetime
model to describe.

## Open Questions

- Whether a later change adds a listing-facing description that narrows `ImageInfo` and
  `RawImageInfo` to the handful of fields a file list shows. Deferrable: it is additive, it belongs to
  the change that builds the file list, and answering it either way leaves this slice's specs,
  approach and tasks unchanged.
