## 1. Calibration data

- [x] 1.1 Locate and download the exposure-correction set and the Rendered-WB set (design D6) into the session
      scratch directory, never into the repository. Record in design.md the URL, the licence, the subset used and the
      number of scenes. If either set is unavailable, amend design D6 to use the synthetic Kodak variants for that
      signal, and say so. Verify: the directory holds correct and incorrect renderings of the same scenes, paired by
      name.

## 2. Shared primitives

- [x] 2.1 Undo the Lab promotion (design D7): move `imaging::lab` back to `models/colorization/lab.rs`, remove
      `sample_to_lab` and its test, and restore colorization's imports. Nothing outside colorization may name it
      afterwards. Verify with `cargo test -p opai models::colorization`, passing with the same bit-for-bit assertions
      as before the move.
- [x] 2.2 Add an accessor to `imaging::tensor::Sampler` that returns a pixel's alpha alongside `rgb`: `u16::MAX` for
      sources with no alpha channel (design D3). Verify with a test covering 8-bit, 16-bit, alpha and no-alpha sources.

## 3. The pixel pass

- [x] 3.1 Add the one-pass sampler of design D2 to `autopilot`: the stride rule, reading through `Sampler`, and
      skipping samples below half alpha. It fills the luma histogram (design D4) and the Shades of Gray sums of 6th
      powers over the evidence band (design D5), replacing the CIELab moments and chroma histogram of the first
      version. Verify with tests showing:
      - the stride is 1 at or below 1,048,576 pixels, and the sample count stays bounded above that;
      - a 16-bit gradient lands in more than 256 distinct luma bins;
      - a fully transparent image yields no samples;
      - a grey pixel adds equally to all three sums, and a clipped or black one adds nothing.
- [x] 3.2 Add the light signal's decision (design D4): one linear score per direction over the three percentiles, with
      starting weights as named constants. Verify with synthetic-image tests for each of the spec's light scenarios:
      -2 EV, +2 EV, correctly exposed, the night scene with lights, the subject on white, and the cut-out on
      transparent.
- [x] 3.3 Add the colour signal's decision (design D5): the Shades of Gray estimate, and one linear score over its
      angle from neutral and its chromaticity, with starting weights as named constants. Verify with synthetic-image
      tests for:
      - a warm cast and a cool cast;
      - a greyscale image (no suggestion, and in particular no hue assigned to R = G = B);
      - a colourful photograph with neutral surfaces (no suggestion);
      - a stronger cast detected wherever a weaker one of the same hue is, from a photograph not itself suggested.

## 4. Calibrate

- [x] 4.1 Add the `#[ignore]`d calibration test of design D6. It reads the dataset directory from an environment
      variable, pairs each input with its scene's correct rendering, measures the actual error (stops, or cast
      angle), and prints false-alarm rates and detection by actual error on the sample design D6 describes. The
      labelled rates (±1.5 EV, each preset) are printed for information only. The first version, which prints the
      labelled rates, exists; the pairing and actual-error rates are what remain. Verify: it runs over the task 1.1 data
      and prints every rate in design D6's table.
- [x] 4.2 Fit the weights of both signals against the calibration data, and replace the starting constants. Record the
      achieved rates in design D6 in place of the targets. If a target cannot be met, stop and report the measured
      rates before continuing (design, Risks). Verify: every rate meets its target, and the synthetic tests of 3.2 and
      3.3 still pass with the fitted values. Outcome: every target is met except light detection (66.7% under and
      64.4% over, against 80%). The rates were reported, and the 5% false-alarm budget was kept rather than loosened,
      so the lower detection ships knowingly (design D4 and D6).

## 5. Into the analysis

- [x] 5.1 Add the `Suggestion::LightAdjustment` and `Suggestion::ColorBalance` arms and their `family()`. Run the pixel
      pass inside `suggest` after geometry and before face, on the blocking pool, with the cancellation checks of
      design D2. Verify with tests showing:
      - both arms come back in `Family::ALL` order alongside face recovery and upscale;
      - a face model that will not open still returns the pixel signals' suggestions, with `incomplete` naming face
        only;
      - a cancellation during the pass returns `InferenceError::Cancelled` and no suggestions.
- [x] 5.2 Extend `every_suggestion_names_a_family_the_enhancement_path_can_apply`, `a_suggestion_names_its_own_family`
      and the order-divergence test's suggestible set to cover both new arms (design D8). Verify with
      `cargo test -p opai autopilot`.
- [x] 5.3 Log each pixel signal's measurements at `debug` (design D9). Replace the module comment's parity-gap paragraph
      with the divergences this change records, and update the module diagram. Verify with a `logging::records_of`
      test that finds the measurements at `debug` and not at `info`.

## 6. Finish

- [x] 6.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and fix
      anything they report. Verify: all three pass clean.
