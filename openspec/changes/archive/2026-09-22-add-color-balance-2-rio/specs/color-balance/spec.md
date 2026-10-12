## Purpose

What a colour balance run does to a photograph: the one fixed square its model accepts, why the extension added to
fill that square never reaches the answer, why the result is the photograph re-coloured rather than the model's output
enlarged, what the per-run bias means in each direction, and what a run reports, stops on and refuses.

## ADDED Requirements

### Requirement: The photograph is presented to the model at one fixed square, whatever size it is

Each colour balance variant's model accepts exactly one square size and no other. The system SHALL present the whole
photograph at that square, by scaling it so that its **longer** side lands on the square exactly, preserving the aspect
ratio, and extending the shorter side out to fill the square.

The scaling SHALL apply to a photograph smaller than the square as readily as to one larger than it. A fixed-shape
graph accepts one size, so there is no path on which a small image runs at its own resolution.

The system SHALL fill the remainder of the square by **reflecting the photograph's own edge pixels outwards**, and
SHALL NOT stretch the photograph to fill the square instead, and SHALL NOT fill it with a constant colour. The choice
is observable in the result rather than a matter of taste: scored on the final full-resolution image over 72
photographs — three sources, six aspect ratios, four illuminant casts — against what a dynamic-shape graph rendered,
stretching measures a 44.5 dB median where reflecting measures 52.7 dB.

The extension SHALL be applied only past the photograph's own extent, so the pixels the model is shown inside that
extent are the photograph's own, unchanged by the presence of the extension.

The square SHALL be a property of the variant rather than of the family, so that a model re-exported at a different
square is presented at its own.

A consequence is recorded rather than left to be discovered: **the model's share of a run costs the same for every
photograph.** A 400x300 thumbnail and a 6000x4000 photograph are both run at the square, and what separates a large
photograph from a small one is everything after the model, which is proportional to the photograph.

#### Scenario: A landscape photograph

- **WHEN** a photograph wider than it is tall has its colour balanced
- **THEN** its width lands on the model's square exactly and its height is scaled by the same factor
- **AND** the rows below it are the photograph's own bottom rows reflected upwards

#### Scenario: A portrait photograph

- **WHEN** a photograph taller than it is wide has its colour balanced
- **THEN** its height lands on the model's square exactly and its width is scaled by the same factor
- **AND** the columns beside it are the photograph's own edge columns reflected inwards

#### Scenario: A photograph smaller than the square

- **WHEN** a photograph whose longer side is shorter than the model's square has its colour balanced
- **THEN** it is enlarged so that its longer side lands on the square, rather than being run at its own size

#### Scenario: A photograph already square

- **WHEN** a square photograph has its colour balance run
- **THEN** it is scaled onto the model's square with no extension at all

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

#### Scenario: A photograph that needed an extension

- **WHEN** a photograph whose aspect ratio is not square has its colour balance run
- **THEN** no pixel of the result is derived from the reflected extension
- **AND** pixels far from the extended edge are affected by whether the extension was discarded

#### Scenario: A photograph that needed none

- **WHEN** a square photograph and a non-square one are run
- **THEN** both are corrected from the photograph's own pixels alone

### Requirement: The result is the photograph re-coloured, not the model's output enlarged

The model is run at a square smaller than most photographs, so its output is at a lower resolution than the
photograph. The system SHALL NOT return that output enlarged.

Instead the system SHALL fit a **single global colour mapping** from what the model was shown to what the model
produced, and SHALL evaluate that mapping over the photograph's **own** pixels at the photograph's own resolution. The
result therefore carries the photograph's own resolution and its own detail, and what the model contributes is the
colour transform rather than the pixels.

This is the whole of why a fixed square is affordable here, and it is a different argument from the one a family that
derives a per-pixel correction makes. The mapping is fitted from a few hundred thousand samples and barely moves with
the resolution those samples were taken at, which is why the sampling resolution can be traded and the photograph's
detail cannot.

The mapping SHALL be fitted over a colour feature set richer than a per-channel scale, so that a cast which is not a
simple per-channel gain can be corrected. Every channel of the result SHALL be bounded to the range that channel can
carry.

The result SHALL carry colour only. Alpha is not preserved through a model run, and a run SHALL NOT report an opacity
the model was never given.

Where the caller asked for more than 8 bits per channel, the mapping SHALL be evaluated without first reducing the
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

### Requirement: A colour mapping that cannot be fitted is refused rather than rendered

Where the system cannot solve for the colour mapping, it SHALL fail the run and SHALL produce no image.

An unsolvable system yields weights that are not numbers, and evaluating those over a photograph produces a destroyed
picture with nothing anywhere to say why. A caller handed that has no way to tell it from a correction it dislikes.
The refusal SHALL name the operation, so a front end can say which enhancement failed.

