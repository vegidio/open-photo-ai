## Purpose

What a light adjustment run does to a photograph: the one fixed square its model accepts and how an image of any size
and aspect is presented to it, why what the model produces is never the thing returned, what the per-run bias means in
each direction, and what the run reports, stops on and refuses.

## ADDED Requirements

### Requirement: The photograph is presented to the model at one fixed square, whatever size it is

Each light adjustment variant's model accepts exactly one square size and no other. The system SHALL present the whole
photograph at that square, by scaling it so that its **longer** side lands on the square exactly, preserving the aspect
ratio, and extending the shorter side out to fill the square.

The scaling SHALL apply to a photograph smaller than the square as readily as to one larger than it. A fixed-shape
graph accepts one size, so there is no path on which a small image runs at its own resolution.

The system SHALL fill the remainder of the square by **reflecting the photograph's own edge pixels outwards**, and
SHALL NOT fill it with a constant colour, and SHALL NOT stretch the photograph to fill the square instead. All three
are observable in the result rather than being matters of taste:

- Stretching changes the pixels the model is shown, and it changes them most at the edge the stretch was needed for. On
  a 1000x750 photograph the reference measured the last column differing from its neighbour by **40.8 levels on
  average**, against an interior column-to-column gradient of 3.9 — a hard one-pixel line down the side of the picture.
  Reflecting brings that to 2.9, which is the interior.
- A constant fill is content the model reads as picture and adjusts, and whatever it makes of a flat rectangle then
  influences the exposure it chooses for the picture beside it.

The extension SHALL be applied only past the photograph's own extent, so the pixels the model is shown inside that
extent are the photograph's own, unchanged by the presence of the extension.

The system SHALL remove the extension from the model's output before that output is used for anything, so that nothing
downstream is derived from pixels the photograph did not have.

The square SHALL be a property of the variant rather than of the family, so that a model re-exported at a different
square is presented at its own.

A consequence of this requirement is recorded rather than left to be discovered: **the model's share of a run costs the
same for every photograph.** A 400x300 thumbnail and a 6000x4000 photograph are both run at the square, so the run's
cost does not fall away for a small image, and the rest of the run — which is proportional to the photograph — is what
separates a large one from a small one.

#### Scenario: A landscape photograph

- **WHEN** a photograph wider than it is tall is adjusted
- **THEN** its width lands on the model's square exactly and its height is scaled by the same factor
- **AND** the rows below it are the photograph's own bottom rows reflected upwards

#### Scenario: A portrait photograph

- **WHEN** a photograph taller than it is wide is adjusted
- **THEN** its height lands on the model's square exactly and its width is scaled by the same factor
- **AND** the columns beside it are the photograph's own edge columns reflected inwards

#### Scenario: A photograph smaller than the square

- **WHEN** a photograph whose longer side is shorter than the model's square is adjusted
- **THEN** it is enlarged so that its longer side lands on the square, rather than being run at its own size

#### Scenario: A photograph already square

- **WHEN** a square photograph is adjusted
- **THEN** it is scaled onto the model's square with no extension at all

#### Scenario: The extension does not reach the result

- **WHEN** any photograph that needed an extension is adjusted
- **THEN** no pixel of the result is derived from the extension

### Requirement: The result is the photograph corrected, not the model's output enlarged

The model is run at a square smaller than most photographs, so its output is at a lower resolution than the photograph.
The system SHALL NOT return that output enlarged.

Instead the system SHALL derive, at the photograph's **own** resolution, the per-channel ratio between what the model
produced and what the model was shown, and SHALL apply that ratio to the photograph's own pixels. The result therefore
carries the photograph's own resolution and its own detail, and what the model contributes is the change in light
rather than the pixels.

This is the whole of why a fixed square is affordable. Detail finer than the square is neither seen by the model nor
lost, because the pixels returned are the photograph's, multiplied.

The system SHALL guard the ratio's denominator so that a near-black pixel does not produce an unbounded correction, and
SHALL bound every channel of the result to the range that channel can carry.

The result SHALL carry colour only. Alpha is not preserved through a model run, and a run SHALL NOT report an opacity
the model was never given.

Where the caller asked for more than 8 bits per channel, the correction SHALL be derived without first reducing the
model's output or the photograph to 8 bits. A depth the caller asked for is not honoured by writing an 8-bit answer
into a 16-bit result.

