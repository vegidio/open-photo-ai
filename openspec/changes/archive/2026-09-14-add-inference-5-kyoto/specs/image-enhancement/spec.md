## Purpose

Running a chosen enhancement over an image end to end: what a run accepts and what it refuses, the
bit depth the result carries, how a requested scale that no single set of weights serves is covered by
a sequence of passes and corrected afterwards, what a run reports while it works, and what happens
when a caller stops one - because naming a model, putting it on disk, opening it and tiling an image
are four things this application can already do separately and none of them is an enhancement.

## ADDED Requirements

### Requirement: An image and an ordered chain of operations produce an enhanced image

The system SHALL accept an image together with an ordered sequence of operations and produce one
enhanced image, each operation applied to the result of the one before it.

An empty sequence SHALL return the image unchanged rather than being refused: applying nothing is a
meaningful request, and it is what a caller with a user who has toggled every enhancement off sends.

The result SHALL carry an identity composed from the input's identity and every operation applied, so
that two runs differing only in which operations were applied are distinguishable, and a second run
over the result of a first is not mistaken for the first. The result SHALL NOT carry the input's own
identity, which would make a chained pair of runs indistinguishable from the first run alone.

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

- **WHEN** an image is run through one operation and its result is then run through a second
- **THEN** the final identity is the same as running both operations in one chain
- **AND** it differs from the identity of either single run

### Requirement: A run is refused rather than served wrongly where no pipeline exists

Where the system has no pipeline for an operation in the chain, the run SHALL fail with an error
naming that operation, and SHALL produce no image.

It SHALL NOT fall back to returning the image unchanged, nor to any result produced without the model
the operation names: a caller handed an unenhanced image with no error has no way to tell it from an
enhancement, and would show it to a user as one. This is the same refusal the tiling of a zero-area
image makes, for the same reason.

A refusal SHALL happen before any model is installed or opened, so that a chain naming an operation
that cannot be served does not first transfer gigabytes on its behalf.

#### Scenario: An operation whose family has no pipeline

- **WHEN** a chain names an operation from a family the system cannot yet run
- **THEN** the run fails with an error naming that operation
- **AND** no image is produced and no model is transferred

#### Scenario: An unservable operation later in the chain

- **WHEN** a chain's first operation can be served and its second cannot
- **THEN** the run fails before the first operation's model is installed or opened
- **AND** no image is produced

### Requirement: The result carries 8 bits per channel unless the caller asks otherwise

A caller that states no depth SHALL receive a result carrying 8 bits per channel. A model's output is
floating point and carries more precision than any channel depth, so no depth is the natural one and
one has to be chosen; 8 is chosen because it is what every enhancement produced before a choice
existed, so a caller that says nothing is not silently given something new.

A caller MAY instead state that the result is to carry 16 bits per channel, or whatever the source
image carried, and the system SHALL then produce that depth. Carrying more than 8 bits is therefore a
deliberate request and never an accident.

Where the caller asks for the source's own depth, it SHALL be resolved against the image actually
loaded: a source carrying more than 8 bits per channel resolves to 16, and any other source resolves
to 8.

Where a chain applies several operations, every operation SHALL produce the same depth, so that the
depth of the result does not depend on how many enhancements a user happened to select.

#### Scenario: A caller that states no depth

- **WHEN** an image is enhanced with no depth stated
- **THEN** the result carries 8 bits per channel

#### Scenario: A 16-bit source with no depth stated

- **WHEN** a 16-bit image is enhanced with no depth stated
- **THEN** the result carries 8 bits per channel, because carrying more is a request rather than a
  default

#### Scenario: A 16-bit source at the source's own depth

- **WHEN** a 16-bit image is enhanced with the result asked to carry whatever the source carried
- **THEN** the result carries 16 bits per channel

#### Scenario: An 8-bit source at the source's own depth

- **WHEN** an 8-bit image is enhanced with the result asked to carry whatever the source carried
- **THEN** the result carries 8 bits per channel

#### Scenario: A depth asked for against the source's own

- **WHEN** an 8-bit image is enhanced with the result asked to carry 16 bits per channel
- **THEN** the result carries 16 bits per channel

### Requirement: A requested scale is covered by native passes and corrected afterwards

An upscale's weights are published per whole factor, and a user may ask for any scale in the
supported range. The system SHALL cover a requested scale by running the sequence of native passes
that reaches or exceeds it, each pass taking the previous pass's result.

