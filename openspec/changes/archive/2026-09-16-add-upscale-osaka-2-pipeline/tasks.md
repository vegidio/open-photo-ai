## 1. The padded extent, without materialising it

- [x] 1.1 Add the whole-image, non-square sibling to `image_to_chw` in `inference/tensor.rs`, reading a
      padded extent out of a `Sampler` through `pad::source_offset` (design.md D2). Record in its
      comment why it is a sibling rather than two more parameters on `image_to_chw`. Verify tests cover
      a plane read back unchanged where the extent equals the source, the extension being the mirror of
      the pixels beside it on each axis independently, a buffer whose length disagrees with the extent
      being an error naming both lengths, and `Normalisation::Signed` landing the values in `[-1, 1]`.
- [x] 1.2 Add Osaka's geometry constants beside the ones slice 1 confirmed — the 128-pixel region
      overlap and the 16-pixel alignment — and the target-size arithmetic: the requested scale applied
      to the source's dimensions and rounded, then each axis raised to a multiple of 16 and to at least
      one `REGION_EDGE`. Attribute the two figures to the reference in their comments, including why
      128 does not scale with the region. Verify tests cover a scale of 1 leaving the target at the
      source's dimensions, an axis already aligned being left alone, an axis below one region being
      raised to exactly one, and the rounding matching `correct_overshoot`'s on an odd dimension.

## 2. The accumulator

- [x] 2.1 Add `inference/osaka/canvas.rs`: a planar float sum plus a shared weight plane, `add` taking
      a region and the widths it actually shares with its neighbours, and `resolve` dividing **in
      place** (design.md D3, D8). Feather through `blend::ramp_weight` rather than a second curve, and
      leave edges on the image border unramped. Verify tests cover a uniform image tiled at Osaka's
      geometry resolving back to that uniform value everywhere, a pixel covered by four regions being
      the weighted average of all four **and** unchanged when the four are added in a different order,
      an unramped border edge keeping its own value, and a pixel no region covered being left visibly
      at zero rather than divided by it.

## 3. The colour fix

- [x] 3.1 Add `inference/osaka/colorfix.rs`'s à trous low-frequency extraction: five levels of the
      B3-spline kernel `[1,4,6,4,1]/16` at doubling dilation, separable, replicating the edge pixel
      rather than zero-padding, with the horizontal and vertical passes written separately for the
      reason design.md D9 records. Verify tests cover a constant plane being its own low frequency at
      every level, a plane of pure high-frequency detail extracting to near its own mean, the border
      not darkening the way zero-padding would, and the separable result matching a direct
      two-dimensional convolution over a small plane.
- [x] 3.2 Add the correction itself: per channel, add the reference's low frequency and subtract the
      result's, in place over the assembled buffer (design.md D5). Record in the module's comments that
      it runs once on the assembled image and never per region, and that its reference is the resampled
      image rather than the original input. Verify tests cover correcting an image against itself being
      the identity, correcting against a uniformly shifted copy recovering the shift, high-frequency
      detail surviving the correction, and mismatched buffer lengths being refused rather than
      truncating.

## 4. The pipeline

- [x] 4.1 Add the driver in `inference/osaka/mod.rs`: resample the source to the target size with
      `FilterType::Lanczos3`, convert the padded extent through task 1.1, then walk `TileGrid` at
      960/128 over the padded dimensions, clipping each tile to the resampled image's own extent before
      `restore_region` and accumulating what comes back (design.md D2, D7). Dispatch `ChannelDepth`
      once at the top. Verify tests against a fake backend cover every pixel of the padded extent being
      covered, each region arriving at exactly 960x960, an image smaller than one region running as a
      single region, and an image with no area being refused as `TilingError::Untileable` with no
      picture produced.
- [x] 4.2 Finish the pipeline: resolve the canvas, apply the colour fix against the padded base, convert
      to the caller's depth through `chw_to_image`, and crop the alignment extension off. Verify tests
      cover the result's dimensions being the source's multiplied by the requested scale and rounded at
      several scales including 1, the extension not appearing in the result, and a 16-bit request coming
      back as `ImageRgb16` rather than being narrowed on the last line.
