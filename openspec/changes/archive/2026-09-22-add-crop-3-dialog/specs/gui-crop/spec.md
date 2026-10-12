## Purpose

The Crop/Rotate surface: how a user frames one open photograph — mirroring it, turning it and choosing the
rectangle to keep — and what applying or abandoning that framing leaves behind. It is the one place in the
application a crop can be set, and everything else that draws or enhances a framed photograph reads what it
records.

## ADDED Requirements

### Requirement: The user frames the current image on a surface of its own

The application SHALL offer a Crop/Rotate surface for the current image, opened from the window and drawn
over it, large enough to work in: the photograph and the framing controls on one side, the framing's
settings on the other.

It SHALL be available only while an image is open, since there is otherwise nothing to frame.

While it is open it SHALL be the only thing the user can act on, so a framing is chosen against a photograph
that is not being changed underneath it.

It SHALL draw the dotted surface behind the photograph, whatever surface the preview canvas is set to
draw. This is a work surface rather than a place to look at a photograph: what the user is judging is
where an edge falls against the background behind it, and the animated alternative the canvas offers
competes with the rectangle being placed. The dotted surface is the same one the canvas draws when it
is chosen there - one definition, drawn unconditionally here.

#### Scenario: Opening the surface

- **WHEN** the user operates the control that opens Crop/Rotate while an image is open
- **THEN** the Crop/Rotate surface is shown over the window, with the current image on it

#### Scenario: The background behind the photograph

- **WHEN** the user opens Crop/Rotate
- **THEN** the surface draws the dotted background behind the photograph

#### Scenario: The background for a user whose canvas is not dotted

- **WHEN** the user has chosen the animated background for the preview canvas and opens Crop/Rotate
- **THEN** the surface still draws the dotted background behind the photograph, and the preview canvas
  keeps the background they chose

#### Scenario: The rest of the window while it is open

- **WHEN** the Crop/Rotate surface is open
- **THEN** the controls behind it cannot be operated until it is dismissed

### Requirement: The surface frames the whole photograph, not the framing already in force

The photograph the surface shows SHALL be the whole of the current image, unframed, whatever framing is
already recorded for it. A framing is a choice about the whole photograph and SHALL stay editable and
reversible; a surface that showed the already-framed picture could only ever cut further into it.

Where a framing is already recorded, the surface SHALL open with that framing in force — the same mirroring,
the same turn and the same rectangle — so that opening it again shows what was chosen rather than starting
over.

#### Scenario: An image with no framing

- **WHEN** the user opens Crop/Rotate for an image that carries no framing
- **THEN** the whole photograph is shown with the rectangle covering all of it, no turn and no mirroring

#### Scenario: An image that is already framed

- **WHEN** the user opens Crop/Rotate for an image that carries a framing
- **THEN** the whole photograph is shown, with that framing's mirroring, turn and rectangle in force

#### Scenario: Widening a framing that was already cut down

- **WHEN** the user opens Crop/Rotate for an image framed to a small rectangle and drags that rectangle
  outwards
- **THEN** the rectangle can be enlarged to the whole photograph again

### Requirement: The rectangle to keep is dragged on the photograph

The user SHALL be able to move the rectangle and resize it from its edges and corners, with a handle drawn
at each edge and each corner.

What is kept SHALL be distinguishable from what is not. The rectangle SHALL carry a visible outline and a
thirds grid across it, and that SHALL be the whole of how the distinction is drawn: the area outside the
rectangle SHALL NOT be dimmed. This follows the reference implementation. The widget draws a surround as a
shadow spreading from the rectangle to the edge of whatever contains it, so a visible surround covers the
whole working area rather than the photograph - and since the rectangle covers the whole photograph when the
surface opens, the only thing it would ever dim is the surface around it.

The rectangle SHALL never extend beyond the photograph, and SHALL never be reduced below a small minimum on
either edge, so that a framing always names some pixels.

#### Scenario: Dragging the rectangle's corner

- **WHEN** the user drags a corner of the rectangle
- **THEN** the rectangle is resized, and its outline and thirds grid follow it

#### Scenario: The photograph fills the space it is given

- **WHEN** the user opens Crop/Rotate
- **THEN** the photograph is drawn as large as the area reserved for it allows, and every handle of the
  rectangle is drawn whole rather than cut off at the photograph's edge

#### Scenario: Dragging past the photograph's edge

- **WHEN** the user drags the rectangle past the edge of the photograph
- **THEN** the rectangle stops at the edge

