## ADDED Requirements

### Requirement: A second launch of the graphical application raises the window that already exists

Launching the graphical application while an instance of it is already running for the same user
SHALL NOT open a second window and SHALL NOT report an error. The second launch SHALL end
successfully, and the running instance's window SHALL be made visible, restored if it was minimised,
and given keyboard focus.

Raising the window SHALL NOT change its size or position. A window the user had maximised SHALL
still be maximised afterwards, since the user asked to see the application rather than to resize it.

Anything supplied on the second launch — command-line arguments, the directory it was launched from —
SHALL NOT alter the running application's state. A second launch is a request to see the application,
and nothing else.

#### Scenario: The running window is minimised

- **WHEN** the graphical application is launched while an instance of it is already running for the
  same user and that instance's window is minimised
- **THEN** the window is restored and given focus
- **AND** no second window is opened

#### Scenario: The running window is maximised

- **WHEN** the graphical application is launched while an instance of it is already running for the
  same user and that instance's window is maximised
- **THEN** the window is given focus and is still maximised

#### Scenario: The second launch is not a failure

- **WHEN** the graphical application is launched while an instance of it is already running for the
  same user
- **THEN** the second launch ends successfully, reporting no error to the user and to whatever
  started it

#### Scenario: Arguments on the second launch change nothing

- **WHEN** the graphical application is launched a second time with command-line arguments
- **THEN** the running instance is raised and is otherwise unchanged

### Requirement: A second launch is harmless when the window cannot be raised

Where the running instance has not yet opened its window, a second launch SHALL still end without
opening a window of its own, and the running instance SHALL be unaffected and SHALL continue
starting.

#### Scenario: The running instance is still starting

- **WHEN** the graphical application is launched while an instance of it is already running for the
  same user but that instance has not yet opened its window
- **THEN** the second launch ends without opening a window
- **AND** the running instance continues starting normally

### Requirement: A second launch is recognised across versions of the application

Two builds of the graphical application SHALL be treated as the same application regardless of their
version numbers, so that launching one while the other is running raises the running one rather than
starting a second application alongside it.

Two versions running side by side would open the same configuration directory, which is the outcome
the single-instance claim exists to prevent.

#### Scenario: A different version is launched

- **WHEN** one version of the graphical application is launched while a different version of it is
  already running for the same user
- **THEN** the running instance is raised and the launched one ends without opening a window

### Requirement: Raising the window is scoped to the launching user

The behaviour above SHALL be scoped to the user who launched the application, matching the scope of
the single-instance claim. Two users signed into one machine SHALL each run their own graphical
application, and a launch by one SHALL NOT raise a window belonging to the other.

#### Scenario: Two users on one machine

- **WHEN** two users signed into the same machine each launch the graphical application
- **THEN** both run, each with its own window
- **AND** neither user's launch raises the other user's window

### Requirement: An unavailable activation mechanism does not stop the application from starting

Where the facility the platform provides for reaching an already-running instance is unavailable,
unreachable or misconfigured, the graphical application SHALL start normally without it, rather than
failing to start, exiting silently or reporting an error.

Losing the raise costs a user one extra window. Failing to start costs them the application.

#### Scenario: The platform facility is unreachable

- **WHEN** the graphical application is launched on a system where the facility for reaching an
  already-running instance is not running
- **THEN** the application starts normally

#### Scenario: The platform facility is misconfigured

- **WHEN** the graphical application is launched on a system where that facility is configured with a
  value that cannot be used
- **THEN** the application starts normally
