## ADDED Requirements

### Requirement: Tiles may instead be combined by averaging every contribution

Where a model's per-tile output is stochastic - two tiles covering one pixel do not agree on it even
when the model is working correctly - the system SHALL combine tiles by weighting every contribution
and averaging them, rather than by writing each tile over what is already in the picture.

Writing each tile over the last can average at most two tiles. Where four tiles meet, the result then
depends on the order they were written, and that corner is precisely where a seam shows for a model
whose tiles disagree. Averaging every contribution SHALL make each pixel's value independent of the
order the tiles were produced in.

Each contribution SHALL be weighted by the same smooth ramp used where tiles are written over one
another, across the width the two tiles genuinely share, so that the two ways of combining tiles
cannot drift apart. An edge that lies on the border of the image SHALL NOT be ramped: nothing overlaps
it, so ramping there drives the accumulated weight towards zero along the outside of the picture and
amplifies whatever survived the averaging.

Every pixel of the image SHALL be covered by at least one tile before the average is taken, and the
system SHALL NOT produce a value for a pixel no tile covered.

Accumulation SHALL happen at the precision the model produces, and the result SHALL be reduced to the
caller's channel depth once, after the average - so a tile is not quantised to a channel depth and
back on its way into the picture.

#### Scenario: Four tiles meeting at one corner

- **WHEN** a pixel is covered by four tiles whose outputs disagree
- **THEN** its value is the weighted average of all four
- **AND** the value does not depend on the order the four were produced in

#### Scenario: A tile edge lying on the image border

- **WHEN** a tile's edge is on the border of the image, with no tile overlapping it
- **THEN** that edge contributes at full weight
- **AND** the pixels along it are not darkened or amplified by the average

#### Scenario: A seam wider than the configured overlap

- **WHEN** the last column of tiles was moved back and so overlaps its neighbour by far more than the
  configured overlap
- **THEN** the contributions are weighted across the whole width the two share
- **AND** no abrupt step in value appears anywhere across that seam

#### Scenario: A result at a caller's channel depth

- **WHEN** tiles are averaged into a picture the caller asked for at a given channel depth
- **THEN** the average is taken before the depth is applied
- **AND** no tile is reduced to that depth and back on its way into the picture
