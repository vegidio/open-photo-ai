## 1. Verify the published exports before writing anything against them

- [x] 1.1 Install the three FP16 Osaka artifacts (`up_osaka_fp16`, `up_osaka_vae_encoder_fp16`,
      `up_osaka_vae_decoder_fp16` — ~6.8 GB, and none is on this machine) and inspect them against
      every constant design.md D7 takes on trust from the reference. Verify the encoder is
      `[1,3,H,W] -> [1,16,H/8,W/8]` and the decoder its inverse, confirming the VAE stride of 8; that
      the transformer takes **33** channels and returns 16; that it accepts a 960x960 region and
      nothing else; and that its only input is the packed latent, with **no timestep input** — the
      reference states it was folded to a constant, and `schedulerStep`'s arithmetic collapses only
      because of that. **Stop and raise design.md's D7 if any of these differs**; do not write tasks
      4.1-4.3 around a different shape.
- [x] 1.2 Confirm the load behaviour the profile exists for, through this project's own binding rather
      than a local Python install — the reference records that being how it was once wrongly declared
      fixed. Verify the transformer **fails** session creation under `ort` 2.0.0-rc.13 against the
      pinned ONNX Runtime 1.26 with every optimizer on, that the failure names a node the rewrite
      created, and that it **succeeds** with `ReshapeFusion` and `SimplifiedLayerNormFusion` disabled.
      If it loads either way, say so and record it — the setting then costs nothing but its
      justification in the spec delta is wrong and must be corrected rather than left standing.

## 2. The measured settings

- [x] 2.1 Replace `EpProfile::default()` in `UpscaleVariant::profile`'s `Osaka` arm with the
      declaration transcribed in the `execution-providers` delta — sequential execution, memory
      planner off, the two disabled optimizers, CoreML on the CPU and GPU, and
      `trt_builder_optimization_level` at 3 — with each figure attributed to the reference in the
      arm's comment, since design.md D4 records that none was re-measured here. Document why
      TensorRT's FP16 mode is refused at **both** precisions and not merely absent. Verify with
      variant-level tests in `variant.rs` asserting every field at FP16 and at INT8, and an
      option-map test in `providers/options.rs` asserting the level reaches TensorRT as 3 while
      another model's reaches it as 5.
- [x] 2.2 Apply the transformer's `cuda_prefer_nhwc` override where the graph set is resolved, so it
      is a property of the graph rather than something a caller remembers (design.md D3, D4), and
      drop the `expect(dead_code)` markers the first declaration of `disabled_optimizers` and
      `trt_options` now claims. Verify a test asserts the two VAE halves resolve with NHWC set and the
      transformer with it clear, all three from one operation; and that `cargo clippy` is clean, an
      `expect` that is no longer needed being a build failure and therefore the check.

## 3. Widening the inference seam

- [x] 3.1 Add `Backend::run_graph` and its `inference/session.rs` implementation, taking the input and
      expected output shapes explicitly and leaving `run_tile` untouched (design.md D2). Keep the three
      properties `run_tile`'s comment defends — the input borrowed through `TensorRef::from_array_view`
      rather than copied, one copy out of the runtime's buffer, the lock taken per call. Verify tests
      cover a non-three-channel input, an output at a different resolution than the input, an output
      whose length disagrees with the declared shape being an error naming both lengths with no
      partial write, and a graph returning no tensor being an error rather than a panic.
- [x] 3.2 Split `chain::Planned` into its two contracts — a pass sequence with one shared profile, or
      a role-addressed graph list where each entry carries its own (design.md D3) — and have the
      chain's acquisition loop take a handle per graph. Leave `plan_one`'s `Resolution::Graphs`
      refusal **up** (design.md D8). Verify the existing chain tests pass unchanged, including the one
      asserting Osaka is refused and the one asserting a chain whose second operation is unsupported
      installs nothing for its first; and add a test that an Osaka operation's three graphs each plan
      with the profile task 2.2 gave them.

## 4. The region

- [x] 4.1 Add the latent module: the constants confirmed by task 1.1 (16 latent channels, 33
      transformer channels, VAE stride 8), the `[noise | encoded_latent | ones]` packing, and the
      scheduler step `x0 = noise - prediction` (design.md D7). Record in the module's comments that
      the packing order and the absence of any 0.9152 rescale are conventions from SeedVR2's reference
      implementation rather than anything the graph declares. Verify tests cover the packed buffer's
      three channel groups landing at the right offsets with the mask plane all ones, and the
      scheduler step over a known pair.
- [x] 4.2 Add `rand_chacha` — with `rand_core`, and **not** `rand` (design.md D5) — and the seeded
      per-region Gaussian noise, keyed from `(seed, origin_x, origin_y)` into a 32-byte key and shaped
      by Box-Muller. Record in the manifest comment why a named algorithm is required rather than
      `StdRng`, and in the code that Go's stream is not reproduced (design.md D6). Verify tests cover
      the same origin giving the same field twice, two origins giving different fields, an origin's
      field being independent of how many regions preceded it, and the distribution's mean and
      variance landing near 0 and 1 over a large draw.
- [x] 4.3 Add the region driver: one 960x960 region through `image_to_chw` at
      `Normalisation::Signed`, then encode, noise, pack, transform, scheduler step and decode, holding
      all three sessions across the whole region (design.md D10). Drop `Normalisation::Signed`'s
      `expect(dead_code)`, which this is the first caller to claim. Verify against a fake backend that
      the three graphs are called in order with the shapes task 1.1 confirmed, that a failure in any
      one produces no partial region, and that `cargo clippy` is clean.

## 5. Confirming it against the real model

- [x] 5.1 Add the `#[ignore]`d region test that drives the real FP16 weights on the CPU provider and
      on CoreML, stating in its reason that CI has neither the 6.8 GB of weights nor the time.
      Verify one 960x960 region encodes, restores and decodes to an image of the same dimensions, and
      that the CPU and CoreML results agree to within the cosine similarity the reference records for
      this graph. Do not stub the model or fabricate output if the weights are unavailable — leave
      the test failing with the reason stated.
- [x] 5.2 Verify the two readings that can be wrong without failing (design.md D1) by differential
      test rather than by inspection: assert the decoded region correlates strongly with its input,
      and that **skipping** the scheduler step and **permuting** the noise and condition channel
      groups each produce a *worse* correlation. Record the measured figures in the test's doc
      comment. If either mutation scores the same or better, stop — the reading is wrong and tasks
      4.1 and 4.3 need revisiting, not the test.

## 6. Closing out

- [x] 6.1 Document what landed and what deliberately did not: the per-graph profile contract, the
      shaped-inference seam, and the two parity gaps this slice knowingly leaves — the noise field
      diverging from the Go application's (design.md D6) and the absence of any memory warning or
      budget against three resident sessions totalling ~7 GB (design.md D9). Verify `cargo doc -p opai`
      emits no warnings, and that nothing in `crates/opai/src` claims Osaka runs — the refusal is
      still up and the docs must still say so.
- [x] 6.2 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and
      `cargo test --locked`, and verify all three are green on the ubuntu, macos and windows CI jobs
      with nothing newly `#[ignore]`d beyond tasks 5.1 and 5.2. No frontend script is needed — this
      change touches `opai` alone and nothing it adds is user-visible yet.
