## Why

Autopilot suggests only face recovery and upscaling. The reference also suggests light adjustment and colour balance,
and both families can now be run here (Paris and Lyon; Rio and São Paulo), so a dark or colour-cast photograph is
currently told it can be enlarged and nothing about its exposure or its colour.

Porting the reference's two heuristics would not close that gap, because they do not detect what they are for. Ported
exactly and run over the Kodak 24 set with synthetic exposure and white-balance errors applied:

| Variant (24 photographs)       | Reference: light | Reference: colour |
|--------------------------------|------------------|-------------------|
| unaltered                      | 0                | 0                 |
| -2 EV                          | 0                | 0                 |
| -3 EV                          | 2                | 0                 |
| +2 EV                          | 6                | 2                 |
| tungsten cast (R x1.4, B x0.6) | 0                | 1                 |
| strong cool cast (R x0.7, B x1.4) | 0             | 2                 |

A correctly exposed night scene and a product shot on a white background both trigger its light suggestion. Colour
balance fires on overexposed images with no cast at all, because a pixel with R = G = B has no hue and is counted in the
red hue bin. It is detected *less* often as a cast gets stronger, because the cast pushes the near-grey pixels the
heuristic relies on out of its near-grey set. The reference's unit tests pass because they use flat single-colour
images, which have none of these properties.

## What Changes

- Autopilot reads a **light** signal and suggests light adjustment for a photograph that is underexposed (its whole
  tonal range compressed into the shadows) or overexposed (its whole tonal range lifted off the shadows). It does not
  suggest it for a photograph that is merely dark or bright by intent: a low-key or night scene that still reaches
  its highlights, or a high-key scene that still has shadows.
- Autopilot reads a **colour** signal and suggests colour balance for a photograph whose colours are shifted as a
  whole towards one hue, judged by an estimate of the colour of the light it was taken under (Shades of Gray). It does
  not suggest it for a photograph that is merely colourful.
- Both signals are pure image statistics, computed in **one pass** over a bounded sample of the pixels. They run no
  model, install nothing and cannot fail. They read 16-bit photographs at 16 bits, and they ignore transparent pixels.
- Their thresholds are **calibrated before they are fixed**, against public datasets that hold correct and incorrect
  renderings of the same scenes. Detection is measured against how wrong each rendering actually is next to its
  scene's correct one, not against its label: a "-1.5 EV" or "Tungsten" rendering is often fine. The measured
  detection and false-alarm rates are recorded in the design.
- Suggestions still name the family alone. Which of a family's models fixes the problem stays the user's choice.

Estimated at 12 tasks: one change, not a series.

### Deliberate divergences from the reference's behaviour

- **Different detectors, so different answers.** The reference's `scanImage`, `shouldLightAdjustment` and
  `shouldColorBalance` are replaced, not ported. Many photographs will get a different light or colour suggestion than
  the Go app gives them. The table above is why that parity gap is worth taking: matching the reference would mean
  matching a detector that misses almost everything it exists to find.
- **16-bit sources are measured at 16 bits.** The reference truncates every sample to 8 bits first.
- **Transparent pixels are excluded.** The reference counts them as whatever colour they premultiply to, which for a
  cut-out on a transparent background is black.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `autopilot`: two new signals (light and colour) and what each concludes, the rule that these signals read the
  pixels without running a model and cannot leave an analysis incomplete, and the family-order requirement's checked
  set of suggestible families, which grows to include both.

## Impact

- **Crates:** `opai` (`autopilot`) and `imaging` (`Sampler` gains a way to read alpha). No change to `gui` or `cli`. Wiring autopilot into the
  GUI is separate work, and nothing here is front-end logic.
- **Public API:** `Suggestion` gains two unit arms, `LightAdjustment` and `ColorBalance`. It is not
  `#[non_exhaustive]`, so an exhaustive `match` in a caller outside `opai` stops compiling. No such caller exists
  today.
- **`rust-sak`:** nothing it provides applies. This is pixel statistics over an already-decoded image, with no
  hashing, fetching, codecs or caching involved.
- **Dependencies:** none new.
- **Calibration data:** downloaded to a scratch directory while developing, never committed and never fetched by the
  application.
