## MODIFIED Requirements

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

Those diagnostics SHALL be delivered to the application rather than written to the process's terminal
output. Where the application has installed a log sink they are recorded in its log, under the
`logging` capability's terms; where it has not, they are discarded. Neither outcome changes the level
above: what the runtime reports is decided here, and where it goes is decided by the application.

#### Scenario: The runtime emits diagnostics

- **WHEN** the runtime reports a warning or an error
- **THEN** it is reported
- **AND** informational and verbose runtime messages are not

#### Scenario: Where a diagnostic goes

- **WHEN** the runtime reports a warning or an error while the application has a log sink installed
- **THEN** it is recorded in the application's log
- **AND** it is not written to the process's terminal output

#### Scenario: A diagnostic with no sink installed

- **WHEN** the runtime reports a warning or an error and the application has installed no log sink
- **THEN** it is discarded
- **AND** nothing is written to the process's terminal output

#### Scenario: Telemetry

- **WHEN** initialization starts the environment
- **THEN** the runtime's telemetry is disabled for every session created under it
