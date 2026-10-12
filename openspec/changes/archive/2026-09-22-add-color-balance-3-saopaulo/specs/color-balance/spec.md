## MODIFIED Requirements

### Requirement: The extension is discarded before anything is derived from the model's answer

The system SHALL remove the extension from **both** what the model was shown and what the model produced, before
either is used to derive the correction, and SHALL derive the correction only from the region the photograph itself
occupies.

This is stronger than "the extension does not appear in the result", and the difference is the whole of why it is
stated. The correction this family derives is **global** — one answer fitted across the whole photograph and then
applied to every pixel of it — so an extension left in does not affect only the border. It re-weights the mirrored
border content against the rest of the picture and moves every pixel of the result. Measured on the same 72
photographs, discarding the extension first takes the median from 47.9 dB to 52.7 dB and the worst case from 38.8 dB
to 41.0 dB.

Where a model additionally produces a **per-pixel map** covering the square, the system SHALL read that map only
over the region the photograph occupies. A map read across the whole square would be stretched against the
photograph it is meant to line up with, which displaces every value in it rather than only those near the extended
edge.

#### Scenario: A photograph that needed an extension

- **WHEN** a photograph whose aspect ratio is not square has its colour balance run
- **THEN** no pixel of the result is derived from the reflected extension
- **AND** pixels far from the extended edge are affected by whether the extension was discarded

#### Scenario: A photograph that needed none

- **WHEN** a square photograph and a non-square one are run
- **THEN** both are corrected from the photograph's own pixels alone

#### Scenario: A per-pixel map over an extended square

- **WHEN** a non-square photograph is run by a model that produces a per-pixel map alongside its other output
- **THEN** the map is read only over the region the photograph occupies in the square
- **AND** a feature of the map lines up with the place in the photograph it was predicted for

### Requirement: The result is the photograph re-coloured, not the model's output enlarged

The model is run at a square smaller than most photographs, so its output is at a lower resolution than the
photograph. The system SHALL NOT return that output enlarged.

Instead the system SHALL derive a **colour transform** from what the model was shown to what the model produced, and
SHALL evaluate that transform over the photograph's **own** pixels at the photograph's own resolution. The result
therefore carries the photograph's own resolution and its own detail, and what the model contributes is the colour
transform rather than the pixels.

The transform SHALL be composed of one or more **global colour mappings**, each fitted between the model's input and
one of its renderings of that input. A model that produces a single corrected rendering contributes one mapping,
which is applied everywhere; a model that produces several renderings contributes one per rendering, combined as the
requirement on mixed-illuminant models describes.

This is the whole of why a fixed square is affordable here, and it is a different argument from the one a family that
derives a per-pixel correction makes. A mapping is fitted from a few hundred thousand samples and barely moves with
the resolution those samples were taken at, which is why the sampling resolution can be traded and the photograph's
detail cannot.

Each mapping SHALL be fitted over a colour feature set richer than a per-channel scale, so that a cast which is not a
simple per-channel gain can be corrected. Every channel of the result SHALL be bounded to the range that channel can
carry.

The result SHALL carry colour only. Alpha is not preserved through a model run, and a run SHALL NOT report an opacity
the model was never given.

Where the caller asked for more than 8 bits per channel, the transform SHALL be evaluated without first reducing the
photograph or the model's output to 8 bits. A depth the caller asked for is not honoured by writing an 8-bit answer
into a 16-bit result.

#### Scenario: A photograph much larger than the model's square

- **WHEN** a photograph many times the model's square has its colour balance run
- **THEN** the result carries the photograph's own width and height
- **AND** detail finer than the model's square is present in the result

#### Scenario: A 16-bit photograph run at 16 bits

- **WHEN** a 16-bit photograph is colour balanced with the result asked to carry 16 bits per channel
- **THEN** a smooth gradient in the result carries more than 256 distinct levels per channel

#### Scenario: A photograph with transparency

- **WHEN** a photograph carrying an alpha channel is colour balanced
- **THEN** the result carries colour only

#### Scenario: A cast no per-channel gain would fix

- **WHEN** a photograph whose cast varies with brightness is colour balanced
- **THEN** the correction applied to a dark pixel differs from the one applied to a bright pixel of the same hue

#### Scenario: A model producing several renderings

