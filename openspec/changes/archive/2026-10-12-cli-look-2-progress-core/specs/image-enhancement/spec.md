## MODIFIED Requirements

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

Each report SHALL also say which operation of the chain it concerns, as that operation's position in the chain as it
is run, and how many operations the chain has. The position SHALL count the chain in the order it is actually
applied, which is not necessarily the order the caller listed it in, so that a caller can number the steps of a run
without reproducing the ordering itself. Every report about one operation SHALL carry the same position, whatever its
stage, and a report about the only operation of a single-operation run SHALL say it is the first of one.

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

#### Scenario: Which step of the chain a report concerns

- **WHEN** an image is run through a chain of two operations, listed in the opposite order to the one they are applied
  in, with a progress callback
- **THEN** every report about the operation applied first says it is the first of two, and every report about the
  other says it is the second of two
- **AND** this holds for reports of a transfer, of a run, and of a result that was already known

#### Scenario: A single-operation run

- **WHEN** a single operation is run on its own with a progress callback
- **THEN** every report says it concerns the first of one operation
