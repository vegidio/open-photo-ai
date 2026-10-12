## 1. Establish the separator before writing anything against it

- [x] 1.1 Measure how ONNX Runtime 1.26 parses `optimization.disable_specified_optimizers`, through
      this project's own `ort` binding against the pinned runtime rather than a local Python install
      (design.md D1), using Osaka's FP16 transformer on the CPU provider as the instrument — it loads
      only when `SimplifiedLayerNormFusion` is actually disabled, and an unrecognised name is ignored
      in silence. Verify that `"ReshapeFusion;SimplifiedLayerNormFusion"` **loads** where the
      comma-joined pair fails, and check the reverse order and a pair whose second name is not a
      transformer, so that "the separator parsed" is distinguished from "one of the two happened to be
      read". **Record the table in the change before task 2 begins.** If the semicolon does not load,
      stop: tasks 2 and 3 do not happen, the requirement stands, and the work becomes task 5 alone —
      correcting the reasoning in the comments to say what was and was not established.

## 2. The field and the join

- [x] 2.1 Widen `EpProfile::disabled_optimizer` back to a list of names, restoring what the field was
      before `add-upscale-osaka-1-region` narrowed it, and carry the list through
      `providers/options.rs` and `sessions/build.rs` (design.md D2). Put the separator in a named
      constant beside the field rather than as a literal in a `.join()` call, with its doc comment
      stating that it is the runtime's contract, how task 1.1 established it, and that a wrong
      separator fails silently. Verify tests cover a model naming one, a model naming two, and a model
      naming none leaving the setting unwritten rather than written empty; and an options-map test
      asserting the joined value reaches the session exactly as task 1.1 measured it.

## 3. Osaka's declaration

- [x] 3.1 Restore both `ReshapeFusion` and `SimplifiedLayerNormFusion` to `UpscaleVariant::profile`'s
      Osaka arm, so the profile matches the reference field for field (design.md D3). Verify a
      variant-level test in `variant.rs` asserts every field of the Osaka profile at FP16 and at INT8
      against the reference's `profileFor` — both optimizer names, sequential execution, the memory
      planner off, CoreML on the CPU and GPU, `cuda_prefer_nhwc` set, and builder optimization level 3
      — and that the existing per-graph test still shows the transformer alone taking
      `cuda_prefer_nhwc` back off.

## 4. Withdrawing the requirement

- [x] 4.1 Apply the `execution-providers` delta: remove *A model disables at most one graph optimizer*,
      add *A model may disable several graph optimizers* in its place, and take the singular back out
      of *A model's measured settings include how the graph itself is run* and *Osaka's measured
      settings* (design.md D4). Verify `openspec validate fix-osaka-disabled-optimizers --strict`
      passes and that no other requirement in the capability still refers to one optimizer.

## 5. The comments that carried the wrong finding

- [x] 5.1 Rewrite the three doc comments asserting the runtime limitation as measured fact —
      `BROKEN_OPTIMIZER`'s in `models/upscale/variant.rs`, `EpProfile::disabled_optimizer`'s in
      `providers/profile.rs`, and the note in `sessions/build.rs` — to record what actually happened:
      the separator was hardcoded to a comma, the sweep could not vary it, and every result was
      consistent with that rather than with a one-name setting. Keep the failing node name and the
      version-specificity, which are correct and useful, and keep the warning that an unrecognised name
      is ignored in silence. Verify `cargo doc -p opai` emits no warnings and that nothing in
      `crates/opai/src` still claims the runtime accepts only one optimizer name.

## 6. Closing out

- [x] 6.1 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and
      `cargo test --locked`, and verify all three are green on the ubuntu, macos and windows CI jobs,
      with slice 1's `#[ignore]`d live region test still passing on the real FP16 weights under the
      restored declaration — which is the standing check that the pair loads. No frontend script is
      needed: this change touches `opai` alone and nothing it changes is user-visible.
