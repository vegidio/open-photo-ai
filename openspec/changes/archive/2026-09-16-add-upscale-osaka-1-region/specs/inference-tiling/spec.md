## ADDED Requirements

### Requirement: A model may be fed a tensor whose shape is declared rather than inferred

Where a model's input is not three colour planes of a square region, the caller SHALL state the shape
of the tensor it is supplying and the shape it expects back, and the system SHALL feed the model
exactly that.

Every model the system ran before this took one shape: three channels, square, at one fixed edge
length, so the shape could be recovered from the length of the buffer alone. A diffusion upscaler has
no such shape. It runs three graphs of three different shapes - colour planes in, a compressed latent
of sixteen channels out, a packed thirty-three-channel latent through the transformer - and two of
them are not three-channel and not at the image's own resolution. Recovering a shape from a buffer
length cannot distinguish them, and would silently feed one graph another's geometry.

Where the model returns a tensor of a different length than the caller declared, the system SHALL
report an error naming both lengths and SHALL NOT write a partial result. A graph whose output
disagrees with the shape it was run for is a malformed or mismatched export, and the buffer-length
check is the only place that disagreement becomes visible before the pixels are wrong.

#### Scenario: A model whose input is not three channels

- **WHEN** a model declaring a sixteen-channel input is run with that shape stated
- **THEN** it is fed a sixteen-channel tensor of the stated spatial size

#### Scenario: A model whose output is at a different resolution than its input

- **WHEN** a model is run with one input shape stated and a smaller output shape expected
- **THEN** the result is read back at the expected shape

#### Scenario: A model that returns the wrong number of values

- **WHEN** a model returns a tensor whose length differs from the declared output shape
- **THEN** an error naming the expected and actual lengths is returned
- **AND** no partial result is written

#### Scenario: A model that returns no tensor at all

- **WHEN** a model returns no output tensor
- **THEN** an error is returned rather than the run failing on an absent value
