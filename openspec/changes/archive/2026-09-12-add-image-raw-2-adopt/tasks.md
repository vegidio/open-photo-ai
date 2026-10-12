## 1. Recognition and coverage

- [x] 1.1 Add `ari`, `crm` and `ori` to `RAW_EXTENSIONS` in `crates/opai/src/image/raw.rs`, each in the vendor group it belongs to (ARRI, Canon, Olympus), and update the constant's doc comment — it currently says the eventual decoder "may be narrower", which is now a fact with a number rather than a possibility; verify the existing `every_published_raw_extension_is_recognised` test covers all 47 entries and that a new test asserts the three additions are recognised
- [x] 1.2 Add a `pub(crate) fn decodable(extension: &str) -> bool` (or equivalent) in `raw.rs` that answers from `RawFormat::from_extension`, never from a second list (design.md — D4); verify tests assert all 29 decodable extensions come back `true`, all 18 refused ones come back `false`, the two sets partition `RAW_EXTENSIONS` exactly, and `tif`/`tiff` are in neither

## 2. Loading

- [x] 2.1 Route loading in `crates/opai/src/image/load.rs` per design.md — D2: refuse a recognised-but-undecodable RAW extension with `ImageIoError::UnsupportedRaw` before the file is opened, then read once, hash the buffer, and decode a decodable RAW extension through `rust_sak::image::decode_raw_bytes` and everything else through `decode_bytes`; verify a test loads the DNG fixture from task 4.1 and asserts its pixels, path and identity, and that the identity still equals `xxh3_file` of the same file
- [x] 2.2 Map the RAW decoder's failures per the table in design.md — D5 (`ImageError::Raw` and `ImageError::NotRaw` to `Decode`, `ImageError::Io` to `Read`), and reword `ImageIoError::UnsupportedRaw`'s message and doc comment in `image/error.rs` — "`{extension}` images cannot be opened yet" now has to mean "this RAW format has no decoder" rather than "RAW is not supported yet"; verify tests assert a `.nef` holding PNG bytes returns `Decode` carrying `NotRaw`, a `.dng` holding corrupt RAW-shaped bytes returns `Decode` carrying `Raw`, and the reworded message still names both the file and the extension
- [x] 2.3 Rewrite the three tests that assume every RAW extension is refused (design.md — Risks): `load.rs`'s TIFF-bytes-as-`.nef` test becomes a decodable-extension decode assertion plus an undecodable-extension refusal, its missing-`.cr3` test moves to an undecodable extension, and the genuine-TIFF test stays as it is; verify the refusal-before-read property is still asserted — an undecodable RAW under a directory that does not exist must return `UnsupportedRaw`, not `Read`

## 3. Probing

- [x] 3.1 Add `probe_raw` and `probe_raw_blocking` to `crates/opai/src/image/probe.rs` over `rust_sak::image::probe_raw_file`, returning `RawImageInfo`, with the `UnsupportedRaw` pre-check for undecodable extensions and the error mapping from design.md — D5, and the whole-file read documented on both; verify tests assert the fixture's format, dimensions, make, model and DNG flag, that a missing file returns `Read`, and that an undecodable RAW extension returns `UnsupportedRaw`
- [x] 3.2 Add the test that `probe_raw` and `load` agree about a RAW file's dimensions — the property a file listing depends on — and the test pinning `probe` on a RAW name as a `Codec` failure rather than something new (design.md — D6); verify both pass against the fixture and that the `Codec` assertion names the file

## 4. The fixture

- [x] 4.1 Port the minimal-DNG builder from `../rust-sak/src/image/tests.rs` into `crates/opai/src/image/test_support.rs` — the TIFF-tag writer, `minimal_dng`, a `written_dng`-style helper, and the channel-mean helper — with a comment saying where it came from and why it is duplicated rather than shared (design.md — D7); verify it develops to the picture it encodes (a red-dominant CFA yields a red-dominant image) and that transposing the channel order moves the dominant channel
- [x] 4.2 Port the samples-directory test so a local folder of real camera files can be run through this crate's own `load` and `probe_raw`, skipping silently when the environment variable is unset; verify it is skipped in a normal `cargo test` run and that it passes against a local folder of camera files

## 5. Surface and documentation

- [x] 5.1 Make the surface public per design.md — D8: `is_raw` public and re-exported (documenting that it answers the routing question `rust_sak::image::is_raw_bytes` cannot), and `RawFormat` and `RawImageInfo` re-exported from `image/mod.rs` and the crate root beside `ImageInfo` and `ImageFormat` with the same rationale the existing re-export comment gives; verify `cargo doc` renders all three from `opai::image` and `opai`, that `is_raw_bytes` is **not** re-exported anywhere in this crate, and that nothing else from `image-raw` leaks into the surface
- [x] 5.2 Rewrite the "Camera RAW is refused, for now" section of `image/mod.rs`'s module docs: RAW loads for 29 extensions and is refused for 18, probing RAW is its own operation and reads the whole file, a developed RAW is 16-bit, and GIF and WebP refuse 16-bit until slice 4; verify the module page no longer claims RAW cannot be opened and that `load.rs`'s and `probe.rs`'s own `# Errors` sections match what they now return
- [x] 5.3 Update `crates/opai/tests/public_api.rs`: the `UnsupportedRaw` assertion moves to an undecodable extension, and a consumer-side RAW path is added — recognise a RAW name with `is_raw`, load the DNG fixture, and read a `RawImageInfo` back from `probe_raw_blocking` naming both types from outside the module; verify the test compiles without naming `rust_sak` and passes

## 6. Verification

- [x] 6.1 Confirm the 16-bit encode gap is exactly as design.md — Risks / Trade-offs describes it, so slice 4 starts from a measured fact rather than a read of the encoders: write the developed fixture through `save_blocking` in all eight formats; verify BMP, JPEG, PNG, TIFF, AVIF and HEIF succeed, that GIF and WebP fail with an unsupported-colour error, and that the two failures are recorded in the spec's scenario wording
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --locked` across the workspace; verify all three are green and that `openspec validate --changes add-image-raw-2-adopt --strict` passes
