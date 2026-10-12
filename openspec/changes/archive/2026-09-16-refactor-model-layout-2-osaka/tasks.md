## 1. The one promotion

- [x] 1.1 Move `inference/osaka/canvas.rs` to `inference/canvas.rs` with `git mv`, declare it in
      `inference/mod.rs`, drop it from `inference/osaka/mod.rs`, and repoint its two `use` statements from
      `crate::inference::{blend,grid}` to `super::{blend,grid}`. Its test module does not survive the move
      intact — see design.md D1: the two tests written at Osaka's real geometry import
      `latent::{REGION_EDGE, REGION_OVERLAP}`, so they move to `osaka/mod.rs` and the placement helpers they
      shared with the tests that stay (`flat`, `regions`) are lifted into a `pub(crate) mod fixtures` here,
      with `axis_weights` widened to `pub(crate)`. Verify `cargo build -p opai` is clean, that a
      rename-detecting diff (`git diff -M`) reports the new path as a rename of the old rather than as an add,
      that both relocated tests exist under `osaka/` and still assert the same values, and that
      `canvas::Shared` and `pipeline::Shared` coexist without either being renamed. The `git log --follow`
      check on the new path can only run once the move is committed — before that there is no commit at the
      new path for it to walk. Confirmed after the fact: `git log --follow` on
      `crates/opai/src/inference/canvas.rs` reaches `8a454fb`, the commit that created the file, through the
      rename `inference/{osaka => }/canvas.rs` recorded in `47bf763`.

## 2. The model's directory

- [x] 2.1 `git mv` `inference/osaka/{mod,latent,region,noise,colorfix,live}.rs` to
      `models/upscale/osaka/`, declare `pub(crate) mod osaka;` in `models/upscale/mod.rs`, delete
      `pub(crate) mod osaka;` from `inference/mod.rs`, and repair only the paths: `super::` links inside the
      six files that now resolve elsewhere, and `use canvas::{Canvas, Shared}` becoming
      `crate::inference::canvas::…`. No body is edited. Verify `cargo build -p opai --all-targets` is clean
      and that `git diff -M --stat` reports the six as renames, not as adds.
- [x] 2.2 Repoint `models/upscale/mod.rs`'s contract seam — `use crate::inference::osaka::Diffusion`
      becomes `use osaka::Diffusion` — and repoint the remaining cross-module references to the old path in
      `providers/profile.rs` and `models/upscale/variant.rs`'s `inference::osaka::live` citations. Leave the
      `Resolution::Graphs` arm and its comment otherwise untouched. Verify by grepping the whole crate for
      `inference::osaka` and finding nothing, and that the slice-1 test that each of the four variants
      reaches its expected arm still passes unedited.

## 3. The graph vocabulary

- [x] 3.1 Move `GraphRole`, `Graph`, `DiffusionVariant` and `UnknownRole` out of
      `models/upscale/variant.rs` and `GraphSet` and `ResolvedGraph` out of `models/upscale/resolve.rs` into
      a new `models/upscale/osaka/graph.rs`, carrying every doc comment verbatim — including the
      reorder-exchanges-the-encoder-and-the-decoder reasoning on `GraphSet` and the `pinned` /
      `cuda_prefer_nhwc` rationale on `Graph`. Verify the four test modules involved still hold the same
      assertions and that `UnknownRole`'s `#[error]` string is byte-identical.
- [x] 3.2 Move the diffusion branch of `Upscale::resolve()` and the body of `Upscale::graph()` into
      `graph.rs` as the two functions design.md D2 names, leaving `resolve.rs` holding `Pass`, `Resolution`
      and the convolutional branch and calling into `osaka::graph` for the other. Verify `resolve.rs` no
      longer composes an Osaka artifact name, by grepping **its non-test body** for `suffix`, `pinned` and
      `_vae_` and finding nothing — its test module keeps asserting the composed names, which is the point
      of the tests and not a leak — and that every existing `resolve.rs` Osaka test passes from wherever it
      now lives.
- [x] 3.3 Repoint `models/mod.rs`'s re-exports so `GraphRole`, `UnknownRole` and `GraphSet` are published
      from their new module at the paths they have today, and confirm `ResolvedGraph` keeps its
      `pub(crate)` re-export. Verify `crates/opai/tests/public_api.rs` compiles and passes **with no edit
      to it**, since it imports all three from `opai::` by name.

