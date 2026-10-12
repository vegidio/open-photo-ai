## Context

`opai::autopilot` reads five signals: area, geometry, light, colour and face. Light and colour share one pass
(`autopilot/pass.rs`) over a strided grid of single pixels, at most 1,048,576 of them, with no neighbour read. See
proposal.md (Why) for why noise and sharpness are added, and why neither is ported.

What already exists and shapes the approach:

- `imaging::tensor::Sampler::rgb_and_alpha` reads any `DynamicImage` as 16-bit RGB, premultiplied against black, with
  its alpha. Nothing new is needed from `imaging`.
- The light and colour change (archived as `2026-09-23-add-autopilot-light-color`) set the method this change follows:
  statistics rather than a model, one bounded pass, detectors fitted on public paired data against each image's
  **actual** error, fixed targets, and an `#[ignore]`d calibration harness in `autopilot/calibration.rs`.
- Both new families run today, so the suggestibility rule admits them.

## Goals / Non-Goals

**Goals:**

- Two detectors that meet the calibration targets under Decision 6, including on phone photographs.
- One bounded-cost pass at full resolution, shared by both detectors, that never needs a model.

**Non-Goals:**

- Choosing a variant. Telling motion blur (Novgorod, St. Petersburg) from defocus (Moscow) is possible, from the
  direction the blur runs in, but a suggestion names only its family. It is a later detector and a spec change, as
  choosing Paris or Lyon was for light.
- Rain streaks. Malmö removes them, but detecting rain is a different problem from grain.
- Compression artefacts as their own fault. They are handled only so far as they must not read as grain (Decision 3).
- Wiring autopilot into the GUI.

## Decisions

### 1. Statistics, not a model probe

For the reasons the light and colour design gave: a probe (running Stockholm or Moscow on a crop and measuring how much
it changes) would install and run up to two more models on every photograph a user opens. The alternative of a learned
no-reference quality model (BRISQUE, NIQE, or a network) was rejected for the same cost, and because none is trained
for "would the denoise or sharpen model help here".

### 2. A second pass, over full-resolution blocks

The strided grid cannot be reused. Grain and blur exist only between neighbouring pixels, and a stride skips the
neighbours. A resize is worse: averaging removes grain and makes blur look sharp.

```
 light / colour pass                   noise / sharpness pass
 +--------------------------+          +--------------------------+
 | .   .   .   .   .   .    |          | [##]      [##]      [##] |
 | .   .   .   .   .   .    |          |      [##]      [##]      |
 | .   .   .   .   .   .    |          | [##]      [##]      [##] |
 +--------------------------+          +--------------------------+
  one pixel in N, no neighbours         full-resolution blocks on a
                                        regular grid, same budget
```

The pass reads square **blocks** of 64 by 64 pixels at the photograph's own resolution, on a regular grid spread over
the whole frame, at most 256 of them: 1,048,576 pixels, the budget the strided pass already has. Below that budget,
the grid tiles the frame and every whole block is read. Partial blocks at the edges are not read. A photograph with no
whole block concludes nothing for either signal.

- **Block origins are multiples of 8.** A JPEG's 8x8 compression blocks then sit at known positions in every block
  (Decision 3).
- **A regular grid, not random blocks.** The same photograph always gives the same answer, which the run cache and
  the tests both need.
- **64 is a starting size.** Large enough for the sharpness measure's 9-tap re-blur and a few JPEG blocks. Small enough
  that a subject covers several blocks. The harness compares 32 and 64 at the same pixel budget (1,024 blocks against
  256), and the size with better detection at the false-alarm budget ships. **Measured: 64 ships.** For noise both
  sizes detected 99.6% of the real pairs with visible grain; for sharpness 32 detected 59.8% of genuinely blurred
  images against 64's 63.7%.
- **A block with any sample below half alpha is skipped whole.** Its transparent edge would read as a hard edge to one
  signal and as a smooth area to the other.

Every sample is read through `Sampler::rgb_and_alpha`, so a 16-bit source is measured at 16 bits. It is converted to
gamma-encoded **luma** (Rec. 709 weights, as the light signal uses) and two **chroma** differences, `B - Y` and
`R - Y`, as `f32` on `[0, 1]`. Encoded rather than linear, because both signals are about what a person sees.

