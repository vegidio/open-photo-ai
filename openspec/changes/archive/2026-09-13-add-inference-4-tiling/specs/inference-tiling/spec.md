## Purpose

Running a model that accepts one fixed-size tile over an image of any size: how the image is divided,
how a region of pixels becomes the tensor a model reads and how the tensor it returns becomes pixels
again, at what bit depth, and how overlapping results are combined into one picture with no visible
seam - because every model the application ships accepts one shape and no photograph has it.

## ADDED Requirements

### Requirement: An image of any size is run through a model that accepts one fixed size

The system SHALL run a model that accepts a single fixed tile shape over an image of any dimensions,
by dividing the image into tiles of that shape, running the model once per tile, and combining the
results into one picture.

Every pixel of the image SHALL be covered by at least one tile. No tile SHALL extend outside the
image. No two tiles SHALL cover the same position.

An image with no area SHALL be refused with an error, and no picture SHALL be produced for it. This
is a failure and not an empty success: a caller handed an untouched buffer with no error has no way
to tell it from a finished enhancement, and would show it to a user as one.

#### Scenario: An image far larger than one tile

- **WHEN** an image many times the tile shape in both dimensions is run through a model
- **THEN** it is divided into tiles of the model's shape
- **AND** every pixel of the image is covered by at least one of them
- **AND** no tile extends beyond the image and no position is covered by two tiles

#### Scenario: An image smaller than one tile

- **WHEN** an image smaller than the tile shape in both dimensions is run through a model
- **THEN** it is covered by a single tile
- **AND** the result covers the whole image

#### Scenario: An image with no area

- **WHEN** an image with a width or a height of zero is run through a model
- **THEN** the run fails with an error
- **AND** no picture is produced

### Requirement: A tile that would fall outside the image is moved rather than shrunk

Where the last tile along an axis would extend past the edge of the image, the system SHALL move that
tile back so that it ends flush with the edge, rather than reducing its size. Every tile is therefore
the shape the model accepts, which is what lets one fixed-shape model cover an image of any
dimensions.

Where moving a tile places it at exactly the position of the tile before it, that tile SHALL be
dropped: running the model a second time over identical pixels produces an identical result at the
full cost of a run. Dropping it SHALL never leave a pixel uncovered.

#### Scenario: The last tile along an axis would overhang the edge

- **WHEN** an image's dimension is not an exact number of tile strides
- **THEN** the last tile along that axis ends flush with the edge of the image
- **AND** it is the same size as every other tile

#### Scenario: The moved tile lands on its predecessor

- **WHEN** moving the last tile back places it at exactly the position of the previous tile
- **THEN** only one of the two is run
- **AND** every pixel along that axis is still covered

### Requirement: A region of pixels becomes the tensor a model reads

The system SHALL convert a region of an image into the numeric input a model accepts: one value per
channel per pixel as 32-bit floating point, with the three colour channels held one after another
rather than interleaved, and the red, green and blue channels in that order.

The values SHALL be normalised either to the range 0 to 1 or to the range -1 to 1, as a caller
states, because the models the application ships were trained against both.

The conversion SHALL read a 16-bit source at its full precision rather than narrowing it to 8 bits
first. An 8-bit source SHALL produce exactly the values it produced before any 16-bit support
existed, so that the depth of the file changes the precision of the input and nothing else about it.

Where the region is smaller than the shape the model accepts - which happens only when the whole
image is smaller than one tile - the region SHALL be extended to that shape by mirroring its own edge
pixels outwards. It SHALL NOT be extended with a constant colour, which the model would treat as
picture content and enhance. Where the extension is wider than the region has pixels to mirror, the
outermost pixel SHALL be repeated rather than any value from outside the region being read.

#### Scenario: An 8-bit and a 16-bit version of one picture

- **WHEN** an 8-bit image and a 16-bit image of the same picture are each converted
- **THEN** both produce values normalised over the same range
- **AND** the 16-bit one carries the precision its file holds rather than the precision an 8-bit file
  would have held

#### Scenario: A region smaller than the model's shape

- **WHEN** an image smaller than the tile shape is converted
- **THEN** the input is extended to the model's shape by mirroring the image's edge pixels
- **AND** no constant-coloured border appears in it

#### Scenario: An extension wider than the region

- **WHEN** an image only a few pixels across is extended to the full tile shape
- **THEN** the outermost pixels are repeated to fill what cannot be mirrored
- **AND** no value from outside the image is read

