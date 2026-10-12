## 1. São Paulo the model

- [x] 1.1 Create `crates/opai/src/models/color_balance/saopaulo/mod.rs` and declare it in the family's `mod.rs`:
  `CANVAS: u32 = 656`, `RANGE = Normalisation::Unit`, the channel layout derived from one `SETTINGS = 3`
  (`RENDERINGS`, `CHANNELS`, `rendering_offset`) and `profile`, with the prose behind each — which weights these
  are, why the canvas **cannot** be traded the way Rio's can (3.7 dB at 512, 7.6 dB at 384), why the layout is the
  conversion script's contract rather than an implementation detail, and why the defaults are a measurement here
  rather than an omission (design D2, D10). Verify with tests that pin `CHANNELS == 9` and both rendering offsets
  as literals with no session open, and that `cargo doc -p opai --no-deps` builds with no broken intra-doc links.
- [x] 1.2 Point the variant seams in `crates/opai/src/models/color_balance/variant.rs` at that file: `canvas()`'s
  São Paulo arm to `saopaulo::CANVAS` in place of the literal, and `profile()`'s to `saopaulo::profile` in place of
  the inline `EpProfile::default()` (design D10). Verify with tests that São Paulo reports 656 at both precisions,
  that its options are the provider defaults with nothing added or removed at **both** precisions, and — asserted
  beside it, where a tidy-up would have to walk past it — that Rio's FP16 arm still declares the fast-prediction
  specialization, which is the contrast that says a profile is not carried between two models of one family.

## 2. The weight map's geometry

- [x] 2.1 Create `crates/opai/src/models/color_balance/saopaulo/weights.rs`: the per-axis bilinear tables
  (`s = (i + 0.5) * src_len / dst_len - 0.5`, clamped at zero, both taps clamped to the last source index) resolved
  once per axis, and the read of one canvas-resolution plane through them, summing along x within each row and then
  between the rows as the reference does (design D3). Verify against hand-computed numbers: a destination pixel
  landing exactly on a source centre reads that sample alone, both edge clamps hold, the four weights sum to one,
  and an upsample of a plane that is constant is that constant everywhere — and pin the align-**centres** convention
  with a test that fails on the align-corners mapping, since that mistake shifts every weight by up to half a pixel
  and nothing else in the slice catches it (risk 2).

## 3. São Paulo's contract

- [x] 3.1 Create `crates/opai/src/models/color_balance/saopaulo/process.rs` with the pipeline type, its `Model` and
  `ImagePipeline` impls, and the front half: the zero-area refusal, `plan`, `presented` at the 656 square, and one
  `run_graph` declaring `GraphShape::new(3, …)` in and `GraphShape::new(9, …)` out. Verify against `NoBackend` that
  it asks for exactly one artifact under its own profile, that a zero-width or zero-height image is refused before
  anything is allocated, and with a fake backend that the two shapes reaching the graph are those two and that it
  runs exactly once.
- [x] 3.2 Add the fit: one `Samples` over the input scratch for the source, one per rendering over the output tensor
  at `rendering_offset(i)`, and a single `mapping::fit` taking both destinations (design D4). Verify with a fake
  backend whose two renderings are two **different** known casts that both mappings are recovered, that each is what
  it would be if fitted alone, and that a rendering read at the wrong offset yields a different photograph — which
  is what makes the offsets worth pinning (risk 1). **Neither `samples.rs` nor `mapping.rs` is edited**; if either
  needs it, report that slice 2's placement was wrong rather than patching it.
- [x] 3.3 Add the fused full-resolution pass: the kernel built once per pixel and reused across both mappings, the
  daylight term taken as the photograph itself, each synthesized rendering bounded to `[0, 1]` **before** its weight
  multiplies it, the three weighted by the upsampled map, and the accumulation written plainly with no `mul_add`
  (design D5). Expose the kernel half of `Mapping::evaluate` without changing what `evaluate` answers — `mapping.rs`'s
  existing tests are what say so (risk 4). Verify that an all-daylight map returns the photograph, that an
  all-shade map returns that rendering's mapping applied globally, that a map split down the middle produces the
  boundary at the photograph's own middle, and that a rendering whose fit extrapolates past 1 does not pull the
  blend past what the renderings span (risk 3).
- [x] 3.4 Add the bias blend through `inference::mix::blended`, the depth dispatch once at the top (design D8) and
  the five-step progress schedule with a cancellation check at each boundary (design D7). Verify the result carries
  the photograph's own dimensions, that a 16-bit source asked for at 16 bits keeps more than 256 levels per channel
  in a gradient — which the reference's 8-bit output cannot — that alpha is dropped, that a bias of 0 returns the
  photograph byte for byte, that reports never decrease and the last is the end of the operation's share, and that a
  run cancelled at each of the five boundaries returns `InferenceError::Cancelled` and **no image**.

## 4. The seam closes

- [x] 4.1 Replace `ColorBalance::pipeline`'s São Paulo refusal arm in
  `crates/opai/src/models/color_balance/mod.rs` with `saopaulo::process::pipeline`, built from what `params()`
  publishes and the canvas the **variant** reports, keeping the `Result` the seam already returns and updating the
  doc comment that explains the refusal (design D9). Verify against `NoBackend` that both variants at both
  precisions reach a pipeline naming that variant's one artifact and profile, that one session serves every bias,
  and that no colour balance operation is refused.
- [x] 4.2 Move São Paulo from refused to served in `crates/opai/src/models/operation.rs`'s test `served` predicate.
  Verify `every_operation_without_a_pipeline_is_refused_and_every_one_with_a_pipeline_is_not` passes over every
  operation the library can name, with both colour balance variants served and nothing else moved with them.

## 5. The live check

- [x] 5.1 Add São Paulo to `crates/opai/src/models/color_balance/live.rs` at both precisions and delete the "Why
  São Paulo is not here" section it opens with: the three properties Rio states — the photograph's own dimensions, a
  bias of 0 returning it unchanged, a positive and a negative bias moving mean chromaticity in opposite directions —
  plus the one that is this model's, that the result's luminance structure tracks the photograph's, which a weight
  plane read as a rendering destroys while still producing an image (design D12). Verify it compiles under
  `cargo test -p opai --no-run`.

## 6. Toolchain and confirmation

- [x] 6.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and confirm
  `cargo test -p opai --test public_api` passes **unchanged** — this slice adds no public type, error variant or
  spelling, and that file not changing is the mechanical sign of it.
- [x] 6.2 Run the live check by hand on macOS/CoreML —
  `cargo test -p opai models::color_balance::live -- --ignored --nocapture --test-threads=1` — and then the
  confirming sweep through `perftest` at both precisions: the CoreML defaults against fast prediction and against
  the CPU-and-Neural-Engine restriction, to re-derive the wash and the +62%/+732% the profile rests on. Confirm both
  graphs are still one CoreML partition on the runtime this application pins before trusting any of it — a newer
  runtime absorbs the stray input `Cast` and reads as fully fused either way. If the sweep finds something that
  beats the defaults outside its own run-to-run spread, the profile gains it and the `execution-providers`
  requirement changes with it (risk 5).
