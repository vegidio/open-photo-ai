## MODIFIED Requirements

### Requirement: Only the most recent enhanced result is held

The application SHALL hold the result of the most recent completed run, and SHALL stop serving any
result a later run has displaced.

It SHALL also stop holding a result once the image it was made from is no longer open, so a result is
released by the user finishing with the photograph as well as by a later run replacing it. Holding
only until the next run is what left a large result resident for the whole session when no next run
came.

A request for a result that is no longer held SHALL be refused distinguishably from one naming a
result that never existed, so an interface drawing a stale address can tell the two apart.

Holding one is what bounds the cost: a fourfold enlargement of a large photograph is over a gigabyte
of pixels, and holding every result a user had passed through would grow without limit over a session.

#### Scenario: A superseded result

- **WHEN** the pixels of an enhanced result are requested after a later run has completed
- **THEN** the request is refused, and reported as no longer held rather than as unknown

#### Scenario: A result whose image has been closed

- **WHEN** the pixels of an enhanced result are requested after the image it was made from was closed
- **THEN** the request is refused, and reported as no longer held rather than as unknown

#### Scenario: A result that never existed

- **WHEN** pixels are requested for an identity no run has produced
- **THEN** the request is refused and reported as unknown

## ADDED Requirements

### Requirement: An enhanced result is released when its image is closed

The application SHALL stop holding an enhanced result's pixels once the image it was made from is no
longer open.

Closing one image SHALL release only what was enhanced from that image, and SHALL leave a result
belonging to any other image untouched - including one produced by a run still in flight. Closing
every open image SHALL release every held result.

An image with no identity has produced nothing and SHALL release nothing.

A run that was working on the closed image when it was closed SHALL NOT go on to have its result held.
Stopping a run is cooperative, so one can finish without noticing it was asked to stop; holding what it
produced would leave a closed image's result resident for the session, which is the whole of what
releasing prevents.

#### Scenario: The enhanced image is closed

- **WHEN** an image that has been enhanced is closed
- **THEN** the application no longer holds the enhanced pixels

#### Scenario: Another image is closed while a result is held

- **WHEN** an image is closed and the held result belongs to a different image
- **THEN** the held result is kept

#### Scenario: Every image is closed

- **WHEN** the user closes all open images at once
- **THEN** no enhanced result is held

#### Scenario: A run finishes after the image it was working on was closed

- **WHEN** a run that was enhancing an image finishes after that image was closed
- **THEN** its result is not held, and the run is reported as stopped

### Requirement: The record of produced results is bounded

The record the application keeps of results it has produced SHALL NOT grow without limit for the life
of the process.

An identity old enough to have left that record SHALL be refused as unknown. Nothing can still be
drawing it: an address is held because a pane is drawing it, and a pane draws a result because the run
that produced it was recent.

#### Scenario: A long session producing many results

- **WHEN** the application has produced more results than the record holds
- **THEN** the record has not grown past its limit, and the most recent results are still reported as
  no longer held once released
