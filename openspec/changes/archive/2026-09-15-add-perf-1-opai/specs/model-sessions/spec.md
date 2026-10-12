## MODIFIED Requirements

### Requirement: Every resident session can be released on request

The application SHALL offer a way to release every resident session at once, for shutting down or for
reclaiming memory. Sessions in use SHALL be released when their last use ends, under the rule above,
and the call SHALL NOT wait for a run in progress.

This SHALL be reachable by the application embedding the library, not only from inside it. Shutting
down, giving memory back on request, and making a measurement start from nothing are all decisions
that belong to whoever owns the process, and none of them can be taken by a library that keeps the
capability to itself.

Where nothing is in use, the sessions' resources SHALL be released before the call returns, rather
than at some later point of the library's choosing. A caller that releases everything and then builds
a model again is entitled to have paid the full construction cost for it — that is the whole purpose
of releasing first, and a release that had not finished by the time the call returned would hand back
a still-resident session and make the rebuild look free.

#### Scenario: Clearing with nothing in use

- **WHEN** every resident session is released and none is in use
- **THEN** all of their resources are freed before the call returns
- **AND** a later request for any of those models builds it again

#### Scenario: Clearing with a run in progress

- **WHEN** every resident session is released while one is in use
- **THEN** the call returns without waiting for that run
- **AND** the session in use is released when its run ends

#### Scenario: Releasing from outside the library

- **WHEN** the application embedding the library asks for every resident session to be released
- **THEN** the request is served on the same terms as one made from within
