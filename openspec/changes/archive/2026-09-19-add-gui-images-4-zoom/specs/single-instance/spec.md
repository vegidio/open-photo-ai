## ADDED Requirements

### Requirement: A first launch brings its own window to the front

When the graphical application reveals its window for the first time, it SHALL ask for keyboard focus
as well as for the window to be shown.

A window that is shown but left behind whatever the user was looking at is a launch the user has to
go and find. The second-launch raise below already makes this promise for every launch after the
first; the first one is the launch where it is most likely to matter, because there is no running
instance to have raised.

Failing to bring the window to the front SHALL NOT be treated as a failure to launch. What a refusal
costs is one window that opens behind another, not a launch with nothing on screen, and the request
SHALL be made even when showing the window has already failed - a window that is not visible cannot
take focus, so the call costs nothing and the branch that would save it has no reader.

#### Scenario: The window appears

- **WHEN** the graphical application reveals its window
- **THEN** the window is shown and asked to take keyboard focus

#### Scenario: The front refuses

- **WHEN** the window is shown but the platform refuses to bring it to the front
- **THEN** the application goes on running with its window on screen, and records what was refused