**This is a guard on an arithmetic invariant rather than a state a photograph can reach**, and saying so is what
keeps a later reader from mistaking the absence of an end-to-end test for an omission. The fit is regularised, which
is what makes the system solvable for every input a real run carries — including the degenerate one, a photograph of
a single flat colour. The requirement stands because "solvable" is a property of the arithmetic rather than a
promise the floating-point unit makes, and because the alternative to refusing is rendering the destroyed picture.

It does **not** extend to a model that answers with values that are not numbers. That system still solves; it is the
answer that is meaningless, and no correction fitted from it can be told apart from a strong one. A graph in that
state is a broken or wrongly-installed model, which is guarded where a model is installed rather than here.

#### Scenario: A degenerate fit

- **WHEN** the colour mapping for a run cannot be solved
- **THEN** the run fails and no image is produced
- **AND** the failure names the operation

### Requirement: The bias says in which direction and how far, and leaves the photograph alone at zero

A colour balance run carries a bias between -1 and 1 inclusive. The system SHALL apply the model's correction to the
photograph in proportion to it:

- At **0** the run SHALL return the photograph unchanged.
- At **1** the run SHALL return the model's correction in full.
- At a **negative** value the run SHALL apply the same correction in the opposite direction — the photograph moved
  away from the corrected image along the same line, rather than a second model or a second run in reverse.
- Between those the run SHALL move the photograph proportionally towards, or away from, the corrected image.

The negative half has no counterpart in the reference implementation, whose per-run control runs from 0 to 1. It is
the same extrapolation the light adjustment family's bias already offers, and it is deliberate.

The bias SHALL NOT select the weights. One model serves every value of it, so moving a control through a range of
values does not install or open anything a second time.

Two runs over one photograph differing only in their bias SHALL produce different results, and a result kept for one
SHALL NOT be served for the other. A negative bias and its positive counterpart are opposite corrections and SHALL NOT
share a kept result.

#### Scenario: A bias of zero

- **WHEN** a photograph is colour balanced at a bias of 0
- **THEN** the result is the photograph unchanged

#### Scenario: A negative bias

- **WHEN** a photograph is colour balanced at a bias of -1 and again at 1
- **THEN** the two results move the photograph's colour in opposite directions from where it started

#### Scenario: Two biases in one session

- **WHEN** a photograph is colour balanced twice at different biases
- **THEN** the second run is not served the first run's kept result
- **AND** the second run does not install or open the model a second time

### Requirement: Rio runs, and São Paulo is refused by name

The Rio variant SHALL produce a corrected photograph through the behaviour above, at every precision the family
publishes it in.

The São Paulo variant SHALL be refused, and the refusal SHALL name the operation as a user would see it. São Paulo's
graph answers a different question — a per-pixel weight map over several white-balance renderings of the photograph,
rather than one corrected image — so it is a second contract within this family rather than a second set of weights
for this one, and serving it through the behaviour above would render nonsense from correctly loaded weights.

The refusal SHALL be reported in the same terms as any other operation the library cannot yet run, so a caller need
not distinguish a family with no pipeline from a variant with none.

#### Scenario: Rio at either precision

- **WHEN** a photograph is colour balanced by Rio at FP32 and again at FP16
- **THEN** both produce a corrected photograph at the photograph's own resolution

#### Scenario: São Paulo

- **WHEN** a colour balance run is asked for with the São Paulo variant
- **THEN** the run is refused before any model is installed or opened
- **AND** the refusal names the operation

### Requirement: A run reports how far it has got and stops when the caller asks

A colour balance run SHALL report progress over the whole of its share of a chain and SHALL reach the end of that
share when it finishes.

The report SHALL advance during the work that follows the model run as well as around it. The model's share and the
fit are both fixed costs whatever the photograph is, while evaluating the mapping over the photograph is proportional
to it, so on a large photograph a report that advanced only around the model run would sit at one figure for most of
the time the run takes.

A run SHALL stop when the caller cancels it, and SHALL produce **no image at all** — not the photograph, and not the
partly corrected buffer the cancellation landed in. A caller handed a partly corrected picture with no error has no
way to tell it from a finished correction.

#### Scenario: Progress over a large photograph

- **WHEN** a photograph many times the model's square is colour balanced with a progress callback
- **THEN** progress advances after the model has run and before the result is complete
- **AND** the final value reported is the end of the operation's share

#### Scenario: A run cancelled partway

- **WHEN** a caller cancels a colour balance run after the model has run
- **THEN** the run fails rather than returning a result
- **AND** no image is produced

### Requirement: A photograph with no area is refused

Where the photograph has no width or no height, the system SHALL refuse the run and SHALL produce no image.

There is no scaling of an empty photograph onto the square, and no colour mapping to be fitted from no samples. This
is the same refusal the tiling of a zero-area image makes, for the same reason.

#### Scenario: An image with no area

- **WHEN** a colour balance is run over an image with no width or no height
- **THEN** the run fails and no image is produced