## 4. The model's own declarations

- [x] 4.1 Move `OsakaPrecision` from `models/precision.rs` to `models/upscale/osaka/precision.rs` with its
      `ALL`, `narrow` and `From<OsakaPrecision> for Precision` impls and its tests, repoint `models/mod.rs`,
      `catalogue.rs`, `variant.rs` and the four test modules that name it, and verify `models/precision.rs`
      is left holding only the two cross-family types.
- [x] 4.2 Move the `OSAKA` row, `BROKEN_OPTIMIZERS` and `TRT_BUILDER_OPTIMIZATION_LEVEL` into
      `models/upscale/osaka/mod.rs`, and move `UpscaleVariant::profile()`'s `Self::Osaka(_)` arm into an
      `osaka::profile(OsakaPrecision) -> EpProfile` it delegates to. Verify by diffing the moved comment
      text against `HEAD` that the miscompilation transcript, the separator-mistake account and the
      `+1.3%` / `-8.0%` / `-9.4%` NHWC figures are present unchanged, character for character.
- [x] 4.3 Repoint the six exhaustive matches over `UpscaleVariant` in `variant.rs` — `precision`,
      `profile`, `label`, `codename`, `conv_row`, `diffusion_row` — so each `Self::Osaka(_)` arm is a
      one-line delegation into `osaka/`, and repoint `catalogue.rs`'s `OSAKA` import. Verify all six are
      still exhaustive matches with no wildcard arm, and that
      `osaka_declares_every_measured_setting_at_both_precisions` passes asserting the same literal values.

## 5. The prose the tiers rest on

- [x] 5.1 Rewrite `models/mod.rs`'s module header: it opens "Nothing here runs anything", which stops
      being true with a pipeline underneath it. State the three tiers as they now stand and what a model's
      directory contains. Verify no sentence in it describes a structure that no longer exists.
- [x] 5.2 Rewrite `inference/mod.rs`'s header and its "what is deliberately not here" section, which
      currently places the diffusion driver in `[osaka]` beneath it and points at `osaka::canvas` as the
      second blending strategy. Both are now `canvas` here and the driver under `models/`. Repoint the same
      links in `blend.rs` and `tensor.rs`, and update `osaka/live.rs`'s printed command to
      `cargo test -p opai models::upscale::osaka::live -- --ignored`. Verify by **running** that command
      and by `cargo doc -p opai --no-deps` reporting no new rustdoc warning against `HEAD`.
- [x] 5.3 Add `models/upscale/osaka/{graph,latent,region}.rs` to the KNOWN CANDIDATES block in
      `openspec/config.yaml`, each with the one-line reason design.md D4 gives, leaving the doctrine's two
      sections otherwise untouched. Verify with `openspec doctor` and by confirming `colorfix.rs` and
      `noise.rs` are still listed.

## 6. Proving nothing was given up

- [x] 6.1 Compare the test count before and after with `cargo test -p opai -- --list | wc -l` against the
      same on `HEAD`, including the `#[ignore]`d live tests. Verify it has not decreased and that every
      Osaka assertion was repointed rather than dropped when its `mod tests` block was merged.
- [x] 6.2 Verify the public API is unchanged by diffing `cargo public-api` (or `cargo doc`'s index for
      `opai`) against `HEAD`, confirming no exported name moved, gained or lost — with particular attention
      to `GraphRole`, `GraphSet`, `UnknownRole` and `OsakaPrecision`.
- [x] 6.3 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`,
      and verify all three are clean, then verify the doctrine itself holds: `grep -rn "osaka\|Osaka"
      crates/opai/src/inference/` returns **no `use` and no non-test code** naming the model. Three kinds of
      hit are expected and correct — the intra-doc links into `crate::models::upscale::osaka` that task 5.2
      requires in `mod.rs` and `tensor.rs`, the prose in `pipeline.rs` naming an Osaka run as an example, and
      `chain.rs`'s test fixtures, which this slice does not touch and which slice 1 left as the driver's proof
      that a real model plans through the model-blind seam. Also verify `models/upscale/osaka/` contains no
      `use` reaching into `tokyo`, `kyoto` or `saitama` — prose naming them for comparison is fine. No
      frontend scripts: this slice touches no frontend file.
