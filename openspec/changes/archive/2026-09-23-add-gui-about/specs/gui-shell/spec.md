## ADDED Requirements

### Requirement: The navbar's About control opens a dialog identifying the application

The navbar's About control SHALL open the About dialog, whether or not an image is open. The dialog SHALL show, in
order: the application icon, the application name, the running build's version (the same value the navbar shows), the
copyright line, and two links - the project's repository and the author's website.

The version and the copyright line SHALL be selectable, so they can be copied into a bug report. The rest of the
dialog SHALL behave as the rest of the window does and not be selectable.

The dialog SHALL be dismissable by its close control, by pressing Escape, and by clicking outside it. Dismissing it
SHALL change nothing else.

#### Scenario: Opening About

- **WHEN** the user presses About in the navbar
- **THEN** the About dialog opens, showing the application icon, the application name, the running build's version,
  the copyright line and the two links

#### Scenario: About is available with nothing open

- **WHEN** no image is open and the user presses About
- **THEN** the About dialog opens

#### Scenario: Copying the version

- **WHEN** the user drags across the version line, or the copyright line, and copies
- **THEN** the copied text is that line

#### Scenario: Dismissing the dialog

- **WHEN** the About dialog is open and the user presses its close control, presses Escape, or clicks outside it
- **THEN** the dialog closes and the window is as it was before About was pressed

### Requirement: The About dialog's links open in the system browser

Each link in the About dialog SHALL open its page in the system's default browser. Neither SHALL navigate the
application window, however it is clicked.

The window SHALL be able to ask for only those two pages to be opened: which address a link opens SHALL be decided
outside the window, not by anything the window sends.

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

- **WHEN** the window asks for a page to be opened that is not one of the two links
- **THEN** nothing is opened
