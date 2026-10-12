## ADDED Requirements

### Requirement: The navbar offers an update when a newer release is published

Once per launch, the application SHALL compare the running build's version - the one the navbar shows - with the
latest release published on the project's GitHub repository, `vegidio/open-photo-ai`. Drafts and pre-releases SHALL
NOT count as published releases.

When the latest release is newer, the navbar SHALL show an "Update Available" control after the version, drawing
attention to itself with a repeating pulse. When it is not newer - the same version, or an older one - the control
SHALL NOT be shown.

A check that cannot answer SHALL be treated as "no newer release known": when the release cannot be fetched (no
network, a rate limit, no published release) or either version cannot be read as a version, the control SHALL NOT be
shown, nothing SHALL be drawn in its place, and the failure SHALL be logged. The check SHALL NOT delay or block
anything else the window shows, and SHALL NOT be retried during the same launch.

The check SHALL run in every build, including development builds.

Pressing the control SHALL open the project's releases page in the system's default browser, never in the application
window. If the page cannot be opened, the window SHALL stay as it was, the control SHALL stay shown, and the failure
SHALL be logged.

#### Scenario: A newer release is published

- **WHEN** the running build's version is older than the latest published release
- **THEN** the navbar shows "Update Available" after the version, pulsing

#### Scenario: The running build is current

- **WHEN** the running build's version equals the latest published release's, or is newer
- **THEN** the navbar shows no "Update Available" control

#### Scenario: The check fails

- **WHEN** the latest release cannot be fetched, or its version cannot be read
- **THEN** the navbar shows no "Update Available" control and the failure is logged

#### Scenario: The check runs once

- **WHEN** the window has been open for any length of time
- **THEN** the release has been asked for at most once since launch

#### Scenario: Opening the releases page

- **WHEN** the user presses "Update Available"
- **THEN** the project's releases page opens in the system's default browser
- **AND** the application window is unchanged

## MODIFIED Requirements

### Requirement: The About dialog's links open in the system browser

Each link in the About dialog SHALL open its page in the system's default browser. Neither SHALL navigate the
application window, however it is clicked.

The window SHALL be able to ask for only the pages the application links to - the repository, the author's website and
the releases page - to be opened: which address a link opens SHALL be decided outside the window, not by anything the
window sends.

If the page cannot be opened, the dialog SHALL stay open and unchanged, and the failure SHALL be logged.

#### Scenario: Opening the repository

- **WHEN** the user clicks the repository link
- **THEN** the project's repository opens in the system's default browser
- **AND** the application window still shows the About dialog

#### Scenario: Opening the website

- **WHEN** the user clicks the website link
- **THEN** the author's website opens in the system's default browser
- **AND** the application window still shows the About dialog

#### Scenario: The window cannot open an arbitrary address

- **WHEN** the window asks for a page to be opened that is not one of the pages the application links to
- **THEN** nothing is opened
