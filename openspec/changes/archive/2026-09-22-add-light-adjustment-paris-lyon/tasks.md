## 1. The one missing primitive

- [x] 1.1 Add a cropped decode to `inference/tensor.rs`: `chw_to_image` and the new sibling over one private body, the
      sibling reading a sub-extent at the tensor's own stride (design D3). Verify with unit tests that a sub-extent
      decode equals a full decode of the same region, that a sub-extent larger than the tensor returns `TensorShape`
      having written nothing, and that `chw_to_image`'s existing tests still pass unchanged.

## 2. The contract

- [x] 2.1 Write the canvas plan in `models/light_adjustment/process.rs`: longest side onto the square with **rounding**
      (design D2), the shorter side floored to at least one pixel, and the extension widths that follow. Verify with
      unit tests over landscape, portrait, square, a photograph smaller than the square, and extreme aspect ratios —
      and with one test asserting it disagrees with `newyork::input::fit` on a ratio where rounding and truncation
      differ, so the two can never be folded silently.
- [x] 2.2 Write the presentation step: Lanczos3 resample to the planned size, then `tensor::padded_to_chw` at
      `Normalisation::Unit` into a `3 * canvas * canvas` scratch. Verify with a test that the tensor's interior is the
      resampled photograph unchanged and its margin is the mirror of the edge beside it, and one pinning the range as
      `Unit` against face recovery's `Signed`.
- [x] 2.3 Write the correction: decode the top-left `sw x sh` of the output through 1.1, Lanczos3 both it and the
      resampled photograph to the full resolution, and apply `source * out / (in + 1e-3)` in unit space, bounded
      (design D5). Verify with unit tests that a model output equal to its input leaves the photograph unchanged, that
      a uniform brightening scales every channel by that factor, that a near-black region stays in range, and that
      detail finer than the canvas survives.
- [x] 2.4 Write the bias blend: `source + bias * (corrected - source)`, bounded. Verify with unit tests that 0 returns
      the photograph byte-for-byte, that 1 returns the correction unchanged, and that -1 and 1 move the photograph in
      opposite directions from it.
- [x] 2.5 Write `Model` and `ImagePipeline` for the pipeline: one required artifact, one `(artifact, profile)` pair,
      `stages()` of 1, the depth dispatch over `T: Channel` (design D4), the four-step progress count and the four
      cancellation checks (design D6), and the zero-area refusal. Verify with the fake-backend suite in the same file:
      progress reaches exactly the end of its range and never decreases, a cancellation at each of the four boundaries
      returns no image, a zero-area image returns `Untileable`, and an 8-bit and a 16-bit run produce the expected
      pixel types.

## 3. The two models

- [x] 3.1 Add `models/light_adjustment/paris/mod.rs`: the 1024 canvas, the FP16-only profile (CPU and GPU compute
      units, sequential execution), and the prose carried across from the reference — which checkpoint these weights
      are, why the graph folds to a quarter of the architecture, why the canvas is fixed and why it is 1024. Verify
      with a test asserting the FP16 arm carries exactly those two settings and the FP32 arm carries the default.
- [x] 3.2 Add `models/light_adjustment/lyon/mod.rs`: the 1024 canvas, the FP16-only profile (CPU and GPU compute units
      and **no** execution mode), and the prose — why it cannot be tiled, why it cannot be exported with dynamic axes,
      why 1024, the two graph rewrites, and the single-partition precondition for re-measuring. Verify with a test
      asserting the FP16 arm carries the compute units and **not** a sequential mode, and that this differs from
      Paris' at the same precision.
- [x] 3.3 Wire the two seams in `models/light_adjustment/variant.rs`: `canvas()` and `profile()` as exhaustive matches
      reaching the model directories (design D1). Verify with the family's existing `every_variant()` traversal that
      both variants report a canvas and a profile, and that only the FP16 arms declare anything.

## 4. The family seam and the refusal

- [x] 4.1 Add `LightAdjustment::pipeline` in `models/light_adjustment/mod.rs`, built from `LightAdjustmentParams`,
      `artifact()`, `profile()`, `variant().canvas()` and `display_name()`. Verify with a test that a pipeline built
      for each variant reports that variant's one artifact and one session pair.
- [x] 4.2 Remove light adjustment from `Operation::pipeline`'s refusal arm and invert the tests in
      `models/operation.rs` that name the five refused families and that assert only measured variants declare a
      profile. Verify the refusal suite still reports `FamilyNotImplemented` for denoise, sharpen, colour balance and
      colorization, and that every light adjustment variant now plans.

## 5. Documentation, live check and confirmation

- [x] 5.1 Update the three prose sites that enumerate what runs: `Opai::process`'s "What is refused" in `lib.rs`,
      `InferenceError::Unsupported`'s doc comment in `error.rs`, and `crates/perf/README.md`'s unnamed-sweep note —
      which already claims only upscale has a pipeline and is stale twice over. Verify with `cargo doc` and by reading
      each back against the refusal arm as it now stands.
- [x] 5.2 Add an `#[ignore]`d live check that puts `fixtures/test.dat` through Paris and through Lyon end to end via
      `Opai::process`, at both precisions on the CPU provider and on CoreML. Verify by hand with
      `cargo test -p opai -- --ignored --nocapture`: each run returns a photograph at the source's own dimensions, a
      bias of 0 returns it unchanged, and a positive and a negative bias move mean luminance in opposite directions.
- [x] 5.3 Re-confirm both profiles with `perf` sweeps on Apple Silicon before they land, Lyon's only after confirming
      its FP16 graph is still a single CoreML partition (design, Platforms). Verify by recording the measured medians
      in each model's prose and checking they reproduce the direction and rough size of the reference's figures; a
      disagreement is a finding to record, not a number to overwrite.
- [x] 5.4 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and the full test suite clean, with biome scoped to
      any files touched outside Rust. Verify all three exit zero.
