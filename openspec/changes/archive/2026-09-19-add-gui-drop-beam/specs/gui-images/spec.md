## ADDED Requirements

### Requirement: The region that accepts a drop identifies itself while a drag is over it

While files are being dragged over the preview canvas, the canvas SHALL draw a visible region marking where they
will be accepted. The mark SHALL appear when the drag arrives over the canvas and SHALL disappear when the drag
leaves it or the files are let go of, whether or not anything was opened.

The region drawn SHALL be the region that accepts the drop. The canvas already accepts a drop and nowhere else does,
and drawing one rectangle while accepting another would teach the user a boundary the application does not honour -
so what is offered and what is taken up stay the same answer, told twice.

The mark SHALL appear for any drag over the canvas, without regard to what is being dragged. It states that the
region takes files, not that these files are acceptable: what the application will not open is already reported by
name after the drop, in the user's own language, and a mark that had to resolve the question first would appear
late or not at all on a drag passed quickly over the window.

The mark SHALL be drawn in both of the canvas's states, whether or not an image is already open, because the canvas
accepts a drop in both.

A drag over any other region of the window SHALL draw nothing, by the same rule that makes a drop there open
nothing and report nothing.

The mark SHALL be decorative: it SHALL carry no text, SHALL NOT be announced to assistive technology, and SHALL NOT
be reachable by keyboard. What the drop area means is already stated in words by the empty canvas.

#### Scenario: A drag arrives over the canvas

- **WHEN** the user drags files over the window's preview canvas
- **THEN** the canvas draws a region marking where the files will be accepted

#### Scenario: The drag leaves the canvas

- **WHEN** the user drags files over the canvas and then away from it without letting go
- **THEN** the mark is no longer drawn
- **AND** the window's open images are unchanged

#### Scenario: The files are let go of

- **WHEN** the user drops files onto the canvas
- **THEN** the mark is no longer drawn
- **AND** those images are open in the window

#### Scenario: A drag carrying nothing the application can open

- **WHEN** the user drags a file in a format the application does not open over the canvas
- **THEN** the canvas draws the mark exactly as it does for any other drag
- **AND** letting go names that file in a notice and opens nothing

#### Scenario: A drag over another region of the window

- **WHEN** the user drags files over the sidebar, the navbar or the image drawer rather than the canvas
- **THEN** nothing is drawn anywhere in the window

#### Scenario: A drag over the part of the canvas an unfolded drawer is covering

- **WHEN** the image drawer is unfolded over the lower part of the canvas
- **AND** the user drags files over the strip of thumbnails
- **THEN** nothing is drawn there, and the mark the canvas draws above it ends at the top of the strip
- **AND** letting go there opens nothing, by the same rule

#### Scenario: A drag with an image already open

- **WHEN** the user drags files over the canvas while an image is already open
- **THEN** the canvas draws the mark over the photograph it is showing

## MODIFIED Requirements

### Requirement: Images dropped onto the preview canvas are opened

Files dragged onto the window's preview canvas SHALL be opened exactly as files chosen through the
picker are, and what the window then holds SHALL be indistinguishable from the same files having been
picked.

The drop SHALL be accepted over the preview canvas and nowhere else, as the reference implementation
accepts it. The canvas is the region that offers the drop - it is where the window says images can be
dragged and dropped - so the rectangle that makes the offer is the rectangle that takes it up, and
every other region of the window keeps whatever meaning it has of its own.

The strip of thumbnails an unfolded image drawer shows SHALL NOT accept a drop, whatever is behind it.
The drawer covers the lower part of the canvas rather than shortening it, so the region that accepts a
drop ends at the drawer's top edge rather than the canvas's own: what the user is letting go of files
over is the strip, and a region is what it shows rather than what it is drawn on top of. This is the
same rectangle that draws the mark while a drag is over it, so a position that draws nothing cannot
open anything.

A drop let go of outside the canvas SHALL leave the window's open images unchanged and SHALL NOT be
reported to the user as anything: the files were released over a region that never offered to take
them, which is not the same as files the application cannot open.

#### Scenario: Images are dropped on the canvas

- **WHEN** the user drops image files onto the window's preview canvas
- **THEN** those images are open in the window

#### Scenario: A drop away from the canvas

- **WHEN** the user drops image files over the sidebar or the navbar rather than the canvas
- **THEN** no image is opened and nothing is reported to the user

#### Scenario: A drop on the strip an unfolded drawer is showing

- **WHEN** the image drawer is unfolded over the lower part of the canvas
- **AND** the user drops image files over the strip of thumbnails
- **THEN** no image is opened and nothing is reported to the user

#### Scenario: A drop on the canvas above an unfolded drawer

- **WHEN** the image drawer is unfolded
- **AND** the user drops image files over the part of the canvas above the strip
- **THEN** those images are open in the window

#### Scenario: The same file by both routes

- **WHEN** one file is opened by dropping it and the same file is opened through the picker
- **THEN** the window holds the same file with the same name, dimensions and size either way
