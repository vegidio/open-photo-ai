## Why

Slice 1 answered the three questions that needed the real weights — the transformer loads under the
pinned runtime with `SimplifiedLayerNormFusion` disabled, the scheduler step is read correctly, and
the 33-channel packing order is right — and left one 960x960 region provably restored. What it
deliberately did not do is make Osaka reachable: `inference::chain::plan_one` still refuses
`Resolution::Graphs` with *"the diffusion upscaler needs a pipeline of its own"*, and everything under
`inference/osaka/` sits behind `expect(dead_code)` markers whose stated reason is this change.

**This is slice 2, and it is the last one.** What is left is the image-scale pipeline around the
region — the Lanczos resample to the target size, the tile grid, the weighted accumulator, the wavelet
colour fix — plus lifting the refusal. Slice 1's design cut the series this way on purpose (its D1):
everything here is deterministic arithmetic over float buffers that a unit test either passes or does
not, so none of it can be silently wrong the way the three questions above could.

Osaka is the last of the four upscale variants refused. When this archives, every upscale variant
runs.

## What Changes

- **The image is resampled to its target size first, and detail is restored at that size.** This is
  not a fallback for a model that cannot enlarge — it is how SeedVR2 is meant to be driven. The
  network is resolution-preserving throughout: the VAE compresses 8x and expands 8x and the
  transformer between them does not change the token grid, so the reference resizes to the target
  resolution *before* inference. Lanczos both upsamples and, at a scale of 1, leaves the image alone;
  either way that resampled image is what the model is conditioned on and what the colour fix corrects
  towards.
- **Two constraints on the size handed to the model.** Every dimension must be a multiple of 16 — the
  VAE's 8x on top of the transformer's 2x patchify — and neither may be below one 960-pixel region,
  since the graph accepts exactly one region size. The image is padded up to satisfy both, by
  mirroring its own edge pixels, and the padding is trimmed off the result. Padding rather than
  cropping keeps the edge pixels the user asked for.
- **A weighted float-CHW accumulator, rather than the existing tile blender.** `blend::blend_tile`
  writes each tile over what is already in the buffer, which cannot average more than two tiles: where
  four meet, the result depends on the order they were written. With per-region stochastic content
  that corner is exactly where a seam shows. The accumulator weights every contribution and divides
  once at the end, so every pixel is a proper weighted average of every region that covered it. It
  accumulates in planar float as the decoder produces it, with no per-region round trip through 8-bit.
  Both blenders share `blend::ramp_weight` — a tuning change applied to only one of them is a seam.
- **The wavelet colour fix**, once, on the assembled image. A single diffusion step drifts in overall
  colour and brightness, and because each region drifts independently the drift reads as seams no
  amount of blending removes — the regions genuinely disagree about the colour they share. An à trous
  B3-spline transform separates low frequency from detail; the result's low frequencies are replaced
  with the resampled input's and its own detail is kept.
- **The refusal comes down.** `plan_one`'s `Resolution::Graphs` arm becomes the call to `plan_graphs`
  that slice 1 wrote and left unreachable, the chain's run loop gains the diffusion branch, and every
  `expect(dead_code)` naming this slice is dropped — `clippy` failing on an `expect` that is no longer
  needed is the check that they all were.
- **No new public API, and no new dependency.** Osaka is already published in the catalogue and is
  already selectable in both front ends; it fails at run time today and succeeds after this. Nothing
  about `Upscale`'s serialized form, its cache tag or the catalogue changes.

### Deliberate divergence from the reference's behaviour

**Regions are feathered over the width they actually share, not over the configured 128.** The
reference's accumulator ramps every interior edge over the configured overlap and leans on the final
normalization to make the result correct where two regions share more — which the move-the-last-tile-
back rule guarantees for the last column and row. This project already decided that question the other
way for the convolutional path, and `inference-tiling` carries it as a requirement: *"The width
combined SHALL be the overlap those two tiles actually have"*. Ramping the real width makes each pair
of ramps a partition of unity before the division rather than after it, so the two paths answer the
same question the same way. The reference's result is not wrong, it just relies on a later step.

