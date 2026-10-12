## Context

`opai::autopilot` reads two signals today: geometry (the pixel count) and face (a detection run through `execute`).
Its module comment records light and colour as a knowingly open parity gap. See proposal.md (Why) for why the
reference's heuristics are replaced rather than ported.

What already exists and shapes the approach:

- `imaging::tensor::Sampler` reads any `DynamicImage` as 16-bit RGB with alpha premultiplied against black. It is the
  one place every model reads pixels from, and it does not expose alpha.
- `crate::task::spawn_blocking` is how `opai` moves CPU-bound work off the async executor.

The first measurements came from a scratch Go port of the reference heuristics and two prototype detectors, run over
the Kodak 24 set with synthetic exposure shifts (in linear light) and per-channel white-balance gains applied. Those
prototypes were then measured on real data (Decision 6) and did not hold up. Decisions 4 and 5 record the detectors
that replaced them, and why.

## Goals / Non-Goals

**Goals:**

- Two detectors that meet the calibration targets under Decision 6, on real renderings rather than only synthetic
  ones.
- One bounded-cost pass, shared by both detectors, that never needs a model.

**Non-Goals:**

- Choosing a variant. Telling a locally dark photograph (Paris) from a globally mis-exposed one (Lyon), or a single
  cast (Rio) from mixed illuminants (São Paulo), is a separate detector and a spec change, because a suggestion names
  only its family.
- Low-contrast or hazy photographs. Neither light adjustment variant targets them, and a suggestion the model does
  not act on is worse than none.
- Wiring autopilot into the GUI.

## Decisions

### 1. Statistics, not a model probe

Both signals are statistics over the pixels. The alternative was running Rio or Lyon as a hidden probe and suggesting
the family when the model's output differs visibly from its input. That probe would be self-calibrating and would warm
the run cache for an accepted suggestion. It was rejected because it would install and run up to two more models on
every photograph a user opens, and because the detection should be a property of the photograph, independent of any one
model the user may not even choose.

Porting the reference was rejected on the measurements in proposal.md.

### 2. One pass over a strided sample

The pass reads a regular grid of pixels, with the stride chosen so the grid holds at most 1,048,576 samples:
`stride = ceil(sqrt(pixels / 1_048_576))`, with a stride of 1 at or below that. Rounding each axis up can leave that
grid a row or a column over the bound, so the stride is then widened by one until the grid fits. Every sample is read
through `Sampler::rgb_and_alpha` (Decision 3), so 8- and 16-bit sources take one path and a 16-bit source is measured
at 16 bits.

Why a stride rather than a resize: a resize costs a full-resolution pass, which is exactly what the bound exists to
avoid, and averaging neighbours changes the very distribution being measured. Its tails, which the light signal reads
directly, get narrower. A regular subsample of a photograph with a million samples reproduces the whole-frame
histogram's percentiles and the colour sums far more closely than the thresholds need.

Every accumulator is a per-sample statistic, with no neighbour read. That is what rules out the colour detectors that
need gradients (Decision 5).

The pass runs on the blocking pool through `crate::task::spawn_blocking`. It checks the caller's cancellation token
before it starts and after it ends, so a cancellation during the pass fails the analysis as the spec requires. A pass
of a million samples is a few tens of milliseconds, so checking inside the loop is not worth its cost.

### 3. Transparent pixels are skipped, which needs alpha from `Sampler`

`Sampler` gains a method that returns the pixel's alpha alongside its premultiplied RGB. A sample whose alpha is below
half is skipped by both signals. Its premultiplied colour is mostly the black it was multiplied against, so counting it
would make every cut-out look dark.

Why at half rather than at zero: the soft edge of a cut-out is premultiplied towards black too. Anti-aliased edges are
thin, so the exact cutoff matters little, but zero would let the darkest part of that edge through.

An image with no sample left after skipping (fully transparent) concludes nothing for either signal.

### 4. Light: percentiles of a brightness histogram

The pass fills a 1,024-bin histogram of luma, computed as Rec. 709 weights over the gamma-encoded channels on
`[0, 1]`. From it the signal reads a low percentile (the darkest tail, around the 1st), the median, and a high
percentile (the brightest tail, around the 99.5th).

Each direction is decided by **one linear score** over those three values, `w0 + w1·low + w2·median + w3·high`, with
its own fitted weights, and suggests when the score is positive:

- **Underexposed:** the score rises as the brightest tail and the median fall. Even the brightest part never gets
  bright, and the photograph as a whole is dark.
- **Overexposed:** the score rises as the darkest tail and the median rise. Even the darkest part never gets dark,
  and the photograph as a whole is bright.

The weights are fitted in calibration so that the two directions together raise at most 5% false alarms, and so that
every light scenario of the spec, drawn as a synthetic image in the tests, stays on its side of the boundary with a
margin. The second condition is what keeps the median's weight in the overexposure score near zero: left free, the fit
leans on the median, and a subject on a white background then reads as overexposed, which the spec forbids.

