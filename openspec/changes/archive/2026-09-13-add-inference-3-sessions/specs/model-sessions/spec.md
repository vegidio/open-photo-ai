## Purpose

Turning a named model artifact into a loaded, runnable inference session and deciding how long that
session lives: building it on the provider the machine could serve, falling back rather than failing
when it cannot, caching what a provider compiles from the model, keeping a built session resident so
the next tile and the next image do not pay for it again, and releasing what it holds once nothing is
using it - because opening a model costs anything from a fifth of a second to several minutes of
engine compilation, and what it holds is native memory that nothing else can reclaim.

## ADDED Requirements

### Requirement: A named model artifact becomes a runnable session

The application SHALL turn a named model artifact into a session that can run that model, given the
execution provider the caller asked for.

Obtaining the artifact's files is part of it: a request for an artifact whose files are not on disk
SHALL install them first, under the rules the model install capability already states, and report the
progress of that transfer to a caller that asked for it. A request for an artifact that is already on
disk SHALL transfer nothing.

A request that cannot be served SHALL fail with an error that distinguishes a model that could not be
put on disk from a model that is on disk and could not be opened.

#### Scenario: An artifact whose files are not yet on disk

- **WHEN** a session is requested for an artifact that has never been installed
- **THEN** the artifact's files are installed and verified before the model is opened
- **AND** the resulting session can run that model

#### Scenario: An artifact that is already installed

- **WHEN** a session is requested for an artifact whose files are already on disk and intact
- **THEN** nothing is transferred
- **AND** the session is built from the files already there

#### Scenario: A model that cannot be put on disk

- **WHEN** the artifact's files cannot be installed
- **THEN** the request fails reporting the install failure
- **AND** no session is built

### Requirement: A session is built with the settings resolved for that model and machine

A session SHALL be built from the resolution the execution-providers capability produces for the
requested provider, the machine's provider report and that model's measured profile: every provider
in it attached in the order it gives, each configured with the options resolved for it, and the
session settings applied to the session itself.

A session SHALL report which execution provider it was built on.

This is the first point at which those options reach the inference runtime. An option the runtime
rejects SHALL fail the build of that provider rather than being ignored, so that a wrong option is a
reported failure rather than a silently slower run.

#### Scenario: A machine that supports a GPU provider

- **WHEN** a session is requested on a machine whose report supports a GPU provider, and that
  provider builds the model
- **THEN** the session reports that provider as the one it was built on

#### Scenario: A CPU request

- **WHEN** a session is requested on the CPU
- **THEN** no execution provider is attached
- **AND** the session reports the CPU as the one it was built on

#### Scenario: An option the runtime will not accept

- **WHEN** a provider is attached with an option the runtime rejects
- **THEN** building that provider fails rather than continuing without the option

### Requirement: A model the requested provider cannot open is opened on the CPU

Where a session cannot be built on the providers resolved for the request, the application SHALL
build it with no provider attached rather than failing the request, and the resulting session SHALL
carry what was asked for beside what it was actually built on, so that a caller can tell a downgraded
run from an honoured one without inspecting anything else.

A failure that is not about the execution provider - a model file that is missing or unreadable -
SHALL NOT be retried on the CPU, because it would fail there in the same way.

A downgrade SHALL NOT be latched: a later request for a different model, or for the same model again,
SHALL attempt the provider it asks for. The reference implementation latches the first such failure
for the rest of the process and clears it only when the user changes processor; this application has
no way to clear it, so a latched downgrade would outlive the condition that caused it.

#### Scenario: A GPU provider that cannot build the model

- **WHEN** a session is requested on a GPU provider and that provider fails to build the model
- **THEN** the session is built with no provider attached
- **AND** it reports the requested provider beside the CPU it was built on
- **AND** the request succeeds

#### Scenario: A model file that cannot be read

- **WHEN** a session is requested and the model file on disk cannot be opened at all
- **THEN** the request fails
- **AND** no second attempt is made on the CPU

#### Scenario: A second request after a downgrade

- **WHEN** one model has been downgraded to the CPU and a session is requested for a different model
  on the same provider
- **THEN** that provider is attempted for it

### Requirement: A built session is kept resident and reused

A session that has been built SHALL be kept in memory and reused by a later request for the same
model on the same provider, so that opening the same model twice in a row costs one build.

Sessions SHALL be separated by the execution provider they were built on: the same model built on two
providers is two sessions holding two separate sets of native resources, and a request naming a
provider SHALL only ever be served a session built on that provider.

Sessions SHALL be separated by the model artifact rather than by the operation that asked for one, so
that two operations needing the same model graph share one session. The reference implementation
keys on the operation instead, which loads one graph twice when two operations share it.

#### Scenario: The same model requested twice

- **WHEN** a session is requested twice for the same model on the same provider
- **THEN** the model is opened once
- **AND** both requests are served the same session

#### Scenario: The same model on two providers

- **WHEN** a session is requested for one model on two different providers
- **THEN** two sessions exist
- **AND** each request is served the one built on the provider it asked for

