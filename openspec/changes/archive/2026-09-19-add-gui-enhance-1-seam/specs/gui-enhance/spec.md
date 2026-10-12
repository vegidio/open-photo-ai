## Purpose

Running enhancements over an open image from the window: asking for a chain, stopping one that is
still running, what the application reports while it works, and how the enhanced pixels reach the
interface that asked for them.

## ADDED Requirements

### Requirement: The interface can ask for an open image to be enhanced

The application SHALL accept a request to run an ordered chain of operations over one open image, and
SHALL answer with the enhanced result's own identity together with its dimensions.

The source SHALL be named by the identity the application already addresses its pixels by, so the
interface names a photograph the same way whatever it is asking for. A request naming an identity the
application has not admitted SHALL be refused.

Each operation SHALL name its family, the model within that family, the precision to run it at, and
the values that model takes. The interface SHALL NOT be required to compose those into a single
encoded string: every part is published by the application's own catalogue, and naming them
separately is what keeps the two from drifting.

An empty chain SHALL be accepted and SHALL answer with the source unchanged. A user who has toggled
every enhancement off is asking a meaningful question, not making a mistake.

#### Scenario: A chain is run

- **WHEN** the interface asks for an admitted image to be enhanced by a chain of one or more
  operations
- **THEN** the chain is run in the order given, each operation over the result of the one before it,
  and the enhanced result's identity and dimensions are answered

#### Scenario: The result is identified by what produced it

- **WHEN** two different chains are run over the same source image
- **THEN** the two results carry different identities, and neither carries the source's own

#### Scenario: A source that was never opened

- **WHEN** a request names a source identity the application has not admitted
- **THEN** it is refused and nothing is run

#### Scenario: An empty chain

- **WHEN** the interface asks for an admitted image to be enhanced by no operations at all
- **THEN** the request succeeds and the source's pixels are what the result denotes

### Requirement: A requested scale is brought inside what the application can run

An operation carrying a scale SHALL be run at that scale where the application permits it, and at the
nearest permitted value otherwise, rather than being refused.

The control the user drives is already bounded, so a value outside the range can only reach the
application through a fault. Refusing it would turn that fault into a failed enhancement in front of
the user; bringing it inside the range produces a picture and leaves the fault visible in the logs.

#### Scenario: A scale within range

- **WHEN** a scale the application permits is requested
- **THEN** the operation runs at exactly that scale

#### Scenario: A scale outside the range

- **WHEN** a scale above or below what the application permits is requested
- **THEN** the operation runs at the nearest permitted scale rather than failing

### Requirement: An operation the application cannot run is refused before anything is fetched

A request naming a family, model or precision the application does not publish SHALL be refused, and
the refusal SHALL name what could not be served.

The whole chain SHALL be judged before any of it begins, so a chain whose later operation cannot be
served does not first spend minutes fetching a model for its earlier one.

#### Scenario: A family that cannot be run

- **WHEN** a chain names a family the application cannot run
- **THEN** the request is refused, the refusal names that family, and no model is fetched for any
  operation in the chain

#### Scenario: A model and precision that were never published together

- **WHEN** a chain names a model at a precision it is not published at
- **THEN** the request is refused rather than a model being fetched that does not exist

### Requirement: A run in flight can be stopped from the window

The interface SHALL be able to stop a run it has asked for, and a stopped run SHALL produce no image
at all - neither the operations that had already finished, nor the partly-covered result the stop
landed in.

The interface SHALL identify the run it means to stop, and a request naming a run that is no longer
the one in flight SHALL do nothing. A stop that arrives before its run has begun SHALL still take
effect on it, since the two cross the boundary independently and either may arrive first.

Stopping SHALL be reported distinguishably from failing. A user who has changed their mind has not
been told their enhancement broke.

#### Scenario: Stopping a run

- **WHEN** the interface stops a run it asked for while that run is still working
- **THEN** the run stops, no image is produced, and the outcome is reported as stopped rather than as
  failed

#### Scenario: Stopping a run that has already been replaced

- **WHEN** the interface asks to stop a run that is no longer the one in flight
- **THEN** nothing is stopped and the run that is in flight continues

#### Scenario: A stop that overtakes its own run

- **WHEN** a stop for a run arrives before the request that starts it
- **THEN** that run does not go on to do work that nobody is waiting for

