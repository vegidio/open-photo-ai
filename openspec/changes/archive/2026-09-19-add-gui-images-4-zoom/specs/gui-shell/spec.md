## ADDED Requirements

### Requirement: The zoom control magnifies the current image

The drawer's zoom control SHALL become available once an image is open, and SHALL carry a slider
across the whole range the current image can be magnified over together with a control at each end
that steps the magnification down and up by a fixed amount.

The slider SHALL report the current image's magnification as well as set it, so that magnifying by
any other route moves the thumb, and making another image current shows that image's magnification
rather than the one just left.

Stepping past either end of the range SHALL hold at that end rather than being refused, and a step
that would change nothing SHALL change nothing.

Magnifying through this control SHALL hold the middle of the pane still, because the control is not
pointed at any particular part of the photograph.

The slider SHALL state the magnification it is sitting at, in words, while the user is pointing at it
or moving it. It is the only control in the window whose value is written nowhere else, and a slider
along an unlabelled range is a control the user can move but cannot aim.

#### Scenario: The control with an image open

- **WHEN** an image is open
- **THEN** the zoom control can be used

#### Scenario: Setting a magnification

- **WHEN** the user moves the zoom slider
- **THEN** the canvas draws the current image at that magnification, about the middle of the pane

#### Scenario: Stepping

- **WHEN** the user presses the control at either end of the slider
- **THEN** the current image's magnification changes by one step in that direction

#### Scenario: Stepping past the end

- **WHEN** the current image is at the largest magnification and the user steps up again
- **THEN** it stays at the largest magnification

#### Scenario: Reading off a magnification

- **WHEN** the user rests the pointer on the slider's thumb, or moves it
- **THEN** the magnification the current image is drawn at is stated beside the thumb

#### Scenario: The slider follows the image

- **WHEN** the current image is magnified by some other route, or another image is made current
- **THEN** the slider reports the magnification of the image the canvas is drawing

### Requirement: The wheel and the trackpad pinch magnify the canvas

Turning the mouse wheel or pinching on a trackpad while the pointer is over the preview canvas SHALL
magnify the current image, up towards the pointer and down away from it, within the range that image
can be magnified over.

The point of the photograph under the pointer SHALL stay under the pointer as the magnification
changes, so that the detail being examined is what the magnification is about rather than something
that slides out of the pane while it happens.

The gesture SHALL NOT scroll the window or the page it is over.

#### Scenario: Magnifying with the wheel

- **WHEN** the user turns the wheel up with the pointer over the canvas
- **THEN** the current image is magnified, and the part of it under the pointer is still under the
  pointer

#### Scenario: Reducing with the wheel

- **WHEN** the user turns the wheel down with the pointer over the canvas
- **THEN** the current image's magnification is reduced, no further than the size that fits

#### Scenario: The gesture away from the canvas

- **WHEN** the user turns the wheel with the pointer over the sidebar or the drawer
- **THEN** the current image's magnification does not change

### Requirement: Dragging the canvas moves the image within it

While the current image is drawn larger than the pane it is in, dragging inside that pane SHALL move
the image under the pointer, and the image SHALL follow the pointer for as long as the drag lasts.

The photograph SHALL NOT be draggable away from the pane: no edge of it SHALL come inside the pane
while the image is larger than the pane, so there is never empty space beside a photograph that has
more of itself to show.

While the image is no larger than the pane there SHALL be nothing to move, and it SHALL stay centred
however it is dragged.

#### Scenario: Panning a magnified image

- **WHEN** the current image is magnified and the user drags inside the pane
- **THEN** a different part of the photograph is shown, following the drag

#### Scenario: Dragging to the edge

- **WHEN** the user drags a magnified image past its own edge
- **THEN** the image stops with its edge at the edge of the pane

#### Scenario: Dragging a fitted image

- **WHEN** the current image is at the size that fits and the user drags inside the pane
- **THEN** the image stays whole and centred

### Requirement: The two panes are looked at together

While the canvas draws two panes - side by side, or superimposed and split - both SHALL show the same
part of the photograph at the same magnification, and magnifying or moving either of them SHALL move
both.

