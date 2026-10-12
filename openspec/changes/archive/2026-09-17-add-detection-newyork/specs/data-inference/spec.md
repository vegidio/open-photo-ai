## Purpose

Running one operation whose result is data rather than a picture — a set of faces, and whatever a later family
produces — end to end: how the result's type is decided, what such a run reports while it works, what it hands back
beside the result, what stops it, and why nothing it produces is kept. Enhancement is a chain over pixels and is
specified elsewhere; this is the path for the runs that have no picture to hand back and no chain to be part of.

## ADDED Requirements

### Requirement: A single operation whose result is not an image is run on its own

The system SHALL accept an image together with **one** operation whose result is not an image, and produce that
operation's result together with a report of what the run was executed on.

One operation, not a sequence. Chaining is a property of pixels: each enhancement is applied to the picture the one
before it produced, and there is nothing to chain when a run hands back a measurement of the image rather than a new
image. An analysis of an image is a single question asked of it.

The image SHALL be left untouched. A run on this path reads the picture and produces a description of it, so a caller
may run one over the same image as often as it likes without the image changing underneath it.

#### Scenario: One operation over one image

- **WHEN** a caller supplies an image and one operation whose result is not an image
- **THEN** that operation's result is produced, together with a report of what the run was executed on

#### Scenario: The source image is unchanged

- **WHEN** such a run completes
- **THEN** the image supplied is the image the caller still holds, unmodified

### Requirement: The result type is the operation's own, not the caller's to choose

Each operation whose result is not an image SHALL name exactly one result type, and the system SHALL bind a run of
that operation to that type. A caller SHALL NOT be able to ask such a run for a result type the operation does not
produce, and SHALL NOT be able to put an operation that produces an image onto this path.

Neither of those is a refusal the system reports at run time. They are pairings that cannot be expressed: the type a
run produces follows from the operation being run, so there is no second place for a caller to state it and nothing to
state it wrongly. A system that decided this while the model was already loaded would discover a mismatch only after
transferring and opening it, and would report as an error what was never a legitimate request.

#### Scenario: A detection run produces faces

- **WHEN** a detection operation is run on this path
- **THEN** the result is the set of faces that operation produces, and no other type can be asked for

#### Scenario: An operation that produces an image

- **WHEN** an operation whose result is an image is named for this path
- **THEN** the request cannot be expressed, rather than being accepted and then refused

### Requirement: A run reports what it is doing and how far it has got, stops when asked, and fails as a whole

Such a run SHALL report its progress on the same terms an enhancement does: whether the model is being put on disk or
being run, and how far the run has got as a single fraction from nothing to finished.

The fraction SHALL NOT decrease over one run, and SHALL reach its maximum when the run finishes. In particular it
SHALL NOT return to the start when putting the model on disk hands over to running it. A model already on disk SHALL
NOT give up the head of the range to a transfer that never happened.

A caller SHALL be able to stop a run in flight, and a stopped run SHALL produce no result at all. A failed run SHALL
likewise produce no result — not a partial one, and not an empty one standing in for the answer. A caller handed an
empty result with no error has no way to tell it from a genuine answer, and would show it to a user as one.

Reporting SHALL be optional, and a caller that asks for none SHALL be charged for none.

#### Scenario: Progress over a run that installs its model

- **WHEN** a run's model is not on disk and a caller is listening
- **THEN** the transfer reports over the head of the range and the run itself over the rest
- **AND** the fraction never decreases and reaches its maximum when the run finishes

#### Scenario: Progress over a run whose model is already on disk

- **WHEN** a run's model is already on disk
- **THEN** the whole of the range is the run itself, rather than a fifth of it being given to a transfer that did not happen

#### Scenario: A run stopped in flight

- **WHEN** a caller stops a run before it finishes
- **THEN** the run reports that it was stopped and produces no result

#### Scenario: A run that fails

- **WHEN** a run fails for any reason
- **THEN** no result is produced, and an empty result is not returned in its place

### Requirement: A run reports the execution provider it was asked for beside the one it ran on

Such a run SHALL report what it was asked to execute on beside what it actually executed on, as part of what it
returns rather than as something a caller registers interest in beforehand.

A provider the machine does not support, or one that cannot open a particular model, resolves to a run on the
processor rather than an error — the correct outcome. What makes it worth reporting is that it is otherwise invisible:
a downgraded run produces the same result, in the same way, only several times slower, so a benchmark attributes the
slowness to the model and a user attributes it to the application. It is on the result so that a caller who asked for
no progress reporting still receives it.

Where no model was built at all, the report SHALL say so, and SHALL NOT claim the requested provider was used.

#### Scenario: A run downgraded to the processor

- **WHEN** a run asks for a provider that cannot open the model and the run is served on the processor instead
- **THEN** the result is produced, and the report names both what was asked for and what it ran on

#### Scenario: A run that built no model

- **WHEN** a run completes without building a model session
- **THEN** the report says nothing was executed, rather than naming the requested provider

### Requirement: Nothing a non-image run produces is cached

Such a run SHALL NOT read from or write to the image cache, and SHALL NOT be affected by what that cache holds.

The cache exists so that a user dragging a slider does not pay twice for the same enhanced picture, and what it stores
is encoded images keyed on the input and the operations applied. A result that is not an image has none of the
properties that make it worthwhile: it is small, it is cheap against the cost of reading and decoding a stored picture,
and storing it would put a second kind of value into a store whose every entry is a photograph.

#### Scenario: A repeated run over the same image

- **WHEN** the same operation is run twice over the same image
- **THEN** both runs execute the model, and neither reads nor writes the image cache

#### Scenario: A run on a machine whose cache is unavailable

- **WHEN** the image cache cannot be opened
- **THEN** such a run is unaffected and produces its result as normal