### Requirement: The application runs one enhancement at a time

Asking for a run while another is in flight SHALL stop the one in flight before the new one begins.

The window draws one enhanced image, so a second run is always a statement that the first is no
longer wanted - the user has changed the enhancements, or moved to another photograph. Making the
application enforce that, rather than trusting the interface to ask, is what keeps an abandoned run
from holding models in memory and occupying the processor it was given.

#### Scenario: A second run replaces the first

- **WHEN** the interface asks for a run while an earlier run is still working
- **THEN** the earlier run is stopped and produces no image, and the new run proceeds

#### Scenario: The interface never asked for the stop

- **WHEN** an earlier run is displaced without the interface having asked for it to be stopped
- **THEN** it still stops, and still holds no models once it has

### Requirement: A run reports what it is doing while it works

The application SHALL report a run's progress to the interface as it advances, and each report SHALL
carry which operation it is about, how far the whole chain has got, and whether that operation's
model is being fetched or being run.

A report about a model being fetched SHALL additionally carry how far that fetch has got on its own
terms. The two numbers differ and both are needed: a fetch occupies only the head of one operation's
share of the chain, so a bar tracking the chain barely moves during one, and without the fetch's own
figure a multi-gigabyte download is indistinguishable from a stall.

How far the whole chain has got SHALL never decrease over a run and SHALL reach completion exactly
once, however many operations the chain holds and however many fetches happen within it.

An operation whose result the application already knew SHALL advance the chain's figure by that
operation's whole share without being reported as either fetched or run. It did neither, and a run
made entirely of such operations therefore finishes without ever reporting work.

Reports SHALL be limited to a rate an interface can draw. A run over a large photograph produces
thousands of them and a progress indicator has about a hundred distinct positions.

#### Scenario: A model is being run

- **WHEN** an operation's model is running over the image
- **THEN** reports name that operation, say it is being run, and carry how far the whole chain has got

#### Scenario: A model is being fetched

- **WHEN** an operation's model is not yet on the machine and is being fetched
- **THEN** reports name that operation, say it is being fetched, and carry both how far the chain has
  got and how far the fetch itself has got

#### Scenario: The chain only moves forward

- **WHEN** a chain of several operations runs, one of which fetches its model first
- **THEN** the figure for the whole chain never decreases, neither between operations nor when a fetch
  gives way to a run, and it reaches completion exactly once

#### Scenario: An operation whose result was already known

- **WHEN** an operation's result is already held from an earlier run
- **THEN** the chain's figure advances by that operation's whole share and the operation is reported
  as neither fetched nor run

#### Scenario: A run that is entirely already known

- **WHEN** every operation in a chain has a result already held
- **THEN** the run completes without reporting any work having been done

### Requirement: The enhanced result's pixels are served to the interface

The application SHALL serve the pixels of the enhanced result to the interface by the same route and
in the same terms as an open image's own: addressed by the result's identity, bounded on the longest
edge where a bound is asked for, and served at the result's own dimensions where none is.

The interface SHALL NOT receive the pixels as part of the answer to the run itself. A full-resolution
enlargement is tens of megabytes and the window has a way to fetch pixels already; carrying them
through the answer would put them on the message channel and make the interface responsible for
holding them.

#### Scenario: Drawing the enhanced result

- **WHEN** the interface requests the pixels of an enhanced result by its identity
- **THEN** they are served, at the requested bound or at the result's own dimensions

#### Scenario: The answer to a run carries no pixels

- **WHEN** a run finishes
- **THEN** what is answered describes the result and does not contain it

### Requirement: Only the most recent enhanced result is held

The application SHALL hold the result of the most recent completed run, and SHALL stop serving any
result a later run has displaced.

A request for a displaced result SHALL be refused distinguishably from one naming a result that never
existed, so an interface drawing a stale address can tell the two apart.

Holding one is what bounds the cost: a fourfold enlargement of a large photograph is over a gigabyte
of pixels, and holding every result a user had passed through would grow without limit over a session.

#### Scenario: A superseded result

- **WHEN** the pixels of an enhanced result are requested after a later run has completed
- **THEN** the request is refused, and reported as no longer held rather than as unknown

#### Scenario: A result that never existed

- **WHEN** pixels are requested for an identity no run has produced
- **THEN** the request is refused and reported as unknown
