## 1. Calibration data

- [x] 1.1 Locate the sets of design D6 and download only their sampled images into the session scratch directory,
      never into the repository: at most 3,000 images per signal, sampled by a hash of the image name, by range request
      where an archive allows it. Replace D6's data table with each set's URL, licence, size, subset used and image
      count. If a set is unavailable, amend D6 to use synthetic degradations of the clean sets for that part, and say
      so. Verify: the directory holds noisy and clean, and blurred and sharp, versions of the same scenes paired by
      name, plus the SPAQ sample with its scores, and no signal's sample exceeds 3,000 images.

## 2. The block pass

- [x] 2.1 Add the block pass of design D2 to `autopilot`: at most 256 blocks of 64x64 at full resolution on a regular
      grid, origins at multiples of 8, whole blocks only, a block with any sample below half alpha skipped, and
      samples read through `Sampler::rgb_and_alpha` into encoded luma and two chroma differences. Keep the block size a
      parameter so the harness can compare 32. Verify with tests showing:
      - a photograph at or below the budget is tiled with every whole block read, and a larger one is read in at most
        1,048,576 pixels;
      - a photograph with no whole block yields no blocks;
      - a block touching transparency is skipped;
      - a 16-bit gradient is measured at more than 8 bits.
- [x] 2.2 Add the noise signal (design D3): the Immerkær estimate per block and channel, skipping 8-pixel boundary
      positions and positions touching clipped samples; the flattest tenth of the blocks, and at least 4; the median
      luma noise, chroma noise and luma; and one linear score with starting weights as named constants. Verify with
      synthetic-image tests for each noise scenario in the spec: grainy, grain in the colour alone, clean, clean with
      fine texture, and stronger grain found wherever weaker grain is.
- [x] 2.3 Add the sharpness signal (design D4): the Crété-Roffet blur effect per block on luma, over the positions
      whose difference on a 3x3-smoothed copy exceeds `k·σ` of the noise estimate; blocks below the contrast minimum
      excluded; the 90th percentile over at least 4 blocks; and one linear score over it and the base-2 log of the
      pixel count, with starting constants. Verify with synthetic-image tests for each sharpness scenario in the spec:
      out of focus, shaken, sharp, sharp subject on a blurred background, no detail, blurred with grain (both
      suggested), and stronger blur found wherever weaker blur is.

## 3. Calibrate

- [x] 3.1 Extend the `#[ignore]`d harness in `autopilot/calibration.rs` for noise (design D6): pair each noisy image with
      its clean version, measure the actual luma and chroma residuals, and print false alarms, detection and the band
      between. Print SPAQ's least-noisy and noisiest thirds, and whether the starting cut-offs split them. Verify: it
      runs over the task 1.1 noise sample and prints every noise row of D6's target table.
- [x] 3.2 Extend the harness for sharpness: pair each blurred image with its sharp version, measure the detail kept at
      the 90th percentile, and print the same rates, with SPAQ's sharpest and softest thirds. Verify: it runs over the
      task 1.1 blur sample and prints every sharpness row of D6's target table.
- [x] 3.3 Measure the challengers and options of designs D2 to D4 at the 5% false-alarm budget: PCA against Immerkær
      (with the Jacobi eigen-solver written in the harness), CPBD against Crété-Roffet, 32-pixel against 64-pixel
      blocks, and the 8-pixel exclusion on and off. Record each result in the design, and keep each default unless
      its alternative gains at least 3 points of detection. Verify: the design records every comparison, and the code
      holds only the options that won.
- [x] 3.4 Fit both signals' weights, `k` and the contrast minimum, and replace the starting constants. Record the
      achieved rates in design D6 in place of the targets. If a target cannot be met, stop and report the measured
      rates before continuing (design, Risks). Verify: every target is met or knowingly reported as missed, and the
      synthetic tests of 2.2 and 2.3 still pass with the fitted values.

## 4. Into the analysis

- [x] 4.1 Add the `Suggestion::Denoise` and `Suggestion::Sharpen` arms and their `family()`, and run the block pass
      beside the strided pass inside the pixel signals' one blocking task (design D5). Verify with tests showing:
      - both arms come back in `Family::ALL` order beside the other four;
      - a face model that will not open still returns both signals' suggestions, with `incomplete` naming face only;
      - a cancellation during the pass returns `InferenceError::Cancelled` and no suggestions;
      - a photograph too small for a block, and a cut-out on transparency, get neither suggestion.
- [x] 4.2 Extend `every_suggestion_names_a_family_the_enhancement_path_can_apply`, `a_suggestion_names_its_own_family`
      and the order-divergence test's suggestible set to cover both arms. Verify with `cargo test -p opai autopilot`.
- [x] 4.3 Log the block pass's measurements at `debug` (design D7). Replace the module comment's line that no signal is
      invented for denoise or sharpen with this change's divergence, and add the block pass to the module diagram.
      Verify with a `logging::records_of` test that finds the measurements at `debug` and not at `info`.

## 5. Finish

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and fix
      anything they report. Verify: all three pass clean.
