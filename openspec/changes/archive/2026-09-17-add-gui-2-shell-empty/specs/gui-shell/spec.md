## Purpose

The frame the application window always shows, whatever is loaded into it: the navbar across the
top, the preview canvas, the image drawer folded against its bottom edge, and the sidebar down the
right. This capability covers how those regions are arranged, how the window is titled and dragged on
each platform, and which of the frame's controls are live when no image is open.

## ADDED Requirements

### Requirement: The window always shows the same four regions

The application window SHALL present four regions at all times: a navbar across the full width at the
top, a preview canvas, an image drawer anchored to the bottom of the canvas, and a sidebar down the
right of the window below the navbar.

The regions SHALL be present whether or not an image is open. What they contain changes with the
application's state; that there are four of them, and where each one is, does not. The drawer
overlays the canvas rather than displacing it, so folding and unfolding it SHALL NOT change the size
of the canvas.

#### Scenario: The frame with no image open

- **WHEN** the application window is shown with no image open
- **THEN** the navbar, the preview canvas, the image drawer and the sidebar are all present

#### Scenario: Folding the drawer does not resize the canvas

- **WHEN** the image drawer is folded away or unfolded
- **THEN** the preview canvas keeps the size it had, because the drawer is drawn over it

### Requirement: The navbar identifies the application and its version

The navbar SHALL show the application's name and the version the core library reports, the same value
the terminal front end prints. The version SHALL be the running build's, read at run time rather than
written into the interface.

#### Scenario: Name and version are shown

- **WHEN** the application window is shown
- **THEN** the navbar carries the application name and the running build's version

### Requirement: On macOS the window has no title bar of its own

On macOS the window SHALL have no separate title bar: the system's window controls are drawn over the
navbar, the window's title text is not drawn, and the navbar leaves the controls clear rather than
placing anything underneath them.

On every other platform the window SHALL keep the system's own decorations, and the navbar SHALL sit
below them with no reserved space.

The navbar SHALL act as the window's drag handle on every platform, so a window whose title bar has
been taken away can still be moved.

#### Scenario: macOS

- **WHEN** the application window is shown on macOS
- **THEN** no title bar is drawn above the navbar, and the system's window controls sit within the
  navbar with nothing drawn beneath them

#### Scenario: Windows and Linux

- **WHEN** the application window is shown on Windows or Linux
- **THEN** the system's own title bar is drawn, and the navbar reserves no space for window controls

#### Scenario: Dragging the window

- **WHEN** the user drags an empty part of the navbar
- **THEN** the window moves with the pointer

### Requirement: With no image open the canvas invites one

While no image is open, the preview canvas SHALL show an invitation to supply one: that images may be
dragged and dropped onto it, and a control that browses for them instead.

#### Scenario: The empty canvas

- **WHEN** the application window is shown with no image open
- **THEN** the canvas states that images can be dragged and dropped onto it to start editing them,
  and offers a control to browse for images

### Requirement: Controls that act on an image are unavailable while none is open

Every control in the frame whose effect is on an image SHALL be unavailable while no image is open,
and SHALL be shown as unavailable rather than hidden, so that the frame keeps the same shape once an
image is loaded.

This covers folding and unfolding the image drawer, selecting all images, choosing how the preview is
compared, zooming the preview, adding an enhancement, and exporting.

#### Scenario: No image open

- **WHEN** the application window is shown with no image open
- **THEN** the drawer's fold control, the select-all control, the preview-comparison control, the
  zoom control, the add-enhancement control and the export control are all present and all disabled

### Requirement: The image drawer starts folded

The image drawer SHALL start folded, showing its header and none of its body. Folded, the header SHALL
remain visible along the bottom edge of the canvas, because the controls it carries are how the drawer
is unfolded again.

#### Scenario: The drawer at startup

- **WHEN** the application window is shown
- **THEN** the drawer is folded, and its header is visible along the bottom edge of the canvas

### Requirement: The sidebar reports that there is no preview

While no image is open, the sidebar's preview area SHALL say so in words rather than leaving an empty
panel, and the rest of the sidebar SHALL keep its layout so that loading an image changes what the
regions contain and not where they are.

#### Scenario: The sidebar with no image open

- **WHEN** the application window is shown with no image open
- **THEN** the sidebar's preview area states that no preview is available

### Requirement: Autopilot is on by default and is remembered

The frame SHALL carry a control for Autopilot, it SHALL be on the first time the application is run,
and the user's setting SHALL survive a restart.

Autopilot is available whether or not an image is open: it is a standing preference about what should
happen when one is loaded, not an action on the image currently open.

#### Scenario: First run

- **WHEN** the application is run for the first time
- **THEN** Autopilot is on

#### Scenario: The setting survives a restart

- **WHEN** the user turns Autopilot off and the application is restarted
- **THEN** Autopilot is still off

#### Scenario: Available with no image open

- **WHEN** the application window is shown with no image open
- **THEN** the Autopilot control can still be toggled