Encoded luma rather than linear luminance because the thresholds are about how the photograph looks, and the encoded
scale is the perceptual one. 1,024 bins rather than 256 so that a 16-bit source is not quantised back to 8 bits before
its percentiles are read.

Why a score rather than the prototype's rule. The prototype required two separate cutoffs per direction: the high
percentile below 150 and the median below 70 (on the 0 to 255 scale), or the low percentile above 70 and the median
above 185. With the best cutoffs calibration could find, that shape detected only 67% of real renderings at -1.5 EV
and 50% at +1.5 EV at 5% false alarms. Scored against each rendering's actual exposure error, a flexible classifier
over twenty-three global brightness statistics reached 90% (under) and 83% (over), which shows the information is in
the statistics. A linear score over just these three percentiles reached 85% and 81%, so the fault was the shape of
the boundary, not the choice of features. Those two figures allowed 5% false alarms per direction.

Holding the two directions to 5% combined cost far more than the few points expected. Under the shared budget the best
linear score found 71% (under) and 72% (over), and even the boosted classifier over all twenty-three statistics only
76% of each: no global brightness statistic reaches 80% at that budget. The spec's scenarios cost another five to seven
points (above). Reaching 80% each would take about 10% false alarms combined. The 5% budget was kept, because the spec
prefers a missed suggestion to a wrong one, and the lower detection is what this signal achieves (Decision 6).

### 5. Colour: the Shades of Gray estimate of the light

The colour signal estimates the colour of the light the photograph was taken under, and suggests colour balance when
that estimate is far enough from neutral. The estimator is **Shades of Gray** (Finlayson and Trezzi): per channel, the
6th root of the mean of the 6th power of the linear-light value. The pass accumulates the three sums of 6th powers,
linearising each `u16` sample through a table.

A sample is evidence only when its brightest linear channel is above 0.01 and below 0.95. Below, it is too dark to
carry any colour. Above, it is clipped, and its colour is the clipping's rather than the light's. A pixel with
R = G = B adds equally to all three sums, so it pulls the estimate towards neutral rather than towards any hue.

The decision is one linear score over the estimate's angle from neutral and its chromaticity, `r / (r + g + b)` and
`b / (r + g + b)`. The chromaticity terms make it hue-aware. Correctly balanced photographs lean warm on average, so
one angle threshold for every hue is biased towards finding warm casts and missing cool ones. The weights are fitted
in calibration.

Why the estimate moves with the cast, as the spec requires. A cast is a per-channel gain in linear light, and Shades of
Gray is homogeneous: multiplying a channel by `g` multiplies its estimate by `g`. So a stronger cast of the same hue
moves the estimate further along the same direction. Only the samples leaving the evidence band as they clip or darken
can bend that, and the synthetic Kodak casts check that it does not in practice (Decision 6).

**Rejected, on calibration measurements** (share of casts over 15° detected, at 5% false alarms on correct
renderings):

| Detector | Casts over 15° |
|---|---|
| The prototype: CIELab mean shift, spread, least chromatic tenth and hue agreement, best thresholds | 59% |
| Gray World (the mean) | 60% |
| Grey pixels (Yang et al.) | 64% |
| First- and second-order Gray Edge | 68% and 73% |
| max-RGB (99th percentile) | 70% |
| **Shades of Gray, scored by angle alone** | **80%** |
| **Shades of Gray, scored by angle and chromaticity** | **85%** |
| Shades of Gray with second-order Gray Edge | 89% |
| All six estimators combined | 91% |

Gray Edge adds about four points to Shades of Gray, but it needs image gradients over a smoothed image. That means
reading a neighbourhood around each sample or resizing first, which Decision 2 rejects, for a small gain. The
prototype's least-chromatic evidence failed for a structural reason: the least chromatic samples of a cast photograph
are the ones whose own colour cancels the cast, so their mean says little about the light.

### 6. Calibration comes first, and has targets

Thresholds are fixed only after measuring them on public data with correct and incorrect renderings of the same
scenes:

- **Exposure:** Afifi et al.'s multi-scale exposure-correction set (renderings at -1.5, -1, 0, +1 and +1.5 EV), plus the
  synthetic ±2 EV Kodak variants.
- **Colour:** Afifi et al.'s Rendered-WB set (the same scenes rendered under camera white-balance presets, with the
  correct rendering as ground truth), plus the synthetic Kodak casts.

Detection is measured against **how wrong each rendering actually is**, compared with its scene's correct rendering,
not against its label. The labels are relative to what the camera assumed, not to what is correct. The EV offsets are
steps from the camera's own exposure, so a "-1.5 EV" rendering of a scene the camera overexposed can look right. A
preset names the light the camera assumed, so Tungsten on a room lit by tungsten bulbs is correct. No detector can
flag those renderings without flagging correct photographs too. The actual error is:

