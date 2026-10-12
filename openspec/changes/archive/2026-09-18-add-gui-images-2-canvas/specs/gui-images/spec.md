## ADDED Requirements

### Requirement: The formats the application opens can be reported to the interface

The application SHALL be able to report the set of file extensions it can open - the same set the
file picker is filtered to.

The interface needs it because a file can arrive without passing through the picker's filter: a drag
onto the window carries whatever the user dragged. The list is reported rather than written down
beside the interface, so that a format the decoder gains is a format the window accepts, and a format
it loses is one the window stops claiming.

#### Scenario: The interface asks what can be opened

- **WHEN** the interface asks the application which formats it opens
- **THEN** every extension the file picker is filtered to is reported, camera RAW included, and no
  extension the application cannot decode

### Requirement: Images are opened from the window's own controls

The window SHALL offer a control on the empty canvas and a control in the image drawer that each open
the file picker, and every image the user chooses SHALL become one of the window's open images.

Both controls open the same picker and produce the same result. The one on the canvas is how an empty
window stops being empty; the one in the drawer is how images are added to a window that already has
some, which is why it is available whether or not an image is open.

Dismissing the picker SHALL leave the window's open images exactly as they were, and SHALL NOT be
reported to the user as anything - choosing nothing is a choice, and a notice about it would be a
notice about the user changing their mind.

#### Scenario: Browsing from the empty canvas

- **WHEN** the user chooses images through the empty canvas's browse control
- **THEN** those images are open in the window

#### Scenario: Adding images to a window that already has some

- **WHEN** the user chooses images through the drawer's add control while images are already open
- **THEN** those images are open alongside the ones that already were

#### Scenario: The picker is dismissed

- **WHEN** the user dismisses the picker without choosing anything
- **THEN** the window's open images are unchanged and nothing is reported to the user

### Requirement: Images dropped onto the preview canvas are opened

Files dragged onto the window's preview canvas SHALL be opened exactly as files chosen through the
picker are, and what the window then holds SHALL be indistinguishable from the same files having been
picked.

The drop SHALL be accepted over the preview canvas and nowhere else, as the reference implementation
accepts it. The canvas is the region that offers the drop - it is where the window says images can be
dragged and dropped - so the rectangle that makes the offer is the rectangle that takes it up, and
every other region of the window keeps whatever meaning it has of its own.

A drop let go of outside the canvas SHALL leave the window's open images unchanged and SHALL NOT be
reported to the user as anything: the files were released over a region that never offered to take
them, which is not the same as files the application cannot open.

#### Scenario: Images are dropped on the canvas

- **WHEN** the user drops image files onto the window's preview canvas
- **THEN** those images are open in the window

#### Scenario: A drop away from the canvas

- **WHEN** the user drops image files over the sidebar or the navbar rather than the canvas
- **THEN** no image is opened and nothing is reported to the user

#### Scenario: The same file by both routes

- **WHEN** one file is opened by dropping it and the same file is opened through the picker
- **THEN** the window holds the same file with the same name, dimensions and size either way

### Requirement: Files the application cannot open are named and not opened

When files arrive by a route that does not filter them, the application SHALL open those it can and
SHALL NOT open the rest. The files it will not open SHALL be reported to the user by name, in a notice
that states how many there are.

A file is judged by its extension against the set of formats the application opens, which is what the
picker's filter does for the route that has one. Judging it by reading it instead would mean reading
every byte of whatever was dropped - a video, an archive, a disk image - before concluding it is not a
photograph.

Rejecting a file SHALL NOT cost the user the rest of the drop. A folder of photographs dragged in with
a stray document among them is a batch the user assembled, and the document is not a reason to refuse
the photographs.

#### Scenario: A mixed drop

- **WHEN** the user drops several image files and one file in a format the application does not open
- **THEN** the image files are open in the window, and the one that was refused is named in a notice

#### Scenario: Nothing in the drop can be opened

- **WHEN** every file the user drops is in a format the application does not open
- **THEN** no image is opened and the refused files are named in a notice

#### Scenario: Everything in the drop can be opened

- **WHEN** every file the user drops is in a format the application opens
- **THEN** all of them are open and no notice is shown

### Requirement: The window's open images have a current one

The window SHALL hold the images the user has opened as an ordered set with exactly one of them
current, and the current image SHALL be the one the rest of the interface reports and draws.

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
