## 1. Establish the shape on one family

- [x] 1.1 Split `models/denoise.rs` into `models/denoise/mod.rs` (the `Denoise` operation,
      `DenoiseParams`, and the `Display`/`PartialEq`/`Hash` impls) and `models/denoise/variant.rs`
      (`DenoiseVariant`, the `STOCKHOLM`/`GOTHENBURG`/`MALMO` rows and `ALL`), carrying every doc
      comment across unchanged; add `pub(crate) use variant::ALL;` to `denoise/mod.rs`, move
      `every_variant()` into `variant.rs::tests`, and verify `cargo test` still reports all 19
      denoise tests passing with no type, signature or test name changed.

## 2. Apply it to the four remaining flat families

- [x] 2.1 Split `models/sharpen.rs` the same way into `sharpen/mod.rs` + `sharpen/variant.rs`, and
      verify all 19 sharpen tests still pass.
- [x] 2.2 Split `models/light_adjustment.rs` into `light_adjustment/mod.rs` +
      `light_adjustment/variant.rs`, and verify all 19 light adjustment tests still pass.
- [x] 2.3 Split `models/color_balance.rs` into `color_balance/mod.rs` + `color_balance/variant.rs`,
      and verify all 20 colour balance tests still pass.
- [x] 2.4 Split `models/colorization.rs` into `colorization/mod.rs` + `colorization/variant.rs` -
      the one family with no params type and derived `PartialEq`/`Hash` - and verify all 16
      colorization tests still pass.

## 3. Bring upscale onto the same shape

- [x] 3.1 Move `UpscaleVariant` and its `impl` block from `upscale/mod.rs` into
      `upscale/variant.rs`, leaving `mod.rs` holding only the `Upscale` operation, `UpscaleParams`
      and the identity impls; leave `upscale/resolve.rs`'s code untouched apart from the
      `every_variant` import 3.2 forces, and verify `cargo test` reports the same 34 upscale tests
      passing.
- [x] 3.2 Move `every_variant()` from `upscale/mod.rs::tests` into `upscale/variant.rs::tests` and
      relocate the variant-level tests (label, codename, precision, contract-row and
      `is_diffusion` coverage) to sit beside it, verifying the 24 + 10 split becomes a
      variant/operation split with the same total count and no test dropped.
- [x] 3.3 Add `pub(crate) use variant::{KYOTO, OSAKA, SAITAMA, TOKYO};` to `upscale/mod.rs` and
      verify `upscale` exposes its rows at one level like the other five.

## 4. Rewire the cross-family modules

- [x] 4.1 Update `models/catalogue.rs` to name every family at one level - `denoise::ALL` through
      `colorization::ALL` and `upscale::{KYOTO, OSAKA, SAITAMA, TOKYO}` - with no `::variant::` hop
      remaining, and verify its 11 tests pass unchanged.
- [x] 4.2 Update `models/mod.rs` module declarations and its two cross-family tests to reach
      `<family>::variant::tests::every_variant()` for all six families, and verify both tests pass
      and the `pub use` list is byte-for-byte what it was before.
- [x] 4.3 Verify the public surface did not move: `crates/opai/tests/public_api.rs` and
      `crates/opai/src/lib.rs` are untouched by this change and `cargo test --test public_api`
      passes.

## 5. Record the rule and close out

- [x] 5.1 Add a "Module layout" bullet to the `# Conventions` section of `openspec/config.yaml`'s
      `context` block stating the three rules from design.md - a family is a directory with
      `mod.rs` for the operation and `variant.rs` for what can be named, further modules only where
      a family's contract needs one; a file directly under `models/` is shared vocabulary or
      cross-family code; tests split on the same seam - and verify `openspec validate` passes.
- [x] 5.2 Run `cargo fmt`, `cargo clippy` and `cargo test` across the workspace and verify all three
      are clean with the full test count unchanged from before this change.
