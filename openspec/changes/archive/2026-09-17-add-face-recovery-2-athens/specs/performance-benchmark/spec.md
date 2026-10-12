## ADDED Requirements

### Requirement: A model whose input is another model's result is given it once, and is not timed producing it

Where a selected model cannot do any work without a result only another model can produce — a face-recovery model,
which restores the faces it is given and finds none of its own — the system SHALL obtain that result **once**, before
the sweep begins, and SHALL supply it to every model that needs it.

Without it the measurement is of nothing. A face-recovery run over an empty selection composites no face and returns
almost immediately, so a sweep would report a sub-millisecond time for a model that takes seconds on a real
photograph, and would report it as a success — which is worse than the refusal it replaced, because a failure is
visible and a plausible wrong number is not.

The auxiliary run SHALL be made at the sweep's own precision rather than at a pinned one, because that is what the
application does: a run that restores at one precision detects at the same one, and pinning would measure a
combination the application never produces.

Its cost SHALL NOT be charged to any model's timings, and it SHALL NOT be repeated per model. Each model's cold start
already starts from nothing, so a result obtained per model would pay for a second model's full session build inside
another model's measurement — untimed but real wall clock, and on TensorRT larger than everything else in the sweep.

Where the auxiliary run fails, or succeeds and finds nothing to work with, every model that depended on it SHALL fail
with that reason. Failing is the required outcome rather than measuring the empty case: a sweep of a photograph with
no face in it has nothing to say about a face-recovery model, and saying it quickly is the defect this requirement
exists to prevent.

Models that do not need the result SHALL be unaffected — neither delayed by it nor failed by its failure — and a sweep
that selected none of the models that need it SHALL NOT make the auxiliary run at all.

The report SHALL state what the auxiliary run found, alongside the other conditions that produced the numbers, since
the number of faces is what a face-recovery model's timings scale with.

#### Scenario: A sweep including a face-recovery model

- **WHEN** a sweep selects a face-recovery model and an image with faces in it
- **THEN** the faces are obtained once before the sweep and supplied to that model
- **AND** the time taken to obtain them appears in no model's measurement

#### Scenario: A sweep including two face-recovery models

- **WHEN** a sweep selects two models that both need the same auxiliary result
- **THEN** it is obtained once and supplied to both

#### Scenario: An image with no faces in it

- **WHEN** a sweep selects a face-recovery model and an image in which the auxiliary run finds no face
- **THEN** that model is reported as failed with that reason, rather than measured

#### Scenario: A sweep needing no auxiliary result

- **WHEN** a sweep selects only models that need no other model's result
- **THEN** no auxiliary run is made

#### Scenario: The report of a sweep that made one

- **WHEN** a sweep that made an auxiliary run produces a report
- **THEN** the report states what it found, beside the image, the provider, the precision and the other conditions
