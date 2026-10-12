## 1. The promotion, before the second caller is written

- [x] 1.1 `git mv` `models/face_recovery/athens/{transform,warp,mask,composite}.rs` to
      `models/face_recovery/`, move their `mod` declarations from `athens/mod.rs` to `face_recovery/mod.rs`, and fix
      the `use super::…` paths and the one header sentence per file that names the model they belonged to
      (design D1). Verify `cargo test -p opai` passes with **every test in those four files unedited**, and verify
      the move is a move: `git diff -M --stat` reports renames, and `git diff -M` shows no change outside a `mod`
      line, a `use` line or a header comment.
- [x] 1.2 Move the pipeline out of `athens/mod.rs` into a new `models/face_recovery/restore.rs` — the tile size, the
      graph's normalisation, the steps-per-face constant, the `Model`/`ImagePipeline` impls, the per-face loop, the
      depth dispatch, the full-frame carry and the pipeline's tests — leaving `athens/mod.rs` holding **only**
      `profile` and its prose (design D2). Keep the weight a plain `f32` for now so this task is a pure move; 2.1
      widens it. Verify `cargo test -p opai` passes with the moved tests unedited, and that `athens/mod.rs` names no
      geometry and no session call.
- [x] 1.3 Remove the four face-recovery entries from `models/mod.rs`'s list of files parked at model tier, leaving
      Osaka's five, and update the sentence naming the change that promotes them so it records the move as done
      rather than pending. Verify the list names no file that is no longer at model tier, and `cargo doc -p opai
      --no-deps` is clean.

## 2. One contract, two calls

- [x] 2.1 Widen `restore.rs` to hold the bound weight as `Option<f32>` and match it once, inside the per-face loop:
      `Backend::run_weighted` with a value and `Backend::run_graph` without (design D5). No trait method is added
      and no test double outside this file changes. Verify `cargo build -p opai --all-targets` is clean and that
      `inference/session.rs` and `inference/process.rs` are unedited.
- [x] 2.2 Resolve the weight on the variant row: `FaceRecoveryVariant` answers `Some(carried.unwrap_or(
      Fidelity::MAX))` for Athens and `None` for Santorini, with the doc comment recording why it cannot be read off
      `FaceRecoveryParams.fidelity` alone — a deserialized Athens operation carries `None` for a different reason
      than Santorini does (design D4). Verify with unit tests that Athens resolves an explicit fidelity unchanged,
      resolves a missing one to the maximum, and that Santorini resolves to nothing at either precision.
- [x] 2.3 Reduce `FaceRecovery::pipeline` to one arm over both variants, built from `params()` and the weight from
      2.2, returning `Shared<B>` rather than a `Result`, and wrap it in `Ok` at `Operation::pipeline`'s
      face-recovery arm (design D6). Delete the Santorini refusal and the doc comment describing it. Verify with
      tests that both variants resolve to a pipeline, that the five deferred families still answer
      `UnsupportedReason::FamilyNotImplemented` unchanged, and that no operation the library can name is refused for
      a reason it should not be.

## 3. Santorini's measured provider tuning

- [x] 3.1 Add `models/face_recovery/santorini/mod.rs` with `profile(precision)` —
      `CoreMlSpecialization::FastPrediction` and `ExecutionMode::Sequential` over the defaults — and point
      `FaceRecoveryVariant::profile`'s Santorini arm at it in place of `EpProfile::default()`. Carry
      `santorini.go`'s prose: the 1.04x FP16 specialization figure and the 20-30ms FP32 cold-start price it pays,
      the 1.09x/1.04x CUDA execution-mode figures with the -9.8% end-to-end contrast and why both are recorded, the
      ~9% cost of copying Athens' compute-unit restriction **and the export rather than the architecture as the
      reason**, and the two CUDA options that fail rather than slow this graph (design D7).
- [x] 3.2 Pin the profile with tests mirroring `santorini_test.go`: both precisions declare the fast-prediction
      specialization and sequential execution; **neither restricts CoreML's compute units**, with the assertion's
      message saying that Santorini's FP16 graph is faster on the Neural Engine; and neither declares the CUDA NHWC
      layout. Verify Athens' own profile tests still pass untouched.

## 4. The pipeline end to end

- [x] 4.1 Extend `restore.rs`'s fake backend — **no ONNX Runtime** — to serve both calls and record which one it
      received (design D8), then test Santorini through it: a run over two faces performs two unweighted graph runs
      and composites both; **no weight of any value reaches the graph**; an empty selection runs no graph, reports
      1.0 and returns the source at the requested depth; a zero-area image is refused; progress never decreases,
      reaches 1.0 once and reports two steps per face; a cancellation at each of the three points produces no image;
      and a run at `ChannelDepth::Sixteen` returns a 16-bit image.
- [x] 4.2 Pin that the widening did not move Athens: an Athens operation reaches the **weighted** call with the
      fidelity it carries; one carrying none reaches the weighted call with `Fidelity::MAX` rather than the
      unweighted one; and a run of each variant over the same photograph, the same selection and the same fake
      restoration composites into the same pixels — which is the family-wide framing and feathering requirement.
- [x] 4.3 Update `crates/opai/tests/public_api.rs` so Santorini joins the operations a consumer queues and asks for
      the artifacts of, rather than one it would have been refused. **Servedness itself is not observable from
      outside the crate** — `Operation::pipeline` is `pub(crate)`, so 2.3's traversal over every operation the
      library can name is where that is pinned, and this file cannot repeat it. What it verifies instead is what a
      consumer can actually read: that Santorini names its one graph and no detector alongside Athens in a queue,
      that its cache tag carries no fidelity segment and is stable across two builds of one selection — which is the
      requirement's "one selection twice is one result" half — that the two variants are two identities with two
      tags, that the catalogue still publishes a fidelity parameter for Athens alone, and that the deferral a front
      end greys an enhancement out on still spells and matches the way it did.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` and verify
      all three are clean, then `cargo doc -p opai --no-deps` and verify no new rustdoc warning against `HEAD`.
      Verify the doctrine holds: no `use` in `models/face_recovery/athens/` or `models/face_recovery/santorini/`
      reaches into the other's directory, `inference/` gained no file and lost none, and both model directories hold
      nothing but a profile and its prose. Verify `crates/perf` is unedited and that a sweep selecting `santorini`
      resolves, receives the detection pass's faces and is no longer refused. No frontend scripts — this change
      touches no frontend file.
