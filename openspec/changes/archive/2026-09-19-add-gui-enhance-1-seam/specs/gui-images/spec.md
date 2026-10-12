## MODIFIED Requirements

### Requirement: Only files the user has opened can be served

The application SHALL serve pixels only for files it has admitted through one of the two routes
above, or for an enhanced result it has itself produced from such a file, and SHALL address both by
identity rather than by location.

A request naming an identity the application has not admitted SHALL be refused, and refused
distinguishably from a file that was admitted and has since become unreadable.

No file on the machine outside that set SHALL be reachable through this route, whatever is named in
the request. The set of images the user has opened, together with what this application has produced
from them, is the whole of what the interface is entitled to read.

An enhanced result is reachable only while the application still holds it. It is produced from
admitted files and never read from disk, so it widens what the interface may draw without widening
what the interface may reach on the machine.

#### Scenario: An unknown identity

- **WHEN** pixels are requested for an identity no admitted file carries
- **THEN** the request is refused, and reported as unknown rather than as unreadable

#### Scenario: An admitted file that has gone away

- **WHEN** pixels are requested for an admitted file that has since been deleted or become unreadable
- **THEN** the request is refused, and reported as unreadable rather than as unknown

#### Scenario: A location outside the opened set

- **WHEN** a request names a location on the machine rather than an admitted identity
- **THEN** nothing is served

#### Scenario: An enhanced result the application holds

- **WHEN** pixels are requested for the identity of an enhanced result the application still holds
- **THEN** they are served, by the same route and in the same terms as an admitted file's

#### Scenario: An enhanced result the application no longer holds

- **WHEN** pixels are requested for the identity of an enhanced result a later run has displaced
- **THEN** the request is refused rather than the file behind it being read
