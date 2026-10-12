## ADDED Requirements

### Requirement: Every unit of work reaches the collector as a span

A record says that something happened. A trace says where the time went and what caused what. While telemetry is
running, each unit of work this library does SHALL reach the collector as a span:
- initialization, and loading the inference runtime within it
- installing a dependency or a model
- a request for a model's session, and building that session
- an enhancement, and each operation of it
- an analysis whose result is not a picture, and an automatic analysis
- opening, describing, fingerprinting, encoding and saving an image
- reading from and writing to the run cache

A unit done on behalf of another SHALL be a child of it. So the installs, session builds, operations and cache access
of one enhancement belong to that enhancement's trace, including when the work was handed to another thread. A record
emitted inside a unit SHALL carry that unit's trace.

#### Scenario: An enhancement that installs its model

- **WHEN** an enhancement is run while telemetry is running, and its model has to be installed and built first
- **THEN** the collector receives one trace for the enhancement
- **AND** that trace holds a span for the enhancement, one for each of its operations, and, beneath the operation that
  needed it, a span for the session request, the build and the install

#### Scenario: Describing a photograph

- **WHEN** an application asks for a photograph's description while telemetry is running
- **THEN** the collector receives a span naming that operation

#### Scenario: A failure record inside a unit

- **WHEN** a unit of work fails and records its failure while telemetry is running
- **THEN** the record the collector receives carries the trace of that unit's span

### Requirement: A unit's span says how it ended

A unit's span SHALL say whether the unit finished, stopped or failed, matching the record that closed it. A failed
unit's span SHALL be marked as failed, carrying the reason, so a trace view shows it failed without its records being
read. A stopped unit's span SHALL NOT be marked as failed, for the same reason its record is not a warning.

#### Scenario: A failed enhancement

- **WHEN** an enhancement fails part of the way through while telemetry is running
- **THEN** the enhancement's span says it failed and is marked as failed with the reason
- **AND** the span of the operation that failed is marked as failed too

#### Scenario: A cancelled enhancement

- **WHEN** an enhancement is cancelled by its caller while telemetry is running
- **THEN** the enhancement's span says it stopped
- **AND** it is not marked as failed

### Requirement: Requests that wait on one build are linked to it

Several requests may wait on one build of a model, as the file already records. The build SHALL be one span, in the
trace of the request that started it. Every other request that waited on it SHALL be linked to that span, so their
traces show the time spent waiting and where it went, without a second build that did not happen.

#### Scenario: Two enhancements need the same model at once

- **WHEN** two enhancements request the same model while it is not ready, and telemetry is running
- **THEN** the collector receives exactly one span for building that model
- **AND** the request span of the enhancement that did not start the build carries a link to it

### Requirement: The file does not carry the collector's spans

The spans sent to the collector SHALL NOT change the file. A record SHALL be written to `opai.log` with exactly the
fields it would have if those spans did not exist: no field of an enclosing unit's span is added to it. Only the
contexts that existed in the file before, such as the model a runtime diagnostic was emitted while building, SHALL
decorate the records inside them.

#### Scenario: A record inside an enhancement

- **WHEN** an enhancement is run at the default verbosity
- **THEN** each of its records in the file carries only the fields it was emitted with, and those of the contexts the
  file already wrote before this change

#### Scenario: A runtime diagnostic during a build

- **WHEN** the inference runtime emits a warning while a model's session is being built
- **THEN** the record in the file still names the model and the execution provider being attached

### Requirement: Spans are bounded by what was done, not by how big the picture was

The number of spans a unit of work produces SHALL NOT grow with the size of the image it was given, for the same reason
the number of its records does not. There SHALL be no span per tile, per region or per pixel.

#### Scenario: A large image and a small one

- **WHEN** the same enhancement is run with telemetry running over a small image, and again over one many times larger
- **THEN** both send the same number of spans

### Requirement: The work is measured while telemetry runs

The collector SHALL receive measurements that answer, across every installation that sends, questions no single record
does:
- how long each kind of unit takes, and how often it finishes, stops or fails
- how long each model's operation takes on the execution provider that actually ran it
- how often the run cache serves a result, and how often it misses
- how often a requested execution provider could not be used and another ran the work instead, by the provider that
  was attempted
- how many failures of each kind each kind of unit has
- how many models are held in memory, by execution provider

Measurements SHALL be sent under the same rules as records: only once telemetry has started, only from a build that
names a collector, and nothing after it stops. Each measurement SHALL be identified only by values from small, fixed
sets: a kind of unit, an outcome, a model, an execution provider, a kind of failure. No path, photograph identity,
parameter value or error message SHALL identify a measurement.

#### Scenario: A finished enhancement is measured

- **WHEN** an enhancement finishes while telemetry is running
- **THEN** the collector receives its duration, identified as an enhancement that finished
- **AND** the duration of each operation it computed, identified by the model and the execution provider that ran it

#### Scenario: A step served from the store

- **WHEN** a step of an enhancement is served from the run cache while telemetry is running
- **THEN** the collector's count of cache hits for steps grows by one

#### Scenario: A provider falls back

- **WHEN** the execution provider requested for a model cannot open it and the CPU runs it instead, while telemetry is
  running
- **THEN** the collector's count of fallbacks from that requested provider grows by one

#### Scenario: A failure is counted by kind

- **WHEN** an enhancement fails while telemetry is running
- **THEN** the collector's count of enhancement failures of that error's kind grows by one
- **AND** the kind is one of a fixed set of names, never the error's text

#### Scenario: An application that never starts telemetry

- **WHEN** an application runs work and never starts telemetry
- **THEN** no measurement leaves the process
