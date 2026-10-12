## Why

Colorization (Delhi, Mumbai, Jaipur) can be run now, but autopilot never suggests it. A scanned black-and-white
photograph is told it needs light, colour balance or upscaling, and nothing about the one thing it most obviously lacks.
The reference app has no signal for colorization, so there is nothing to port: the detector is new.

The obvious test, "is the palette gray?", is wrong both ways. A black-and-white print that was toned (sepia, selenium,
cyanotype), has aged, or was scanned in colour carries a clear hue in every pixel and has no colour of its own. And
because colorization **replaces** a photograph's colour instead of blending with it, a false alarm on a muted colour
photograph wipes out colour that was really there.

## What Changes

- Autopilot reads a **monochrome** signal and suggests colorization for a **grayscale** photograph: one that carries no
  colour at all, whether it is stored with one channel, stored as colour with its channels equal, or saved as colour
  and compressed. What decides it is the colour left once each brightness level's own tint is removed.
- **Toned, aged and colour-scanned prints are left alone.** The change set out to find them too. Calibration found that
  a real print's colour that brightness does not explain (stains, uneven toning, the scanner's noise) is more than the
  haziest and darkest colour photographs carry, under every variant of the measure tried. At a 1% false-alarm budget
  none found more than 3% of real scans. Finding them needs a learned classifier, which is a separate change.
- It is **precision-first**. The constant sits where no colour photograph measured was flagged, including a
  deliberately hard set of muted ones (haze, low light, the least colourful tenth of the rest).
- It is an image statistic read in the **existing strided pass**, beside the light and colour signals. It runs no
  model, installs nothing, cannot fail, and adds only a few accumulators per brightness level.
- As with the other pixel signals, the constant was **calibrated before it was fixed**, against real black-and-white
  scans, toned and noisy versions synthesised from colour photographs, and colour photographs including the hard muted
  set. **At most 3,000 images were read from any dataset**; larger sets were sampled.
- The canonical family order moves colorization **before light adjustment and colour balance**, which is where the
  reference presents and applies it. This settles the disagreement the autopilot spec pinned for this change to settle.
- The suggestion names the family alone. Which colorizer to run stays the user's choice.

Out of scope: a black-and-white photograph surrounded by colour it does not own, such as a coloured mat or frame, a
stamp, a caption, or a print photographed on a coloured table. Telling that apart from a colour photograph with a gray
sky and a small coloured subject needs a spatial rule, and getting it wrong destroys the subject's colour. Such an image
is left alone.

Estimated at 11 tasks: one change, not a series.

### Deliberate divergences from the reference's behaviour

- **Colorization is suggested at all.** The reference's autopilot never suggests it. This is the third signal invented
  here, after noise and sharpness, for the same reason: parity with an absent signal leaves a user who opens an old
  black-and-white scan without the one suggestion it most plainly calls for.
- **None on the order.** Moving colorization makes `opai`'s family order agree with the reference's present-and-apply
  order exactly, so this removes a divergence rather than adding one.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `autopilot`: a new monochrome signal and what it concludes (colorization for a grayscale photograph, never for a
  colour photograph however muted, and not required for a toned or colour-scanned print), and the family-order
  requirement, which now agrees with the reference on every family and no longer pins a disagreement.

## Impact

- **Crates:** `opai` only (`autopilot` and `models::artifact`'s `Family::ALL`). No change to `gui` or `cli`: wiring
  autopilot into the GUI is separate work, and nothing here is front-end logic. `Family::ALL`'s other users iterate it
  for coverage and do not depend on its order. The catalogue pins its own presentation order.
- **Public API:** `Suggestion` gains a unit arm, `Colorization`. It is not `#[non_exhaustive]`, so an exhaustive
  `match` in a caller outside `opai` stops compiling. No such caller exists today. `Family::ALL` changes order.
- **`rust-sak`:** nothing it provides applies. This is pixel statistics over an already-decoded image, with no hashing,
  fetching, codecs or caching involved.
- **Dependencies:** none new.
- **Calibration data:** downloaded to a scratch directory while developing, never committed and never fetched by the
  application.
