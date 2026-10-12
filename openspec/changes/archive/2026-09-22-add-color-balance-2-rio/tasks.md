## 1. The family's shared vocabulary

- [x] 1.1 Create `crates/opai/src/models/color_balance/samples.rs` and declare it in the family's `mod.rs`.
  `Samples<'a>` borrows a planar CHW tensor at an offset and answers `len()` and one RGB triple at an index,
  cropped to the plan's `scaled_width` x `scaled_height` and decoded through the `Normalisation` it was
  constructed with (design D1, D2). The offset takes zero here and is what slice 3 builds its per-rendering views
  from. Verify with tests over a hand-laid tensor that the count is the crop's rather than the canvas's, that the
  **last** sample is the photograph's own corner rather than a mirrored one, that each channel comes from its own
  plane, and that a signed `Normalisation` decodes differently from `Unit` — a count-only assertion passes on a
  reader that crops the right number of pixels from the wrong place (risk 2).
- [x] 1.2 Create `crates/opai/src/models/color_balance/mapping.rs`: the 11-feature kernel
  `[r, g, b, rg, rb, gb, r², g², b², rgb, 1]`, `fit` taking one source view and **several** destination views
  (Rio passes one), and the evaluation of one fitted mapping at one pixel. `f64` accumulation, a `1e-8` ridge on
  the diagonal, Gauss-Jordan with partial pivoting, `f32` weights out; the accumulation written plainly with no
  `mul_add` (design D3, D6). Verify with tests that an exactly-linear non-symmetric colour map is recovered to
  round-off, that a feature order permuted by one fails, that two destinations fitted together give what each
  gives alone, and that the kernel's term order is pinned as a literal (risk 3).
- [x] 1.3 Add `InferenceError::ColourMapping { operation, column }` in `crates/opai/src/error.rs`, returned by
  `fit` on a zero pivot rather than dividing into NaN weights (design D5). Verify the singular case by driving
  `eliminate` directly with the ridge removed — the only honest way to build the system the ridge exists to make
  impossible, since the variant guards an arithmetic invariant rather than a state a photograph can reach — and
  state the other half beside it: a flat-colour photograph, the worst input a real run carries, still fits.
  Verify also that `cargo test -p opai --test public_api` fails until the snapshot records the new variant, then
  update it — that file changing is the mechanical sign this slice is visible from outside the crate, which
  slice 1's was not.

## 2. Rio the model

- [x] 2.1 Create `crates/opai/src/models/color_balance/rio/mod.rs` with `CANVAS: u32 = 656` and `profile`, and the
  prose behind both: which weights these are, why the canvas is a fixed square, why 656 and what it costs, and —
  the load-bearing part — why this graph **wants** the Neural Engine where Paris and Lyon refuse it. FP16 declares
  CoreML's fast-prediction specialization; FP32 declares nothing; neither restricts the compute units. Verify
  `cargo doc -p opai --no-deps` builds with no broken intra-doc links.
- [x] 2.2 Wire the variant seams in `crates/opai/src/models/color_balance/variant.rs`: `canvas()` returning 656 for
  both variants, and `profile()` as a match reaching the model's own file rather than answering
  `EpProfile::default()` for everything. Verify with tests that every variant reports a canvas, that **only** Rio's
  FP16 arm declares a profile, and that São Paulo still answers the default — a profile applied to a graph nothing
  measured it against is how a model gets pessimised. Verify the cross-family half as well, because Rio's half
  alone reads like a model with no compute-unit answer: Rio's units at FP16 and Paris's are **opposite
  measurements** rather than one house style, asserted side by side where a tidy-up would have to walk past it.

## 3. Rio's contract

