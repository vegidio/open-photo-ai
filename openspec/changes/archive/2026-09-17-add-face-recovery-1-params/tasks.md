## 1. The fidelity parameter and the face value

- [x] 1.1 Add `crates/opai/src/models/fidelity.rs` with `Fidelity` - `MIN` 0.0, `MAX` 1.0, `NAME` `"fidelity"`, `new`
      returning `RangeError` and `clamped`, quantized to three decimal places on acceptance - modelled on
      `strength.rs` and re-exported from `models/mod.rs`. Verify its tests cover a value in range, a refusal naming
      the range, a clamp of an out-of-range value and of a value that is not a number, and two values that quantize
      alike comparing equal and hashing alike.
- [x] 1.2 Quantize `Face::new`'s coordinates and `Confidence::new` to two decimal places - at the face rather than at
      the point, because `Point::new` is reached three times on the way out of the detector and only the last of
      those is in the image's own space. Hand-write `PartialEq`/`Eq`/`Hash` on `Point` over the canonical bit
      pattern, since a derived `Eq` would demand `f32: Eq`, and derive them on `Rect`, `Confidence`, `Face` and
      `Faces`. Rewrite the `face.rs` header paragraph that argues against
      quantization on acceptance so it records D5's reasoning instead of contradicting it. Verify tests cover a
      coordinate finer than two decimals being reported quantized, two faces differing below the second decimal
      comparing equal and hashing alike, and a confidence outside 0.0-1.0 still being refused.

## 2. Operation identity

- [x] 2.1 Delete the hand-written `PartialEq`/`Hash` impls on `Upscale`, `Denoise`, `Sharpen`, `Colorization`,
      `LightAdjustment`, `ColorBalance`, `Detection` and `FaceRecovery` and replace them with derives, leaving
      `cache_tag` untouched. Verify each family's identity tests now assert that two parameters are two operations,
      that two scales of Osaka are two operations, and that two selections of one face-recovery variant are two
      operations - and that the doc comments citing a registry keyed on an operation are rewritten to describe
      artifact-keyed session reuse.

## 3. Constructors and carriers

- [x] 3.1 Add the `Analysis` carrier beside `Operation` in `models/operation.rs`, move the sealed `DataOperation`
      seam onto it, and change `Opai::execute` to take `&Analysis`. Verify the `compile_fail` doctest still refuses
      an operation whose result is an image, that a detection run still hands back `Executed<Faces>` with nothing to
      unwrap, and that neither carrier can be passed to the other's entry point.
- [x] 3.2 Add `Upscale::tokyo`, `::kyoto`, `::saitama` (each `FloatPrecision`, `Scale`) and `::osaka`
      (`OsakaPrecision`, `Scale`), all returning `Operation`. Verify a test asserts each names the artifact it is
      published as, and that `compile_fail` doctests on `Upscale::osaka` pin both halves of its precision guarantee -
      that no `OsakaPrecision` spells FP32, and that a `FloatPrecision` is refused for its type rather than its
      value.
- [x] 3.3 Add the thirteen enhancement constructors - `Denoise::stockholm/gothenburg/malmo`,
      `Sharpen::moscow/stpetersburg/novgorod`, `Colorization::delhi/mumbai/jaipur`, `LightAdjustment::paris/lyon`,
      `ColorBalance::rio/saopaulo` - each taking `FloatPrecision` plus that family's parameter where it has one, and
      returning `Operation`. Verify the existing per-family artifact-name tests are driven through them.
- [x] 3.4 Add `FaceRecovery::athens(FloatPrecision, Faces, Fidelity)` and `FaceRecovery::santorini(FloatPrecision,
      Faces)` returning `Operation`, with a private `Option<Fidelity>` field and a `fidelity()` accessor, plus
      `Detection::newyork(FloatPrecision)` returning `Analysis`. Verify tests cover Athens reporting its fidelity,
      Santorini reporting `None`, and both still reporting exactly one required artifact and never a detection graph.

## 4. Parameters, cache tags and the pipeline seam

- [x] 4.1 Add `fidelity: Option<Fidelity>` to `FaceRecoveryParams` and a `-w<value>` segment to
      `FaceRecovery::cache_tag`, written for Athens and absent for Santorini. Verify tests pin the literal tags -
      `fr-athens-fp32-w0.500-f…`, `fr-athens-fp32-w1.000-f…` and `fr-santorini-fp32-f…` - assert two fidelities do
      not collide, assert a repeated fidelity reports one tag, and assert no Santorini tag carries a `w` segment.
- [x] 4.2 Make `params()` load-bearing: build `PassSequence` and `Diffusion` from `UpscaleParams` in
      `Upscale::pipeline` rather than from `self.scale()`. Verify the existing upscale pipeline tests pass unchanged
      and that upscale - the one family with a pipeline today - reaches its parameters only through `params()`. The
      other five publish one with no caller outside `#[cfg(test)]` until the slice that writes their pipeline
      arrives; face recovery's is slice 2.

## 5. The catalogue

- [x] 5.1 Add `ParameterKind` (`Range { min, max }` and `Faces`), move `parameters` from `FamilyEntry` to
      `VariantEntry`, and publish the fidelity range on Athens alone and `ParameterKind::Faces` on both face-recovery
      variants. Verify tests assert Athens publishes a fidelity bounded 0 to 1 while Santorini publishes none, that
      face recovery and colorization no longer publish identical entries, and that every published range matches the
      bounds its constructor enforces.
- [x] 5.2 Rework the catalogue round trip so `from_codename` resolves a variant and each variant names the
      constructor that builds it, removing the two-entry-point inverse. Verify a test walks every row the catalogue
      publishes, builds it with the parameters that row published, and asserts each resulting operation reports at
      least one required artifact - and that an unpublished pairing still names nothing.
- [x] 5.3 Migrate `crates/perf`, the one crate already built on the two entry points 5.2 removes. `select.rs` builds
      each swept row through the model's own constructor from the parameters that row published, and refuses
      detection with a `NotAnEnhancement` error now that it is carried in `Analysis` rather than in an enhancement
      chain; `cli.rs` gains a `--fidelity` flag defaulting to `Fidelity::MAX` and answers `None` for a parameter
      whose kind is not a range; `main.rs`, `view.rs` and `viewport.rs` carry a swept row as a `Subject`, which spans
      both carriers. Verify `cargo test -p perf` passes and that every enhancement row the catalogue publishes still
      builds from the flags this binary offers.

## 6. Verification

- [x] 6.1 Re-export `Fidelity`, `Analysis`, `ParameterKind` and `Subject` from `crates/opai/src/lib.rs` and rewrite
      `crates/opai/tests/public_api.rs` to construct every operation the new way, inverting the equality assertion at
      its line 199. Verify `cargo test -p opai --test public_api` passes and that the file reaches the new API only
      through the crate root, as a front end would.
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy` and `cargo test` across the workspace and verify all three pass
      with no warnings.
- [x] 6.3 Run `openspec validate add-face-recovery-1-params --strict` and verify it reports no errors.