### Requirement: A model's output becomes pixels at the bit depth the caller asks for

The system SHALL convert a model's numeric output back into pixels, reversing the normalisation it
was produced under.

The caller SHALL state whether the result is to carry 8 or 16 bits per channel, and the system SHALL
produce that depth. A model's output is floating point and carries more precision than either, so
neither depth is the natural one and fixing it at 8 would discard what a 16-bit source and a 16-bit
export both preserve.

A value outside the range the model was to produce SHALL be clamped to the nearest value the channel
can hold, rather than wrapping to the far end of it.

The result SHALL carry colour only. Alpha is not preserved through a model run, and a result that
carried an alpha channel would be claiming otherwise.

#### Scenario: The same output at two depths

- **WHEN** one model output is converted at 8 bits per channel and at 16 bits per channel
- **THEN** each result carries the depth that was asked for
- **AND** both describe the same picture

#### Scenario: An output beyond the expected range

- **WHEN** a model returns values above or below the range it normalises over
- **THEN** they are clamped to the extremes of the channel
- **AND** no value wraps to the opposite extreme

### Requirement: Overlapping tiles are combined without a visible seam

Tiles overlap their neighbours, and where two tiles cover one pixel the system SHALL combine them
across the whole width they genuinely share, weighting each tile so that its contribution rises
smoothly from the edge of the shared band to full strength at the far side of it. An edge that meets
no already-written neighbour SHALL be written at full strength.

The width combined SHALL be the overlap those two tiles actually have, which is not in general the
overlap the division was configured with: a tile moved back to sit flush with the edge of the image
overlaps its predecessor by however far it was moved, which at the last column and the last row can
be many times the configured figure. Combining such a seam over the configured width instead leaves
everything past it written at full strength, which is a hard edge down the last column and along the
last row.

Where a pixel lies in a shared band on both axes, both weightings SHALL apply to it.

#### Scenario: A seam far wider than the configured overlap

- **WHEN** the last column of tiles was moved back and so overlaps its neighbour by far more than the
  configured overlap
- **THEN** the two are combined across the whole width they share
- **AND** no abrupt step in value appears anywhere across that seam

#### Scenario: The first tile of a row

- **WHEN** a tile has no already-written neighbour on an edge
- **THEN** that edge is written at full strength
- **AND** nothing outside the image is read to weight it

#### Scenario: A corner covered on both axes

- **WHEN** a pixel lies within the shared band of both a horizontal and a vertical neighbour
- **THEN** both weightings are applied to it

### Requirement: The result is the image at the model's own scale

The system SHALL produce a result whose dimensions are the input's, each multiplied by the scale the
model applies - unchanged for a model that returns an image the size of its input, and multiplied for
one that enlarges.

The extension added to a region smaller than the model's shape SHALL NOT appear in the result: it is
removed after the run, at the same scale the model applied.

#### Scenario: A model that enlarges

- **WHEN** an image is run through a model that returns four times the width and height it is given
- **THEN** the result is four times the input's width and height

#### Scenario: A model that preserves size

- **WHEN** an image is run through a model that returns an image the size of its input
- **THEN** the result has the input's dimensions
- **AND** the mirrored extension of any partial region does not appear in it

### Requirement: A run reports its progress, stops when asked, and fails as a whole

The system SHALL report progress to a caller that asks for it as tiles complete, advancing
monotonically and reaching exactly 1 when the last tile is done rather than stopping just short of
it. A caller that does not ask for progress SHALL not be charged for reporting it.

The system SHALL stop between tiles when the caller signals that the run is no longer wanted, and
SHALL report that it stopped rather than returning a partly-finished picture as a result.

Where the model fails on one tile, the run SHALL fail reporting that failure. It SHALL NOT return the
tiles that had already succeeded: a picture whose remaining area is whatever the buffer held is not a
partial result, it is a wrong one.

#### Scenario: Progress across a multi-tile run

- **WHEN** an image covered by many tiles is run to completion with a progress callback
- **THEN** progress never decreases
- **AND** the final value reported is exactly 1

#### Scenario: A caller that no longer wants the run

- **WHEN** the caller signals cancellation partway through a run
- **THEN** the run stops before starting the next tile
- **AND** it reports having been cancelled rather than returning a picture

#### Scenario: A model that fails on one tile

- **WHEN** the model fails on a tile partway through a run
- **THEN** the run fails reporting that failure
- **AND** no picture is returned
