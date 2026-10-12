# sharpen Specification

## Purpose

What a sharpen run does to a photograph: that it returns the photograph at its own resolution, what the per-run strength
means at zero, at one and past it, what the divergence guard keeps and for which model, and what the run reports, stops
on and refuses.

## Requirements

### Requirement: The photograph is sharpened at its own resolution

A sharpen run SHALL return a photograph with exactly the width and height of the photograph it was given. The model
accepts one fixed tile size, so the system SHALL run it over the whole photograph in overlapping tiles and combine their
results into one picture with no visible seam, as every tiled model in this application is run.

A photograph smaller than one tile SHALL be sharpened as readily as a larger one, and SHALL come back at its own size
rather than at the tile's.

The result SHALL carry the bit depth the caller asked for, as every operation's does.

#### Scenario: A photograph larger than one tile

- **WHEN** a photograph several tiles wide and high is sharpened
- **THEN** the result has the photograph's own width and height
- **AND** no seam is visible where neighbouring tiles met

#### Scenario: A photograph smaller than one tile

- **WHEN** a photograph smaller than the model's tile is sharpened
- **THEN** the result has the photograph's own width and height

### Requirement: The strength says how much of the sharpening lands, and may go past it

A sharpen run carries a strength between 0 and 3 inclusive. The system SHALL apply the model's sharpening to the
photograph in proportion to it:

- At **0** the run SHALL return the photograph unchanged.
- At **1** the run SHALL return the model's output unchanged.
- Between 0 and 1 the run SHALL move the photograph proportionally towards the model's output.
- Above **1** the run SHALL move the photograph further along the same line, past the model's output. That is an
  amplified sharpening, not a second run, and it is bounded to the range a channel can hold.

The strength SHALL NOT select the weights. One model serves every value of it, so moving a control through a range of
values does not install or open anything a second time.

Two runs over one photograph differing only in their strength SHALL produce different results, and a result kept for
one SHALL NOT be served for the other.

#### Scenario: A strength of zero

- **WHEN** a photograph is sharpened at a strength of 0
- **THEN** the result is the photograph unchanged

#### Scenario: A strength of one

- **WHEN** a photograph is sharpened at a strength of 1
- **THEN** the result is the model's output unchanged

#### Scenario: A strength past one

- **WHEN** a photograph is sharpened at a strength of 1 and again at 2
- **THEN** the second result differs from the photograph by more than the first does, in the same direction
- **AND** no channel of either result falls outside the range a channel can hold

#### Scenario: Two strengths in one session

- **WHEN** a photograph is sharpened twice at different strengths
- **THEN** the second run is not served the first run's kept result
- **AND** the second run does not install or open the model a second time

### Requirement: A tile Petersburg diverges on keeps the photograph's own pixels

Petersburg is a NAFNet model, the architecture that occasionally produces a numerically exploded output on a tile unlike
what it was trained on, which would otherwise appear as a solid saturated block. Where any value Petersburg returns for a
tile, before it is turned into pixels, exceeds **3.0** in magnitude, the system SHALL discard that tile's output and
SHALL use the tile's own input pixels in its place, and SHALL continue the run.

The kept pixels SHALL be the photograph's at the bit depth the run produces, not a reduced-depth copy of them. Tiles
around a kept tile SHALL be combined with it exactly as with any other tile, so the kept region meets its neighbours
without a seam.

Moscow and Novgorod SHALL NOT be guarded: every tile of theirs contributes its output, whatever its magnitude.

The threshold is inherited from the reference implementation. It places the threshold safely above legitimate output
(of the order of 1) and far below the observed NAFNet blow-up (of the order of 1000). No photograph known to trigger it
on Petersburg was available when this requirement was written, so it is verified against a model that is made to
diverge rather than against Petersburg itself.

#### Scenario: A tile that diverges under Petersburg

- **WHEN** Petersburg returns a value greater than 3.0 in magnitude anywhere in one tile's output
- **THEN** that tile's region of the result is the photograph's own pixels before the strength is applied
- **AND** every other tile's region is the model's output
- **AND** the run completes and returns an image

#### Scenario: A large value under a model that is not guarded

- **WHEN** Moscow or Novgorod returns a value greater than 3.0 in magnitude in a tile's output
- **THEN** that tile's output is used like any other's

#### Scenario: A value at the threshold

- **WHEN** Petersburg's largest output magnitude on a tile is exactly 3.0
- **THEN** that tile's output is used

### Requirement: All three of the family's variants run, through one contract

Every variant this family publishes SHALL produce a sharpened photograph through the behaviour above. The variants
differ in their weights, in whether they are guarded, and in the provider settings measured for them. They SHALL NOT
differ in how a photograph is tiled, in how their output becomes pixels, or in what the strength means.

Choosing a different variant at the same strength SHALL produce a different result, and a result kept for one SHALL NOT
be served for the other.

#### Scenario: Each variant over one photograph

- **WHEN** a photograph is sharpened by each of the family's variants in turn at the same strength
- **THEN** each produces a sharpened photograph at the photograph's own resolution
- **AND** no two of the results are identical

### Requirement: A run reports how far it has got and stops when the caller asks

A sharpen run SHALL report progress over the whole of its share of a chain, and SHALL never report a value lower than
one it reported before. It SHALL reach the end of that share only when the result is complete: after the strength has
been applied, not when the last tile has run.

A run SHALL stop when the caller cancels it, and SHALL produce **no image at all**: not the photograph, and not the
partly covered buffer the cancellation landed in.

#### Scenario: Progress over a photograph of many tiles

- **WHEN** a photograph of many tiles is sharpened with a progress callback
- **THEN** progress advances as tiles complete
- **AND** the end of the operation's share is reported only once the strength has been applied

#### Scenario: A run cancelled partway

- **WHEN** a caller cancels a sharpen run between two tiles
- **THEN** the run fails rather than returning a result
- **AND** no image is produced

### Requirement: A photograph with no area is refused

Where the photograph has no width or no height, the system SHALL refuse the run and SHALL produce no image. This is the
same refusal the tiling of any zero-area image makes.

#### Scenario: An image with no area

- **WHEN** a sharpen is run over an image with no width or no height
- **THEN** the run is refused
- **AND** no image is produced
