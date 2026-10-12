## 1. Foundations

- [x] 1.1 Add the `image` feature to the `rust-sak` dependency and add the `image` crate with `default-features = false`, pinned to the version `rust-sak` builds against; verify `cargo tree -p opai -i image` shows a single version and `cargo build -p opai` succeeds.
- [x] 1.2 Generalise `task::spawn_blocking` to `spawn_blocking<T, E, F>(..) -> Result<T, E> where E: From<Cancelled>`, add the `Cancelled` marker error and `impl From<Cancelled> for InitError` keeping `InitError::Cancelled` a unit variant, and update the four call sites (`lib.rs:249`, `install.rs:62`, `install.rs:185`, `install.rs:207`); verify the existing `task.rs` and `error.rs` tests pass unchanged.
- [x] 1.3 Add `image::error::ImageIoError` with `thiserror`, covering unreadable/unwritable paths (carrying the path), undecodable content, unsupported RAW extensions (carrying the extension), the `rust-sak` codec error, and cancellation via `From<Cancelled>`; verify a test asserts the RAW, decode and IO cases are distinguishable and that their messages name the file or extension.

## 2. The image value

- [x] 2.1 Add `image::source::SourceImage` with private `path`, `Arc<DynamicImage>` pixels and identity, accessors for each, a derived `dimensions()`, and a constructor that takes pixels together with their matching identity; verify tests cover that `dimensions()` agrees with the pixels and that cloning shares rather than copies the pixel buffer.

## 3. Operations

- [x] 3.1 Add `image::raw` with the camera RAW extension list and its detection predicate, matching the formats Go's `IsRawExtension` covers; verify tests assert the predicate is case-insensitive, accepts the full list, and rejects `tif`/`tiff`.
- [x] 3.2 Implement `load_blocking` (RAW extension check first, then one `std::fs::read`, then `xxh3_bytes` and `decode_bytes` over the same buffer) and the `load` wrapper over `spawn_blocking`; verify tests cover a successful decode, identity equal to `xxh3_file` of the same file, a JPEG under a `.png` name decoding as JPEG, a non-image refused, and a missing file refused.
- [x] 3.3 Implement `save_blocking` (create the destination, `encode_writer` with the caller's explicit format, count and return the bytes written) and the `save` wrapper; verify tests cover a round trip, that the source file is untouched, that a JPEG written to a `.png` destination contains JPEG bytes, that options targeting another format are refused, and that an unwritable destination errors.
- [x] 3.4 Implement `probe_blocking` over `rust_sak::image::probe_file` and the `probe` wrapper, re-exporting `ImageInfo`; verify tests assert the reported width and height equal those from loading the same file, and that a missing file errors.

## 4. Public surface

- [x] 4.1 Declare the module in `lib.rs`, re-export `SourceImage`, the error, `ImageFormat`, `ImageInfo` and `EncodeOptions`, and write the module documentation covering the load/save/probe contract, the identity invariant and the RAW gap; verify `cargo doc -p opai --no-deps` builds without warnings and the `public_api` test is updated to include the new exports.

## 5. Verification

- [x] 5.1 Add the test fixtures and the cross-cutting round-trip test covering all eight formats, asserting that every format survives encode and decode and that both forms of each operation agree; verify no AVIF fixture is smaller than 16x16, since `rust-sak` records that the AVIF encoder hangs rather than errors on sub-16px frames.
- [x] 5.2 Run `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test -p opai`, and confirm all three pass with no new warnings.