- **WHEN** a photograph is run by a model whose output is several renderings of it rather than one corrected image
- **THEN** one mapping is fitted per rendering
- **AND** the result still carries the photograph's own resolution and detail

## ADDED Requirements

### Requirement: A model that renders several illuminants blends them per pixel

A colour balance model MAY answer with several white-balance **renderings** of the photograph together with a
per-pixel **weight map** over them, rather than with one corrected image. Where it does, the system SHALL produce the
result by evaluating each rendering's mapping at each of the photograph's pixels and combining them by the weight
that pixel carries for each rendering.

This is what lets one photograph be corrected in more than one direction at once — a window-lit room with a warm
lamp, a subject in shade against a sunlit background — which a single global mapping cannot express whatever feature
set it is fitted over.

The rendering that corresponds to leaving the photograph as it is SHALL be taken as the photograph itself rather than
from the model. A model that returned it would be handing back what it was given.

Each rendering's value SHALL be bounded to the displayable range **before** it is weighted, rather than the blend
bounded afterwards. The mappings extrapolate outside that range on saturated pixels, and bounding only at the end
lets a blown highlight pull the blend somewhere no rendering goes, by however much its own weight allows.

The weight map SHALL be carried up to the photograph's own resolution, and SHALL be sampled so that a weight lands on
the place in the photograph it was predicted for. The system SHALL NOT reduce the photograph to the weight map's
resolution instead.

Where the weights for a pixel already sum to one, the system SHALL NOT renormalise them after carrying them up.

#### Scenario: A photograph lit by two sources

- **WHEN** a photograph whose two halves are lit by different illuminants is balanced by a model that renders several
- **THEN** the correction applied in one half differs from the correction applied in the other

#### Scenario: A weight map favouring one rendering everywhere

- **WHEN** the weight map selects a single rendering across the whole photograph
- **THEN** the result is that rendering's mapping applied globally, as a single-output model would apply it

#### Scenario: The photograph's own rendering

- **WHEN** the weight map selects the unaltered rendering across the whole photograph at a bias of 1
- **THEN** the result is the photograph itself rather than a rendering of it the model produced

#### Scenario: A blown highlight

- **WHEN** a photograph containing saturated highlights is balanced by a model that renders several
- **THEN** no pixel of the blend lies outside the range the renderings themselves can occupy

#### Scenario: A weight boundary

- **WHEN** the weight map changes from one rendering to another partway across the photograph
- **THEN** the change appears at the corresponding place in the result rather than displaced from it

### Requirement: Both colour balance variants run

Every variant this family publishes SHALL produce a corrected photograph, at every precision it is published in.
Neither variant SHALL be refused on the grounds that the library has not implemented it.

The two answer different questions of the photograph — one corrects it globally, one blends several renderings of it
by a predicted weight map — so a caller SHALL be able to ask for either without knowing which contract it runs, and
SHALL receive a photograph at its own resolution from both.

#### Scenario: Rio at either precision

- **WHEN** a photograph is colour balanced by Rio at FP32 and again at FP16
- **THEN** both produce a corrected photograph at the photograph's own resolution

#### Scenario: São Paulo at either precision

- **WHEN** a photograph is colour balanced by São Paulo at FP32 and again at FP16
- **THEN** both produce a corrected photograph at the photograph's own resolution

#### Scenario: Every published variant

- **WHEN** each colour balance variant the library publishes is asked for in turn
- **THEN** none of them is refused as unimplemented

## REMOVED Requirements

### Requirement: Rio runs, and São Paulo is refused by name

**Reason**: São Paulo now runs. The requirement existed to state that this family was published one variant at a
time and that the second variant's contract was refused rather than served through the first one's, which would have
rendered nonsense from correctly loaded weights. Both halves are now obsolete: the refusal is gone and the second
contract is implemented. It is replaced by *Both colour balance variants run*, which states what the family now
answers for, and by *A model that renders several illuminants blends them per pixel*, which states the behaviour São
Paulo was refused for the absence of.

**Migration**: A caller that treated a São Paulo operation as unavailable — hiding it from a chooser, or handling the
unsupported-operation failure it raised — SHALL offer it as it offers Rio. No operation, parameter or result changes
spelling: the same São Paulo operation that was refused now produces a photograph.