Both passes run inside the one `spawn_blocking` the pixel signals already use, with the same cancellation checks before
and after. The block pass does a few filter taps per pixel over a million pixels, so it adds a few tens of
milliseconds.

### 3. Noise: the Immerkær estimate over the flattest blocks

Per block and per channel (luma and both chroma), the signal computes **Immerkær's fast noise estimate** (1996): the
mean absolute response of the 3x3 kernel `[1 -2 1; -2 4 -2; 1 -2 1]`, scaled by `sqrt(pi/2) / 6`. The kernel is zero
on constant regions, linear ramps and smooth curvature, so what it responds to is mostly noise. Two exclusions:

- **Positions whose 3x3 window crosses an 8-pixel boundary are skipped.** On a JPEG, the compression block edges would
  otherwise add a response that is not grain. About 44% of positions are skipped, which still leaves over 2,000 per
  block. The harness measures the estimate with and without the exclusion, and the exclusion is dropped if it makes no
  difference. **Measured: dropped.** With and without it, 99.6% of the real pairs with visible grain were detected at
  the 5% budget, over JPEG and PNG sources alike. Block origins stay on the 8-pixel grid; it costs nothing.
- **Positions touching a clipped sample are skipped.** Clipping removes noise, so a block of blown sky would
  otherwise read as clean.

On a textured block the estimate also responds to texture, so only the **flattest** blocks are read. Each block's
activity is its mean luma gradient magnitude. The signal takes the flattest tenth of the blocks, and at least 4 of
them. With fewer than 4 blocks it concludes nothing. From those blocks it reads three values: the median luma noise, the
median chroma noise (the root-sum-square of the two chroma estimates), and the median luma. The luma matters because
sensor noise grows with brightness, so the same noise in shadows and mid-tones looks different. Reading it is the
low-cost form of a full noise-level function.

The decision is **one linear score** over those three values, `w0 + w1·luma_noise + w2·chroma_noise + w3·median`,
fitted in calibration. It suggests when the score is positive. The shape is the one the light signal settled on.

**Challenger, measured in the harness only:** Chen, Zhu and Heng's PCA estimator (ICCV 2015), over the same flattest
blocks. It reads noise from the smallest eigenvalues of the covariance of the 5x5 patches in a block, and on synthetic
noise it is more accurate than Immerkær's estimate. Real noise has been smoothed by demosaicing and compression, which
narrows that lead, so it has to be measured. The harness computes the eigenvalues with a hand-written Jacobi solver
over a 25x25 matrix, so no linear-algebra crate is added. PCA replaces Immerkær only if it detects at least 3 points
more at the same false-alarm budget. **Measured: not taken.** PCA detected 99.5% of the real pairs with visible grain,
Immerkær 99.6%. Its code was removed from the harness once measured.

**The fitted score** is `-0.782 + luma_noise + 0.6·chroma_noise - 4.5·median`. An unconstrained fit reached the same
detection with most of its weight on chroma and a positive weight on the median, and then read the colour edges of a
detailed clean photograph in the tests as grain. The weights were therefore searched with every spec scenario held on
its side with a margin of 0.5, and the constant is set by that margin rather than by the 5% budget, so false alarms
land below it. The median weighs against suggesting: at equal measured noise, grain shows most in shadows.

**Rejected without measuring:**

| Method | Why not |
|---|---|
| Wavelet median absolute deviation (Donoho) | The same information as the Immerkær estimate, plus a transform |
| A mean over all blocks | Reads texture as grain, which the spec forbids |
| A full noise-level function (Liu and Freeman 2006) | Many more parameters than the calibration data can support; the median luma term is its low-cost form |

### 4. Sharpness: the Crété-Roffet blur effect, pooled at the sharpest blocks

Per block, the signal computes the **blur effect** of Crété-Roffet et al. (2007) on luma. It blurs the block with a
9-tap box filter along each axis. Along each axis it sums the absolute neighbour differences of the original, `D_F`,
and how much of each difference the re-blur removes, `max(0, D_F - D_B)`. The block's blur is the larger of the two
axes' `1 - removed / D_F`, on `[0, 1]`. Its sharpness is `1 - blur`. A sharp block loses much of its detail when
re-blurred, and a blurred block has little left to lose. Because the measure is a ratio, it depends far less on the
block's content than the variance of the Laplacian does.

