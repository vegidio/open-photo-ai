## Context

`opai::autopilot` reads seven signals: area, geometry, light, colour, noise, sharpness and face. Light and colour share
one pass (`autopilot/pass.rs`) over a strided grid of at most 1,048,576 single pixels, read through
`imaging::tensor::Sampler::rgb_and_alpha` at 16 bits, skipping mostly transparent samples, with no neighbour read. Noise
and sharpness share a second pass over at most 256 full-resolution blocks. See proposal.md (Why) for why colorization is
added, and why nothing is ported.

What already exists and shapes the approach:

- The light and colour, and noise and sharpness changes (archived as `2026-09-23-add-autopilot-light-color` and
  `2026-09-23-add-autopilot-noise-sharpness`) set the method this change follows. It uses statistics rather than a model,
  a bounded pass, a detector fitted on public data against each image's **actual** state, fixed targets, and the
  `#[ignore]`d harness in `autopilot/calibration.rs`.
- The colorization pipeline replaces the photograph's chroma with the model's prediction (the colorization spec). This
  is why a false alarm is expensive, and why the budget here is 1% rather than the 5% the other signals were fitted to.
- `the_order_suggestions_come_back_in_diverges_from_the_reference_on_colorization_alone` pins the one place `opai`'s
  family order disagrees with the reference, and says the change that makes colorization suggestible settles it.
- CIELab exists only in `models/colorization/lab.rs`, `pub(super)` to that family and pinned bit for bit.

## Goals / Non-Goals

**Goals:**

- One detector that meets the targets under Decision 6, including on a hard set of muted colour photographs. **Met for
  grayscale photographs only**: toned and colour-scanned prints could not be found at the budget (Decision 2, Outcome).
- No new pass: the signal rides the strided pass the light and colour signals already pay for.

**Non-Goals:**

- Black-and-white content inside colour it does not own (frames, mats, stamps, captions). See proposal.md.
- Choosing a colorizer variant. The suggestion names the family.
- Hand-tinted or "colour splash" photographs. They carry colour of their own, and are left alone by design.
- Wiring autopilot into the GUI.

## Decisions

### 1. Statistics, not a model