### Requirement: The photograph can be turned and mirrored

The surface SHALL offer a fine rotation from -90 to +90 degrees, marked every 15 degrees, a quarter-turn
action, a horizontal mirror and a vertical mirror. A positive fine rotation SHALL turn the photograph
clockwise, which is the direction the framing that is recorded means by a positive angle.

As the photograph is turned, the space it is framed in SHALL grow to hold its turned corners, so that the
rectangle can be placed anywhere the turned photograph now reaches.

The quarter-turn action SHALL turn the photograph a further quarter clockwise and SHALL return the fine
rotation to zero, because a user squaring a leaning photograph up is not asking to keep the lean.

Mirroring SHALL be its own state rather than a rotation, and applying the same mirror twice SHALL return the
photograph to how it was.

#### Scenario: A positive fine rotation

- **WHEN** the user moves the fine rotation to +12 degrees
- **THEN** the photograph leans 12 degrees clockwise and the space it is framed in grows to hold its corners

#### Scenario: A quarter turn after a fine rotation

- **WHEN** the user sets a fine rotation of 7 degrees and then operates the quarter-turn action
- **THEN** the photograph stands a quarter turn clockwise from where it began and the fine rotation reads
  zero

#### Scenario: Mirroring twice

- **WHEN** the user operates the horizontal mirror twice
- **THEN** the photograph is drawn as it was before either

### Requirement: An aspect ratio can be locked, and every ratio has its inverse

The surface SHALL offer a grid of aspect ratios — a free ratio, a square, and the landscape and portrait
forms of 5:4, 4:3, 3:2 and 16:9 — and SHALL show which one is in force. Each SHALL be drawn as a rectangle
in its own proportions, so the grid says what each ratio is without being read.

Choosing a ratio SHALL reshape the rectangle to the largest rectangle of that ratio that fits the
photograph, centred on the rectangle the user had. It SHALL NOT be fitted inside the current rectangle:
switching between ratios would then only ever shrink the selection, and no sequence of choices could grow it
back. **This diverges from the reference implementation**, which fits the new ratio inside the existing box.

While a ratio is in force, every way the rectangle can change — dragging, resizing, typing a dimension —
SHALL keep that ratio.

Choosing the free ratio SHALL release the lock and leave the rectangle where it is.

#### Scenario: Choosing a ratio

- **WHEN** the user chooses 16:9
- **THEN** the rectangle becomes the largest 16:9 rectangle that fits the photograph, centred where the
  rectangle was, and 16:9 is shown as the one in force

#### Scenario: Switching between two ratios

- **WHEN** the user chooses 16:9, then 3:2, then 16:9 again
- **THEN** the rectangle is the same size as it was the first time 16:9 was chosen

#### Scenario: Resizing under a locked ratio

- **WHEN** the user drags a corner of the rectangle while 4:3 is in force
- **THEN** the rectangle keeps its 4:3 proportions

### Requirement: The rectangle's dimensions can be read and typed, in the photograph's own pixels

The surface SHALL show the rectangle's width and height and SHALL let the user type either, and both SHALL
be expressed in the pixels of the photograph itself rather than of whatever copy of it the surface happens
to be drawing. A user typing 2560 SHALL get 2560 pixels of the photograph.

The displayed dimensions SHALL follow the rectangle as it is dragged or reshaped.

A typed dimension SHALL be brought within what is possible — no larger than the photograph, no smaller than
the minimum — rather than refused, and where a ratio is in force the other dimension SHALL follow from it.

#### Scenario: Reading the rectangle

- **WHEN** the user drags the rectangle's edge
- **THEN** the displayed width and height change to the rectangle's own, in the photograph's pixels

#### Scenario: Typing a width under a locked ratio

- **WHEN** the user types a width while 16:9 is in force
- **THEN** the rectangle takes that width and the height that 16:9 gives it

#### Scenario: Typing a width larger than the photograph

- **WHEN** the user types a width larger than the photograph's own
- **THEN** the rectangle is no wider than the photograph

### Requirement: The two dimensions can be swapped, and a locked ratio swaps with them

The surface SHALL offer a control that exchanges the rectangle's width and height.

Where a ratio is in force, swapping SHALL select that ratio's inverse and keep the lock — 16:9 becomes 9:16
and stays locked. **This diverges from the reference implementation**, which transposes the rectangle and
drops the lock to free, losing the constraint the user chose. Every ratio the grid offers has its inverse in
the grid, which is what makes this expressible.