**No memory-pressure warning, still.** Slice 1's D9 recorded this: the reference warns before a run it
expects to exhaust the pool by reading a per-pool budget from its model registry, and this project has
no memory budget to read. Nothing changes here, and the gap is carried forward rather than half-built.
What slice 2 adds is a second number for the change that eventually closes it — see Impact.

**The noise field still differs from Go's**, so the same image at the same settings is not
pixel-identical between the two applications. Unchanged from slice 1's D6, restated because this is
the slice where a user can actually observe it.

## Capabilities

### New Capabilities

None. Everything here extends a capability that already exists.

### Modified Capabilities

- `image-enhancement`: *A requested scale is covered by native passes and corrected afterwards* is
  today written as though every upscale works that way, which stops being true here — it is narrowed
  to the upscalers whose weights are published per whole factor. Two requirements are added for the
  other contract: the diffusion upscaler reaching a requested scale by resampling first and restoring
  at that size, and its colour being corrected once against the image it was conditioned on.
- `inference-tiling`: tiles may instead be combined by **averaging every contribution**, which is what
  a pipeline whose per-tile content is stochastic needs where more than two tiles meet — the existing
  requirement describes writing each tile over the last, which cannot average three.

## Impact

**Crates**: `opai` only. Neither front end changes: Osaka is already in the catalogue and already
selectable, so no i18n key, no component and no Tauri command is added — what changes is that choosing
it stops failing. `perf` needs nothing either; it selects operations from the catalogue and Osaka was
always selectable there.

**rust-sak**: checked, and it has **nothing** for either piece of infrastructure this slice needs. Its
`image` module is decode, encode and probe — eight formats behind one API — with no resampling, no
filtering and no convolution of any kind, and no other module is close. The resample is served by the
`image` crate already in the dependency tree, through the same `FilterType::Lanczos3` that
`upscale::correct_overshoot` resamples with today. The à trous wavelet is not a gap worth filling in
`rust-sak` either: it is SeedVR2's own correction for a defect of single-step diffusion, not a
general-purpose image operation another project would want, so it belongs in `opai`. **No dependency
is added by this slice.**

**Files**: a new `inference/osaka/canvas.rs` and `inference/osaka/colorfix.rs`, the pipeline driver in
`inference/osaka/mod.rs`, a non-square whole-image sibling to `image_to_chw` in `inference/tensor.rs`,
and `inference/chain.rs` for the refusal and the run loop's diffusion branch. `inference/grid.rs`,
`inference/pad.rs` and `inference/blend.rs` are reused unchanged.

**Memory**: this is the slice where the figure gets worse, and it is recorded rather than acted on. On
top of the three resident sessions slice 1 measured at **29.0 GB** peak for a single region, the
image-scale buffers are float and proportional to the *output*, which at 4x is sixteen times the
input. Over a 12-megapixel photograph that is a 192-megapixel output, where one planar float buffer is
**2.3 GB** and the pipeline holds three of them at its peak — the resampled base, the accumulator, and
either the accumulator's weight plane or the colour fix's scratch — for roughly **7 GB of float on top
of the model**. Nothing in this project budgets, admits or refuses that, and on the host pool the
failure mode is ONNX Runtime's allocator taking the process down. The memory budget stays deferred —
it is its own change and a guess is worse than nothing — and this is the second figure for it, now
covering the whole-image path rather than one region.

**Task count**: 14, against the 12 slice 1 estimated for this slice. Two were added: splitting
progress and cancellation out of the pipeline driver, which is the same contract the convolutional
driver already has and is checked separately from the geometry; and a head-to-head `perf` run against
the Go application over CoreML, which is the one check that can only be made once the refusal is
actually lifted. Within the 15-task limit, and the series ends here.