A probe that ran a colorizer and measured how much it changed the photograph would install a multi-gigabyte graph and
spend seconds on every photograph a user opens. The colorizer is also the most expensive model the application ships
(the reference's registry keeps it resident for that reason). A learned black-and-white classifier was rejected too.
None is trained for "toned or noisy scan counts as monochrome", and it would be one more model to install. What
decides the question is a property of the pixels that can be measured directly (Decision 2).

### 2. The measure: colour that brightness does not explain

A black-and-white photograph, however it was toned or aged, has a colour that is a **function of its brightness**:
every pixel of a given lightness carries about the same tint. A colour photograph breaks that. A red door and a green
hedge of equal brightness have different chroma.

```
 colour photograph                     toned black-and-white
 chroma at one brightness level        chroma at one brightness level

   Cr  .    x   o                        Cr
       |  x   o  +                          |     ooo
       | x  .  +                            |     ooo   <- one tight cluster per level,
  -----+--------- Cb                   -----+--------- Cb    wherever it sits
       |   +  .                             |
   wide spread -> colour of its own     tight -> a tint, not colour
```

The signal bins every usable sample by its encoded Rec. 709 luma into **32 levels**. For each level it keeps the count
and the sums of `B - Y`, `R - Y` and their squares. The **residual chroma** is the pooled within-level standard
deviation: the root of the count-weighted mean of each level's variance in both differences. A neutral gray, a sepia, a
cyanotype and a split-tone (whose hue changes with brightness) all leave only noise. Colour of its own leaves more.

- **A usable sample** is opaque enough (the pass's existing rule) and has no channel within half an 8-bit step of
  either end of its range. Clipped samples have their chroma forced towards zero whatever the scene. Counting them
  would pull a high-key or low-key colour photograph towards "monochrome", which is the direction of a false alarm.
- **Too little evidence** means fewer than 1,000 usable samples. The signal concludes nothing, as the spec asks.
- **The decision** is one score, the log of the residual chroma, against a constant fitted so that at most 1% of colour
  photographs are flagged (Decision 6). It is suggested below the constant.
- **A single-channel file** reads as R = G = B through `Sampler`, so its residual is exactly zero with no special case.

**Why the pooled standard deviation rather than a tail statistic.** A small, strongly coloured subject on a gray scene
(the case that must not be flagged) raises a pooled standard deviation a great deal: 3% of the frame at a chroma of 0.3
alone gives about 0.05, several times a scanner's colour noise. A percentile would ignore that subject until it covers
more of the frame than the percentile's tail. That makes it the dangerous choice for a precision-first signal. The
fraction of samples above a fixed residual is measured as a challenger (task 3.2). It needs a second walk of the grid,
and is kept only if it gains at least 3 points of detection at the budget, the rule the earlier signals used.

**Why encoded `B - Y` and `R - Y` rather than CIELab a\*b\*.** The property the signal rests on holds in any opponent
space where gray has zero chroma. These are also the differences the block pass already works in, on the same encoded,
roughly perceptual scale. Lab lives at the colorization family's tier. A second caller would first have to promote it
out of `models/`, per the tier rules, for a gain nobody has measured. If calibration misses its targets in a way that
points at the space, for example misses concentrated in the shadows, promoting Lab becomes its own slice ahead of a
retry. It is not a local copy.

**Outcome: grayscale photographs only.** Calibration (Decision 6) found the premise holds for synthetic prints and
fails for real ones. A real print's colour that brightness does not explain (stains, uneven toning, fading, the
scanner's noise) has a median residual of 0.023, while the haziest and darkest colour photographs reach below 0.005:
dense smog in RTTS and night scenes in ExDark carry almost no colour in any direction. At the 1% budget the constant
could be at most 0.0048, which finds 1 of 975 real scans. Every alternative measured failed the same way:

| Measure, each fitted to flag at most 1% of colour photographs and of the hard set | Real scans found |
|---|---|
| The pooled residual, as designed | 0.2% |
| The challenger: the share of samples beyond a fixed residual (0.01, 0.02, 0.04, 0.08) | 0% to 0.2% |
| The residual over the centre 80%, to drop a print's mount | 0.5% |
| The residual after averaging down to 512, 256 or 128 pixels, to cancel grain and scanner noise | 0.2% to 0.3% |
| The centre 80%, averaged to 256 pixels | 1.3% |
| The spread off the dominant hue (pooled per level, whole-frame about zero, whole-frame about its mean) | 0.1% |

The challenger therefore does not gain 3 points and is not kept (task 3.2). The best trade-off any measure reached,
the spread off the dominant hue at its best balance, found 89.9% of real scans while flagging 25% of colour
photographs and 43% of the hard set, which colorization would have wiped.

So the constant was set where it flags no colour photograph: **0.0005**. A clean grayscale photograph measures exactly
zero, and still does after a JPEG round trip, and the least residual of a colour-set image once the grayscale files
among them were removed is 0.0008. The measure is kept rather than replaced by an exact R = G = B test, because a
grayscale photograph saved as a colour JPEG comes back with a few pixels a level or two off grey. Finding toned and
colour-scanned prints is left to a learned classifier (Decision 1 rejected one for the whole question; a small one for
this narrower question is a separate change).

### 3. In the strided pass, not the block pass

The accumulators are per sample with no neighbour read, which is exactly what `pass.rs` allows. The strided grid covers
the whole frame evenly. The block pass covers about 2% of a 50 MP frame, so a small coloured subject could fall between
its blocks and be flagged as monochrome. A block pass would average colour noise away, but noise only causes misses,
which the budget accepts, while missed coverage causes false alarms.

`Pass` gains a `Levels` accumulator beside `Shades`, and `PixelSignals` gains a `monochrome` field read from it like
`light` and `cast`. The cost is five additions per usable sample on top of a pass that already costs tens of
milliseconds.

### 4. Colour balance stays independent

The two signals are judged separately and neither suppresses the other. With the grayscale-only constant they cannot
fire together: a grayscale photograph's Shades of Gray estimate is neutral, so it never reads as cast. Were a later
detector to find toned prints, a toned print that reads as cast would get both, and colour balance would then run on
the colorized result, which is a trade-off to settle then.

### 5. The family order agrees with the reference

`Family::ALL` becomes `Detection, Denoise, FaceRecovery, Colorization, LightAdjustment, ColorBalance, Sharpen, Upscale`,
and the enum's declaration moves with it, because `ALL` documents itself as the declaration order. Serde names
variants, so nothing serialised changes. Its other users iterate it for coverage and do not depend on its order. The
catalogue pins its own presentation order separately, and is unchanged.

The divergence test is replaced by an agreement test: over the families a user can add (all but detection),
`Family::ALL` equals the reference's `['dn', 'fr', 'cl', 'la', 'cb', 'sh', 'up']` transcribed as families. The reason
for the reference's order holds on its own merits. Colorization discards chroma, so correcting exposure and colour
before it corrects colour that is then thrown away.

### 6. Calibration comes first, and has targets

**At most 3,000 images are read from any dataset.** Larger sets are sampled by a hash of the image name, and only the
sampled files are fetched, by range request where an archive allows it.

**Negatives (colour photographs):** FiveK expert C, Kodak 24 and the SPAQ sample, already on disk from the earlier
calibrations. **The hard set:** RTTS and ExDark in full, and the least colourful tenth of the other three by the Hasler
and Süsstrunk colourfulness metric (15.8 or less), computed in the harness: 2,135 photographs.

**Positives (black and white):** real prints from the Rijksmuseum, scanned in colour. The Library of Congress was not
used: its search API answers every request with a Cloudflare challenge (HTTP 403), so the Rijksmuseum is the only
real source. **Synthetic** from the first 120 clean colour photographs of FiveK and Kodak: neutral grey (Rec. 709
luma), sepia, selenium, cyanotype and a split-tone, each clean and with Gaussian colour noise of 1.5 and 3 on the 0 to
255 scale, and each of those again after a JPEG round trip at quality 85. They add no images read.

**The data**, downloaded to a scratch directory on 2026-09-23. Each set was sampled by the SHA-256 of the image name
(the Rijksmuseum's by object number, per print process), and only the sampled files were fetched.

| Set | Source | Licence | Subset used | On disk |
|---|---|---|---|---|
| MIT-Adobe FiveK, expert C | Already on disk from the light and colour calibration, about 1 MP | Research use | All of it | 1,181 |
| Kodak 24 | Already on disk | Unrestricted, per its host | All of it | 24 |
| SPAQ (Fang et al., CVPR 2020) | Already on disk from the noise and sharpness calibration | Research use only, non-commercial | The 1,000-photograph sample | 1,000 |
| RESIDE RTTS (Li et al., TIP 2019) | Hugging Face `fuhao7i/RTTS`, `VOCdevkit/VOC2007/JPEGImages`, per file | Research use, per RESIDE | 1,000 of the 4,322 real hazy photographs | 1,000 |
| ExDark (Loh and Chan, CVIU 2019) | Hugging Face `fuhao7i/ExDark`, `JPEGImages/IMGS`, per file; the official release is one Google Drive zip | BSD-3-Clause; commercial use by arrangement | 1,000 of the 7,363 low-light photographs | 1,000 |
| Rijksmuseum photographs | `data.rijksmuseum.nl` linked-art search by technique, image through its IIIF server (`iiif.micr.io`) at most 3,000 pixels on the long side | Public Domain Mark 1.0 or CC0, marked downloadable | Albumen 300 of 52,421, gelatin silver 300 of 36,666, salted paper 150 of 1,555, cyanotype 150 of 1,024, carbon 75 of 1,033; platinum had 15, none usable. Objects with any colour, tint or hand-colouring in their technique, material or description were skipped | 975 |

**Negatives removed.** A negative is colour by provenance, and one found to be black and white is removed with its name
recorded in the harness's `EXCLUDED`. Inspected: every negative below the synthetic noise floor was printed, and the
lowest were looked at. Removed: 86, all from the hard sets. 81 (67 ExDark, 14 RTTS) have R = G = B in every pixel, four
of them stored with a single channel; a contact sheet of all 81 shows black-and-white photographs. Five more (ExDark
`2015_05149`, `2015_02235`, `2015_03859`, `2015_04786`, RTTS `HEB_Baidu_530`) have at most 0.2% of their pixels off
grey, by at most 3 levels: grayscale files with an encoder's rounding. Several more ExDark images between 0.0008 and
0.006 look like black-and-white conversions with a trace of tint, and some RTTS images below 0.007 are dense smog that
looks colourless. They were kept, because provenance could not be settled by eye, and the shipped constant is below all
of them.

**The targets and what was achieved**, at the shipped constant of 0.0005:

| Measure | Target | Achieved |
|---|---|---|
| False alarms on colour photographs | at most 1% | 0% (0 of 4,119) |
| False alarms on the hard muted set | at most 1% | 0% (0 of 2,135) |
| Detection of synthetic black-and-white, every tone and noise level | at least 95% | **Missed.** Neutral grey clean and after JPEG 100% (240 of 240); every toned or noisy cell 0%, but selenium clean 24.2% and split-tone clean 0.8% |
| Detection of real black-and-white scans | at least 90% | **Missed.** 0% (0 of 975), by process albumen, gelatin silver, salted paper, cyanotype and carbon all 0% |
| A less colourful version missed where a more colourful one of the same photograph is found, desaturating in steps | never | never (0 of 120); found at no colour 100%, at a sixty-fourth of it 5% |
| Detection by tone and noise level, and the residual distribution of each set | reported | Median residual: colour photographs 0.056, hard set 0.032, RTTS 0.025, ExDark 0.045, real scans 0.023 (cyanotype 0.041, salted paper 0.017), clean synthetic 0.0015, noisy synthetic 0.0095 and 0.019 |

The two detection targets were missed at the budget, and the budget was kept rather than loosened. That was a
decision, not a default (Decision 2, Outcome). At the loosest constant a 1% budget allows (0.0048), 31.4% of the
synthetic prints and 0.1% of the real scans would be found.

Every spec scenario, drawn as a synthetic image in the tests, stays on its side of the boundary with a margin.

### 7. The measurements are logged

At `debug`, beside the existing pixel measurements: the usable-sample count, the residual chroma and the score. A
report of a wrong colorization suggestion is then diagnosable from the log alone.

### Crates and platforms

- **`opai`:** the accumulator, the signal, the suggestion arm, the family reorder and the harness. **`imaging`, `gui`
  and `cli`:** unchanged.
- **`rust-sak`:** no feature applies; this is arithmetic over pixels already decoded. **New crates:** none.
- This is CPU arithmetic with no model, session or execution provider involved. It will be validated on macOS (Apple
  silicon) and is platform-independent. Windows and Linux are covered by the test suite in CI only.

### Slices

Not sliced. The work is estimated at 11 tasks and delivered as this one change.

## Risks / Trade-offs

- [A muted colour photograph (fog, snow, dusk) has a residual close to a noisy scan's] → Measured, and worse than feared:
  lower than most real scans. The constant is set below every colour photograph, and toned and colour-scanned prints
  are not found. A learned classifier is the way to find them.
- [The hard set is small, so 1% there is a handful of images and the rate is uncertain] → RTTS and ExDark were
  available and widen it to 2,135 photographs.
- [A print that faded unevenly has a tint that varies across the frame at the same brightness] → It reads as colour and
  is missed. Accepted. The real scans measure how often.
- [Heavily desaturated colour grading (bleach bypass, near-monochrome film looks)] → It sits near the boundary and is
  usually left alone, which is the preferred side.
- [A target is missed at the 1% budget] → Both detection targets were. The rates were reported, and the decision was
  to keep the budget and ship grayscale-only (Decision 2, Outcome).
- [Calibration data is licensed for research use only (SPAQ, FiveK)] → A single constant is shipped, never the data.
- [Encoded chroma turns out to be the wrong space] → Decision 2: promoting Lab is its own slice, never a copy.