- **Exposure:** the difference in stops between the median linear luminance of the rendering and that of expert C's
  rendering of the same scene.
- **Colour:** the angle, in degrees, of the per-channel gain taking the correct rendering to the input. Each channel's
  gain is the median ratio over mid-tone pixels, in linear light.

The targets, and what the fitted weights achieved on the sample below:

| Measure                                                                         | Target            | Achieved |
|---------------------------------------------------------------------------------|-------------------|----------|
| False alarms on correct renderings, both light directions together              | at most 5%        | 4.6% (83 of 1,800) |
| Light: detection of renderings 1.5 stops or more under, and 1.5 or more over    | at least 80% each | **66.7% (184 of 276) and 64.4% (96 of 149): missed** |
| Light: detection of the synthetic ±2 EV Kodak variants                          | at least 80%      | 95.8% and 95.8% (23 of 24 each) |
| Colour: false alarms on correct renderings                                      | at most 5%        | 5.0% (30 of 600; Cube+ 3.7%, Set 1 6.3%) |
| Colour: detection of casts over 15°                                             | at least 80%      | 88.1% (1,019 of 1,157) |
| Colour: detection of casts between 10° and 15°                                  | reported          | 25.6% (65 of 254) |
| Colour: a stronger synthetic Kodak cast missed where a weaker one of the same hue was found, starting from an unaltered photograph that is not itself suggested | never | never (0 of 14, warm and cool) |

The light detection target is missed, knowingly. Decision 4 records why no detector of this kind can meet it at the
false-alarm budget, and that the budget was kept rather than loosened when that was measured. Detection rises with the
size of the error: 85% of renderings more than 2.5 stops under are found, and 87% of those more than 2.5 over.

Reported for information, and not counted against the targets:

- **Kodak originals.** Of the 24 unaltered scans, 4 are suggested light adjustment and 10 colour balance. The scans
  sit about 9° warm of neutral, where the correct renderings of the white-balance sets sit about 2.5° away, so a
  detector fitted on camera pipelines reads these film scans' warmth as a cast. This is the transfer risk named under
  Risks, measured.
- **Kodak casts by strength.** A warm cast is found in 92% of the scans at half the proposal's strength and in all of
  them at full strength. A cool one is found in 50% at full strength and 96% at one and a half: the hue-aware score
  flags cool estimates at a smaller angle, but the warm lean of these originals partly cancels a cool cast.
- The labelled rates (±1.5 EV, each preset) are printed by the harness.

False alarms carry the tightest target on purpose: the spec prefers a miss to a wrong first impression.

The measurements behind Decisions 4 and 5 were taken on samples of about 3,000 images per signal, which is enough to
read a rate to within a few points. The light sample was 1,200 exposure renderings with a measured error and 1,800
expert renderings. The colour sample was 2,400 Cube+ and Set 1 renderings with a measured cast and 600 correct
renderings. The fitted weights and the final rates of task 4.2 use the same sampling.

The data used, downloaded to a scratch directory on 2026-09-23 (about 4.5 GB unpacked):

| Set | Source | Licence | Subset used | Scenes |
|---|---|---|---|---|
| Exposure correction (Afifi et al., CVPR 2021) | github.com/mahmoudnafifi/Exposure_Correction, the **testing** archive (Google Drive mirror `1uxiD6-DOeLnLyI_51DUHMRxORHmUWtgz`, 1.4 GB) | Follows MIT-Adobe FiveK's: research use | All of it: `INPUT_IMAGES` at -1.5, -1, 0, +1, +1.5 EV, and the five experts' renderings as the correct ones | 1,181 |
| Rendered WB, Cube+ rendering (Afifi et al., CVPR 2019) | cvil.eecs.yorku.ca/projects/public_html/sRGB_WB_correction/dataset.html, "rendered version of the Cube+ dataset", inputs and ground truth (sync.com, 0.93 + 0.16 GB) | Research use only; a licence is needed for commercial use | All of it: the D, F, S and T presets and two auto modes, with the correct rendering | 1,707 (one camera) |
| Rendered WB, Set 1 | The same page. Ground truth without the colour chart (sync.com, 1.67 GB). Inputs from the JPEG archive without the chart (Google Drive mirror `1p8X-328dHw0KxkEgKfUHiDd-sV1e0kKV`, 4.8 GB), read by HTTP range request so only the sample was fetched | As above | Every scene, one picture style per scene (chosen by a hash of the scene name), all five presets (C, D, F, S, T): 9,663 inputs | 1,991 (six camera families) |
| Kodak 24 | r0k.us/graphics/kodak | Released by Kodak for unrestricted use, per its host; no formal licence | All 24, with the synthetic variants made in the harness | 24 |

