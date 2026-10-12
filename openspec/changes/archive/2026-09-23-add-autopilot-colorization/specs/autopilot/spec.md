## ADDED Requirements

### Requirement: A grayscale photograph calls for colorization

The system SHALL read a monochrome signal and SHALL suggest colorization for a **grayscale** photograph: one that
carries no colour at all. That includes a photograph stored with a single brightness channel, one stored as colour
with its three channels equal, and one of either kind that was saved as colour and compressed, whose encoder left a
trace of rounding off grey.

The system SHALL NOT be required to suggest colorization for a black-and-white print that was toned (sepia, selenium,
cyanotype), split-toned, yellowed with age or scanned in colour. Measured on real scans of such prints, the colour
their brightness does not explain (stains, uneven toning, the scanner's noise) is more than the haziest and darkest
colour photographs carry, so no statistic of this kind can find them without flagging those photographs. Leaving them
alone is the accepted cost.

Colorization replaces a photograph's colour rather than blending with it, so a false alarm destroys colour that was
really there. The system SHALL NOT suggest colorization for a colour photograph, **including a muted one**: fog, snow,
night, overcast, or a scene of one dominant hue. Where the evidence is borderline, it SHALL prefer not suggesting.

The system SHALL judge the photograph as a whole, and a small coloured subject on a grey scene SHALL NOT be a reason to
suggest colorization.

The system SHALL read this signal from the image's own pixels. It SHALL NOT run a model, install anything or touch the
network to read it, and it SHALL NOT be a reason an analysis is reported incomplete. It SHALL cost no more for a large
photograph than for a photograph of a bounded size. A photograph that carries 16 bits per channel SHALL be measured at
16 bits, and transparent pixels SHALL NOT contribute. A photograph with nothing left to measure SHALL conclude nothing
for this signal rather than a guess. A cancelled analysis SHALL still return nothing.

The reference implementation reads no monochrome signal and never suggests colorization. This signal is new rather
than ported.

#### Scenario: A neutral black-and-white photograph

- **WHEN** a photograph whose every pixel is equal in all three channels is analysed
- **THEN** colorization is among the suggestions

#### Scenario: A photograph stored with a single channel

- **WHEN** a photograph stored as grayscale, with one brightness channel and no colour channels, is analysed
- **THEN** colorization is among the suggestions

#### Scenario: A grayscale photograph saved as a colour JPEG

- **WHEN** a grayscale photograph that was saved as a colour JPEG is analysed
- **THEN** colorization is among the suggestions

#### Scenario: A colour photograph

- **WHEN** a colour photograph is analysed
- **THEN** colorization is not among the suggestions

#### Scenario: A muted colour photograph

- **WHEN** a colour photograph of a scene with little colour, such as fog, snow or an overcast street, is analysed
- **THEN** colorization is not among the suggestions

#### Scenario: A grayscale photograph that is also underexposed

- **WHEN** a grayscale photograph that the light signal concludes is underexposed is analysed
- **THEN** both colorization and light adjustment are among the suggestions, colorization first

#### Scenario: No model is available

- **WHEN** a grayscale photograph is analysed and no model can be installed or opened
- **THEN** colorization is still among the suggestions

#### Scenario: A cut-out on a transparent background

- **WHEN** a colour subject surrounded by fully transparent pixels is analysed
- **THEN** colorization is not among the suggestions

### Requirement: Suggestions come back in the reference's family order

The system SHALL return suggestions in the single canonical order of families the library names, rather than in the
order the signals happen to be read in.

Ordering SHALL be a property of the family, so that a family added later takes its place in that one order without
the ordering having to be revisited.

This is deliberately *not* stated as "the order the enhancement path applies them". The enhancement path applies
operations in the order its caller supplies them and imposes no family order at all, so there is no such order in
the library to defer to. The reference implementation does hold one list that is both the order enhancements are
presented in and the order they are applied in, because an add menu offering them in one order while the pipeline
ran them in another is a silent inconsistency: denoise, face recovery, colorization, light adjustment, colour balance,
sharpen, upscale.

The library's canonical order SHALL agree with that list on **every** family a user can add. Colorization comes before
light adjustment and colour balance: colorization replaces the photograph's colour, so exposure and colour are
corrected on the colour it produced rather than on colour it will discard. The agreement SHALL be a checked fact
rather than described in prose, so that a change moving either order has to settle the disagreement rather than
introduce it silently.

#### Scenario: Several suggestions at once

- **WHEN** an image is analysed and more than one signal concludes it needs an enhancement
- **THEN** the suggestions come back in the library's canonical family order, whatever order their signals were read
  in

#### Scenario: Colorization beside light adjustment and colour balance

- **WHEN** an image is analysed and colorization, light adjustment and colour balance are all suggested
- **THEN** colorization comes back before light adjustment, and light adjustment before colour balance

#### Scenario: The reference's order

- **WHEN** the library's canonical family order is compared with the reference's present-and-apply order, over the
  families a user can add
- **THEN** the two are the same order
- **AND** a check fails if either order moves

## REMOVED Requirements

### Requirement: Suggestions come back in one canonical family order

**Reason**: It pinned a disagreement with the reference about colorization's place, for the change that made
colorization suggestible to settle. This change settles it, so the requirement's substance changes from "a
disagreement is recorded" to "the orders agree", and its disagreement scenario no longer describes anything.

**Migration**: Replaced by "Suggestions come back in the reference's family order", which keeps the rest of it: one
canonical order that is a property of the family, and a check that fails if either order moves.