Two panes showing different parts of a photograph are not a comparison, which is the whole of what
the second pane is for.

#### Scenario: Magnifying one pane

- **WHEN** the side-by-side comparison is chosen and the user magnifies one of the panes
- **THEN** both panes are magnified by the same amount

#### Scenario: Panning one pane

- **WHEN** the side-by-side comparison is chosen and the user drags one of the panes
- **THEN** both panes show the same part of the photograph

#### Scenario: Changing the comparison

- **WHEN** the user magnifies the current image and then chooses a different comparison
- **THEN** the image is drawn at the same magnification in the comparison chosen

### Requirement: The split comparison's divider is moved by the user

The divider of the split comparison SHALL start at the middle of the canvas and SHALL be draggable
across it, revealing more of the enhanced image as it moves one way and more of the original as it
moves the other.

Moving the divider SHALL NOT move either photograph: it changes how much of the enhanced image is
revealed over the original and nothing else, so that a feature the user is comparing stays at the
same point on the screen as the divider passes over it.

Where the divider sits SHALL NOT be a property of any photograph. Making another image current while
the split comparison is showing SHALL leave the divider where the user put it.

#### Scenario: Dragging the divider

- **WHEN** the split comparison is chosen and the user drags the divider
- **THEN** more of one image and less of the other is revealed, and neither photograph moves

#### Scenario: Switching images under the divider

- **WHEN** the user moves the divider and then makes another image the current one
- **THEN** the divider is still where the user put it

### Requirement: The sidebar's miniature reports which part of the image the canvas is showing

While an image is open, the sidebar's miniature SHALL draw an outline over itself around the portion
of the photograph the canvas is currently showing, positioned and sized against the miniature the way
that portion is positioned and sized within the whole photograph.

The outline SHALL follow every change to what the canvas is showing - magnifying, moving, changing
the comparison, resizing the window, and making another image current.

While the current image is drawn whole, the outline SHALL surround the whole miniature, which is the
true answer rather than a special case: all of the photograph is being shown.

#### Scenario: A magnified image

- **WHEN** the current image is magnified so that part of it is outside the pane
- **THEN** the sidebar's miniature draws an outline around the part that is inside the pane

#### Scenario: Moving the image

- **WHEN** the user pans the current image
- **THEN** the outline on the miniature moves to the part now being shown

#### Scenario: A fitted image

- **WHEN** the current image is at the size that fits
- **THEN** the outline surrounds the whole miniature

#### Scenario: Another image

- **WHEN** the user makes another image the current one
- **THEN** the miniature and its outline both report that image

### Requirement: The canvas changes photograph in one step

When the user makes another image the current one, the canvas SHALL go on drawing the photograph it
is drawing until the new one can be painted, and SHALL then show the new one whole - its shape, its
size and its pixels together.

No photograph SHALL be drawn to another photograph's shape. The canvas works out for itself what size
to draw an image at, from what the application recorded about that file; a canvas that took the new
image's measurements while still holding the old image's pixels would stretch the one into the other
for as long as the decode took, which reads as the photograph distorting rather than as it changing.

Only the canvas SHALL wait. The rest of the window - the file's name, the miniature, the mark against
the chosen thumbnail - SHALL report the new image at once, because those are the acknowledgement that
the user's choice landed.

An image whose pixels cannot be served SHALL NOT be waited for, and SHALL become current immediately.

#### Scenario: Choosing another image

- **WHEN** the user makes another open image the current one
- **THEN** the canvas keeps drawing the previous photograph until the new one can be painted
- **AND** then draws the new one at its own shape and size

#### Scenario: The rest of the window

- **WHEN** the user makes another open image the current one
- **THEN** its name, the sidebar's miniature and the mark on the chosen thumbnail report it at once,
  without waiting for the canvas

#### Scenario: An image that cannot be drawn

- **WHEN** the user makes an image whose pixels cannot be served the current one
- **THEN** the canvas stops drawing the previous photograph at once rather than waiting for pixels
  that will never arrive

#### Scenario: Choosing again while the canvas is waiting

- **WHEN** the user chooses one image and then another before the first can be painted
- **THEN** the canvas draws the second, and never the first
