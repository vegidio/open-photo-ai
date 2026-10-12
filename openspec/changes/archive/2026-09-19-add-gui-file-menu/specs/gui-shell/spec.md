## ADDED Requirements

### Requirement: Each open image offers a file options menu

The window SHALL offer a file options menu for an open image in two places: on that image's thumbnail
in the drawer's strip, and beside the current image's file name in the navbar. Both SHALL be reached
by an overflow control drawn in place, and both SHALL offer the same three actions - close this
image, close every open image, and show this image in the platform's file manager - with the third
separated from the first two.

The two are one menu offered twice rather than two menus: a user who has learned what the control on
a thumbnail does has learned what the one in the navbar does.

The thumbnail's menu SHALL act on the image whose thumbnail carries it, which is not necessarily the
current one. The navbar's menu SHALL act on the current image, which is the image the navbar names.

The two SHALL be anchored differently, because they open from opposite edges of the window. The
navbar's menu SHALL drop below its control, centred on it. The thumbnail's menu SHALL rise above its
control and extend away from it, its lower leading corner set against the control's upper trailing
corner and clear of it rather than flush against it, so that the menu does not cover the thumbnail it
was opened from.

#### Scenario: The menu on a thumbnail

- **WHEN** the user operates the overflow control on a thumbnail in the strip
- **THEN** a menu opens offering to close that image, to close every open image, and to show that
  image in the file manager

#### Scenario: The menu in the navbar

- **WHEN** an image is open and the user operates the overflow control beside its name in the navbar
- **THEN** the same three actions are offered for the current image

#### Scenario: The thumbnail's menu acts on its own image

- **WHEN** the user opens the menu on a thumbnail that is not the current image and closes that image
- **THEN** that image is closed and the current image is unaffected

#### Scenario: Where each menu opens

- **WHEN** the navbar's menu and a thumbnail's menu are opened
- **THEN** the navbar's drops below its control and the thumbnail's rises above its own, clear of the
  thumbnail it was opened from

#### Scenario: No image open

- **WHEN** no image is open
- **THEN** the navbar offers no overflow control, because it names no file

### Requirement: Closing the last open image folds the drawer

When the last open image is closed, whether singly or by closing all of them, the drawer SHALL fold.

An unfolded drawer over an empty strip reports nothing and covers the canvas that has just become the
only thing left to say - which is the state the window starts in, and closing everything returns the
window to it.

#### Scenario: Closing every image

- **WHEN** the drawer is unfolded and the user closes all open images
- **THEN** the drawer folds

#### Scenario: Closing the last image singly

- **WHEN** the drawer is unfolded with one image open and the user closes it
- **THEN** the drawer folds

#### Scenario: Closing one of several

- **WHEN** the drawer is unfolded with several images open and the user closes one of them
- **THEN** the drawer stays unfolded

## MODIFIED Requirements

### Requirement: The navbar reports the current image

While an image is open, the navbar SHALL show the current image's file name and its pixel dimensions,
labelled as such, and SHALL offer a control beside the name that opens the file options menu for that
image. While no image is open it SHALL show none of the three.

The name shown SHALL be the file's own name rather than its location: the navbar answers "which
photograph am I looking at", and a path long enough to push the rest of the navbar off the window
answers it worse than the name does.

#### Scenario: An image is open

- **WHEN** an image is open
- **THEN** the navbar shows its file name and its width and height in pixels, with a control beside
  the name for that image's file options

#### Scenario: No image is open

- **WHEN** no image is open
- **THEN** the navbar shows no file name, no dimensions and no file options control

#### Scenario: A file in a deep folder

- **WHEN** the current image's file lies several folders deep
- **THEN** the navbar shows the file's name and not the folders above it

### Requirement: Clicking away folds the drawer

A click outside the unfolded drawer SHALL fold it, so that the canvas the drawer is covering is
reachable again by the same gesture that moves attention back to it.

A menu opened from within the drawer is part of the drawer for this purpose, however far outside the
drawer's own bounds it is drawn. Operating it SHALL NOT fold the drawer, because a user reaching into
a menu the drawer itself opened is not reaching past the drawer.

#### Scenario: A click on the canvas

- **WHEN** the drawer is unfolded and the user clicks the preview canvas or the sidebar
- **THEN** the drawer folds

#### Scenario: A click inside the drawer

- **WHEN** the drawer is unfolded and the user clicks one of its thumbnails or controls
- **THEN** the drawer stays unfolded

#### Scenario: A click inside a menu the drawer opened

- **WHEN** the user opens a thumbnail's file options menu and chooses one of its actions
- **THEN** the action is taken and the drawer does not fold on account of that click

#### Scenario: Dismissing that menu

- **WHEN** the user opens a thumbnail's file options menu and then clicks the preview canvas
- **THEN** the menu closes and the drawer folds
