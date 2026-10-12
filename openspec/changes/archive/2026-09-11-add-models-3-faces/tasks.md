## 1. The face vocabulary

- [x] 1.1 Add `crates/opai/src/models/face.rs` with `Point`, `Rect` and `Confidence` - `Confidence::new`
      returning `Result<_, RangeError>` over `0.0..=1.0` with `NAME`/`MIN`/`MAX` constants, rejecting
      `NaN` - and verify tests in that file cover an in-range value carried as given, `-0.1`, `1.5` and
      `NaN` each refused with the range named, and the constants matching what the constructor enforces.
- [x] 1.2 Add `Face` (a `Rect`, `[Point; 5]`, a `Confidence`) with its serde derives, the five landmarks
      documented in their fixed order, and verify tests cover coordinates reported back unrounded, a
      round trip through serialization, and a serialized face with four landmarks or a confidence of
      `1.5` being refused.
- [x] 1.3 Add `Faces` over `Arc<[Face]>` plus `pub type DetectionOutput = Faces`, with construction from
      an iterator or `Vec`, an empty value, ordered access, and the two-decimal bounding-box signature
      the cache tag is taken over; verify tests cover an empty set being legitimate, order preserved
      unchanged, a clone comparing equal, and the signature pinned as a literal for a known selection.

## 2. Detection

- [x] 2.1 Add `models/detection/variant.rs` with the `NEW_YORK` row (codename `newyork`, label
      `New York`), `DetectionVariant::NewYork(FloatPrecision)`, `ALL`, and `tests::every_variant()`;
      verify a test asserts the label, codename and both precisions.
- [x] 2.2 Add `models/detection/mod.rs` with `Detection` - derived `PartialEq`/`Eq`/`Hash` over its one
      field, `display_name`, `artifact`, `required_artifacts` and `cache_tag` - and verify tests pin
      `dt_newyork_fp32` / `dt_newyork_fp16`, exactly one required artifact, the `dt-newyork-fp32` tag,
      the `New York (FP32)` display name, a serde round trip, and a serialized INT8 pairing refused.

## 3. Face recovery

- [x] 3.1 Add `models/face_recovery/variant.rs` with the `ATHENS` and `SANTORINI` rows,
      `FaceRecoveryVariant`, `ALL`, and `tests::every_variant()`; verify a test asserts both labels,
      codenames and precisions.
- [x] 3.2 Add `models/face_recovery/mod.rs` with `FaceRecovery` carrying a `Faces`, its
      `FaceRecoveryParams`, `display_name`, `artifact` and `required_artifacts` (its own graph only),
      hand-written `PartialEq`/`Hash` over the variant alone and a manual `impl Eq`; verify tests pin
      the four `fr_*` artifact names, exactly one required artifact with no `dt_` name among them, two
      different selections comparing equal and hashing alike, a lookup by a differing selection hitting
      a registry entry, and the display name carrying no faces.
- [x] 3.3 Implement `FaceRecovery::cache_tag` as `{prefix}-{codename}-{precision}-f0` when empty and
      `-f<signature>` otherwise, the signature being `Faces`' two-decimal bounding-box spelling from
      task 1.3; verify tests pin the tag for a known selection as a literal, and cover two selections
      producing different tags, a reordered selection producing a different tag, a repeated selection
      producing the same tag, two selections differing below the second decimal producing the same tag,
      the empty tag being unreachable from any non-empty selection, and no tag containing `_`.
- [x] 3.4 Add serde to `FaceRecovery` and verify tests cover a round trip preserving the faces and the
      cache tag, an empty selection round-tripping as empty, and a serialized INT8 pairing or an
      out-of-range confidence being refused.

## 4. Wiring and the cross-family invariants

- [x] 4.1 Wire `face`, `detection` and `face_recovery` into `models/mod.rs`, re-export the new public
      types, and extend `tests::every_operation()` to build detection and face-recovery operations over
      a spread of selections (none, one, several, reordered); verify the two existing cross-family tests
      still pass with eight families - no two operations sharing a cache tag, and no tag spelled the way
      an artifact name is.
- [x] 4.2 Append the detection and face recovery entries to `catalogue()` through the existing
      `simple_family` helper with empty parameter lists, leaving the six published entries in their
      current order; verify the catalogue tests assert eight families in that order, New York and both
      recovery variants with their labels and precisions, no parameters on either family, and that
      `every_listed_variant_is_one_that_can_actually_be_built` reaches both new families.
- [x] 4.3 Re-export the new public types from `crates/opai/src/lib.rs` and extend
      `crates/opai/tests/public_api.rs` to reach detection and face recovery the way a front end would -
      building a face from the crate root, constructing both operations, reading both from the
      catalogue - and verify that test passes.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy` and `cargo test` across the workspace and verify all
      three pass with no new warnings.
- [x] 5.2 Run `openspec validate add-models-3-faces --strict` and verify it reports no errors.
