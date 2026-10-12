# inference-tiling Specification

## Purpose

Running a model that accepts one fixed-size tile over an image of any size: how the image is divided,
how a region of pixels becomes the tensor a model reads and how the tensor it returns becomes pixels
again, at what bit depth, and how overlapping results are combined into one picture with no visible
seam - because every model the application ships accepts one shape and no photograph has it.

## Requirements

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

### Requirement: A model may be fed a tensor whose shape is declared rather than inferred


Where a model's input is not three colour planes of a square region, the caller SHALL state the shape
of the tensor it is supplying and the shape it expects back, and the system SHALL feed the model
exactly that.

Every model the system ran before this took one shape: three channels, square, at one fixed edge
length, so the shape could be recovered from the length of the buffer alone. A diffusion upscaler has
no such shape. It runs three graphs of three different shapes - colour planes in, a compressed latent
of sixteen channels out, a packed thirty-three-channel latent through the transformer - and two of
them are not three-channel and not at the image's own resolution. Recovering a shape from a buffer
length cannot distinguish them, and would silently feed one graph another's geometry.

Where the model returns a tensor of a different length than the caller declared, the system SHALL
report an error naming both lengths and SHALL NOT write a partial result. A graph whose output
disagrees with the shape it was run for is a malformed or mismatched export, and the buffer-length
check is the only place that disagreement becomes visible before the pixels are wrong.

#### Scenario: A model whose input is not three channels

- **WHEN** a model declaring a sixteen-channel input is run with that shape stated
- **THEN** it is fed a sixteen-channel tensor of the stated spatial size

#### Scenario: A model whose output is at a different resolution than its input

- **WHEN** a model is run with one input shape stated and a smaller output shape expected
- **THEN** the result is read back at the expected shape

#### Scenario: A model that returns the wrong number of values

- **WHEN** a model returns a tensor whose length differs from the declared output shape
- **THEN** an error naming the expected and actual lengths is returned
- **AND** no partial result is written

#### Scenario: A model that returns no tensor at all

- **WHEN** a model returns no output tensor
- **THEN** an error is returned rather than the run failing on an absent value

### Requirement: A model may return several output tensors, addressed by name

Where a model returns more than one output tensor, the caller SHALL name the outputs it wants and the
system SHALL return exactly those, each read back independently of the others.

Every model the system ran before this returned one tensor, so the output could be taken as "the one
the model produced" with no name spelled anywhere — which is deliberate, and stays that way for those
models: a conventionally exported one-in-one-out graph needs no name, and spelling one would make the
system fail on a graph that declared something else.

That argument inverts the moment a graph returns several. A face detector returns three tensors of
three different lengths and three different meanings — where each candidate is, how confident the
model is about it, and where its landmarks are — and taking them by position makes the system depend
on the order they happen to be declared in. A re-exported graph that declared them in another order
would then be read with confidences interpreted as positions, producing detections that are wrong
rather than absent, with nothing at any layer able to notice.

Where the model does not declare an output the caller named, the system SHALL report an error naming
the output that is missing, and SHALL produce no result at all. Where a named output returns a tensor
of a different length than the caller declared, the system SHALL report an error naming both lengths,
and SHALL NOT write a partial result — the same guarantee a single declared output already carries.

#### Scenario: A model with three named outputs

- **WHEN** a model declaring three outputs is run with all three named
- **THEN** each is read back at its own declared length, matched by name rather than by position

#### Scenario: A model whose outputs are declared in an unexpected order

- **WHEN** a model declares its named outputs in a different order than the caller listed them
- **THEN** each output is still matched to the name the caller asked for

#### Scenario: A model missing one of the named outputs

- **WHEN** a model does not declare an output the caller named
- **THEN** an error naming the missing output is returned
- **AND** no result is produced, including for the outputs that were present

#### Scenario: A named output of the wrong length

- **WHEN** a named output returns a tensor whose length differs from the declared length
- **THEN** an error naming the expected and actual lengths is returned
- **AND** no partial result is written

### Requirement: A model may be fed a second input that is a value rather than an image

Where a model's graph declares a second input carrying a single value the caller chooses — a weight, rather than
pixels — the system SHALL supply that value alongside the image tensor in one run, and SHALL read the output back at
the shape the caller declared, with the same length check a single-input run already carries.

Every model the system ran before this took one input, so the run could be expressed as "the tensor" with nothing
naming which input it was. A restoration model that takes a fidelity weight inverts nothing about that: the image is
still the first input and is still supplied the same way. What is new is that the caller has a second thing to say,
and it is a scalar rather than a tensor of pixels, so it is stated as a value rather than assembled into a buffer with
a shape of its own.

The two inputs SHALL be supplied **by position**, in the order the graph declares them, and SHALL NOT require the
caller to name them. This is the opposite of the rule for a model returning several outputs, and deliberately so: the
outputs of a face detector are three tensors of comparable shape and interchangeable type, so an export that reordered
them would be read wrongly with nothing able to notice. An image tensor and a single value are not interchangeable —
they differ in rank and in extent by orders of magnitude — so an export that swapped them fails at the runtime rather
than producing a plausible wrong answer, and naming them would instead make the system fail on a graph that named its
inputs something else.

Where the model returns a tensor of a different length than the caller declared, or returns none at all, the system
SHALL report an error and SHALL NOT write a partial result — the same guarantee a single-input declared-shape run
already carries.

#### Scenario: A model taking an image and a weight

- **WHEN** a model declaring an image input and a single-value input is run with both supplied
- **THEN** it is fed the image tensor at the declared shape and the value beside it, and the result is read back at the declared output shape

#### Scenario: The value reaching the model

- **WHEN** the same image is run against the same model at two different values
- **THEN** each run supplies the value it was given, and the two runs are not equivalent

#### Scenario: A model that returns the wrong number of values

- **WHEN** a model given an image and a value returns a tensor whose length differs from the declared output shape
- **THEN** an error naming the expected and actual lengths is returned
- **AND** no partial result is written

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
