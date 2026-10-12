## 1. The conversions

- [x] 1.1 Create `crates/opai/src/models/colorization/lab.rs` and declare it in `models/colorization/mod.rs` as
      `pub(super) mod lab;` under `#[cfg_attr(not(test), expect(dead_code, reason = ...))]`, naming
      `add-colorization-2-pipeline` as the first caller. Port the D65 constants, `f`/`f_inv`, `srgb_to_linear`,
      `linear_to_srgb`, `rgb_to_lab`, `lab_to_linear_rgb` and a `#[cfg(test)]` `lab_to_rgb` from the reference's
      `internal/utils/color.go`, all in `f64` internally as there, with `[_; 3]` arrays for channels (design D4). Carry
      the reference's prose: the OpenCV float convention, and the 8-bit encoding it is not. Verify that
      `cargo build -p opai` succeeds with no warning and that nothing in the file is more visible than `pub(super)`.
- [x] 1.2 Port the reference's OpenCV reference-value, round-trip, clamping, gray-has-no-chroma and split-conversion
      tests (design D5). The split-conversion test compares against `u8::from_unit` until 3.1 lands, then against
      `srgb_u8`. Verify with `cargo test -p opai models::colorization::lab`, and check that the reference-value test
      fails when L is temporarily scaled by 255/100.

## 2. The linearization table, and what reads it

- [x] 2.1 Add the `u16`-keyed `LINEAR` table behind a `LazyLock`, computed as
      `srgb_to_linear(f64::from(w as f32 / 65535.0))`, and `lightness([u16; 3]) -> f32` and `gray([u16; 3]) -> f32`
      reading it (design D2). Carry the reference's reasoning: why L needs only Y, why the gray rendering collapses to
      one encode, and why the table is lazy. Verify that `cargo build -p opai` succeeds.
- [x] 2.2 Add the table tests: every entry equals the direct transfer, and `LINEAR[v * 257]` equals the reference's
      8-bit formula for all 256 `v`. Port `lightness`-matches-`rgb_to_lab` **bit for bit**, over the reference's
      strided byte sweep plus a 16-bit sweep that includes samples not of the form `v * 257`. Port `gray` against the
      Lab round trip within `1e-4` (design D5). Verify with `cargo test -p opai models::colorization::lab`.

## 3. The encodes

- [x] 3.1 Add `srgb_u8`, a binary search over 255 thresholds bisected on the `f64` bit pattern behind a `LazyLock`,
      found against `u8::from_unit(linear_to_srgb(c) as f32)` rather than the reference's `+ 0.5` truncation. Add
      `srgb_u16` as `u16::from_unit(linear_to_srgb(c) as f32)` (design D3). Point the split-conversion test from 1.2 at
      `srgb_u8`. Verify that `cargo build -p opai` succeeds.
- [x] 3.2 Port the boundary test: at and just below each of the 255 thresholds, the out-of-range inputs, and the
      200,001-point dense sweep, each against `u8::from_unit(linear_to_srgb(c) as f32)`. Add the 16-bit round trip,
      `srgb_u16(LINEAR[w]) == w` for every `w` (design D5). Verify with `cargo test -p opai models::colorization::lab`.
      Record in the task's completion note whether any threshold differs from the reference's `+ 0.5` rule. If one
      does, record it; do not change the rule.
      **Completion note:** one threshold of the 255 differs. Byte 1's boundary is `1.5176349173908358e-4` here against
      `1.517634737181359e-4` under the reference's `+ 0.5` rule. Between the two, `x * 255` is just under 0.5, and
      `x * 255 + 0.5` rounds up to 1.0 in `f32`: the float-addition tie D3 anticipates. The reference encodes that
      band of about `1.8e-11` linear light to 1, and this crate encodes it to 0. The other 254 thresholds are
      identical. The rule is unchanged.

## 4. Prose and toolchain

- [x] 4.1 Extend `models/colorization/mod.rs`'s header with the Lab contract. The family's graphs predict chroma only,
      the photograph's own L is kept at full resolution, and `lab` is where that round trip lives. Name the family as
      still refused until slice 2. Verify with `cargo doc -p opai --no-deps`: no broken intra-doc links.
- [x] 4.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`. Confirm that the
      `expect(dead_code)` is fulfilled rather than unfulfilled outside tests, and that
      `crates/opai/tests/public_api.rs` needed no change. Verify that all three exit zero.
