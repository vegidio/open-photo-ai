## ADDED Requirements

### Requirement: An upscaler that preserves resolution reaches a scale by resampling first

An upscale variant whose model returns an image the size of what it was given SHALL reach a requested
scale by resampling the input to the requested size and then restoring detail at that size. The
resampled image SHALL be produced with the same high-quality filter the system corrects an overshoot
with, and SHALL be what the model is conditioned on.

This is not a fallback for a model that cannot enlarge. Such a model is resolution-preserving by
construction, so there is no scale it could apply; resampling first is how it is meant to be driven,
and the reference implementation drives it the same way for the same reason.

At a requested scale of 1 the resample SHALL leave the image at its own dimensions, and the run SHALL
still restore detail at that size - a scale of 1 is a restoration request, not a no-op.

Where the model accepts only one region size, or accepts only dimensions that are a multiple of some
figure, the system SHALL extend the resampled image to satisfy both before running, by mirroring its
own edge pixels outwards rather than by adding a border of any colour, and SHALL remove that extension
from the result. Mirroring keeps the edge pixels the user asked for; a flat border is treated by the
model as picture content, and whatever it makes of it bleeds back across the edge into pixels the
image actually has. Cropping instead of extending would discard pixels the user asked for.

The final result's dimensions SHALL be the input's, each multiplied by the requested scale and
rounded - the same arithmetic every other upscale contract produces, so which contract served a
request is not observable in the size of what comes back.

#### Scenario: A scale larger than 1

- **WHEN** an image is upscaled by a resolution-preserving upscaler at a factor above 1
- **THEN** the result's dimensions are the input's multiplied by that factor and rounded
- **AND** the result carries restored detail rather than only the resample's interpolation

#### Scenario: A restoration at a scale of 1

- **WHEN** an image is run through a resolution-preserving upscaler at a scale of 1
- **THEN** the result has the input's own dimensions
- **AND** the image has still been through the model rather than returned unchanged

#### Scenario: An image smaller than the one region the model accepts

- **WHEN** the resampled image is smaller in either dimension than the single region size the model
  accepts
- **THEN** it is extended to at least that size by mirroring its own edge pixels
- **AND** the extension does not appear in the result, whose dimensions are the requested ones

#### Scenario: An image whose size is not a multiple the model requires

- **WHEN** the resampled image's width or height is not a multiple of the figure the model requires
- **THEN** it is extended to the next such multiple by mirroring its own edge pixels
- **AND** the extension does not appear in the result

### Requirement: A diffusion upscaler's colour drift is corrected against what it was conditioned on

A single diffusion step drifts in overall colour and brightness. The system SHALL correct that drift
by separating the restored image into low-frequency and high-frequency content, replacing the
low-frequency content with that of the image the model was conditioned on, and keeping the restored
image's own high-frequency detail. The correction SHALL preserve the detail the model synthesized: it
changes colour and illumination, not texture.

The correction SHALL be applied **once, to the assembled image**, and SHALL NOT be applied to each
region separately. Each region drifts independently, so correcting a region against its own crop bakes
that region's drift in as a step change at its boundary rather than removing it - which reads as a
seam that no amount of combining removes, because the regions genuinely disagree about the colour of
what they share.

The reference for the correction SHALL be the resampled image the model was conditioned on, and not
the original input, so that the correction undoes only what the model changed rather than also undoing
the resample.

#### Scenario: A colour cast across the whole result

- **WHEN** a diffusion upscale produces a result whose overall colour and brightness have drifted from
  the image it was conditioned on
- **THEN** the drift is corrected towards that image
- **AND** the detail the model synthesized is still present in the result

#### Scenario: Regions that drifted differently from one another

- **WHEN** adjacent regions of one image drifted in different directions
- **THEN** the correction is applied once to the assembled image rather than per region
- **AND** no step in colour appears at the boundary between them

## MODIFIED Requirements

### Requirement: A requested scale is covered by native passes and corrected afterwards

An upscale whose weights are published per whole factor SHALL cover a requested scale by running the
sequence of native passes that reaches or exceeds it, each pass taking the previous pass's result. A
user may ask for any scale in the supported range.

Where that sequence produces an image larger than the request - a request no whole factor matches -
the system SHALL resample the result to exactly the requested scale after the last pass, and SHALL
NOT resample between passes: a model's input is the previous model's output, and resampling in
between would feed the next pass an interpolation rather than a restoration.

The final result's dimensions SHALL be the input's, each multiplied by the requested scale and
rounded, whether or not any pass overshot.

Where no sequence of passes covers a requested scale, the run SHALL fail. It SHALL NOT return a plain
resample of the input presented as a successful enhancement.

This is not the only way an upscale reaches a requested scale, and it is stated for the contract it
belongs to rather than for every upscale. An upscaler whose model preserves resolution has no native
factor and no pass sequence at all; it reaches a scale by resampling to the requested size first and
restoring at that size.

#### Scenario: A scale a single pass serves exactly

- **WHEN** an image is upscaled by a factor one set of weights serves natively
- **THEN** one pass runs
- **AND** the result is that factor times the input's width and height, with no resample afterwards

#### Scenario: A scale no whole factor serves

- **WHEN** an image is upscaled by a factor between two native factors
- **THEN** the passes covering it run and the result is resampled once, after the last of them
- **AND** the final dimensions are the input's multiplied by the requested scale

#### Scenario: A scale needing several passes

- **WHEN** an image is upscaled by a factor larger than any single set of weights serves
- **THEN** the passes run in sequence, each over the previous one's result
- **AND** the final dimensions are the input's multiplied by the requested scale

#### Scenario: An upscaler with no native factor

- **WHEN** an image is upscaled by a variant whose model returns an image the size it was given
- **THEN** no pass sequence is resolved for it
- **AND** the requested scale is reached by resampling to the requested size before the model runs
