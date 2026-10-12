## 1. The contract, promoted

- [x] 1.1 Create `crates/opai/src/inference/filter.rs` and declare it in `inference/mod.rs`. Move `diverged`, `TILES`,
      the test-only `GUARDED` counter and `Denoiser` out of `models/denoise/process.rs`, carrying their documentation
      across rather than retyping it. Rename `Denoiser` to `Filter`. `pipeline` takes `norm: Normalisation` and
      `strength: Strength` in place of `RANGE` and `DenoiseParams` (design D1). Verify that `cargo build -p opai`
      succeeds and that nothing moved is `pub`.
- [x] 1.2 Move `Fake`, `FakeSession` and `session()` into `inference/test_support.rs`. Move the four `diverged` tests and
      the whole fake-backend suite into `filter.rs`'s test module, building each pipeline with `filter::pipeline`
      directly, not through `Denoise` (design D2). Add a test that the pipeline honours whichever `Normalisation` it
      is handed. Verify that `cargo test -p opai inference::filter` passes, and that the diff changes each moved test's
      construction line and **no assertion**. An assertion that needed editing means behaviour moved; stop there.
- [x] 1.3 Rewire denoise: move `RANGE` and its literal test into `models/denoise/mod.rs`, point `Denoise::pipeline` at
      `filter::pipeline` with `RANGE` and `params().strength`, and delete `models/denoise/process.rs` (design D1). Add
      the two family seam tests: Stockholm keeps an exploding tile's input, and Gothenburg and Malmö pass it through
      (design D2). Fix the `super::process` doc links in the three denoise model files. Verify that
      `cargo test -p opai models::denoise` passes.
- [x] 1.4 Update the prose describing the old tiers. In `inference/mod.rs`, drop the guard from "What is deliberately
      not here" and add `filter` to the module's list. Update `inference/mix.rs`'s header, which says sharpen "will"
      call it, and any mention in `models/mod.rs` of the contract living with denoise. Verify that
      `cargo doc -p opai --no-deps` builds with no broken intra-doc links.
- [x] 1.5 Make the denoise live check reset `GUARDED` before its runs if it does not already (design D2), then re-run
      it by hand: `cargo test -p opai models::denoise::live -- --ignored --nocapture --test-threads=1`. Verify that
      all twelve runs return the source's own dimensions, that strength 0 is identical to the source, and that
      Stockholm's guard count matches the archived note (0). Record the result in the task's completion note.
      **Done 2026-09-23** on an M2 Max, CPU provider and CoreML, FP32 and FP16: both tests pass (398.8s). All twelve
      runs returned 640x640, strength 0 was identical to the source every time, and Stockholm's guard kept 0 tiles in
      all four of its runs, matching the archived note. No reset was added: the check already counts each run's own
      tiles as a before/after delta of `GUARDED`, which D2's concern needs and a reset would only repeat.

## 2. The three models

- [x] 2.1 Add `models/sharpen/moscow/mod.rs`: `profile()` as a `match precision` giving CoreML `CpuAndGpu` at FP16 and
      the default at FP32 (design D5). Carry the reference's Restormer prose, the 5.067s/2.803s end-to-end figures,
      the +59% Neural Engine confirmation, the −45%/−22% export caveat and the partition-count precondition, and state
      that the figures are ported, not re-measured. Verify with Gothenburg's four tests ported to Moscow.
- [x] 2.2 Add `models/sharpen/novgorod/mod.rs` with the same profile shape and its own prose: 180ms against 143ms per
      tile at FP16, and why that decides whether FP16 is faster than FP32's 152ms. Verify with the same four tests.
- [x] 2.3 Add `models/sharpen/petersburg/mod.rs` with `GUARD = 3.0` and a `profile()` returning the default at both
      precisions, as a measured outcome. Carry the reference's sweep table, the one-partition-on-the-GPU explanation,
      the note that FP16 buys only download size, and the statement that no photograph triggering the guard has been
      observed (design D7). Verify with tests pinning the threshold and pinning the whole profile as the default at
      both precisions, with no specialization and no sequential mode.
- [x] 2.4 Wire `guard()` and `profile()` in `models/sharpen/variant.rs` as exhaustive matches reaching into the model
      directories (design D3). Verify with the family's `every_variant()` traversal: only Petersburg reports a guard,
      the Moscow and Novgorod FP16 arms declare their profile, their FP32 arms are `EpProfile::default()`, and
      Petersburg's arms equal `petersburg::profile()`.

## 3. The family seam and the refusal

- [x] 3.1 Add `RANGE` and its literal test and `Sharpen::pipeline` in `models/sharpen/mod.rs`, built from
      `display_name()`, `artifact()`, `profile()`, `RANGE`, `params().strength` and `variant().guard()`. Add the
      family tests denoise has: a pipeline for each of the six variant-precision pairs names its one artifact and
      session pair, one session serves every strength, Petersburg keeps an exploding tile's input, and Moscow and
      Novgorod pass it through. Verify that `cargo test -p opai models::sharpen` passes.
- [x] 3.2 Remove sharpen from `Operation::pipeline`'s refusal arm and update its doc comment. Move every test and comment
      that uses sharpen as the example of a refused family to colorization (design D4): in `models/operation.rs`,
      `served()`, the deferred-set test and the plan-refusal example; in `inference/process.rs`, `unservable()` and
      the served list; and `autopilot.rs`'s doc comment. Verify that every sharpen variant now plans and that
      colorization still returns `FamilyNotImplemented`.
- [x] 3.3 Update `Opai::process`'s "What is refused" in `crates/opai/src/lib.rs` to name six running families, with
      sharpen described as all three models over a tile grid at the photograph's own resolution, then the strength.
      Name colorization as the one still refused. Verify with `cargo doc -p opai --no-deps`, and by reading the prose
      against the refusal arm as it now stands.

## 4. Live check and toolchain

- [x] 4.1 Add `models/sharpen/live.rs`, an `#[ignore]`d live check in the shape of denoise's. It counts `GUARDED`
      as a delta, runs `fixtures/test.dat` through Moscow, Petersburg and Novgorod via `Opai::process` at both precisions on the CPU
      provider and on CoreML, and prints how many tiles Petersburg's guard kept (design D7). Verify by hand with
      `cargo test -p opai models::sharpen::live -- --ignored --nocapture --test-threads=1`: every run returns the
      source's own dimensions, strength 0 returns the source unchanged, no two variants' results are identical, and
      the guard count is recorded in the task's completion note.
      **Done 2026-09-23** on an M2 Max, CPU provider and CoreML, FP32 and FP16: both tests pass (471.5s). All twelve
      runs returned 640x640, strength 0 was identical to the source every time, no two variants' results were
      identical at either precision, and Petersburg's guard kept 0 tiles on either provider.
- [x] 4.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and confirm that
      `crates/opai/tests/public_api.rs` needed no change (design D1). Verify that all three exit zero.
