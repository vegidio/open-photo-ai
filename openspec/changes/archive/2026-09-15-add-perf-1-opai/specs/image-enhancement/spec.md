## MODIFIED Requirements

### Requirement: An image and an ordered chain of operations produce an enhanced image

The system SHALL accept an image together with an ordered sequence of operations and produce one
enhanced image, each operation applied to the result of the one before it, together with a report of
what the run was executed on — the contract for which is the requirement below.

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

The identity SHALL be composed from the image and the operations alone. What the run was executed on
SHALL NOT enter it: the same chain over the same image produces the same pixels whether it ran on a
GPU or on the CPU, and an identity that moved with the provider would make a result computed on one
machine useless to another.

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

#### Scenario: The same chain run on two different providers

- **WHEN** one image is run through one chain on a GPU provider, and the same image through the same
  chain on the CPU
- **THEN** the two results carry the same identity

## ADDED Requirements

### Requirement: A run reports the execution provider it was asked for beside the one it ran on

A run SHALL report, to the caller that asked for it, both the execution provider that was requested
and the execution provider each session it built was actually built on.

This exists because the two can differ without anything failing. A provider the machine does not
support, or one that cannot open a particular model, resolves to a CPU run rather than an error —
which is the correct outcome and is required elsewhere in this project. What makes it dangerous is
that it is invisible: a run that was downgraded returns the same image, in the same way, only
slower. Without this report, a benchmark attributes the slowness to the model and a user attributes
it to the application, and neither of them can find out otherwise from anything the run produced.

The report SHALL be part of what a successful run returns, not something a caller has to register
interest in beforehand. A caller that asked for no progress reporting and registered no handler
SHALL still receive it: a downgrade is a property of the run's outcome, not an event in its
progress.

The report SHALL cover every session the run built, not only the first to be downgraded. Whether a
provider can open a model is decided per model, so one chain can legitimately run one operation on a
GPU and the next on the CPU, and a report naming only one of them would describe a run that did not
happen. Where a run built several sessions on one provider, that provider SHALL be reported once
rather than repeated per session.

A run that built no session at all — an empty chain, or one every operation of which was served from
the image cache — SHALL report the provider it was asked for and an empty account of what it ran on,
which is distinguishable from having run on the requested provider. Reporting the requested provider
as though it had been used would claim a measurement that was never taken.

#### Scenario: A run on the provider that was asked for

- **WHEN** a chain is run asking for a provider the machine supports and every model opens on it
- **THEN** the run reports that provider as the one requested
- **AND** it reports that provider, and only it, as the one the run was executed on

#### Scenario: A run the requested provider could not serve

- **WHEN** a chain is run asking for a provider that cannot open the model, and the run is completed
  on the CPU instead
- **THEN** the run succeeds and produces the enhanced image
- **AND** it reports the requested provider beside the CPU, so the downgrade is legible from the
  result alone

#### Scenario: A chain downgraded for one model and not another

- **WHEN** a chain of two operations is run asking for one provider, the first model opens on it and
  the second does not
- **THEN** the run reports both the provider the first ran on and the CPU the second ran on

#### Scenario: A downgrade with no progress handler registered

- **WHEN** a downgraded run is asked for by a caller that registered no progress handler
- **THEN** the downgrade is reported on the result exactly as it is for a caller that registered one

#### Scenario: A run that built no session

- **WHEN** a chain is run whose every operation is served from the image cache, or which names no
  operations at all
- **THEN** the run reports the provider that was requested
- **AND** it reports that nothing was executed, rather than reporting the requested provider as
  having been used
