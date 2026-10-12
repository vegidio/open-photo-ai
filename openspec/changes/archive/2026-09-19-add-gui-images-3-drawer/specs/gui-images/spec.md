## ADDED Requirements

### Requirement: The user chooses which open image is the current one

The window SHALL let the user make any one of its open images the current one, and everything that
reports or draws the current image SHALL follow that choice.

Choosing an image SHALL NOT change which images are open, which of them are picked, or the order they
are held in. It is a change of what the window is looking at, not of what it holds.

#### Scenario: Choosing another image

- **WHEN** the user chooses one of the open images
- **THEN** that image becomes the current one and the window reports and draws it

#### Scenario: Choosing changes nothing else

- **WHEN** the user chooses an image other than the current one
- **THEN** the same images are open, in the same order, with the same ones picked

### Requirement: The window keeps a selection of its open images

Each open image SHALL be either picked or not picked, independently of the others and independently of
which image is current, and the user SHALL be able to pick and unpick any of them.

The selection is what a later batch operation - exporting above all - acts on, which is why it is
separate from the current image: the user reads one photograph while choosing which several to act on.

Picking all of the open images and unpicking all of them SHALL both be available as one action.

#### Scenario: Picking an image

- **WHEN** the user picks an open image that was not picked
- **THEN** that image is picked and no other image's state changes

#### Scenario: Unpicking an image

- **WHEN** the user unpicks an image that was picked
- **THEN** that image is no longer picked and no other image's state changes

#### Scenario: Picking is not the same as being current

- **WHEN** the user unpicks the current image
- **THEN** it is still the current image and is still reported and drawn

#### Scenario: Picking every image

- **WHEN** the user picks all
- **THEN** every open image is picked

#### Scenario: Unpicking every image

- **WHEN** the user unpicks all
- **THEN** no open image is picked

### Requirement: Opening images into an empty window picks the first of them

When images are opened into a window that held none, the first of them SHALL be picked as well as made
current, so that a window with one photograph in it is ready to act on it.

Images opened into a window that already had some SHALL NOT be picked by that, because the selection
at that point is a choice the user has made and arriving files are not a reason to revise it.

#### Scenario: The first open

- **WHEN** the user opens images into a window with none open
- **THEN** the first of them is picked

#### Scenario: Opening more images

- **WHEN** the user opens further images into a window that already has some open
- **THEN** the images that were picked are still picked and the newly opened ones are not
