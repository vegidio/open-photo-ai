## 1. Calibration data

- [x] 1.1 Locate the sets of design D6 and download only their sampled images into the session scratch directory, never
      into the repository: at most 3,000 images from any dataset, sampled by a hash of the image name, by range request
      where an archive allows it. Confirm the real black-and-white scan sources (Rijksmuseum CC0, Library of Congress)
      and the hard-set candidates (RESIDE RTTS, ExDark). Replace D6's source lists with a data table giving each set's
      URL, licence, subset used and image count, and if a set is unavailable, amend D6 to its fallback and say so.
      Verify: the directory holds real black-and-white scans scanned in colour, and the colour sets, and no dataset's
      sample exceeds 3,000 images.

## 2. The signal

- [x] 2.1 Add the `Levels` accumulator to the strided pass (design D2, D3): 32 encoded-luma levels, each with the count
      and the sums of `B - Y`, `R - Y` and their squares, over opaque samples with no clipped channel. Verify with tests
      in `pass.rs` showing that a gray sample adds nothing to either chroma sum, a clipped or transparent sample adds
      nothing at all, and a 16-bit source is read at more than 8 bits.
- [x] 2.2 Add the monochrome signal in `autopilot/monochrome.rs`: the pooled within-level residual chroma, `None` below
      1,000 usable samples, one score and a starting constant as a named value. Verify with synthetic-image tests for
      each spec scenario: neutral gray, a single-channel file and a grayscale photograph saved as a colour JPEG are
      suggested; a colour photograph, a muted colour scene with a small coloured subject, and a cut-out on transparency
      are not; and a less colourful version is suggested wherever a more colourful one of the same image is. (Revised
      after calibration: toned, split-toned and noisy prints are no longer required, per design D2, Outcome.)

## 3. Calibrate

- [x] 3.1 Extend the `#[ignore]`d harness in `autopilot/calibration.rs` for the monochrome signal (design D6): build the
      synthetic tones and noise levels, compute the colourfulness metric to select the hard set, print every row of
      D6's target table, and print by name any negative whose residual is below the synthetic noise floor. Verify: it
      runs over the task 1.1 sample and prints every row of the table.
- [x] 3.2 Measure the challenger of design D2 at the 1% budget: the fraction of samples above a fixed residual against
      the pooled standard deviation. Record the result in the design, and keep the default unless the challenger gains
      at least 3 points of detection. Verify: the design records the comparison, and the code holds only the option that
      won.
- [x] 3.3 Inspect the negatives flagged in 3.1 and record any removed. Fit the constant, and replace the targets in D6
      with the achieved rates. If a target cannot be met at the 1% budget, stop and report the measured rates before
      continuing (design, Risks). Verify: every target is met or knowingly reported as missed, and the tests of 2.2
      still pass with the fitted constant. (Both detection targets were missed and reported; the decision was to keep
      the budget and set the constant for grayscale photographs only, design D2, Outcome.)

## 4. Into the analysis

- [x] 4.1 Reorder `Family` and `Family::ALL` to put `Colorization` before `LightAdjustment` (design D5), and replace
      `the_order_suggestions_come_back_in_diverges_from_the_reference_on_colorization_alone` with a test asserting that
      the two orders agree over the families a user can add. Verify with `cargo test -p opai models` and
      `cargo test -p gui catalogue`.
- [x] 4.2 Add the `Suggestion::Colorization` arm and its `family()`, and add the `monochrome` field to `PixelSignals`
      with its conclusion in `suggestions()`. Verify with tests showing:
      - an underexposed grayscale photograph gets colorization and light adjustment, colorization first, and not colour
        balance (revised after calibration: with the grayscale-only constant, colorization and colour balance cannot
        fire together, design D4);
      - a face model that will not open still returns the colorization suggestion, with `incomplete` naming face only;
      - a cancellation during the pass returns `InferenceError::Cancelled` and no suggestions.
- [x] 4.3 Extend `every_suggestion_names_a_family_the_enhancement_path_can_apply`, `a_suggestion_names_its_own_family`
      and the public API test in `crates/opai/tests/public_api.rs` to cover the new arm. Verify with
      `cargo test -p opai`.
- [x] 4.4 Log the usable-sample count, the residual chroma and the score at `debug` (design D7). Replace the module
      comment's line that colorization has no signal, and add the signal to the module diagram. Verify with a
      `logging::records_of` test that finds the measurements at `debug` and not at `info`.

## 5. Finish

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and fix
      anything they report. Verify: all three pass clean.
