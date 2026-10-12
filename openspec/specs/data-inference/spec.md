# data-inference Specification

## Purpose

Running one operation whose result is data rather than a picture — a set of faces, and whatever a later family
produces — end to end: how the result's type is decided, what such a run reports while it works, what it hands back
beside the result, what stops it, and how what it produces is kept and reused. Enhancement is a chain over pixels and is
specified elsewhere; this is the path for the runs that have no picture to hand back and no chain to be part of.

## Requirements

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

Operations whose result is not an image SHALL be carried in a value of their own, distinct from the one carrying
operations whose result is an image. Which of the two an operation is carried in SHALL follow from the model being
named rather than being stated separately, so a caller naming a model is never also asked which path it runs on and
can never answer wrongly.

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

#### Scenario: Naming a model decides its path

- **WHEN** a caller names a model whose result is not an image
- **THEN** the operation produced is carried in the value this path takes, with no further statement from the caller
  about which path it runs on

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

### Requirement: A non-image run's result is kept and reused

Where the same operation has already been run over the same image, the system SHALL serve the result it produced
then rather than running the model again. A stored result SHALL be kept for the same length of time an enhancement's
is, and SHALL be reclaimed on the same terms.

What makes this worth doing is **when** the lookup happens rather than what it saves. A served run SHALL NOT
transfer the model, open a session, or execute anything: the cost a repeated analysis actually pays is putting the
model into memory, which is orders of magnitude larger than the arithmetic the model then performs. A lookup made
after the model was opened would save the cheaper half and pay the expensive one.

A result SHALL be stored under the image it was computed from together with the operation that computed it, so that
the same operation over a different image, and a different operation over the same image, are each a separate entry
and neither is served the other's answer. A stored result SHALL NOT be readable as an enhanced picture, nor a stored
picture as a result, whatever either happens to contain.

The execution provider SHALL NOT be part of what an entry is stored under. The same analysis of the same photograph
is the same analysis whichever processor produced it.

#### Scenario: The same operation run twice over the same image

- **WHEN** an operation whose result is not an image is run a second time over the same image
- **THEN** the result of the first run is produced
- **AND** no model is transferred, opened or executed

#### Scenario: A run with nothing stored for it

- **WHEN** no result is stored for this image and this operation
- **THEN** the model runs, and the result it produces is stored

#### Scenario: A different operation over the same image

- **WHEN** a second, different operation whose result is not an image is run over an image one has already been run over
- **THEN** it is not served the first operation's result, and runs its own model

#### Scenario: The same operation over a different image

- **WHEN** the same operation is run over a different image
- **THEN** it is not served the first image's result, and runs its own model

### Requirement: A served run reports that nothing was executed, and still reports finishing

A run served from the store SHALL report what it was asked to execute on beside the fact that nothing was executed,
rather than naming the provider it would have used. A served run built no model, and a report claiming otherwise
would tell a benchmark that a provider was exercised when it was not.

Such a run SHALL report its progress to the same maximum an executed run reaches. A caller listening to a run that
completed instantly must be told it completed; a bar left short of the end because the work was skipped is
indistinguishable from a run that stalled.

#### Scenario: A run served from the store

- **WHEN** a run is served from the store
- **THEN** the report says nothing was executed, rather than naming the requested provider

#### Scenario: Progress over a run served from the store

- **WHEN** a caller is listening to a run that is served from the store
- **THEN** the fraction reaches its maximum, as it does for a run that executed the model

### Requirement: A caller may run without the store, and losing the store costs a run rather than an error

A caller SHALL be able to ask for a run that neither reads from nor writes to the store. Such a run SHALL produce
exactly what a served run would have produced, and SHALL leave nothing behind — a caller measuring what a model
costs needs the model to actually run, and a measurement that silently stored its result would change what the next
measurement measured.

Whether the store is used SHALL be decided once for a run rather than separately for the read and the write, so that
a run cannot be served from the store and then decline to keep what it produced, or the reverse.

A store that cannot be opened, a read that fails, a write that is declined or breaks, and an entry that no longer
reads back SHALL each cost the run nothing but executing the model. None of them SHALL fail a run or surface to the
caller as an error: the result is still obtainable by running the model, which is what the system does.

#### Scenario: A caller that asks for no store

- **WHEN** a caller asks for a run that does not use the store
- **THEN** the model runs, the result is produced, and nothing is stored

#### Scenario: A machine whose store cannot be opened

- **WHEN** the store cannot be opened
- **THEN** such a run executes its model and produces its result as normal, and reports no error

#### Scenario: A stored entry that no longer reads back

- **WHEN** an entry is found for this image and operation but cannot be read back as a result
- **THEN** the model runs, and the run produces its result rather than failing
