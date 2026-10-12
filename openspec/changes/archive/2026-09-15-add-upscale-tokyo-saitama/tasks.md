## 1. Verify the published exports before writing anything

- [x] 1.1 Inspect the installed `up_saitama_4x_fp16.onnx` (pinned `e1fc6151…`, 8,984,791 bytes) for
      FP32 islands — design.md D5. Verify by locating the two upsample `Resize` nodes and confirming
      there are **no** `Cast` nodes wrapping them: their presence means the pre-fix export shipped,
      in which case `CpuAndNeuralEngine` is the worst of three choices rather than the best. **Stop
      and raise the design's Open Question if they are there**; do not write task 2.2 around it.
- [x] 1.2 Confirm `up_tokyo_4x_fp32.onnx` is the graph the profile describes — design.md D5. Verify
      the node count is 2682 with 36 `Conv` nodes against 452 `Reshape`, 344 `Transpose` and 110
      normalization nodes, so the profile is not attached to a different export of the same name.
      **Outcome: the export matches on every count but one — 2682 nodes, 36 `Conv`, 344 `Transpose`,
      110 `LayerNormalization`, and `Reshape` is 776 rather than the 452 this task was written
      against.** The 452 was carried over from the reference's own note and is wrong; it is the one
      figure here nothing else depended on, and the shape of the graph the profile is attached to is
      confirmed either way. The measured 776 is what the profile comment records, and design.md and
      proposal.md have been corrected to match.

## 2. The measured profiles

- [x] 2.1 Add Tokyo's arm to `UpscaleVariant::profile` — `CoreMlComputeUnits::CpuAndGpu` and
      `ExecutionMode::Sequential` at **both** precisions (design.md D3). Document in the arm's comment
      why the compute-unit setting is an exclusion rather than a preference (the ANE compiler cannot
      compile a SwinIR: `ANECCompile() FAILED` per block, 1067s build, 22x slow), why it is not
      precision-split, and the settings measured and rejected from D7 — `prefer_nhwc`,
      `fuse_conv_bias=1`, `trt_fp16_enable` and the TensorRT single-engine fusion. Verify with a
      variant-level test in `variant.rs` asserting both fields at FP16 and at FP32.
- [x] 2.2 Add Saitama's FP16 arm — `CoreMlComputeUnits::CpuAndNeuralEngine` — leaving the FP32 arm on
      `EpProfile::default()` (design.md D3). Document that this is Kyoto's profile reached
      independently from the same RRDBNet op mix, that the FP32 arm is load-bearing because CoreML's
      typed execution bars an FP32 MLProgram from the Neural Engine outright, and the export
      dependency from D5. Verify with a variant-level test asserting the FP16 arm carries the setting
      **and the FP32 arm carries the default**.
- [x] 2.3 Drop the `#[expect(dead_code)]` attributes from `CoreMlComputeUnits::CpuAndGpu` and
      `ExecutionMode::Sequential` in `providers/profile.rs`, now that a model declares each. Verify
      `cargo clippy` is clean — an `expect` that is no longer needed fails the build, which is the
      check. Leave the other two markers alone — `CoreMlComputeUnits::CpuOnly` and
      `CoreMlSpecialization::FastPrediction`, which is four in the file rather than the five this task
      was written against.

## 3. Lifting the gate

- [x] 3.1 Remove the unmeasured-profile refusal from `inference::chain::plan_one`, leaving Osaka's
      `Resolution::Graphs` branch and the seven non-upscale families refused with their own unchanged
      reasons (design.md D1). Verify the `UpscaleVariant` match still has no catch-all arm, so a
      variant added later cannot compile without an answer.
- [x] 3.2 Invert `the_two_unmeasured_convolutional_variants_are_gated_by_name` into a test that all
      four Tokyo/Saitama precision pairs **plan successfully and carry the profile their arm
      declares** (design.md D6), rather than deleting it — the guarantee belongs at the seam the gate
      occupied. Verify the existing chain tests for Osaka and for the seven families still pass
      unchanged, including the one proving a chain whose second operation is unsupported fails before
      the first operation's model is installed.

## 4. Documentation