Set 2 of Rendered WB, the multi-camera test set, was not used: its two archives are 11.6 GB together, more than the
machine had free. Set 1's sample covers more cameras anyway. Neither set's current licence was confirmed beyond its own
page and README.

The harness is an `#[ignore]`d test beside the signals. It reads a dataset directory named by an environment variable
and prints each rate. It is reproducible by anyone who downloads the sets, and it commits no data. The data stays in a
scratch directory.

### 7. No Lab conversion: it stays with colorization

The first version of this design converted every sample to CIELab, and so moved `models/colorization/lab.rs` to
`imaging::lab` as a second caller arrived. That move was made. Shades of Gray (Decision 5) works in linear RGB and
needs no Lab, which leaves colorization the conversion's only caller again. Under the tier rule, a shared file left
with one caller moves back down, so the move is undone. The forward conversion from `u16` samples that was added for
autopilot, `sample_to_lab`, is removed with it.

The linearisation autopilot does need is the sRGB transfer, a `u16`-keyed table of its own. It is not shared with
colorization's because the two want different outputs: colorization's table is in the form its bit-for-bit tests pin,
and autopilot only needs the 6th power of the value.

### 8. Where the signals sit in the analysis

The order of evaluation stays "cheapest first": area, geometry, then the pixel pass (both signals), then face.
Suggestions are still sorted into `Family::ALL` order afterwards, so the new arms need no placement logic.

`Suggestion` gains two unit arms, `LightAdjustment` and `ColorBalance`. Neither carries a parameter. The direction of
an exposure error is something Lyon decides for itself, and the spec keeps the variant out of a suggestion. The
existing tests change as follows:

- `every_suggestion_names_a_family_the_enhancement_path_can_apply` lists both new arms and builds an operation for
  each.
- The order-divergence test's suggestible set grows to face recovery, light adjustment, colour balance and upscale.
  The relative order of those four is the same in `Family::ALL` and in the reference's list, so its assertion still
  holds.

The module comment's "knowingly open parity gap" paragraph is replaced with the deliberate divergences recorded here.

### 9. The measurements are logged

At `debug`, the analysis records what each pixel signal measured: the three percentiles, the Shades of Gray estimate's
angle from neutral and its chromaticity, each signal's score, and the sample count. A user report that a photograph was given a wrong suggestion is then diagnosable from the log without
the photograph. `debug` rather than `info`, because the conclusion is already at `info` and the measurements are for
whoever investigates it.

### Crates and platforms

- **`opai`:** the signals, the pass and the suggestion arms. **`imaging`:** the alpha accessor.
  **`gui` and `cli`:** unchanged.
- **`rust-sak`:** no feature applies. **New crates:** none.
- This is pure CPU arithmetic with no model, session or execution provider involved. It is validated on macOS
  (Apple silicon) and is platform-independent. Windows and Linux are covered by the test suite in CI only.

### Slices

Not sliced. The work is estimated at 12 tasks and delivered as this one change.

## Risks / Trade-offs

- [The fitted detectors miss their targets once false alarms are held to 5% for both light directions together, most
  likely overexposure] → Stop and bring the measured rates back before implementing further, rather than shipping a
  detector that misses its targets or quietly loosening them. This already happened once, and led to Decisions 4
  and 5 as they now stand. It happened again in task 4.2, for light: the rates were brought back, and the decision was
  to keep the 5% budget and ship the lower detection (Decision 6).
- [The weights are fitted on data licensed for research use only] → The same position as fitting thresholds: a
  handful of numbers is shipped, never the data or a model trained on it.
- [The datasets' renderings come from a few cameras' pipelines, so the thresholds may not transfer to phone
  photographs] → Include the Kodak set (film scans) as a second source, and keep the per-signal measurements in the
  debug log so a wrong suggestion in the field can be traced. The Kodak scans show the risk is real: 10 of the 24
  unaltered scans are suggested colour balance and 4 light adjustment (Decision 6), so the thresholds do not transfer
  cleanly even to a second source. The weights are not refitted here, because fitting them to 24 film scans would
  trade measured detection on thousands of camera renderings for one small set. Measuring both signals' false alarms
  on correctly exposed and balanced phone photographs is a precondition of showing autopilot's suggestions in the GUI,
  and a refit, if one is needed, belongs to that work.
- [Intentional colour grading (warm film looks, teal-and-orange) reads as a cast] → Accepted. The suggestion is only
  a suggestion, and the user can remove it.
- [A stride can alias with a regular pattern in the photograph, such as a fine grid or a halftone] → The statistics
  are distributions, not structure, so aliasing shifts them only when the pattern dominates the frame. Accepted.
- [Behaviour diverges from the Go app on many photographs] → Deliberate, and recorded in the proposal with the
  measurements that justify it.
