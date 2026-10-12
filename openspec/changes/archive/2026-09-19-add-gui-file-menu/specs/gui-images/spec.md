## ADDED Requirements

### Requirement: The user closes an open image

The user SHALL be able to close any one of the open images, which removes it from the window
entirely: it leaves the set of open images, it leaves the selection if it was picked, and how closely
it was being looked at is forgotten.

Closing the current image SHALL make the image that followed it in the order current, so that closing
a photograph moves forward through what is open rather than back to the beginning. Closing the last
image in the order while it is current SHALL make the one before it current. Closing an image that is
not the current one SHALL NOT change which image is current.

Closing is not the same as not having opened: a closed image whose file is opened again SHALL be
admitted as a new one, fitted and unpicked, carrying nothing from before.

#### Scenario: Closing an image that is not the current one

- **WHEN** the user closes an open image other than the current one
- **THEN** that image is no longer open and the current image is still the same photograph

#### Scenario: Closing the current image

- **WHEN** the user closes the current image and another image follows it in the order
- **THEN** the image that followed it becomes the current one and is reported and drawn

#### Scenario: Closing the last image in the order

- **WHEN** the user closes the current image and it was the last in the order
- **THEN** the image before it becomes the current one

#### Scenario: A closed image leaves the selection

- **WHEN** the user closes an image that was picked
- **THEN** it is no longer picked and no other image's picked state changes

#### Scenario: A closed image's view is forgotten

- **WHEN** the user magnifies an image, closes it, and opens its file again
- **THEN** it is drawn whole, in proportion and centred, at the size that fits

#### Scenario: Closing the only open image

- **WHEN** the user closes the only open image
- **THEN** the window holds no open images and reports none

### Requirement: The user closes every open image at once

The user SHALL be able to close all of the open images in one action, leaving the window holding
none, with nothing picked and no image current.

This is offered alongside closing one image rather than instead of it: a user who has dropped the
wrong folder wants the window emptied, not twenty individual closes.

#### Scenario: Closing everything

- **WHEN** the user closes all images with several open
- **THEN** no image is open, none is picked, and the window reports no current image

#### Scenario: Closing everything with one open

- **WHEN** the user closes all images with a single image open
- **THEN** no image is open

### Requirement: An opened image can be shown to the user in the platform's file manager

The user SHALL be able to ask for any open image to be shown in the platform's own file manager, with
the file selected rather than merely its folder opened.

The action SHALL be named after the file manager the platform actually has, so that it says Finder on
macOS and Explorer on Windows, and SHALL fall back to a platform-neutral name elsewhere.

A reveal that does not succeed SHALL be reported to the user. Revealing can fail for a file that has
been moved or deleted since it was opened, and an action that silently does nothing reads as a broken
one.

#### Scenario: Revealing an open image

- **WHEN** the user asks for an open image to be shown in the file manager
- **THEN** the platform's file manager opens with that file selected

#### Scenario: The action is named after the platform

- **WHEN** the action is offered on macOS, on Windows, and on another platform
- **THEN** it is named for Finder, for Explorer, and for the file manager generically, respectively

#### Scenario: A file that has gone away

- **WHEN** the user asks for an open image to be shown and its file has been moved or deleted
- **THEN** the failure is reported to the user rather than passed over

### Requirement: Only files the user has opened can be revealed

The application SHALL show in the file manager only files it has admitted as open images, and a
request naming anything else SHALL be refused with nothing shown.

This is the same entitlement that governs serving pixels, applied to the other door out of the
window: the set of images the user has opened is the whole of what the interface is entitled to
reach, and the file manager is not an exception to that.

Unlike serving pixels, this SHALL hold for an open image the application could not measure. A file
whose header would not be read is still an image the user opened, and it is the one they are most
likely to want to look at on disk.

#### Scenario: A file that was never opened

- **WHEN** a reveal is requested for a file the user has not opened
- **THEN** it is refused and no file manager is opened

#### Scenario: An image the application could not measure

- **WHEN** the user asks for an open image whose header could not be read to be shown
- **THEN** the platform's file manager opens with that file selected

## MODIFIED Requirements

### Requirement: The window's open images have a current one

The window SHALL hold the images the user has opened as an ordered set, with exactly one of them
current while it holds any, and the current image SHALL be the one the rest of the interface reports
and draws. A window holding no images has no current image and reports none.

Opening images SHALL make the first of the newly opened ones current, so that the window shows what
the user just asked for rather than what it was already showing.

A file that is already open SHALL NOT be opened a second time, whichever route it arrives by, and
re-opening it SHALL NOT change which image is current if none of the files in that batch was new.

#### Scenario: The first open

- **WHEN** the user opens images into a window with none open
- **THEN** the first of them is the current image

#### Scenario: Opening more images

- **WHEN** the user opens further images into a window that already has some
- **THEN** the first of the newly opened images becomes the current one and the images already open
  stay open

#### Scenario: The same file opened twice

- **WHEN** the user opens a file that is already open
- **THEN** the window still holds one of it

#### Scenario: A window holding nothing

- **WHEN** the window holds no open images
- **THEN** there is no current image and the interface reports none
