## 1. Measure before choosing the convention

- [x] 1.1 Measure what `react-advanced-cropper` actually does with rotation, in a scratch page rather
  than in this repository: which direction a positive angle turns the image, and what
  `getCoordinates()` reports its coordinates against once the image is rotated - specifically whether
  the space it measures is the canvas expanded to `w*|cos t| + h*|sin t|` by `w*|sin t| + h*|cos t|`.
  Verify by rotating a deliberately asymmetric test image (a wide rectangle with one marked corner) by
  +30 degrees and reading the reported coordinates back; confirm or correct design.md D2 and D3 from
  the result before writing any Rust. This is the fact slice 3 depends on and the reason the API's
  sign is chosen here rather than there.

## 2. The rotation in `../rust-sak`

- [x] 2.1 Add `src/image/rotate.rs` with the module's one-public-function-per-file shape: the
  expanded-canvas bounds from design.md D3, the inverse map and bilinear sampling with `f32`
  intermediates from D5, fully transparent fill from D6, and the alpha-bearing promotion that
  preserves bit depth from D4 - with the clockwise-positive sign convention (D2) and the bounds
  formula written into the function's own rustdoc, because a rotation whose sign is ambiguous is a
  rotation applied backwards somewhere. Wire it into `src/image/mod.rs` behind the existing `image`
  feature, adding no feature of its own (D8). Verify `cargo build --no-default-features --features
  image` succeeds, that the name is absent from a `--no-default-features` build, and that `cargo doc`
  renders it on the module page.
- [x] 2.2 Add `rotate` to the public-function tables in `src/image/README.md`, in the section shape
  the existing decode/probe/encode tables use, stating the sign convention and that the output
  carries alpha. Verify the README's table lists it with its real signature and that no other
  function's row was disturbed.

## 3. Tests, beside the code they cover

- [x] 3.1 Pin the two load-bearing facts: that a positive angle turns clockwise (a marked corner of an
  asymmetric image lands where clockwise puts it, and the assertion fails if the sign is flipped), and
  that the output dimensions equal design.md D3's formula rounded outward for a spread of angles
  including 0, 30, 45, 90 and negatives. Verify both fail when the rotation matrix is transposed.
- [x] 3.2 Assert agreement with the `image` crate's own exact quarter turns - `rotate90`, `rotate180`
  and `rotate270` - allowing only the colour-type promotion to differ, and that rotating by `+t` then
  `-t` returns the original content within a bilinear tolerance on the interior. Verify the
  quarter-turn assertions are exact rather than tolerance-based, since no interpolation should occur
  at those angles.
- [x] 3.3 Assert the colour-type matrix from design.md D4 - `Rgb8` to `Rgba8`, `Rgb16` to `Rgba16`,
  `Luma8` to `LumaA8`, and an already-alpha-bearing input keeping its own type - that a 16-bit source
  keeps sixteen bits through the rotation rather than being flattened, and that the uncovered corners
  are fully transparent while the interior is fully opaque. Verify a 16-bit gradient survives with its
  full range intact.

## 4. Release `rust-sak`

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-features -- -D warnings`, `cargo test
  --all-features` and `cargo test --doc --all-features` in `../rust-sak`; verify all four are green.
- [x] 4.2 **Get the repository owner's explicit go-ahead**, then push, bump the version to the CalVer
  number for the release month, and cut the tag. Verify `gh api repos/vegidio/rust-sak/tags` lists it
  and that the tag points at the pushed commit - consumers pin by tag, so a push here is a release.

## 5. Adopt the pin here

- [x] 5.1 Move the `rust-sak` pin to the new tag in `crates/opai/Cargo.toml` **and in
  `crates/perf/Cargo.toml`**, updating `Cargo.lock`. Both crates pin the same git source by tag, and
  two tags resolve to two `rust-sak` entries in the lock file, so they move together; `perf` takes
  only the `fs` feature and gains nothing from `rotate`, so its bump is bookkeeping. No feature is
  added anywhere - `rotate` is behind the `image` feature `opai` already enables. Verify a **cold**
  `cargo build --locked` resolves and compiles with no source change anywhere in this repository,
  from a cleared registry cache rather than from a warm one, so a tag that resolves only locally is
  caught here, and that `Cargo.lock` holds exactly one `rust-sak` entry.
- [x] 5.2 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test
  --locked` across the workspace; verify all three are green, that `openspec validate --changes
  add-crop-1-rust-sak --strict` passes, and that the application behaves exactly as it did before -
  no image can be rotated and no crop can be set, because this slice deliberately writes no seam.
