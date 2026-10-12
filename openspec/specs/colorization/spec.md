# colorization Specification

## Purpose

What a colorization run does to a photograph. It covers how the whole photograph is shown to the model, why only the
colour comes from the model while the lightness and resolution stay the photograph's, what happens to depth and
transparency, and what the run reports, stops on and refuses.

## Requirements

### Requirement: The whole photograph is shown to the model once, stretched to the model's square

Each colorization variant's model accepts exactly one square size. The system SHALL show the model the **whole**
photograph in **one** run, stretched to that square in both directions independently. A photograph smaller than the
square SHALL be enlarged onto it, and a larger one reduced.

The system SHALL NOT divide the photograph into tiles. A model colours what it sees from the scene's context, so tiles
coloured separately would disagree about the regions they share, and blending their overlap does not resolve a
disagreement.

The system SHALL NOT fit the photograph to the square by preserving its aspect ratio and padding the rest. The model
sees the photograph distorted to the square, and the system SHALL read the model's colour back onto the photograph
with the inverse distortion. As a result, the colour predicted for a region of the photograph lands on that region,
whatever the photograph's aspect ratio.

The square SHALL be a property of the variant, not of the family.

The model's share of a run costs the same for every photograph. A thumbnail and a 24-megapixel photograph are both one
run at the square.

#### Scenario: A photograph much larger than the square

- **WHEN** a photograph many times the model's square is colorized
- **THEN** the model is run exactly once, at its square

#### Scenario: A photograph smaller than the square

- **WHEN** a photograph smaller than the model's square is colorized
- **THEN** the model is run exactly once, at its square, not at the photograph's own size

#### Scenario: A photograph far from square

- **WHEN** a photograph several times wider than it is tall is colorized, and the model colours its left half
  differently from its right half
- **THEN** the result carries the left half's colour on the left half of the photograph and the right half's colour on
  the right half

### Requirement: The result keeps the photograph's own lightness and resolution, and takes only its colour from the model

A colorization model predicts colour only, at its square. The system SHALL NOT return the model's output enlarged, and
SHALL NOT take lightness from the model.

The system SHALL keep the photograph's own lightness at the photograph's own resolution. For every pixel it SHALL
combine that lightness with the colour the model predicted for that position. The result therefore has the
photograph's width, height and detail, and the model contributes colour and nothing else.

Each model SHALL be shown only a gray rendering of the photograph, in the form that model was trained on. The colour a
photograph already carries SHALL therefore be replaced by the model's prediction, not blended with it. Colorizing a
colour photograph recolours it.

Where the model predicts no colour at a position, the result there SHALL be the photograph's own gray at that
position, within one level per channel, with its channels within one level of each other.

#### Scenario: A photograph much larger than the model's square

- **WHEN** a photograph many times the model's square is colorized
- **THEN** the result has the photograph's own width and height
- **AND** detail finer than the model's square is present in the result

#### Scenario: A model that predicts no colour

- **WHEN** a photograph is colorized by a model that predicts no colour anywhere
- **THEN** every pixel of the result is the photograph's own gray at that pixel, within one level per channel

#### Scenario: A model that predicts a colour

- **WHEN** a mid-gray photograph is colorized by a model that predicts one moderate warm colour everywhere
- **THEN** every pixel of the result is warm
- **AND** each pixel's lightness is the photograph's own lightness at that pixel, within the rounding of the result's
  depth

#### Scenario: A colour photograph

- **WHEN** a photograph that already carries colour is colorized
- **THEN** the colour in the result is the model's prediction, not the photograph's original colour

### Requirement: A run keeps the photograph's depth and composites its transparency away

Where the caller asked for more than 8 bits per channel, the system SHALL read the photograph's lightness and compose
the result at that depth, without reducing either the photograph or the result to 8 bits along the way.

The result SHALL carry colour only. A photograph with transparency SHALL be composited against black before the model
sees it and before its lightness is read, so the model and the result see the same photograph. The result SHALL NOT
report an opacity the model was never given.

#### Scenario: A 16-bit photograph colorized at 16 bits

- **WHEN** a 16-bit photograph is colorized with the result asked to carry 16 bits per channel
- **THEN** a smooth gradient in the result carries more than 256 distinct levels per channel

#### Scenario: A photograph with transparency

- **WHEN** a photograph carrying an alpha channel is colorized
- **THEN** the result carries colour only
- **AND** the result's lightness is the lightness of the photograph composited against black

### Requirement: All three of the family's variants run

Every variant this family publishes SHALL produce a colorized photograph through the behaviour above. The variants
differ in their weights, their square, the gray rendering they are shown, the form their prediction takes, and the
provider settings measured for them. They SHALL NOT differ in how the photograph's lightness is kept or in how the
colour is put onto it.

A colorization run takes no parameter beyond its variant and precision. Two runs of one variant at one precision over
one photograph SHALL produce the same result. A result kept for one variant SHALL NOT be served for another.

#### Scenario: Each variant over one photograph

- **WHEN** a photograph is colorized by each of the family's variants in turn
- **THEN** each produces a colorized photograph at the photograph's own resolution
- **AND** no two of the three results are identical

#### Scenario: A colorization chained with another enhancement

- **WHEN** a chain names a colorization and then another family's operation
- **THEN** the run succeeds and produces an image
- **AND** the colorization is not refused

### Requirement: A run reports how far it has got and stops when the caller asks

A colorization run SHALL report progress over the whole of its share of a chain, and SHALL reach the end of that share
when it finishes.

The report SHALL advance after the model has run and before the result is complete. The model's share is a fixed cost
whatever the photograph is, while putting the colour onto the photograph is proportional to the photograph. On a large
photograph, a report that advanced only around the model run would sit at one figure for much of the run.

A run SHALL stop when the caller cancels it, and SHALL produce **no image at all**: not the photograph, and not a
partly composed result.

#### Scenario: Progress over a large photograph

- **WHEN** a photograph many times the model's square is colorized with a progress callback
- **THEN** progress advances after the model has run and before the result is complete
- **AND** the final value reported is the end of the operation's share
- **AND** no reported value is lower than the one before it

#### Scenario: A run cancelled partway

- **WHEN** a caller cancels a colorization run after the model has run
- **THEN** the run fails rather than returning a result
- **AND** no image is produced

### Requirement: A photograph with no area is refused

Where the photograph has no width or no height, the system SHALL refuse the run before anything is allocated, and
SHALL produce no image. An empty photograph cannot be stretched onto a square, and there is no lightness to keep.

#### Scenario: An image with no area

- **WHEN** a colorization is run over an image with no width or no height
- **THEN** the run fails and no image is produced
- **AND** the model is not run
