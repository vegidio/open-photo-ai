## ADDED Requirements

### Requirement: Each request the window makes is one trace with the backend's work

While it is sending, the window SHALL send a span for each request it makes of the application's backend, named
after the request, lasting from when it was made until it was answered. A request that was refused SHALL have its span
marked as failed. The backend's span for that request, and all the work beneath it, SHALL belong to the window's
trace, as children of the window's span.

Two requests SHALL have no span and SHALL name no trace:
- the one that records the analytics choice, which is never reported
- the one that writes the window's own records to the file, which is not a request for work

A request's span SHALL carry nothing beyond its name, its timing, whether it failed, and what every span the window
sends carries. It SHALL NOT carry the request's arguments or its answer, and no text from an error.

While the window is not sending, its requests SHALL name no trace. The backend SHALL then root the request's trace at
its own span, as it does for a window that sends nothing.

#### Scenario: An enhancement asked for by the window

- **WHEN** the user enhances an image while the window and the backend are both sending
- **THEN** Grafana holds one trace for it
- **AND** its root is the window's span for the `enhance` request
- **AND** the backend's span for that request, and the enhancement's operations beneath it, are in that trace

#### Scenario: A request the backend refuses

- **WHEN** the window asks for an enhancement of an image the backend no longer holds, while both halves are sending
- **THEN** the window's span for that request is marked as failed
- **AND** the backend's span for it, in the same trace, is marked as failed with the reason

#### Scenario: A launch where only the window sends

- **WHEN** the window asks for an enhancement in a build that names the window's collector but not the backend's
- **THEN** Grafana holds the window's span for the request, and nothing from the backend

#### Scenario: Declining analytics

- **WHEN** the user turns analytics off and saves
- **THEN** no span is sent for the request that records the choice
- **AND** no later request the window makes in that session names a trace

#### Scenario: A request carries no user content

- **WHEN** the window asks to reveal an exported file in the file manager, while it is sending
- **THEN** the request's span names the request
- **AND** carries neither the file's path nor any other argument
