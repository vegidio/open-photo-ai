## 1. The doctrine, before anything moves

- [x] 1.1 Replace the "Module layout" bullet under `Conventions` in `openspec/config.yaml` with the two
      sections given verbatim in design.md D5 — the three-tier rule with its two-seam statement, and the
      promotion rule with its known candidates. Verify with `openspec doctor` and by confirming the
      replaced bullet no longer appears anywhere in the file.

## 2. The trait

- [x] 2.1 Add `crates/opai/src/inference/pipeline.rs` declaring `Pipeline<B: Backend>` with `required`,
      `sessions`, `stages` and `run` exactly as design.md D1 gives them, and register it in
      `inference/mod.rs`. Carry across the reasoning currently on `Planned`'s two arms about why the two
      contracts are kept apart — one shared profile over a pass sequence against a profile per graph —
      since it is the trait that now rests on it. Verify it compiles and that `cargo doc` renders the
      module.
- [x] 2.2 Implement `Pipeline<B>` for the convolutional contract in `inference/upscale.rs`, wrapping the
      existing `run_passes` with no change to its body or signature. Verify the existing `upscale.rs`
      tests still pass untouched.
- [x] 2.3 Implement `Pipeline<B>` for the diffusion contract in `inference/osaka/mod.rs`, wrapping
      `restore_image` with no change to its body or signature, and move `Work::of`'s role-to-position
      mapping (`GraphRole::{Encoder, Transformer, Decoder}` → indices into the acquired sessions) inside
      it together with its "exchanging two of them compiles, runs, and returns a wrong image" reasoning
      and its `Unsupported` refusal for an undeclared role. Verify by a unit test that a graph set
      missing a role is refused rather than panicking.

## 3. The two seams, in `models/`

- [x] 3.1 Add the contract seam to `models/upscale/mod.rs`: one function matching `Resolution::Passes`
      and `Resolution::Graphs` onto the two impls from 2.2 and 2.3, naming no model. Verify by a test
      that each of the four variants reaches the expected arm.
- [x] 3.2 Add the family seam to `models/operation.rs`: `Operation::pipeline::<B>()`, an exhaustive match
      over the eight families where `Upscale` delegates to 3.1 and the other seven return the
      `Unsupported` refusal they return today, with the same message. Verify by a test asserting the
      refusal text for one non-upscale family is byte-identical to the current one.

## 4. Making `chain.rs` model-blind

- [x] 4.1 Rewrite `plan_one` to produce the boxed pipeline plus its operation, delete `plan_graphs`, and
      delete the `Planned` enum. Keep `plan()`'s whole-chain-before-any-install guarantee and its
      `chain refused` warn record. Verify the existing test that a second operation's refusal happens
      before the first operation's model is installed still passes against the fake backend.
- [x] 4.2 Delete the `Work` enum and replace the `match &work` in `run` with a single `Pipeline::run`
      call. Verify the per-step `debug` record still carries `stages` with the same values — passes for
      a convolutional operation, 3 for a diffusion one.
- [x] 4.3 Remove `use super::osaka::{self, region::RegionGraphs}` and `use super::upscale::run_passes`
      from `chain.rs`. Verify by grepping the file for `osaka`, `kyoto`, `tokyo`, `saitama`,
      `run_passes`, `GraphRole` and `RegionGraphs` and finding no hit outside test fixtures.
- [x] 4.4 Update `chain.rs`'s module header: rewrite the "what is deliberately not here" section, which
      currently describes Osaka's pipeline as refused by `plan`, and state the new seam in its place.
      Verify no sentence in the header describes a structure that no longer exists.

## 5. Proving nothing was given up

- [x] 5.1 Add a test that an Osaka plan's `sessions()` hands back each graph's own profile — the
      transformer at `cuda_prefer_nhwc: false` and both VAE halves at `true` — which is the project's one
      per-graph override and the case that would break silently if the artifact/profile pairing were
      split by the trait.
- [x] 5.2 Compare the test count in `inference/chain.rs` before and after (`cargo test -p opai
      inference::chain -- --list | wc -l` against the same on `HEAD`). Verify it has not decreased and
      that no assertion was dropped rather than repointed.
- [x] 5.3 Verify the whole fake-backend suite still runs with no ONNX Runtime present, on the platform
      in use, and that no new test requires a model file, a GPU or the network.

## 6. Closing out

- [x] 6.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`,
      and verify all three are clean. No frontend scripts: this slice touches no frontend file.
- [x] 6.2 Verify the public API is unchanged by diffing `cargo public-api` (or `cargo doc` output for
      `opai`) against `HEAD`, confirming no exported name moved, gained or lost.
