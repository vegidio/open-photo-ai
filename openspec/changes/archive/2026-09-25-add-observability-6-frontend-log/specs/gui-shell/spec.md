## MODIFIED Requirements

### Requirement: The window always shows the same four regions

The application window SHALL present four regions at all times: a navbar across the full width at the
top, a preview canvas, an image drawer anchored to the bottom of the canvas, and a sidebar down the
right of the window below the navbar.

The regions SHALL be present whether or not an image is open. What they contain changes with the
application's state; that there are four of them, and where each one is, does not. The drawer
overlays the canvas rather than displacing it, so folding and unfolding it SHALL NOT change the size
of the canvas.

The one exception is a window that could not draw itself. It shows the recovery screen in place of all four regions,
until it is reloaded.

#### Scenario: The frame with no image open

- **WHEN** the application window is shown with no image open
- **THEN** the navbar, the preview canvas, the image drawer and the sidebar are all present

#### Scenario: Folding the drawer does not resize the canvas

- **WHEN** the image drawer is folded away or unfolded
- **THEN** the preview canvas keeps the size it had, because the drawer is drawn over it

## ADDED Requirements

### Requirement: A window that cannot draw itself shows a way to recover

When drawing the window fails with an error nothing else handled, the window SHALL NOT be left blank. It SHALL show
a recovery screen that carries:
- a title saying something went wrong
- a sentence saying the application hit an unexpected error, that reloading usually fixes it, and that settings are
  kept but unsaved work in the session is lost
- the error's own text, which can be selected and copied
- **Reload**, which reloads the window
- **Show logs**, which shows the log file in the platform's file manager, as the settings dialog's Show logs does

The screen's text SHALL be in the language the application is running in. If the log file cannot be shown, the user
SHALL be told, as the settings dialog tells them.

The window SHALL still be shown when the failure happens during its very first drawing. A window that ships hidden
and never becomes visible is the blank window this requirement exists to prevent.

#### Scenario: A drawing error

- **WHEN** part of the window throws an error while it is being drawn
- **THEN** the recovery screen replaces the window's contents
- **AND** it shows the error's text

#### Scenario: A crash on the first drawing

- **WHEN** the window's first drawing fails
- **THEN** the window is shown with the recovery screen on it

#### Scenario: Reloading

- **WHEN** the user presses Reload on the recovery screen
- **THEN** the window reloads and draws the application again
- **AND** the user's settings are as they were

#### Scenario: Showing the log from the recovery screen

- **WHEN** the user presses Show logs on the recovery screen
- **THEN** the log file is shown selected in the platform's file manager
- **AND** a failure to show it is reported to the user
