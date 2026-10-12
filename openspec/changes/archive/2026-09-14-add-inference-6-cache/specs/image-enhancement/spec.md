## MODIFIED Requirements

### Requirement: An image and an ordered chain of operations produce an enhanced image

The system SHALL accept an image together with an ordered sequence of operations and produce one
enhanced image, each operation applied to the result of the one before it.

An empty sequence SHALL return the image unchanged rather than being refused: applying nothing is a
meaningful request, and it is what a caller with a user who has toggled every enhancement off sends.

The result SHALL carry an identity composed from the input's identity, every operation applied, and
the bits per channel the result carries, so that two runs differing only in which operations were
applied are distinguishable, two runs differing only in the depth asked for are distinguishable, and
a second run over the result of a first is not mistaken for the first. The result SHALL NOT carry the
input's own identity, which would make a chained pair of runs indistinguishable from the first run
alone.

Where two runs would produce the same pixels they SHALL carry the same identity, and where they would
produce different pixels they SHALL NOT. An identity that did not distinguish depth would name two
different images: the same chain at 8 and at 16 bits per channel produces two results that are not
interchangeable, and anything keyed on that identity would serve one where the other was asked for.

Where an operation's result is already known - because an identical operation over an identical image
has already been carried out and its result kept - the system MAY produce that result without running
the model, provided the result is the one the model would have produced. What is kept and for how long
is the image cache's own contract.

#### Scenario: One operation over an image

- **WHEN** an image is run through a chain of one supported operation
- **THEN** an enhanced image is produced
- **AND** it carries an identity that is neither the input's nor that of the same image run through a
  different operation

#### Scenario: An empty chain

- **WHEN** an image is run through a chain of no operations
- **THEN** the image is returned unchanged
- **AND** the run succeeds

#### Scenario: A run chained onto the result of another

- **WHEN** an image is run through one operation and its result is then run through a second, both at
  the same depth
- **THEN** the final identity is the same as running both operations in one chain at that depth
- **AND** it differs from the identity of either single run

#### Scenario: The same chain at two different depths

- **WHEN** an image is run through the same chain twice, once asking for 8 bits per channel and once
  for 16
- **THEN** the two results carry different identities

#### Scenario: Two ways of asking for the same depth

- **WHEN** a 16-bit image is run through a chain once asking for 16 bits per channel and once asking
  for whatever the source carried
- **THEN** the two results carry the same identity, because they are the same pixels

### Requirement: A run reports what it is doing and how far it has got

The system SHALL report progress to a caller that asks for it, and a caller that does not ask SHALL
not be charged for reporting it.

Each report SHALL name the operation it concerns and SHALL say whether the model is being put on disk,
being run, or was not run at all because the result was already known - so that a caller can
distinguish a transfer from an enhancement and either from neither. The three take different amounts
of time, for different reasons, and a bar that cannot tell them apart cannot explain either a long
wait or an instant one.

Each report SHALL carry how far the whole chain has got. That figure SHALL never decrease over one
run, and SHALL reach exactly 1 when the run finishes rather than stopping just short of it. In
particular it SHALL NOT return to the start when one operation ends and the next begins, when a
transfer hands over to a run, or when one pass of a multi-pass upscale hands over to the next.

An operation whose result was already known SHALL still contribute its full share of that figure, so
that a chain of operations that all had known results still fills the bar rather than leaving it
stalled at the start.

Where an operation is served by several passes over progressively larger images, the passes SHALL
contribute to that figure in proportion to the work they do rather than equally, so that the reported
progress does not stall on whichever pass has the most pixels to cover.

#### Scenario: Progress across a chain of several operations

- **WHEN** an image is run through several operations with a progress callback
- **THEN** the reported progress never decreases
- **AND** it does not return to the start when one operation hands over to the next
- **AND** the final value reported is exactly 1

#### Scenario: A model that has to be installed first

- **WHEN** an operation's model is not yet on disk
- **THEN** the reports during the transfer say the model is being installed and name the operation
- **AND** the reports after it say it is being run
- **AND** progress does not return to the start between the two

#### Scenario: A multi-pass upscale

- **WHEN** an upscale is served by two passes, the second covering far more pixels than the first
- **THEN** progress does not return to the start when the first pass hands over to the second
- **AND** the second pass accounts for the larger share of the operation's progress

#### Scenario: An operation whose result was already known

- **WHEN** an operation of a chain is not run because its result was already known
- **THEN** a report names that operation and says its result was already known
- **AND** progress advances by that operation's full share

#### Scenario: A chain every operation of which was already known

- **WHEN** every operation of a chain has a result that is already known
- **THEN** the run reports progress reaching exactly 1
- **AND** it does not finish with the bar still at the start
