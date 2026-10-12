## Why

Denoise (Stockholm, Gothenburg, Malmö) and sharpen (Moscow, St. Petersburg, Novgorod) can both be run now, but
autopilot never suggests either. A grainy low-light photograph or a shaken handheld shot is told it needs light, colour,
faces or size, and nothing about the two faults that most often ruin one. The reference app has no signal for either
family, so there is nothing to port: both detectors are new.

## What Changes

- Autopilot reads a **noise** signal and suggests denoise for a photograph with **visible grain**, in its brightness or
  in its colour. It does not suggest it for a clean photograph that is full of fine texture, or for noise too faint to
  see.
- Autopilot reads a **sharpness** signal and suggests sharpen for a photograph that is **genuinely blurred**: camera
  shake, subject motion or missed focus, where even the sharpest part of the frame is soft. It does not suggest it for a
  photograph whose background is blurred on purpose around a sharp subject, for one with no detail to judge, or for a
  photograph that is only slightly soft at 100% zoom.
- Both signals are image statistics read from **full-resolution blocks** spread over the frame. Noise and blur only
  show between neighbouring pixels at the photograph's own resolution, so the strided grid the light and colour signals
  read cannot see them, and a downscaled copy hides both. The blocks hold at most as many pixels as that grid, so the
  cost stays bounded whatever the photograph's size. Neither signal runs a model, installs anything or can fail.
- The sharpness signal discounts the photograph's own noise, so a blurred photograph with grain is not read as sharp.
- As with light and colour, thresholds are **calibrated before they are fixed**, against public datasets holding noisy
  and clean, or blurred and sharp, versions of the same scenes, plus phone photographs scored for noisiness and
  sharpness by people. Detection is measured against how noisy or blurred each image actually is next to its clean
  version. **At most 3,000 images are read per signal**; larger sets are sampled.
- Suggestions still name the family alone. Whether blur came from motion (Novgorod, St. Petersburg) or focus (Moscow),
  and which denoiser to use, stays the user's choice.

Estimated at 12 tasks: one change, not a series.

### Deliberate divergences from the reference's behaviour

- **Denoise and sharpen are suggested at all.** The reference's autopilot reads face, light, colour and size, and never
  suggests either family. Its module comment here said none would be invented; this change reverses that. Parity with
  an absent signal would leave the two faults the user is least able to judge unnoticed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `autopilot`: two new signals (noise and sharpness) and what each concludes, and the rule that they read the pixels at
  the photograph's own resolution, without running a model, at a bounded cost, and cannot leave an analysis incomplete.

## Impact

- **Crates:** `opai` (`autopilot`) only. `imaging` already exposes what the blocks need (`Sampler::rgb_and_alpha`). No
  change to `gui` or `cli`; wiring autopilot into the GUI is separate work, and nothing here is front-end logic.
- **Public API:** `Suggestion` gains two unit arms, `Denoise` and `Sharpen`. It is not `#[non_exhaustive]`, so an
  exhaustive `match` in a caller outside `opai` stops compiling. No such caller exists today.
- **`rust-sak`:** nothing it provides applies. This is pixel statistics over an already-decoded image, with no hashing,
  fetching, codecs or caching involved.
- **Dependencies:** none new.
- **Calibration data:** downloaded to a scratch directory while developing, sampled to at most 3,000 images per signal,
  never committed and never fetched by the application.