- [x] 3.1 Create `crates/opai/src/models/color_balance/rio/process.rs` with the pipeline type, its `Model` and
  `ImagePipeline` impls, `RANGE = Normalisation::Unit` stated for this family's graphs, and the front half: the
  zero-area refusal, `plan`, `presented` at the 656 square with the resampled photograph dropped (slice 1's D3),
  and one `run_graph`. Verify against `NoBackend` that it asks for exactly one artifact under its own profile, and
  that a zero-width or zero-height image is refused before anything is allocated.
- [x] 3.2 Add the fit: build the source view over the input scratch and the destination view over the output
  tensor, both cropped to the plan, and solve for one mapping. Verify with a fake backend whose graph returns a
  known scaling of its input that the recovered mapping is that scaling, and with one whose answer is graded by
  each pixel's own brightness — `c·(0.6 + 0.4c)`, which is in the kernel and which no per-channel gain can
  express — that a dark pixel and a bright one of the **same hue** are corrected differently.
  **A fake backend returning NaN does not reach `InferenceError::ColourMapping`, and the test states that rather
  than the opposite**: `XᵀX` is built from the source samples alone, so nothing the graph returns can zero a
  pivot — a NaN output lands entirely in `XᵀY`, the elimination finds its pivots as usual, and the weights come
  out NaN. What that renders is black, which is a broken model rather than anything this pipeline can diagnose.
  Recorded here because the clause this replaces asserted the reverse; see design D5, which the discovery moved.
- [x] 3.3 Add the full-resolution half: evaluate the mapping over the photograph's own pixels, monomorphised over
  `Channel` and dispatched once on `ChannelDepth` (design D8), bounding each channel; then `inference::mix::blended`
  by the run's bias. Verify the result carries the photograph's own dimensions, that a 16-bit source asked for at
  16 bits keeps more than 256 levels per channel in a gradient, that alpha is dropped, and that a bias of 0 returns
  the photograph byte for byte, and that a **non-square** photograph run through a backend returning its input
  unchanged comes back unchanged — which a fit polluted by the mirrored extension does not (risk 2).
- [x] 3.4 Add the five-step progress schedule and the cancellation check at each boundary (design D7). Verify with
  the recording helper that reports never decrease and the last is the end of the operation's share, and that a run
  cancelled at each of the five boundaries returns `InferenceError::Cancelled` and **no image** — not the
  photograph, and not the partly corrected buffer.

## 4. The seams

- [x] 4.1 Add `ColorBalance::pipeline` to `crates/opai/src/models/color_balance/mod.rs` — the family's contract
  seam, returning `Result<Shared<B>, InferenceError>`, one arm per contract: Rio to `rio::process`, São Paulo
  refused with `UnsupportedReason::FamilyNotImplemented` (design D4). Built from what `params()` publishes and the
  canvas the **variant** reports. Verify against `NoBackend` that both of Rio's precisions reach a pipeline naming
  Rio's one artifact and profile, that one session serves every bias, and that São Paulo is refused by name.
- [x] 4.2 Remove `Self::ColorBalance(_)` from `Operation::pipeline`'s refusal arm in
  `crates/opai/src/models/operation.rs` and hand to the family, updating the doc comment that enumerates which
  families are still deferred and the `served` predicate in its tests. Verify
  `every_operation_without_a_pipeline_is_refused_and_every_one_with_a_pipeline_is_not` passes over every operation
  the library can name, with Rio served and São Paulo refused in the same words denoise is.

## 5. The live check

- [x] 5.1 Create `crates/opai/src/models/color_balance/live.rs` as `#[cfg(test)]` and `#[ignore]`d, following
  `models/light_adjustment/live.rs`: install the real `cb_rio_fp32` and `cb_rio_fp16` weights into a temporary
  directory and state three properties — the result carries the photograph's own dimensions, a bias of 0 returns
  the photograph unchanged, and a positive and a negative bias move mean chromaticity in opposite directions
  (design D10). Verify it compiles under `cargo test -p opai --no-run`.

## 6. Toolchain and confirmation

- [x] 6.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and confirm
  `crates/opai/tests/public_api.rs` carries exactly the one added error variant and nothing else — no type removed,
  none re-spelled.
- [x] 6.2 Run the live check by hand on macOS/CoreML —
  `cargo test -p opai models::color_balance::live -- --ignored --nocapture --test-threads=1` — and then the
  confirming sweep through `perftest` at both precisions, with and without the fast-prediction specialization, to
  re-derive the -3.0% at FP16 and the wash at FP32 the profile declares. If the sweep contradicts either, drop the
  profile to the default and remove the requirement with it (risk 4) — a setting this project cannot reproduce is
  not one it declares.
