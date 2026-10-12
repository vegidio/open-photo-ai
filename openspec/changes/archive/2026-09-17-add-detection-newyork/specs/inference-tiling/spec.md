## ADDED Requirements

### Requirement: A model may return several output tensors, addressed by name

Where a model returns more than one output tensor, the caller SHALL name the outputs it wants and the
system SHALL return exactly those, each read back independently of the others.

Every model the system ran before this returned one tensor, so the output could be taken as "the one
the model produced" with no name spelled anywhere — which is deliberate, and stays that way for those
models: a conventionally exported one-in-one-out graph needs no name, and spelling one would make the
system fail on a graph that declared something else.

That argument inverts the moment a graph returns several. A face detector returns three tensors of
three different lengths and three different meanings — where each candidate is, how confident the
model is about it, and where its landmarks are — and taking them by position makes the system depend
on the order they happen to be declared in. A re-exported graph that declared them in another order
would then be read with confidences interpreted as positions, producing detections that are wrong
rather than absent, with nothing at any layer able to notice.

Where the model does not declare an output the caller named, the system SHALL report an error naming
the output that is missing, and SHALL produce no result at all. Where a named output returns a tensor
of a different length than the caller declared, the system SHALL report an error naming both lengths,
and SHALL NOT write a partial result — the same guarantee a single declared output already carries.

#### Scenario: A model with three named outputs

- **WHEN** a model declaring three outputs is run with all three named
- **THEN** each is read back at its own declared length, matched by name rather than by position

#### Scenario: A model whose outputs are declared in an unexpected order

- **WHEN** a model declares its named outputs in a different order than the caller listed them
- **THEN** each output is still matched to the name the caller asked for

#### Scenario: A model missing one of the named outputs

- **WHEN** a model does not declare an output the caller named
- **THEN** an error naming the missing output is returned
- **AND** no result is produced, including for the outputs that were present

#### Scenario: A named output of the wrong length

- **WHEN** a named output returns a tensor whose length differs from the declared length
- **THEN** an error naming the expected and actual lengths is returned
- **AND** no partial result is written