- [x] 4.3 Add progress and cancellation on the same contract the convolutional driver has (design.md
      D7): `0.0` at the start, one report per region, exactly `1.0` at the end, checked once per region
      for cancellation, and failures reported as `TilingError` so `InferenceError::from_tiling` needs no
      new arm. Verify tests cover the reported sequence being monotonic and landing on exactly 1, a
      cancellation partway through producing no image at all, and a failing region naming its index with
      no partial picture.

## 5. Lifting the refusal

- [x] 5.1 Replace `plan_one`'s `Resolution::Graphs` refusal with the `plan_graphs` call slice 1 wrote
      for it, and give the chain's run loop its diffusion arm, the three handles travelling into the
      same blocking closure the pass sequence's do (design.md D6). Leave the acquisition loop untouched —
      `Planned::sessions` already answers both contracts. Verify the existing chain tests still pass
      with the Osaka-is-refused one replaced by its opposite, and add tests that an Osaka operation
      plans, acquires exactly three sessions under the profile measured for each graph, and that a chain
      mixing Osaka with a convolutional upscale runs both.
- [x] 5.2 Remove every marker this slice was named as the caller of: the `expect(dead_code)` on
      `osaka::{latent, noise, region}`, on `plan_graphs`, on `Planned::Graphs`'s helpers, and the now
      unused `DIFFUSION_UNSUPPORTED`. Verify `cargo clippy --all-targets -- -D warnings` is clean, an
      `expect` that is no longer needed being a build failure and therefore the check that all of them
      were claimed, and that `grep -rn "slice 2" crates/` returns nothing.

## 6. Confirming it, and closing the series

- [x] 6.1 Add the `#[ignore]`d end-to-end test driving the real FP16 weights through the public API on
      the CPU provider, stating in its reason that CI has neither the 6.8 GB of weights nor the time.
      Verify an image several regions across upscales to the requested dimensions, that the result
      correlates more strongly with a Lanczos resample of the input than that resample does with the
      input itself, that no step in value appears across a region boundary, and — the property the
      colour fix exists for — that per-channel means track the resampled reference's rather than
      drifting. Do not stub the model or fabricate output if the weights are unavailable; leave the test
      failing with the reason stated. **The correlation check above was restated after the first run
      showed it cannot discriminate** — correlating the result against a resample of the input means
      enlarging the input first, so the baseline becomes two interpolations of one photograph, which
      agree whatever the model did. See design.md's *What the end-to-end run showed* for the measured
      figures and for the detail-energy inequality that replaced it.
- [x] 6.2 Document what landed and what deliberately did not: the two ways of combining tiles and when
      each applies, the divergence in D4, and the two parity gaps the series leaves open — the noise
      field differing from the Go application's, and the absence of any memory budget, admission check
      or warning against the figures in design.md D8. Update every comment naming slice 2 as future
      work, and the `models` and `inference` module docs that still say Osaka is refused. Verify
      `cargo doc -p opai` emits no warnings and that nothing in `crates/opai/src` still claims Osaka
      cannot run.
- [x] 6.3 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and
      `cargo test --locked`, and verify all three are green on the ubuntu, macos and windows CI jobs
      with nothing newly `#[ignore]`d beyond task 6.1. No frontend script is needed: this change touches
      `opai` alone, and Osaka is already published in the catalogue and already selectable in both front
      ends, so nothing about the GUI or the CLI changes.

## 7. Against the Go application, on the same machine

- [x] 7.1 Run the two `perf` apps head to head on Osaka over CoreML and compare what comes back — the
      Rust `crates/perf` and the Go `cmd/perf`, both at FP16, both `--runs 5` with the default single
      warm-up, over the same input image on the same machine. Verify the Rust sweep reports **five
      timed inferences rather than a refusal**, which is the end-to-end proof that the refusal is
      genuinely lifted through the public API and not only in a test; that the two report the same
      output dimensions for the same input and scale; and that the steady-state per-run times are of
      the same order, with any large gap investigated rather than recorded. **Pixel-identical output is
      not the test and must not be asserted** — the noise fields differ by design (design.md's series
      note and slice 1's D6), so the images are compared structurally, not bit for bit. Record the two
      sweeps' figures in the change. This is a manual cross-implementation check needing the 6.8 GB of
      weights, a CoreML machine and a checkout of `../open-photo-ai`, so it is not added to the test
      suite or to CI.
