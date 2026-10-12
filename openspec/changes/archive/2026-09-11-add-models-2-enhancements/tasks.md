## 1. Shared vocabulary

- [x] 1.1 Rename `ConvPrecision` to `FloatPrecision` in `models/precision.rs` and follow it through
      `upscale/mod.rs`, `catalogue.rs`, `models/mod.rs`, `lib.rs` and `tests/public_api.rs`; verify
      `cargo test -p opai` passes unchanged and that the serialized spellings are still `"fp32"` and
      `"fp16"`.
- [x] 1.2 Add `Strength` in `models/strength.rs` - bounded 0.0-3.0, quantized to three decimals over a
      `u16`, with `MIN`/`MAX`/`NAME`, a rejecting `new` returning `RangeError`, a clamping `clamped`,
      `get`, `Display` and serde; verify unit tests cover acceptance, refusal, clamping, quantization
      and the serde round trip including refusal of an out-of-range value.
- [x] 1.3 Add `Bias` in `models/bias.rs` - bounded -1.0-1.0, quantized to three decimals over an `i16`,
      with the same surface as `Strength`; verify unit tests cover the negative half of the range and
      that a bias of exactly 0 round-trips and renders without a sign.
- [x] 1.4 Add the shared `SimpleVariant { codename, label }` row type and the one helper that composes a
      single-graph `ArtifactId` from a family, a codename and a precision; verify a unit test pins the
      composed name for one variant of each of the five families.

## 2. The five families

- [x] 2.1 Add `models/denoise.rs` - `DenoiseVariant` (Stockholm, Gothenburg, Malmo, each carrying
      `FloatPrecision`), the `Denoise` operation carrying a `Strength`, hand-written `PartialEq`/`Hash`
      over the variant alone, `display_name`, `cache_tag`, `params`, `artifact` and
      `required_artifacts`; verify tests pin all six artifact ids, that two strengths compare equal and
      hash alike, and that their cache tags differ.
- [x] 2.2 Add `models/sharpen.rs` with the same shape for Moscow, Petersburg and Novgorod; verify tests
      pin all six artifact ids and the `Moscow (FP32)` display name.
- [x] 2.3 Add `models/light_adjustment.rs` - Paris and Lyon, carrying a `Bias`; verify tests pin all
      four artifact ids and that a negative bias produces a distinct cache tag from its positive
      counterpart.
- [x] 2.4 Add `models/color_balance.rs` - Rio and São Paulo, carrying a `Bias`; verify tests pin
      `cb_saopaulo_fp32`/`cb_saopaulo_fp16` and the `São Paulo (FP16)` display name, so the codename and
      the accented label are held apart.
- [x] 2.5 Add `models/colorization.rs` - Delhi, Mumbai and Jaipur with no parameter field and a derived
      `PartialEq`/`Hash`; verify tests pin all six artifact ids and that the operation's identity and
      cache tag are determined by variant and precision alone.

## 3. Catalogue and public surface

- [x] 3.1 Extend `catalogue()` with the five family entries, built from the variant rows and the
      `MIN`/`MAX`/`NAME` constants; verify the existing bounds test generalizes so that every published
      parameter range is one the matching constructor accepts at both ends and refuses just outside.
- [x] 3.2 Make colorization report an empty parameter list and verify a test asserts the family is
      listed with its three variants and no parameters, rather than being absent.
- [x] 3.3 Generalize `every_listed_variant_is_one_that_can_actually_be_built` to all six families;
      verify it constructs every catalogue-listed variant and precision pairing and that each resolves
      to at least one artifact.
- [x] 3.4 Re-export the new types from `models/mod.rs` and `lib.rs`, and extend `tests/public_api.rs` to
      reach each of the five operations, both parameter types and the six-family catalogue the way a
      front end would; verify `cargo test -p opai --test public_api` passes.

## 4. Verification

- [x] 4.1 Add a cross-family test asserting no two of the sixteen operations share a cache tag and that
      no cache tag matches any published artifact name, so the two namespaces cannot collide.
- [x] 4.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`
      and verify all three pass clean.
