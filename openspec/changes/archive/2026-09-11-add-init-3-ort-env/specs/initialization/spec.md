## ADDED Requirements

### Requirement: The installed runtime is loaded and its inference environment started

Initialization SHALL load the shared library the pinned runtime archive names, from the `runtime`
directory it was installed into, and SHALL start the inference environment every model session is
later built under. Both happen before initialization reports success.

The library that is loaded SHALL be the one this application installed and verified. Initialization
MUST NOT load a runtime found elsewhere on the machine, and MUST NOT obtain one by any means other
than the install described by the requirements above - the pinned archive, proven against its
published hash, is the only runtime the application ever runs.

The environment SHALL be created during initialization rather than deferred to the first time a
model is needed, so that a runtime which cannot be loaded, or an environment which cannot be
started, is a failed initialization rather than a failure in the middle of a user's first
enhancement. Either failure SHALL be reported as an error naming the library path it was attempted
against.

#### Scenario: A successful first launch

- **WHEN** initialization has installed the pinned runtime and starts its environment
- **THEN** the shared library named by the pinned archive is loaded from the application's `runtime`
  directory
- **AND** the inference environment is running by the time initialization reports success

#### Scenario: The runtime library is missing or unreadable

- **WHEN** the shared library named by the pinned archive is not present in the `runtime` directory,
  or cannot be loaded
- **THEN** initialization fails with an error naming the library path
- **AND** no inference environment is left running

#### Scenario: The library is not a runtime this build can use

- **WHEN** the library at that path loads but is not an ONNX Runtime of a version this build
  supports
- **THEN** initialization fails with an error naming the library path and the version found
- **AND** the application does not fall back to any other runtime on the machine

### Requirement: The inference environment is started once and lives as long as the process

The inference environment is process-wide. Initialization SHALL start at most one for the life of
the process, and that environment SHALL stay valid until the process exits. There is no operation
that tears it down while the application is running.

Initialization MAY be called more than once in one process - a front end that restarts its
application layer after an error does exactly that. A repeated initialization SHALL NOT start a
second environment, SHALL NOT fail because one is already running, and SHALL leave the running one
in place and usable. Everything else initialization does still happens: the dependency installs are
re-checked and the provider report is produced as though it were the first call.

A repeated initialization under a **different** application name SHALL install into that name's
configuration directory as it would on a first call, but SHALL continue to use the runtime library
loaded by the first call. The first runtime a process loads is the runtime that process uses.

Where the first initialization **failed** to load the runtime, no later initialization in that
process SHALL report success. The application MUST NOT treat a retry as having loaded a runtime it
did not load. Every later initialization SHALL fail with an error naming the library path the first
attempt was made against and what it reported, so that a front end can tell the user that the
process has to be restarted rather than retried.

#### Scenario: Initialization runs a second time in the same process

- **WHEN** initialization is called again in a process where it has already succeeded
- **THEN** it succeeds
- **AND** no second environment is started and the existing one remains usable

#### Scenario: A second initialization under a different application name

- **WHEN** initialization is called again with a different application name
- **THEN** the dependencies are installed under that name's configuration directory
- **AND** the runtime library loaded by the first call is the one still in use

#### Scenario: Initialization is retried after the runtime failed to load

- **WHEN** initialization has already failed because the runtime library could not be loaded, and it
  is called again in the same process
- **THEN** it fails with an error naming the library path the first attempt used and what that
  attempt reported
- **AND** it does not report success, and no inference environment is started

#### Scenario: Every handle to the initialized application is released

- **WHEN** the last handle returned by initialization is dropped and initialization is then called
  again
- **THEN** it succeeds and the environment is still usable
- **AND** nothing created under the environment is invalidated

### Requirement: Runtime diagnostics are reported at warning level and telemetry is off

The inference runtime produces its own diagnostics. Initialization SHALL set their level to warning,
which is where the node-assignment and execution-provider fallback messages that explain a slow or
unexpectedly-CPU-bound session appear, without the per-node flood that the informational and verbose
levels produce.

Initialization SHALL disable the runtime's telemetry. The application MUST NOT report what its user
is editing - graph and operator metadata, execution provider names, error text or inference counts -
to any third party.

The environment SHALL be identified by the application name initialization was given, so that a
diagnostic can be attributed to the application that produced it.

#### Scenario: The runtime emits diagnostics

- **WHEN** the runtime reports a warning or an error
- **THEN** it is reported
- **AND** informational and verbose runtime messages are not

#### Scenario: Telemetry

- **WHEN** initialization starts the environment
- **THEN** the runtime's telemetry is disabled for every session created under it
