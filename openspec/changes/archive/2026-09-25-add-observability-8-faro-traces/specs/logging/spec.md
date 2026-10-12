## MODIFIED Requirements

### Requirement: A graphical application's requests are units of work

Each request a graphical application's window makes of it SHALL be a unit of work, sent to the collector as a span
while telemetry is running. The units of work this library does to serve a request SHALL be its children, including
work handed to another thread. So one enhancement the window asked for is one trace.

Where that trace begins SHALL depend on the window:
- **When the request names a trace the window started,** the request's span SHALL continue that trace, as a child of
  the span the request names. So the window's work and the backend's are one trace, rooted in the window.
- **When the request names no trace, or names one that cannot be read,** the request's span SHALL root a new trace,
  and the request SHALL be served exactly as it would otherwise be. A trace the window named badly SHALL NOT fail
  the request.

What the request names SHALL NOT reach the file. The file SHALL carry the request's identifier as before, and nothing
else of the request's context.

A request's span SHALL say whether the request finished, stopped or failed. A failed request's span SHALL be marked as
failed, carrying the reason. A stopped one SHALL NOT be.

The one request exempt is the one that records the user's analytics choice. Its whole contract is that turning
analytics off is not reported anywhere.

#### Scenario: An enhancement from the window

- **WHEN** the window asks for an enhancement while telemetry is running, naming no trace
- **THEN** the collector receives one trace whose root is the window's request
- **AND** the enhancement's span is a child of that request

#### Scenario: An enhancement in a trace the window started

- **WHEN** the window asks for an enhancement while telemetry is running, naming a trace and a span of its own
- **THEN** the request's span belongs to that trace, as a child of the span the window named
- **AND** the enhancement's span is a child of the request's span, in the same trace

#### Scenario: A trace that cannot be read

- **WHEN** the window asks for an enhancement naming a trace in a form that cannot be read
- **THEN** the enhancement is served as it would be had the window named no trace
- **AND** the request's span roots a new trace

#### Scenario: The named trace stays out of the file

- **WHEN** an enhancement asked for under a named trace records a failure in the file
- **THEN** the record carries the run's identifier
- **AND** carries nothing of the trace the window named

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