#### Scenario: Swapping under a locked ratio

- **WHEN** the user operates the swap control while 16:9 is in force
- **THEN** the rectangle is transposed, 9:16 is shown as the ratio in force, and the lock is still on

#### Scenario: Swapping with no ratio locked

- **WHEN** the user operates the swap control while the free ratio is in force
- **THEN** the rectangle's width and height are exchanged and the ratio stays free

### Requirement: The photograph can be magnified within the surface

The user SHALL be able to magnify the photograph within the surface with the mouse wheel or a trackpad
pinch, so that a rectangle's edge can be placed precisely on a large photograph.

Magnifying SHALL change only what the surface draws. It SHALL NOT change the rectangle, the turn, the
mirroring or the dimensions reported, and it SHALL NOT be recorded in the framing.

Every pixel of the photograph the surface draws SHALL be reachable by the rectangle, at any
magnification. The surface draws the photograph in a slightly smaller area than it reserves, so that
the handles sitting on the rectangle's edge are drawn whole rather than sliced off at the
photograph's own edge; that margin is for the handles, and SHALL NOT come to hold photograph. A
strip the user can see but cannot frame reads as a rectangle that refuses to be dragged any further.

The surface SHALL say that this is possible, since nothing else on it indicates it.

#### Scenario: Magnifying the photograph

- **WHEN** the user turns the mouse wheel over the photograph
- **THEN** the photograph is drawn larger and the reported width and height are unchanged

#### Scenario: Framing a magnified photograph out to its edge

- **WHEN** the user magnifies the photograph until it fills the area reserved for it, and drags the
  rectangle's edge outwards as far as it will go
- **THEN** the rectangle reaches the edge of the photograph as drawn, with no strip of photograph
  left outside what the rectangle can cover

### Requirement: Reset returns every control to where it started

The surface SHALL offer a reset that returns the fine rotation to zero, undoes any quarter turns and
mirroring, releases any locked ratio, and returns the rectangle to the whole photograph.

Reset SHALL change nothing outside the surface: it returns the controls to their defaults, not the image to
its recorded framing, and a user who resets and then dismisses SHALL find the recorded framing untouched.

#### Scenario: Resetting after framing

- **WHEN** the user turns, mirrors, locks a ratio and drags the rectangle in, then operates reset
- **THEN** the whole photograph is shown unturned and unmirrored, the free ratio is in force and the
  rectangle covers all of it

#### Scenario: Resetting and then dismissing

- **WHEN** the user opens Crop/Rotate for a framed image, operates reset and then dismisses the surface
- **THEN** the image is still framed as it was

### Requirement: Applying records the framing, and an unchanged framing records none

Applying SHALL record the framing shown — the mirroring, the turn and the rectangle — against the current
image and dismiss the surface. Everything that draws or enhances that image SHALL then use it.

Where what is applied frames the whole photograph unturned and unmirrored, the application SHALL record no
framing at all rather than one that frames everything. Opening the surface, looking and applying is how a
user backs out of framing, and a framing that changes nothing would otherwise describe the same pixels by a
different name and cost the work of producing them again.

#### Scenario: Applying a framing

- **WHEN** the user drags the rectangle in and applies
- **THEN** the surface is dismissed and the canvas, the sidebar's miniature and the navbar's dimensions all
  show the framed photograph

#### Scenario: Applying a framing that changes nothing

- **WHEN** the user opens Crop/Rotate, changes nothing and applies
- **THEN** the image carries no framing and nothing about how it is drawn changes

#### Scenario: Applying a framing that undoes an earlier one

- **WHEN** the user opens Crop/Rotate for a framed image, resets and applies
- **THEN** the image carries no framing and is drawn whole again

### Requirement: Dismissing the surface changes nothing

Cancelling, the close control and the keyboard's dismissal SHALL each leave the surface with the image's
recorded framing exactly as it was, whatever was done on the surface before.

A click outside the surface SHALL NOT dismiss it, for the reason the settings dialog refuses the same: a
stray click is not an answer to a framing the user has been working on.

#### Scenario: Cancelling after framing

- **WHEN** the user turns and reframes the photograph and then cancels
- **THEN** the image is drawn exactly as it was before the surface was opened

#### Scenario: Dismissing from the keyboard

- **WHEN** the user dismisses the surface from the keyboard
- **THEN** nothing about the image's framing changes

#### Scenario: Clicking outside the surface

- **WHEN** the user clicks the area outside the surface
- **THEN** the surface stays open
