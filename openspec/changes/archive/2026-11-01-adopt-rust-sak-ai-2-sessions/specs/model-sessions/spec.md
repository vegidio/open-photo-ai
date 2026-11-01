## MODIFIED Requirements

### Requirement: What a provider compiles from a model is cached per model

An execution provider that compiles something from a model's weights - an engine, a compiled model
format - SHALL be given a cache directory belonging to that model alone, so that what was compiled
from one model's weights can never be attributed to or reused against another's.

Within a model's cache directory, each provider that compiles SHALL be given a directory of its own,
so that what one provider compiled can be found, inspected or discarded without touching what
another compiled from the same model. Discarding a model's cache directory SHALL discard every
provider's directory inside it.

The TensorRT timing cache SHALL be shared by every model instead, because measured kernel timings
carry between graphs: building a model against a cache other models have seeded is measurably faster
than building it against an empty one, and a per-model timing cache would be cleared by the same
event that clears the engine it was accelerating.

#### Scenario: Two models compiled by a provider

- **WHEN** two different models are built on a provider that compiles from their weights
- **THEN** each model's compiled output is kept separately from the other's

#### Scenario: Two providers compiling one model

- **WHEN** one model is built on TensorRT and on CoreML
- **THEN** each provider's compiled output is kept in a directory of its own inside that model's
  cache directory

#### Scenario: The timing cache across models

- **WHEN** two different models are built with TensorRT attached
- **THEN** both are pointed at one shared timing cache
- **AND** that timing cache is outside either model's cache directory
