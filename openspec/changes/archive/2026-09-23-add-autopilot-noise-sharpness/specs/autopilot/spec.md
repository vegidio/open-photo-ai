## ADDED Requirements

### Requirement: A photograph with visible grain calls for denoise

The system SHALL read a noise signal and SHALL suggest denoise when the photograph shows **visible grain**: random
variation from pixel to pixel, in its brightness or in its colour, strong enough to see where the photograph ought to
be smooth.

The system SHALL judge grain by the parts of the photograph with the least detail of their own. Where a photograph is
detailed, fine texture and grain look alike, and a photograph full of grass, fabric or foliage is not noisy.

Grain that is only in the colour — blotches of hue over surfaces of even brightness — SHALL count as grain. It is a
common form of high-sensitivity sensor noise, and the denoise models remove it.

The system SHALL NOT suggest denoise for noise too faint to see. Every photograph carries some measurable noise, and a
suggestion on all of them would say nothing.

Where the evidence is borderline, the system SHALL prefer not suggesting, for the same reason light adjustment does.

The reference implementation reads no noise signal and never suggests denoise. This signal is new rather than ported.

#### Scenario: A grainy photograph

- **WHEN** a clean photograph with strong sensor-like noise added to it is analysed
- **THEN** denoise is among the suggestions

#### Scenario: Grain in the colour alone

- **WHEN** a clean photograph with strong noise added to its colour but not to its brightness is analysed
- **THEN** denoise is among the suggestions

#### Scenario: A clean photograph

- **WHEN** a clean photograph is analysed
- **THEN** denoise is not among the suggestions

#### Scenario: A clean photograph full of fine texture

- **WHEN** a clean photograph whose surfaces are covered in fine, high-contrast texture is analysed
- **THEN** denoise is not among the suggestions

#### Scenario: Stronger grain is not missed where weaker grain is found

- **GIVEN** a clean photograph that is not suggested denoise
- **WHEN** it has noise added and that version is suggested denoise
- **THEN** the same photograph with stronger noise of the same kind added is suggested denoise too

### Requirement: A genuinely blurred photograph calls for sharpen

The system SHALL read a sharpness signal and SHALL suggest sharpen when the photograph is **genuinely blurred**: soft
across the whole frame, so that **even its sharpest part** lacks the detail a focused, steady exposure would have
recorded. Camera shake, subject motion and missed focus all produce this, and the sharpen models repair all three.

The system SHALL NOT suggest sharpen only because much of a photograph is blurred. A portrait or a close-up whose
background is out of focus around a sharp subject is blurred by intent, as a night scene is dark by intent.

The system SHALL NOT suggest sharpen for a photograph with too little detail to judge — a clear sky, a plain wall, a
fog — since it has no edge that could be sharp or soft. It SHALL NOT suggest sharpen for a photograph that is only
slightly soft when viewed pixel for pixel, as most photographs from high-resolution cameras are.

Grain SHALL NOT be read as sharpness. A blurred photograph with visible grain SHALL be judged on its blur, so that it
is suggested sharpen as well as denoise rather than denoise alone.

Where the evidence is borderline, the system SHALL prefer not suggesting.

The reference implementation reads no sharpness signal and never suggests sharpen. This signal is new rather than
ported.

#### Scenario: An out-of-focus photograph

- **WHEN** a sharp photograph blurred as a lens out of focus would blur it is analysed
- **THEN** sharpen is among the suggestions

#### Scenario: A shaken photograph

- **WHEN** a sharp photograph blurred along one direction, as camera shake would blur it, is analysed
- **THEN** sharpen is among the suggestions

#### Scenario: A sharp photograph

- **WHEN** a sharp photograph is analysed
- **THEN** sharpen is not among the suggestions

#### Scenario: A sharp subject against a blurred background

- **WHEN** a photograph whose centre is sharp and whose surroundings are heavily out of focus is analysed
- **THEN** sharpen is not among the suggestions

#### Scenario: A photograph with no detail

- **WHEN** a photograph of smooth gradients with no edges in it is analysed
- **THEN** sharpen is not among the suggestions, and the analysis is not a failure

#### Scenario: A blurred photograph with grain in it

- **WHEN** a sharp photograph is blurred out of focus, strong noise is then added to it, and it is analysed
- **THEN** sharpen and denoise are both among the suggestions

#### Scenario: Stronger blur is not missed where weaker blur is found

- **GIVEN** a sharp photograph that is not suggested sharpen
- **WHEN** it is blurred and that version is suggested sharpen
- **THEN** the same photograph blurred more strongly in the same way is suggested sharpen too

### Requirement: The noise and sharpness signals read the pixels at full resolution and nothing else

The system SHALL read the noise and sharpness signals from the image's own pixels. It SHALL NOT run a model, install
anything or touch the network to read them, and neither signal SHALL be a reason an analysis is reported incomplete.

Both signals SHALL be read at the photograph's **own resolution**. Grain and blur exist only between neighbouring
pixels: a reduced copy of a grainy or blurred photograph looks clean and sharp, so a conclusion drawn from one is a
conclusion about a different photograph.

The two signals SHALL be read from one pass, and that pass SHALL cost no more for a large photograph than for a
photograph of a bounded size: it reads parts of a large photograph at full resolution rather than all of it.

A photograph that carries 16 bits per channel SHALL be measured at 16 bits. Transparent pixels SHALL NOT contribute to
either signal.

A photograph too small to hold the smallest region either signal measures SHALL conclude nothing for that signal,
rather than a guess.

A cancelled analysis SHALL still return nothing, whichever signal it was reading when it was cancelled.

#### Scenario: No model is available

- **WHEN** an image is analysed and no model can be installed or opened
- **THEN** the noise and sharpness signals are still read and their suggestions still come back

#### Scenario: A very large photograph

- **WHEN** a grainy, or a blurred, photograph of tens of megapixels is analysed
- **THEN** it is suggested denoise, or sharpen, as a crop of it at its own resolution is

#### Scenario: A cut-out on a transparent background

- **WHEN** a clean, sharp subject surrounded by fully transparent pixels is analysed
- **THEN** neither denoise nor sharpen is among the suggestions

#### Scenario: A photograph too small to measure

- **WHEN** a photograph smaller than the region the noise and sharpness signals measure is analysed
- **THEN** neither denoise nor sharpen is among the suggestions, and the analysis is not a failure or incomplete