- [x] 4.1 Rewrite the three places that describe the gate so they describe what actually runs:
      `lib.rs`'s crate-level "What runs, and what is refused" section, `Opai::process`'s "What is
      refused" section, and `plan_one`'s inline comment. Verify by grepping `crates/opai/src` for
      "Tokyo" and "Saitama" and confirming every remaining hit is either a profile comment, a test,
      or a variant row — no surviving text claims either one is refused or unmeasured.

## 5. Confirming on the hardware this change has

- [x] 5.1 Run `cargo run --release -p perf -- tokyo --provider coreml` at both precisions. Verify the
      **absence** of `ANECompiler`/`ANECCompile` records in `opai.log` (`ort`'s `tracing` feature
      forwards the runtime's C++ diagnostics), and that the cold start is seconds rather than the
      ~1067s an unrestricted compute-unit setting produces. This is the load-bearing confirmation of
      design.md D1.
- [x] 5.2 Run `cargo run --release -p perf -- saitama --provider coreml` at both precisions. Verify
      FP16 is faster than the same model with the profile's compute units left at the default, and
      that FP32 is unaffected — confirming both halves of D3's asymmetry rather than only the setting
      that is present.
- [x] 5.3 Measure the Tokyo-vs-Kyoto sequential **sign flip** on the CPU provider —
      `perftest tokyo kyoto --provider cpu` with and without the execution mode — design.md D4.
      Verify Tokyo gains where Kyoto loses; that is the local test of the graph-shape claim standing
      in for the CUDA figures this change cannot reproduce. Record the outcome in the profile comment
      whichever way it lands, including if it does not reproduce.

## 6. Closing out

- [x] 6.1 Run `cargo fmt --check`, `cargo clippy` and `cargo test`, and verify the full suite passes.
      No frontend script is needed — this change touches `opai` alone.

## 7. Raised by verification, before archive

- [x] 7.1 Cover the spec delta's `A sequential model's output` scenario, which landed with nothing behind it: the
      bit-identical claim was inherited from the reference and no test, task or measurement here checked it. Add it
      as a live check in `inference/live.rs` — one Tokyo tile through the session directly, once in each execution
      mode with a `release_sessions` between them so the artifact-keyed cache does not serve the first session twice
      — and verify by running it on **both** CoreML and the CPU provider, the latter being where ONNX Runtime has an
      inter-op pool to schedule against at all. **Outcome: all 3,145,728 floats bit-identical on both providers.**
      Record in the test's doc comment that its timings include a session build and are not a mode benchmark, and
      that the CPU pair's sign does not match the profile's `perftest` margin for that reason.
- [x] 7.2 Extend the option-map assertions to the two new arms, which were covered only compositionally — the
      variant's profile in one test and the enum's rendering in another, with nothing joining them at the seam the
      spec's wording describes. Add Tokyo at both precisions and Saitama at both to `providers/options.rs` beside
      `kyoto_at_fp16_reaches_the_cpu_and_the_neural_engine_and_at_fp32_takes_the_default`, plus one driving Tokyo's
      own profile through `resolve` on four providers to check the session-level setting survives the resolution.
- [x] 7.3 Correct the stale statements verification turned up: `providers/options.rs`'s "the one measured
      declaration in the library" (three declare now, and task 4.1's grep was scoped to Tokyo and Saitama so it
      could not catch a comment naming only Kyoto), the profile comment's "two of them wrapped around the upsamples"
      (four `Cast` nodes wrap the two `Resize` nodes, one in and one out of each), and this file's own counts in
      1.2 and 2.3.

- [x] 7.4 Correct `crates/perf/README.md`'s unnamed-sweep note, which said "within it only Kyoto" and is the last
      place in the repository still describing the gate. Task 4.1's grep could not reach it twice over: it was scoped
      to `crates/opai/src`, and the sentence names Kyoto rather than either gated variant — the same blind spot 7.3
      found in `providers/options.rs`. Verify by re-running that grep for "only Kyoto" across the repository and
      finding nothing. proposal.md's "crates/perf is used but not modified" is corrected alongside it, since the
      README is now the one file outside `opai` this change touches.