#### Scenario: Two operations needing one graph

- **WHEN** two different operations each require the same model artifact
- **THEN** the model is opened once
- **AND** both are served that session

### Requirement: Concurrent requests for one model are served by a single build

Where several requests for the same model on the same provider arrive while none is resident, the
model SHALL be opened exactly once and every one of those requests SHALL be served that session.

A build that fails SHALL fail every request waiting on it, rather than each of them starting its own
attempt at something that has just proved impossible.

#### Scenario: Several callers racing for one model

- **WHEN** several requests for the same model on the same provider arrive at once with nothing
  resident
- **THEN** the model is opened exactly once
- **AND** every request is served that session

#### Scenario: A build that fails with callers waiting

- **WHEN** a build that other requests are waiting on fails
- **THEN** each waiting request fails
- **AND** no further build is started on their behalf

### Requirement: A session is released 15 minutes after it was last used

A session that has gone unused for 15 minutes SHALL be released, and the native resources behind it
freed. The interval SHALL be measured from the last time the session was used, not from when it was
built, so a model that is in use every few minutes stays resident.

A session that something is currently using SHALL NOT be released, however long it has been held: a
single run can legitimately last far longer than the interval.

The reference implementation instead scales the interval with what the model cost to build - a
five-minute floor, extended to twenty times the build cost, capped at thirty minutes - because it
cooperates with a memory budget this application does not yet have. One interval inside that same
range is carried here in its place.

#### Scenario: A model left unused

- **WHEN** a resident session goes unused for longer than 15 minutes
- **THEN** it is released
- **AND** a later request for that model builds it again

#### Scenario: A model used regularly

- **WHEN** a resident session is used again before 15 minutes have passed
- **THEN** the interval restarts from that use
- **AND** the session stays resident

#### Scenario: A long-running use

- **WHEN** one use of a session lasts longer than 15 minutes
- **THEN** the session is not released while that use is in progress

### Requirement: A session's native resources are released exactly once, when nothing is using it

The native resources behind a session SHALL be released exactly once, and only once nothing is using
that session. A session that is released while a run is still using it is a use-after-free in native
code rather than a recoverable error, and the application SHALL make that outcome unreachable rather
than merely avoid it.

This SHALL hold for every path that stops a session being reused - the idle interval above, and
clearing the cache below.

#### Scenario: A session dropped from the cache while in use

- **WHEN** a session is removed from the cache while something is still using it
- **THEN** the session remains usable to whoever holds it
- **AND** its resources are released when that use ends

#### Scenario: A session removed and then requested again

- **WHEN** a session is removed from the cache while in use, and the same model is requested again
- **THEN** a new session is built for the new request
- **AND** the two are separate, each released when its own last user lets go

### Requirement: Every resident session can be released on request

The application SHALL offer a way to release every resident session at once, for shutting down or for
reclaiming memory. Sessions in use SHALL be released when their last use ends, under the rule above,
and the call SHALL NOT wait for a run in progress.

#### Scenario: Clearing with nothing in use

- **WHEN** every resident session is released and none is in use
- **THEN** all of their resources are freed
- **AND** a later request for any of those models builds it again

#### Scenario: Clearing with a run in progress

- **WHEN** every resident session is released while one is in use
- **THEN** the call returns without waiting for that run
- **AND** the session in use is released when its run ends

### Requirement: What a provider compiles from a model is cached per model

An execution provider that compiles something from a model's weights - an engine, a compiled model
format - SHALL be given a cache directory belonging to that model alone, so that what was compiled
from one model's weights can never be attributed to or reused against another's.

The TensorRT timing cache SHALL be shared by every model instead, because measured kernel timings
carry between graphs: building a model against a cache other models have seeded is measurably faster
than building it against an empty one, and a per-model timing cache would be cleared by the same
event that clears the engine it was accelerating.

#### Scenario: Two models compiled by a provider

- **WHEN** two different models are built on a provider that compiles from their weights
- **THEN** each model's compiled output is kept separately from the other's

#### Scenario: The timing cache across models

- **WHEN** two different models are built with TensorRT attached
- **THEN** both are pointed at one shared timing cache

### Requirement: Builds that attach TensorRT do not run concurrently

Where TensorRT is among the providers being attached, the application SHALL NOT build two sessions at
the same time, because the shared timing cache is one file that the runtime reads at the start of a
build and rewrites at the end with no locking of its own.

Where such a build fails, the shared timing cache SHALL be discarded. A half-written timing cache
fails every later TensorRT build with a symptom - every model running somewhere slower - that points
nowhere near a cache file.

#### Scenario: Two models built at once with TensorRT attached

- **WHEN** two session builds that both attach TensorRT are requested at the same time
- **THEN** they are built one after the other

#### Scenario: A failed build with TensorRT attached

- **WHEN** a session build with TensorRT attached fails
- **THEN** the shared timing cache is discarded
- **AND** the failure is still reported to the caller

#### Scenario: A failed build with TensorRT not attached

- **WHEN** a session build that does not attach TensorRT fails
- **THEN** the shared timing cache is left alone