Where that sequence produces an image larger than the request - a request no whole factor matches -
the system SHALL resample the result to exactly the requested scale after the last pass, and SHALL
NOT resample between passes: a model's input is the previous model's output, and resampling in
between would feed the next pass an interpolation rather than a restoration.

The final result's dimensions SHALL be the input's, each multiplied by the requested scale and
rounded, whether or not any pass overshot.

Where no sequence of passes covers a requested scale, the run SHALL fail. It SHALL NOT return a plain
resample of the input presented as a successful enhancement.

#### Scenario: A scale a single pass serves exactly

- **WHEN** an image is upscaled by a factor one set of weights serves natively
- **THEN** one pass runs
- **AND** the result is that factor times the input's width and height, with no resample afterwards

#### Scenario: A scale no whole factor serves

- **WHEN** an image is upscaled by a factor between two native factors
- **THEN** the passes covering it run and the result is resampled once, after the last of them
- **AND** the final dimensions are the input's multiplied by the requested scale

#### Scenario: A scale needing several passes

- **WHEN** an image is upscaled by a factor larger than any single set of weights serves
- **THEN** the passes run in sequence, each over the previous one's result
- **AND** the final dimensions are the input's multiplied by the requested scale

### Requirement: A run reports what it is doing and how far it has got

The system SHALL report progress to a caller that asks for it, and a caller that does not ask SHALL
not be charged for reporting it.

Each report SHALL name the operation it concerns and SHALL say whether the model is being put on disk
or being run, so that a caller can distinguish a transfer from an enhancement - the two take
different amounts of time, for different reasons, and a bar that cannot tell them apart cannot explain
a long wait.

Each report SHALL carry how far the whole chain has got. That figure SHALL never decrease over one
run, and SHALL reach exactly 1 when the run finishes rather than stopping just short of it. In
particular it SHALL NOT return to the start when one operation ends and the next begins, when a
transfer hands over to a run, or when one pass of a multi-pass upscale hands over to the next.

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

### Requirement: A run stops when the caller asks, and fails as a whole

The system SHALL stop a run when the caller signals that it is no longer wanted, and SHALL report
that it stopped rather than returning a partly-finished picture.

A run SHALL be stoppable while a model is being run, not only between operations: an upscale of a
large photograph is the longest single wait this application asks a user to sit through, and a cancel
that took effect only at the end of it would not be a cancel.

Where any operation of a chain fails, the whole run SHALL fail reporting that failure, and SHALL NOT
return the results of the operations that had already succeeded. A failure SHALL be distinguishable
by a caller between a model that could not be put on disk, a model that could not be opened, and a
model that failed while running - the three are reported to a user differently and are acted on
differently.

A run stopped by the caller SHALL be distinguishable from a run that failed, so that a caller does not
tell a user their enhancement broke when they cancelled it themselves.

#### Scenario: A caller that cancels partway through a long run

- **WHEN** the caller signals cancellation while a model is running over an image
- **THEN** the run stops without finishing the remaining work
- **AND** it reports having been cancelled rather than returning a picture

#### Scenario: An operation that fails partway through a chain

- **WHEN** the second operation of a chain fails
- **THEN** the run fails reporting that failure
- **AND** the first operation's result is not returned

#### Scenario: A model that cannot be put on disk

- **WHEN** an operation's model cannot be installed
- **THEN** the run fails reporting that the model could not be installed
- **AND** the failure is distinguishable from one where the model was opened and failed while running

### Requirement: A run does not block the caller's event loop, and holds its models until it finishes

Running a model is seconds to minutes of uninterruptible computation. The system SHALL carry it out
without preventing the caller from continuing to run its own work - including the reporting of the
progress the run itself produces, which a caller that was blocked could not display.

Every model a chain uses SHALL stay open until the whole chain finishes, not merely until the
operation using it returns. Releasing one earlier would let it be reclaimed while a later operation is
still running, and the cost of opening it again would be charged to the run that is already in
progress.

A model in use SHALL NOT be reclaimed by anything, whatever else asks for it, for as long as the run
holds it.

#### Scenario: A long run alongside other work

- **WHEN** a chain is running over a large image
- **THEN** the caller continues to make progress on its own work
- **AND** the progress the run reports reaches the caller while the run is still going

#### Scenario: A model reclaimed while a later operation still needs it

- **WHEN** a chain's first operation finishes and something else asks for that model to be released
- **THEN** the chain's later operations still run against a usable model
- **AND** the model is not opened a second time on the running chain's account
