## 1. The seam in `opai`

- [x] 1.1 Add the refusal type naming the two ways a request can be wrong - a codename the family does
      not publish, and a precision the variant is not published at - with `thiserror`, matching the
      crate's other error types.
- [x] 1.2 Add the lookup from `(family, codename)` to the `VariantEntry` that publishes it, against
      `catalogue()` and nothing else (D1), and verify tests cover a published codename, one no family
      publishes, and one published by a different family than the one asked for.
- [x] 1.3 Add the construction from a variant entry plus a precision and the parameter values to the
      `Operation` it names (D1, D2), refusing a precision the entry does not list, and verify tests
      cover each upscale variant at each precision it publishes.
- [x] 1.4 Verify a test covers Osaka refusing FP32 through the entry's own precision list rather than
      through a special case (D3), and a test covers the three convolutional upscalers refusing INT8
      the same way.
- [x] 1.5 Verify a test walks the whole catalogue and builds every variant at every precision it
      publishes, so a model that is published but not constructible is a test failure rather than a
      refusal a user discovers.

## 2. The GUI stops naming models

- [x] 2.1 Replace `Requested::resolve`'s codename match with the library call, keeping the scale
      clamping and its warning where they are (D2), and rewrite the comment that calls this "the one
      place that names a model" to record that no place in `gui` does any more.
- [x] 2.2 Delete `published()`, `float()` and `osaka()`, and map the library's refusal onto the
      existing `UnknownOperation` variants so what crosses the wire to the window is unchanged.
- [x] 2.3 Verify `operation.rs`'s tests still cover an unknown codename and an unpublished precision
      being refused, now through the library, and that the wire shape of each refusal is unchanged.
- [x] 2.4 Confirm no `codename` string literal naming a model remains in `crates/gui` outside `#[cfg(test)]`
      code, and verify a test asserts a model that is published can be resolved - the case that is a
      refusal today. Tests keep their literals: naming Osaka at FP32 is how the refusal is asserted,
      and the test that covers "a model the library publishes" is driven off the catalogue instead.

## 3. Verification

- [x] 3.1 Check `crates/opai/tests/public_api.rs` covers the new public surface, since this is an
      addition to the library's API.
- [x] 3.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`.
- [x] 3.3 Run `pnpm typecheck`, `pnpm test` and Biome over any touched frontend files, and confirm the
      enhancement runs end to end from the sidebar for each upscale model the catalogue publishes.
      `pnpm typecheck` and `pnpm test` (672 tests) pass; no frontend file was touched, so Biome had
      nothing to run over. The sidebar was driven by hand for Tokyo HD and Kyoto SD, both of which
      resolved, installed and ran end to end; the remaining variants were not exercised by hand at
      the user's direction.

No i18n keys: the refusals already have theirs, and their wire shape does not change.
