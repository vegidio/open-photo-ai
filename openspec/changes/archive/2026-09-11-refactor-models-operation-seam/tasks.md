## 1. Ship the catalogue's inverse

- [x] 1.1 Add `pub fn from_codename(codename: &str, precision: Precision) -> Option<Self>` to
      `DenoiseVariant`, `SharpenVariant`, `LightAdjustmentVariant` and `ColorBalanceVariant`, each a
      match over that family's `ALL` rows beside the existing `row()` match, and verify a round trip
      from every row's codename back to the variant for all four families.
- [x] 1.2 Add the same to `ColorizationVariant`, `DetectionVariant`, `FaceRecoveryVariant` and
      `UpscaleVariant` - the last taking `OsakaPrecision` into account so that Osaka at FP32 returns
      `None` rather than an unpublishable variant - and verify every published pairing round-trips
      and every unpublished one returns `None`.

## 2. Add the seam

- [x] 2.1 Add `models/operation.rs` declaring `pub enum Operation` with one arm per family holding
      that family's type, deriving `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize` with
      `#[serde(tag = "family")]`, and re-export it from `models/mod.rs` and `lib.rs`.
- [x] 2.2 Forward `family()`, `precision()`, `display_name()`, `required_artifacts()` and
      `cache_tag()` from `Operation` to the arm, each an exhaustive match with no catch-all arm, and
      verify each forwards to the same value the typed operation reports.
- [x] 2.3 Add `Operation::from_catalogue(family, codename, precision)` returning `Option<Operation>`
      for the parameterless families and a parameter-taking counterpart for the five that need one,
      built on the `from_codename` constructors from section 1, and verify every catalogue row builds.
- [x] 2.4 Document on both `Operation` and the eight family types which one a caller should reach
      for - the typed form to build, `Operation` to store, send or dispatch on - so the two ways to
      name an operation are not left for a reader to guess between.

## 3. Move the cross-family invariants onto the seam

- [x] 3.1 Rewrite `catalogue.rs`'s `#[cfg(test)] build()` to call the public inverse from 2.3,
      deleting the 80-line `match (family, codename)` and verifying the catalogue tests pass with the
      third hand-maintained listing of all twenty variants gone.
- [x] 3.2 Replace the `checked == 40` and `checked == 5` counters with traversals over `Family::ALL`
      and the catalogue rows, and verify the tests still fail when a row is removed from the
      catalogue - a counter that is merely bumped would not.
- [x] 3.3 Rewrite `models/mod.rs`'s `every_operation()` to build through `Operation`, and verify the
      two cross-family tests - no shared cache tag, no tag spelled as an artifact name - still pass
      over the same set of operations.

## 4. Pin the new surface

- [x] 4.1 Add a serde round-trip test pinning `Operation`'s tag and field names for one family of
      each shape - parameterless, scalar-parameter, and face-carrying - since a persisted `Operation`
      makes that representation a compatibility surface.
- [x] 4.2 Extend `crates/opai/tests/public_api.rs` with `Operation` and the `from_codename`
      constructors reached from the crate root, and verify the existing 8 tests still pass unedited.

## 5. Close out

- [x] 5.1 Update `openspec/specs/models/spec.md` via this change's spec delta and verify
      `openspec validate` passes.
- [x] 5.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and
      verify all three are clean with no test lost from the previous count.
