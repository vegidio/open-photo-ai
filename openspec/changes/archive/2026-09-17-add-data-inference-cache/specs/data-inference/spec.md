## REMOVED Requirements

### Requirement: Nothing a non-image run produces is cached

**Reason**: The requirement's own justification measured the wrong thing. It argued that a result which is not an
image "is small, it is cheap against the cost of reading and decoding a stored picture" — which is true of the
result and false of the run that produces it. What a repeated run pays for is not the arithmetic but putting the
model in memory, and that is seconds. The requirement therefore forbade an optimisation on the grounds that it would
save something it was never going to save.

**Migration**: None for a caller. The replacement requirement below produces the same result from the same inputs;
what changes is that the second identical run is served rather than executed. A caller that needs every run to
actually execute — a benchmark measuring the model — asks for the store to be bypassed for that run, which the
replacement requires. Nothing that was previously returned stops being returned, and no error becomes possible that
was not possible before.

## ADDED Requirements

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
