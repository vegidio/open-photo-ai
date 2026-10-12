## ADDED Requirements

### Requirement: The image drawer can be folded and unfolded

While at least one image is open, the drawer's fold control SHALL unfold the drawer onto its body and
fold it away again, and SHALL report which of the two states the drawer is in.

Unfolding SHALL NOT change the size of the preview canvas, because the drawer is drawn over it. The
drawer's header SHALL stay visible in both states, since it carries the control that unfolds it again.

#### Scenario: Unfolding the drawer

- **WHEN** the user operates the fold control while the drawer is folded
- **THEN** the drawer's body is shown and the control reports the drawer as unfolded

#### Scenario: Folding the drawer again

- **WHEN** the user operates the fold control while the drawer is unfolded
- **THEN** the drawer's body is hidden, its header stays visible, and the control reports the drawer
  as folded

#### Scenario: The canvas is not resized

- **WHEN** the drawer is unfolded and folded again
- **THEN** the preview canvas keeps the size it had throughout

### Requirement: The drawer unfolds itself when there is more than one image to choose from

The drawer SHALL unfold on its own whenever the number of open images changes to more than one, and
SHALL NOT unfold itself while the window holds one image or none.

With one image open the strip says nothing the rest of the window is not already saying. With two, the
strip is the only place the second one is visible at all, and a window that had just accepted twenty
files while showing one of them would be reporting a fraction of what the user asked for.

Every batch that adds an image to a window that already had one unfolds the drawer again, including
one the user has folded away in between - as the reference implementation does it. Arriving images are
treated as the more recent statement of what the user wants to see.

#### Scenario: A second image arrives

- **WHEN** the window holds one open image and the user opens another
- **THEN** the drawer unfolds

#### Scenario: One image only

- **WHEN** the user opens a single image into an empty window
- **THEN** the drawer stays folded

#### Scenario: More images after the drawer was folded

- **WHEN** the user folds the drawer with several images open and then opens further images
- **THEN** the drawer unfolds again

#### Scenario: A batch that adds nothing

- **WHEN** the user folds the drawer and then re-opens files that are all already open
- **THEN** the drawer stays folded, because the number of open images did not change

### Requirement: Clicking away folds the drawer

A click outside the unfolded drawer SHALL fold it, so that the canvas the drawer is covering is
reachable again by the same gesture that moves attention back to it.

#### Scenario: A click on the canvas

- **WHEN** the drawer is unfolded and the user clicks the preview canvas or the sidebar
- **THEN** the drawer folds

#### Scenario: A click inside the drawer

- **WHEN** the drawer is unfolded and the user clicks one of its thumbnails or controls
- **THEN** the drawer stays unfolded

### Requirement: The drawer's body is the strip of open images

Unfolded, the drawer SHALL show one thumbnail per open image, in the order the images were opened, and
SHALL scroll horizontally when there are more of them than fit across the window.

Each thumbnail SHALL draw the image it stands for, SHALL be labelled with that image's file name
rather than its location, and SHALL carry a control for picking that image out of the open set.

#### Scenario: One thumbnail per open image

- **WHEN** the drawer is unfolded with several images open
- **THEN** it shows one thumbnail per image, in the order they were opened, each labelled with its
  file name

#### Scenario: More images than fit

- **WHEN** more images are open than fit across the strip
- **THEN** the strip scrolls horizontally to reach the rest

#### Scenario: An image whose format the interface cannot decode

- **WHEN** an open image is a camera RAW, a TIFF or a HEIF
- **THEN** its thumbnail draws the photograph like any other

### Requirement: The strip marks which image is the current one

The thumbnail of the current image SHALL be distinguished from the others - outlined, with its name
drawn inverted - and exactly one thumbnail SHALL be marked at a time.

The strip is where the user chooses between images, so it is where the answer to "which one am I
looking at" has to be visible without reading the navbar.

#### Scenario: The current image is marked

- **WHEN** the drawer is unfolded with several images open
- **THEN** the thumbnail of the current image is marked as current and no other thumbnail is

#### Scenario: The mark follows the current image

- **WHEN** the current image changes
- **THEN** the mark moves to that image's thumbnail

### Requirement: The select-all control reports how much of the strip is picked

The select-all control SHALL report three states - nothing picked, some of the open images picked, and
all of them picked - and SHALL read as an instruction to unpick once every open image is picked.

Operating it SHALL pick every open image when not all of them are picked, and unpick every one of them
when they all are.

#### Scenario: Nothing is picked

- **WHEN** no open image is picked
- **THEN** the control reports nothing picked and offers to select all

#### Scenario: Some are picked

- **WHEN** some but not all of the open images are picked
- **THEN** the control reports a partial selection

#### Scenario: All are picked

- **WHEN** every open image is picked
- **THEN** the control reports all picked and offers to unselect all

#### Scenario: Picking everything

- **WHEN** the user operates the control while only some images are picked
- **THEN** every open image is picked

#### Scenario: Unpicking everything

- **WHEN** the user operates the control while every image is picked
- **THEN** no image is picked

### Requirement: A large set of open images does not cost what it would cost to draw them all

The strip SHALL draw only the thumbnails at or near the part of it the user can see, and the pixels of
an image SHALL NOT be requested until its thumbnail is close to being seen.

Opening several hundred images is a supported thing to do - a folder dropped onto the window is
routinely that - and a strip that showed a dozen of them at a time must not pay for the rest. Scrolling
SHALL reach every open image regardless, and the strip's total scrolling extent SHALL account for the
images it has not drawn.

#### Scenario: Several hundred images are opened

- **WHEN** the user opens several hundred images and unfolds the drawer
- **THEN** only the thumbnails at or near the visible part of the strip are drawn, and the strip
  scrolls as though all of them were

#### Scenario: Pixels are fetched when they are needed

- **WHEN** a thumbnail is far from the visible part of the strip
- **THEN** that image's pixels are not requested until it is scrolled close to view

### Requirement: What the window draws at the bottom of the canvas stays clear of the drawer

The drawer is drawn over the canvas rather than beside it, so whatever the window anchors to the
canvas's bottom edge SHALL be drawn clear of whichever of the drawer's two states is showing - the
unfolded body or the folded header - rather than behind or over it.

This covers the notices the window raises and the labels naming each side of a comparison. Both say
which of several things the user is looking at, and both are unreadable behind the strip; the drawer
moving is what makes where they sit a question at all.

#### Scenario: A notice with the drawer unfolded

- **WHEN** a notice is raised while the drawer is unfolded
- **THEN** it is drawn clear of the drawer's body

#### Scenario: A notice with the drawer folded

- **WHEN** a notice is raised while the drawer is folded
- **THEN** it is drawn clear of the drawer's header

#### Scenario: The drawer moves under a notice already shown

- **WHEN** the drawer unfolds while a notice is on screen
- **THEN** the notice moves with it and stays clear of the strip

#### Scenario: The pane labels with the drawer unfolded

- **WHEN** the drawer is unfolded
- **THEN** each pane's label is drawn clear of the drawer's body, in every comparison

#### Scenario: The pane labels with the drawer folded

- **WHEN** the drawer is folded
- **THEN** each pane's label is drawn clear of the canvas's bottom edge
