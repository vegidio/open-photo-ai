## MODIFIED Requirements

### Requirement: The file does not carry the collector's spans

The spans sent to the collector SHALL NOT change the file. A record SHALL be written to `opai.log` with exactly the
fields it would have if those spans did not exist: no field of an enclosing unit's span is added to it. Only the
contexts that existed in the file before, such as the model a runtime diagnostic was emitted while building, SHALL
decorate the records inside them.

There is one other context the file SHALL carry: the request a graphical application's window made. A record emitted
while that request is served SHALL carry the request's identifier when the request has one, so that a failure the
library recorded can be traced back to the window's request. The identifier SHALL appear on a record once, however
many layers wrote records while serving that request. No other field of the request's context SHALL be added to a
record.

#### Scenario: A record inside an enhancement

- **WHEN** an enhancement is run at the default verbosity
- **THEN** each of its records in the file carries only the fields it was emitted with, the contexts the file already
  wrote before this change, and the identifier of the window's request that asked for it

#### Scenario: A runtime diagnostic during a build

- **WHEN** the inference runtime emits a warning while a model's session is being built
- **THEN** the record in the file still names the model and the execution provider being attached

#### Scenario: A failure the library recorded for the window

- **WHEN** an enhancement the window asked for under a given run fails in one of its operations
- **THEN** the library's record of the failure in the file carries that run's identifier exactly once

## ADDED Requirements

### Requirement: A graphical application's requests are units of work

Each request a graphical application's window makes of it SHALL be a unit of work, sent to the collector as a span
while telemetry is running. The units of work this library does to serve a request SHALL be its children, including
work handed to another thread. So one enhancement the window asked for is one trace, rooted at the window's request.

A request's span SHALL say whether the request finished, stopped or failed. A failed request's span SHALL be marked as
failed, carrying the reason. A stopped one SHALL NOT be.

The one request exempt is the one that records the user's analytics choice. Its whole contract is that turning
analytics off is not reported anywhere.

#### Scenario: An enhancement from the window

- **WHEN** the window asks for an enhancement while telemetry is running
- **THEN** the collector receives one trace whose root is the window's request
- **AND** the enhancement's span is a child of that request

#### Scenario: A photograph described on another thread

- **WHEN** the window asks for photographs to be described while telemetry is running
- **THEN** each description's span belongs to the trace of the window's request

#### Scenario: A request the window withdrew

- **WHEN** the window cancels a run it asked for while telemetry is running
- **THEN** the span of the request that asked for the run says it stopped
- **AND** neither that span nor the cancellation's own span is marked as failed

#### Scenario: Declining analytics

- **WHEN** the user turns analytics off
- **THEN** the collector receives no span for the request that recorded the choice

### Requirement: A graphical application records each failure only it observes, once

A graphical application can fail at things this library never sees: a link the system would not open, a file manager
that would not show a file, a destination it could not claim, a request that arrived before the application was ready
or named a photograph it never opened. Each such failure of a window's request SHALL be recorded at warning severity,
once, naming the request and the reason.

A failure this library already recorded SHALL NOT be recorded again by the graphical application, at any severity,
as the requirement that a failure is recorded once, where it happened, already demands. A stop SHALL NOT be recorded as
a failure.

Every failure a request can return SHALL fall into exactly one of those three cases: recorded by the library, recorded
by the graphical application, or not a failure.

#### Scenario: A link that will not open

- **WHEN** the window asks for a link to be opened and the system refuses
- **THEN** exactly one record at warning severity names the link and the reason

#### Scenario: A photograph that cannot be read for an enhancement

- **WHEN** the window asks for an enhancement of a photograph that can no longer be read
- **THEN** exactly one record at warning severity or above describes that failure, and it is the library's

#### Scenario: A destination that cannot be claimed

- **WHEN** an export's destination cannot be claimed
- **THEN** exactly one record at warning severity names the destination and the reason
- **AND** no record at error severity is written for it

### Requirement: The end of a graphical session is recorded

A graphical session's file starts with a header that says who started it. Its end SHALL be recorded too, at
informational severity, with how long the session lasted. While telemetry is running the record SHALL reach the
collector, so that a session that ended normally can be told apart from one that stopped sending because it crashed.

A second launch asks the application already running to raise its window. When the running application cannot show,
restore or focus that window, it SHALL record why, in the file rather than on a console nobody reads.

#### Scenario: Quitting the application

- **WHEN** the user quits the graphical application while telemetry is running
- **THEN** the file holds a record saying the session ended and how long it lasted
- **AND** the collector receives that record

#### Scenario: A second launch whose window cannot be raised

- **WHEN** a second launch is refused and the running application cannot show or focus its window
- **THEN** the running application's file holds a warning saying so, with the reason
