## MODIFIED Requirements

### Requirement: Records are also sent to a collector once the application starts telemetry

The file is the account a user can be asked for. A collector is where records arrive without their being asked. Once
an application has started telemetry, the records that reach the file SHALL also be sent to this project's collector
if the build names one: this library's, the front end's and the inference runtime's. Units of work that were
recorded as spans SHALL be sent as traces, and a record emitted inside one SHALL carry that trace.

An application SHALL be able to mark a record as the file's alone. A record so marked SHALL be written to the file as
any other, and SHALL NOT be sent to the collector. This exists for records whose account reaches the collector by
another path, so the collector does not receive the same fact twice.

Sending SHALL NOT change the file. A record SHALL be written to `opai.log` exactly as it would be with telemetry never
started, and a record discarded on the way to the collector SHALL still reach the file. The only records telemetry
adds to the file are its own: that it started, and that sending failed.

#### Scenario: An application starts telemetry

- **WHEN** an application whose build names a collector installs the log sink and then starts telemetry
- **THEN** a record emitted afterwards at informational severity is received by the collector
- **AND** the same record is written to the file

#### Scenario: A record inside a unit of work

- **WHEN** a record is emitted inside a span while telemetry is running
- **THEN** the collector receives the span as part of a trace
- **AND** the record it receives carries that trace's identifiers

#### Scenario: The file is the same either way

- **WHEN** the same work is run once with telemetry started and once without
- **THEN** the records the work produced are written to the file identically in both runs

#### Scenario: A record marked as the file's alone

- **WHEN** an application emits a warning marked as the file's alone while telemetry is running
- **THEN** the warning is written to the file
- **AND** the collector does not receive it

### Requirement: A graphical application's window records the failures only it observes

A graphical application's window runs code this library never sees, and its failures otherwise go only to a
console no user can open. Each failure only the window observes SHALL be recorded in the file, under the graphical
application's identity. That covers:
- an error raised while drawing the window
- an error nothing caught
- a promise rejection nothing handled
- a failure the window catches and continues through

The window's records SHALL be the file's alone. While the window is sending, each of these failures SHALL reach
Grafana once, through the window's own reporting (see `gui-analytics`), and not a second time as a record.

A record SHALL carry what the window said about the failure, and the error's own text. It SHALL also carry the stack
and, for a drawing error, the drawn component path, when the error has them. Each SHALL be written on one line of
the file, however many lines the stack has.

**Severity:**
- An error raised while drawing the window SHALL be recorded at error severity. It is what the window does not
  survive: nothing is left on screen except the way to recover.
- Every other failure the window observes SHALL be recorded at warning severity.

**Once:**
- A failure the window receives in answer to one of its requests SHALL NOT be recorded by the window, at any
  severity, when the graphical application or this library has already recorded it or it was a stop. The requirement
  that a failure is recorded once, where it happened, already demands that.
- A request that failed without ever reaching the graphical application's handling of it SHALL be recorded by the
  window, because nothing else can. This includes a request that could not be delivered.

Recording a failure SHALL NOT change what the window does about it, and SHALL NOT be a unit of work of its own.
Recording is not a request of the window's that is sent to the collector as a span.

#### Scenario: A drawing error

- **WHEN** part of the window throws an error while it is being drawn
- **THEN** one record at error severity in the file names the error and carries the drawn component path

#### Scenario: A rejection nothing handled

- **WHEN** a promise in the window rejects and nothing handles it
- **THEN** one record at warning severity in the file carries the rejection's reason

#### Scenario: A failure the window catches

- **WHEN** copying a failure's reason to the clipboard is refused
- **THEN** one record at warning severity in the file says the copy was refused and why

#### Scenario: A failure the library already recorded

- **WHEN** the window asks for an enhancement, the enhancement fails, and the window shows the user a notice
- **THEN** exactly one record at warning severity or above describes that failure, and it is the library's

#### Scenario: A stop

- **WHEN** the window stops an analysis it asked for, and the analysis answers that it was stopped
- **THEN** the window records nothing about it

#### Scenario: Records while telemetry runs

- **WHEN** the window records a failure while telemetry is running
- **THEN** the file holds the record
- **AND** the collector receives no record of it and no span for the recording itself