**Grain is gated out.** A position counts only where its difference on a 3x3-smoothed copy of the block exceeds
`k·σ`. Here `σ` is the photograph's luma noise from Decision 3, and `k` is fitted in calibration, starting at 1.5. The
smoothing divides grain by three and barely touches an edge, so the gate keeps edges, however soft, and drops the
differences that are only grain. At the positions that count, the ratio is Crété-Roffet's over the block's own
differences. A blurred photograph with grain is then judged on its blur (the spec's "blurred photograph with grain in
it").

This replaces the discount first planned, which reduced every difference by `k·σ` before the sums, clamped at zero.
Measured on the synthetic fixtures during implementation, no `k` worked. A 4-pixel defocus reads 0.29 sharpness clean.
With strong grain added, it read 0.81 at `k = 0`, 0.99 to 1.00 at `k` of 1 and 2, and had no block with detail left from
3 up; a sharp photograph reads 0.89. The grain differences that survive a discount are removed entirely by the re-blur,
which is what sharp detail looks like, and a discount large enough to remove them removes a soft edge too. With the
gate, the same photograph reads 0.49 at `k = 1` and 0.41 at `k = 2`, and grain on a sharp photograph leaves it at 0.80
to 0.83.

**Only blocks with detail count.** A block's contrast is its neighbour differences summed over the positions that
count, in all four directions, and averaged over every position, gated or not. A block whose contrast falls below a
contrast minimum has nothing to be sharp or soft about: sky, wall or fog. With fewer than 4 blocks left, the signal concludes nothing
(the spec's "photograph with no detail").

**Pooled at the 90th percentile, not the mean.** The spec's rule is that even the sharpest part is soft, the same
shape as the light signal's "even the brightest part never gets bright". A portrait with a blurred background has a
few sharp blocks on its subject, so its 90th percentile is high and it is not suggested sharpen. A shaken or misfocused
photograph has no sharp block anywhere.

The decision is one linear score over the 90th-percentile sharpness and the base-2 log of the pixel count, fitted in
calibration. The pixel-count term exists because, pixel for pixel, a photograph from a high-resolution camera is softer
than a small one. The spec forbids suggesting sharpen for that softness alone, and a fixed threshold would suggest it
for most photographs from a modern phone.

**Challenger, measured in the harness only:** CPBD (Narvekar and Karam 2011). It finds edges, measures their widths,
and turns each width into the probability that a person would notice the blur at that contrast. It is pooled per block
the same way. It is strong on the synthetic blur benchmarks and more code. It replaces Crété-Roffet only if it detects
at least 3 points more at the same false-alarm budget.

**Measured: not taken, by decision.** CPBD, as implemented in the harness (horizontal edge widths as published, edges
found by Sobel rather than Canny), detected 72.1% of genuinely blurred images to Crété-Roffet's 63.7%, which passes the
rule above. The lead was entirely synthetic: CPBD found 51 of 73 synthetic 2-pixel defocus images to 23, and 28 of 47
synthetic 5-pixel shakes to 3, the mildest blur in the sample. On the real pairs Crété-Roffet was ahead on every set:
116 of 139 GoPro images to 98, 39 of 91 RealBlur to 29, 9 of 31 DPDD to 4, or 63% to 50%. False alarms were alike. The
rule was set aside for this comparison and Crété-Roffet kept, because real photographs are what a user opens. Their
mean was measured too: 72.3% overall and 58.6% on the real pairs, so it gained nothing on real photographs over
Crété-Roffet alone and would run both. It was not taken.

**Four directions, not two axes.** Crété-Roffet reads the two axes. Along an axis at an angle to a shake, the structure
across the shake is still crossed at full resolution, so an oblique shake reads sharp: a 25-pixel shake at 30 degrees
over the Kodak set read 0.67 along the axes, against 0.37 for a 5-pixel defocus. The blur is therefore read along both
diagonals as well, and the most blurred of the four directions kept, so that no shake is more than 22.5 degrees off one.
Measured at the 5% budget, four directions detected 62.9% of genuinely blurred images and the axes 58.3%.

**The fitted values** are `k = 1.5`, a contrast minimum of 0.5, and the score `0.4509 - sharpest + 0.0077·log2(pixels)`.
`k = 1` detected 0.8 points more, within the sample's noise, but let grain move a blurred photograph in the tests more
than a third of the way towards a sharp one. A contrast minimum of 0.25 detected 1.5 points more, but let a steep smooth
gradient count as detail and read as blurred, which put a scenario in the tests on the wrong side. The pixel-count term
came out small and positive rather than negative as expected: once the sharpest blocks are read, larger photographs in
this sample were not softer pixel for pixel, and SPAQ's 12-megapixel phone photographs are flagged at 2.7% to 5.1%.

**Rejected without measuring:**

| Method | Why not |
|---|---|
| Variance of the Laplacian (OpenCV's usual blur check) | Unbounded and content-dependent: plain scenes read as blurred, and grain inflates it |
| A mean or median over blocks | Suggests sharpen for every photograph with a blurred background |
| Spectral or phase-coherence measures (S3, LPC-SI) | Several times the cost per block, for gains reported on synthetic blur |

### 5. Where the signals sit in the analysis

The order of evaluation stays cheapest first: area, geometry, the pixel signals (both passes, one blocking task), then
face. Suggestions are still sorted into `Family::ALL` order afterwards.

`Suggestion` gains two unit arms, `Denoise` and `Sharpen`, with no parameter: which denoiser to use, and whether the blur
came from motion or focus, are the user's choice. The tests change as follows:

- `every_suggestion_names_a_family_the_enhancement_path_can_apply` and `a_suggestion_names_its_own_family` cover both
  arms.
- The order-divergence test's suggestible set grows to six families. Denoise leads both `Family::ALL` and the
  reference's list, and sharpen sits just before upscale in both, so the assertion that suggestible families keep one
  relative order still holds. Colorization remains the only disagreement.

The module comment's line that no signal is invented for denoise or sharpen is replaced with this change's
divergence, and the module diagram gains the block pass.

### 6. Calibration comes first, and has targets

Thresholds are fixed only after measuring them on public data holding noisy and clean, or blurred and sharp, versions
of the same scenes. As before, detection is measured against **how wrong each image actually is**, not its label: a
low-ISO pair in a noise set can be clean, and a blur-set frame can be caught while the camera was still.

**At most 3,000 images are read per signal.** Larger sets are sampled by a hash of the image name, and only the sampled
images are fetched, by range request where an archive allows it. Each signal's sample is roughly one third damaged
images, one third clean or sharp ones, and one third SPAQ. The one SPAQ sample serves both signals.

**The actual error:**

- **Noise:** the standard deviation of the difference between the noisy image and its clean version, on the 0 to 255
  scale, over the clean version's flattest blocks (Decision 3's selection). It is computed for luma and for chroma.
  Visible grain is a luma residual of at least 4, or a chroma residual of at least 6. Clean is below 1.5 for both. The
  band between is reported but not scored. These cut-offs are starting values chosen as visible at 100% zoom. Before
  fitting, they are checked against the SPAQ photographs people rated noisiest and least noisy, and moved if they
  split those groups badly. **Measured: the check could not be made, and the cut-offs stand.** SPAQ's noisiness scores
  do not track the estimate at all (a correlation of -0.01 with the luma noise, 0.11 with the chroma), nor its
  sharpness scores the 90th-percentile sharpness (-0.02). Phone processing has smoothed away most of the grain and
  fine detail people would have scored, so their scores follow something else. SPAQ still measures false alarms on the
  photographs rated best, as the targets ask.
- **Blur:** the detail kept. For each of the sharp version's blocks with detail, it is the ratio of the blurred
  version's luma gradient energy to the sharp version's, over the same aligned block. The detail kept is the 90th
  percentile of those ratios, so that only an image blurred everywhere counts. Genuinely blurred is at most 0.5. Sharp
  is at least 0.85, and every sharp version is 1 by definition. The band between is reported.

The sharpness detection rates quoted in Decisions 2 and 4 (63.7%, 62.9%, 58.3%) were each measured when that
comparison was made, at the 5% budget with the signal as it stood then, so they differ from one another and from the
final fit below. Each is only comparable with the alternative it was measured beside.

**The targets, and what the fitted values achieved** on the sample below:

| Measure | Target | Achieved |
|---|---|---|
| Noise: false alarms on clean images | at most 5% | 3.4% (45 of 1,317) |
| Noise: detection of visible grain | at least 80% | 96.0% (580 of 604); real pairs alone 90.2% (220 of 244) |
| Noise: false alarms on the third of SPAQ rated least noisy | at most 5% | 1.8% (6 of 333) |
| Sharpness: false alarms on sharp images | at most 5% | 2.7% (31 of 1,150) |
| Sharpness: detection of genuinely blurred images | at least 80% | **62.6% (602 of 961); real pairs alone 63.2% (165 of 261): missed** |
| Sharpness: false alarms on the third of SPAQ rated sharpest | at most 5% | 3.9% (13 of 333) |
| Either: a stronger synthetic degradation missed where a weaker one of the same kind was found | never | Grain 0 of 118, defocus 0 of 118; **shake 2 of 118: missed** |
| Detection in the band between clean and damaged, and on SPAQ's worst-rated third | reported | Noise: band 49.0%, SPAQ noisiest third 2.7%. Sharpness: band 41.6%, SPAQ softest third 2.7% |

Both sharpness misses are knowing, and the false-alarm budget was kept rather than loosened, as for light. **Accepted
on 2026-09-23:** the rates were reported and the change continued with both misses, including the shake cases that
fall outside the spec's "Stronger blur is not missed where weaker blur is found".

- **Detection.** No detector measured reaches 80% at the budget (Decision 4). By set, 72.4% of GoPro's motion-blurred
  frames are found, 36.4% of RealBlur's camera shake, and 18.0% of DPDD's defocus, most of whose photographs have part
  of the scene in focus, the intentional case the spec protects. Detection rises with the blur: every synthetic defocus
  of 5 pixels is found. The cut-off at a detail kept of 0.5 is strict. A 1-pixel synthetic defocus, barely visible at
  100%, already keeps a median of 0.65, and 10 of 120 keep 0.5 or less, so part of what is counted as missed is
  blur the spec asks not to suggest for. It was not moved, because it is a target.
- **Monotonicity.** Two photographs are found at a 9-pixel synthetic shake at 30 degrees and missed at 15. A longer
  shake flattens more blocks below the contrast minimum, and those left are the ones whose edges run along the shake,
  which stay sharp, so the 90th percentile can rise. A contrast minimum of 1 removes it and halves detection; of 0.25
  it leaves one, and has the gradient problem above. Defocus, which blurs every direction, has none.
- **Reported, not scored.** 25% (4 of 16) of the unaltered Kodak scans are suggested denoise: they are film scans, and
  their grain is real. 5.6% of the blurred versions whose detail kept is 0.85 or more are suggested sharpen; most are
  DPDD photographs whose in-focus part carries the percentile.

Every spec scenario, drawn as a synthetic image in the tests, must also stay on its side of the boundary with a margin.
As in the light fit, that condition can outrank a few points of detection, and it did, for both signals (Decisions 3
and 4).

**The data**, downloaded to a scratch directory on 2026-09-23 (9.9 GB). Only the sampled files were fetched, by HTTP
range request into each archive, and no archive was kept. Each set was sampled by the SHA-1 of the image or scene name.

| Set | Source | Licence | Subset used | On disk |
|---|---|---|---|---|
| SIDD (Abdelhamed et al., CVPR 2018) | SIDD-Medium sRGB, the official mirror `130.63.97.225/share/SIDD_Medium_Srgb.zip` (13.2 GB) | MIT, per its page | 280 of the 320 noisy and clean pairs; the server stalled on the last 3. Each pair is centre-cropped to 2048x2048, the same crop for both, as lossless PNG | 277 pairs |
| PolyU real-world noise (Xu et al. 2018) | github.com/csjunxu/PolyU-Real-World-Noisy-Images-Dataset, `OriginalImages/` | Non-commercial: teaching, academic research | All 40 full-resolution pairs of real and mean images | 40 pairs |
| MIT-Adobe FiveK, expert C | Already on disk from the light and colour calibration: the exposure-correction testing set's expert C renderings, about 1 MP | Research use | The clean and sharp controls, and the base for synthetic grain and blur | 1,181 |
| Kodak 24 | Already on disk, as above | Unrestricted, per its host | As FiveK | 24 |
| RealBlur-J (Rim et al., ECCV 2020) | Hugging Face `LRX922/RealBlur` (5.6 GB zip). Its test split matches the official list exactly, and 42 files were checked byte-identical to the official 13.1 GB tar | CC BY 4.0 | 250 of the 980 test pairs | 250 pairs |
| GoPro (Nah et al., CVPR 2017) | Hugging Face `snah/GOPRO_Large`, the author's own account (9.5 GB zip) | CC BY 4.0 | 250 of the 1,111 test frames, 1280x720 | 250 pairs |
| DPDD (Abuolaim and Brown, ECCV 2020) | Hugging Face `JacobLinCool/DPDD`; the official 16.9 GB zip on sync.com cannot be read by range. Its pairs match the official split lists exactly | MIT (the code repository); no separate dataset licence found | The combined-view defocused input against the all-in-focus target, all of test (76) and validation (74), 16-bit 1680x1120 | 150 pairs |
| SPAQ (Fang et al., CVPR 2020) | Hugging Face `AaronCIH/IQA_dataset`, `SPAQ/` (34.8 GB split zip); the official Google Drive folder returns 404 | Research use only, non-commercial, per the bundled readme; not confirmed further | 1,000 of the 11,125 photographs at full resolution, with the MOS and attribute scores (noisiness and sharpness on 0 to 100) | 1,000 |

The noise signal reads 2,634 images (the 317 pairs, 1,000 clean photographs and the SPAQ sample) and the sharpness
signal 2,800 (the 650 pairs, 500 clean photographs and SPAQ), both under the cap; the harness asserts it. Synthetic grain and blur are
made in the harness from the first 120 clean photographs; they add no images read. No set was unavailable, so no part
falls back to synthetic data.

The harness extends the existing `#[ignore]`d test in `autopilot/calibration.rs`: a dataset directory named by an
environment variable, printed rates, no committed data.

### 7. The measurements are logged

At `debug`, the analysis records what the block pass measured: the block count, the luma noise, the chroma noise and
the median luma of the flattest blocks, the number of blocks with detail, the 90th-percentile sharpness, and each
signal's score. As with light and colour, a report of a wrong suggestion is then diagnosable from the log alone.

### Crates and platforms

- **`opai`:** the block pass, both signals, the two suggestion arms and the harness. **`imaging`, `gui` and `cli`:**
  unchanged.
- **`rust-sak`:** no feature applies. **New crates:** none, including in dev-dependencies (the PCA challenger's
  eigen-solver is written in the harness).
- This is CPU arithmetic with no model, session or execution provider involved. It is validated on macOS (Apple
  silicon) and is platform-independent. Windows and Linux are covered by the test suite in CI only.

### Slices

Not sliced. The work is estimated at 12 tasks and delivered as this one change.

## Risks / Trade-offs

- [A target is missed at the 5% false-alarm budget, most likely sharpness detection on phone photographs] → Stop and
  report the measured rates before continuing. The budget is not loosened without a decision, as with light.
- [At 256 blocks on a 50-megapixel photograph, the blocks cover about 2% of the frame, so a small sharp subject can fall
  between them and a deliberately blurred background read as a blurred photograph] → The 32-pixel comparison in
  Decision 2 quadruples the positions sampled. DPDD and SPAQ measure the effect. If it shows, reading more positions of
  a smaller block is the lever, not a larger budget.
- [Photographs from phone processing are smoothed, with little noise and little fine detail, which neither signal fires
  on] → Accepted. Neither model restores texture that processing removed. SPAQ measures how many there are.
- [Heavy compression or strong in-camera sharpening reads as grain or as sharpness] → The 8-pixel exclusion handles
  JPEG blocking. Sharpening halos are accepted: a photograph sharpened in camera is not asking to be sharpened again.
- [Film grain and intentional motion blur (panning, light trails) read as faults] → Accepted, as with intentional
  colour grading. A suggestion is only a suggestion.
- [Calibration data is licensed for research use only] → A handful of weights is shipped, never the data.
- [The reading of the 3,000-image cap as per signal is an assumption] → If it is meant for both signals together, each
  signal's sample is halved to 1,500, which reads a 5% rate to within about ±2 points rather than ±1.5.
