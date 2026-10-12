## ADDED Requirements

### Requirement: A graphical application's window records the failures only it observes

A graphical application's window runs code this library never sees, and its failures otherwise go only to a
console no user can open. Each failure only the window observes SHALL be recorded in the file, under the graphical
application's identity, and SHALL reach the collector while telemetry is running, as the graphical application's
other records do. That covers:
- an error raised while drawing the window
- an error nothing caught
- a promise rejection nothing handled
- a failure the window catches and continues through

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
- **THEN** the collector receives the same record
- **AND** it receives no span for the recording itself

### Requirement: The window's records are bounded

A failure in the window can repeat every time something is drawn, and a file that grows with every frame would bury
everything else. The window's records SHALL therefore be bounded:
- **The same failure** SHALL be recorded once for as long as the window is loaded. A reload starts again.
- **The number of records one load of the window writes** SHALL be bounded. Once the bound is reached, one record
  SHALL say that the window's further records are being dropped, and nothing more is written until a reload.
- **Each text a record carries** SHALL be bounded in length, and a text cut short SHALL say so.

A failure to record SHALL NOT itself be recorded. The window SHALL go on as it would have.

#### Scenario: A failure that repeats

- **WHEN** the same failure happens many times while the window is loaded
- **THEN** the file holds one record of it

#### Scenario: A window that fails without end

- **WHEN** the window observes more distinct failures than one load may record
- **THEN** the file holds records up to the bound, and one record saying the rest are dropped

#### Scenario: A very long stack

- **WHEN** a failure's stack is longer than a record may carry
- **THEN** the record carries the start of it and says it was cut short

#### Scenario: A window with nowhere to record

- **WHEN** a failure is observed while its record cannot be delivered
- **THEN** the window goes on as it would have, and does not try to record that the record was lost
