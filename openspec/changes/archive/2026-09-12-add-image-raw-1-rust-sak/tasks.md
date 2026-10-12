## 1. The `image-raw` feature in `../rust-sak`

- [x] 1.1 Add the `image-raw` feature and `zenraw = { version = "0.2", default-features = false, features = ["rawler", "rawloader"] }` to `../rust-sak/Cargo.toml`; verify `cargo build --no-default-features --features image-raw` succeeds and `cargo tree -f "{p} {l}" --features image-raw` shows `zenraw` AGPL-3.0 with `rawler` and `rawloader` LGPL-2.1 and no other copyleft crate. (`rawloader` is required, not optional: `zenraw`'s `rawler` backend does not compile without it - see design.md - Pinning.)
- [x] 1.2 Keep the licence consequence out of the crate's own documentation, at the repository owner's direction: no notice in the `Cargo.toml` feature definition, the crate README, the `image` module README or its rustdoc. The encumbrance stays opt-in through the feature name alone, and `cargo tree -f "{p} {l}" --features image-raw` is what reports the dependency licences. (A four-place notice was written first and then removed - see design.md, The licence decision.) Verify no AGPL/LGPL/copyleft text remains in those four files, that no link points at a deleted licensing section, and that `cargo doc --features image-raw` still renders the RAW section on the module page

## 2. Vocabulary

- [x] 2.1 Add `RawFormat` in `src/image/raw_format.rs` with `from_extension`, `from_path`, `extension`, `from_magic` and `is_raw_bytes`, mirroring `ImageFormat`'s shape; verify unit tests cover every variant's extension round-trip, case-insensitivity, and that `tif`/`tiff` are not taken for RAW
- [x] 2.2 Add `RawImageInfo` in `src/image/raw_info.rs` (`format: Option<RawFormat>` - see design.md, Probing returns `RawImageInfo`, for why it is optional; `width`, `height`, `bit_depth: Option<u8>`, `make`, `model`, `is_dng`) and the two RAW `ImageError` variants - a failed decode wrapping `zenraw`'s error, and bytes that are not RAW - all behind `#[cfg(feature = "image-raw")]`; verify the crate still builds with the feature off and each variant's `Display` names its failure distinctly

## 3. Decoding and probing

- [x] 3.1 Write the `PixelBuffer` → `DynamicImage` conversion in `src/image/raw_dispatch.rs`, taking the contiguous path when `is_contiguous` holds and copying row by row through `row(y)` otherwise; verify a unit test builds a strided `PixelBuffer` with a known pattern and asserts both paths produce identical pixels
- [x] 3.2 Add `decode_raw_bytes` and `decode_raw_file` returning `DynamicImage::ImageRgb16` via `OutputMode::Develop` and `&enough::Unstoppable`; verify they decode the fixture from task 4.1 and that `decode_raw_bytes` on PNG bytes returns the not-RAW error rather than panicking
- [x] 3.3 Add `probe_raw_bytes` and `probe_raw_file` returning `RawImageInfo`, with the whole-file read documented on `probe_raw_file`; verify a test asserts the probed dimensions equal the decoded image's dimensions for the same fixture
- [x] 3.4 Wire the six new items into `src/image/mod.rs` with `#[cfg(feature = "image-raw")]` on every `mod` and `pub use`, and add `image-raw` to the per-feature isolation matrix in `../rust-sak/.github/workflows/ci.yml`; verify `cargo build --no-default-features --features image` and `--features image-raw` both succeed locally with the RAW names absent from the first, and that the workflow runs the new entry on macOS, Windows and Linux

## 4. Tests

- [x] 4.1 Write the minimal-DNG builder in `src/image/tests.rs` - uncompressed 16-bit Bayer CFA with `DNGVersion`, `CFAPattern`, `CFARepeatPatternDim`, `BlackLevel`, `WhiteLevel`, `ColorMatrix1`, `CalibrationIlluminant1`, `AsShotNeutral` and strip offsets over a generated gradient - and assert it develops to the picture it encodes, a red-dominant CFA pattern yielding a red-dominant `ImageRgb16`; verify the assertion fails if the channel order is transposed
- [x] 4.2 Add the `ZENRAW_SAMPLES_DIR` test that decodes and probes every file in the named directory and skips silently when the variable is unset; verify it is skipped in a normal `cargo test` run and passes against a local folder of camera files

## 5. Release `rust-sak`

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --all-features -- -D warnings`, `cargo test --all-features` and `cargo test --doc --all-features` in `../rust-sak`; verify all four are green
- [x] 5.2 **Get the repository owner's explicit go-ahead** on the change and on how the licence consequence is documented, then push, bump the version to the CalVer number for the release month, and cut the tag; verify `gh api repos/vegidio/rust-sak/tags` lists it

## 6. Adopt the pin here

- [x] 6.1 Move `opai`'s `rust-sak` pin to the new tag and add `image-raw` to its feature list in `crates/opai/Cargo.toml`, updating `Cargo.lock`; verify a cold `cargo build --locked` resolves and compiles with no source change in this repository
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test --locked` across the workspace; verify all three are green and that loading a RAW file still returns `ImageIoError::UnsupportedRaw`, since this slice deliberately does not fill the seam