#### Scenario: A photograph much larger than the model's square

- **WHEN** a photograph many times the model's square is adjusted
- **THEN** the result carries the photograph's own width and height
- **AND** detail finer than the model's square is present in the result

#### Scenario: A near-black region

- **WHEN** a photograph containing near-black pixels is adjusted
- **THEN** those pixels are corrected without producing an out-of-range or wildly amplified value

#### Scenario: A 16-bit photograph adjusted at 16 bits

- **WHEN** a 16-bit photograph is adjusted with the result asked to carry 16 bits per channel
- **THEN** a smooth gradient in the result carries more than 256 distinct levels per channel

#### Scenario: A photograph with transparency

- **WHEN** a photograph carrying an alpha channel is adjusted
- **THEN** the result carries colour only

### Requirement: The bias says in which direction and how far, and leaves the photograph alone at zero

A light adjustment run carries a bias between -1 and 1 inclusive. The system SHALL apply the model's adjustment to the
photograph in proportion to it:

- At **0** the run SHALL return the photograph unchanged.
- At **1** the run SHALL return the model's adjustment in full.
- At a **negative** value the run SHALL apply the same adjustment in the opposite direction — the photograph moved
  away from the adjusted image along the same line, rather than a second model or a second run in reverse.
- Between those the run SHALL move the photograph proportionally towards, or away from, the adjusted image.

The bias SHALL NOT select the weights. One model serves every value of it, so moving a control through a range of
values does not install or open anything a second time.

Two runs over one photograph differing only in their bias SHALL produce different results, and a result kept for one
SHALL NOT be served for the other. A negative bias and its positive counterpart are opposite adjustments and SHALL NOT
share a kept result.

#### Scenario: A bias of zero

- **WHEN** a photograph is adjusted at a bias of 0
- **THEN** the result is the photograph unchanged

#### Scenario: A negative bias

- **WHEN** a photograph is adjusted at a bias of -1 and again at 1
- **THEN** the two results move the photograph in opposite directions from where it started

#### Scenario: Two biases in one session

- **WHEN** a photograph is adjusted twice at different biases
- **THEN** the second run is not served the first run's kept result
- **AND** the second run does not install or open the model a second time

### Requirement: Both of the family's variants run, through one contract

Every variant this family publishes SHALL produce an adjusted photograph through the behaviour above. The variants
differ in their weights and in the provider settings measured for them, and SHALL NOT differ in how a photograph is
presented to them, in how their output is applied, or in what the bias means.

Choosing a different variant at the same bias SHALL produce a different result, and a result kept for one SHALL NOT be
served for the other.

#### Scenario: Either variant over one photograph

- **WHEN** a photograph is adjusted by each of the family's variants in turn at the same bias
- **THEN** both produce an adjusted photograph at the photograph's own resolution
- **AND** the two results differ from each other

### Requirement: A run reports how far it has got and stops when the caller asks

A light adjustment run SHALL report progress over the whole of its share of a chain and SHALL reach the end of that
share when it finishes.

The report SHALL advance during the work that follows the model run as well as around it. The model's share is a fixed
cost whatever the photograph is, while everything after it is proportional to the photograph, so on a large photograph
a report that advanced only around the model run would sit at one figure for most of the time the run takes.

A run SHALL stop when the caller cancels it, and SHALL produce **no image at all** — not the photograph, and not the
partly corrected buffer the cancellation landed in. A caller handed a partly corrected picture with no error has no way
to tell it from a finished adjustment.

#### Scenario: Progress over a large photograph

- **WHEN** a photograph many times the model's square is adjusted with a progress callback
- **THEN** progress advances after the model has run and before the result is complete
- **AND** the final value reported is the end of the operation's share

#### Scenario: A run cancelled partway

- **WHEN** a caller cancels a light adjustment run after the model has run
- **THEN** the run fails rather than returning a result
- **AND** no image is produced

### Requirement: A photograph with no area is refused

Where the photograph has no width or no height, the system SHALL refuse the run and SHALL produce no image.

There is no scaling of an empty photograph onto the square, and a model run over a square containing nothing but a
reflection of nothing is not an adjustment of anything. This is the same refusal the tiling of a zero-area image makes,
for the same reason.

#### Scenario: An image with no area

- **WHEN** a light adjustment is run over an image with no width or no height
- **THEN** the run fails and no image is produced
