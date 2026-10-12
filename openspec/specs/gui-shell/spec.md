# gui-shell Specification

## Purpose

The frame the application window always shows, whatever is loaded into it: the navbar across the
top, the preview canvas, the image drawer folded against its bottom edge, and the sidebar down the
right. This capability covers how those regions are arranged, how the window is titled and dragged on
each platform, and which of the frame's controls are live when no image is open.

## Requirements

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

### Requirement: The navbar identifies the application and its version

The navbar SHALL show the application's name and the version the core library reports, the same value
the terminal front end prints. The version SHALL be the running build's, read at run time rather than
written into the interface.

#### Scenario: Name and version are shown

- **WHEN** the application window is shown
- **THEN** the navbar carries the application name and the running build's version

### Requirement: The navbar offers an update when a newer release is published

Once per launch, the application SHALL compare the running build's version - the one the navbar shows - with the
latest release published on the project's GitHub repository, `vegidio/open-photo-ai`. Drafts and pre-releases SHALL
NOT count as published releases.

When the latest release is newer, the navbar SHALL show an "Update Available" control after the version, drawing
attention to itself with a repeating pulse. When it is not newer - the same version, or an older one - the control
SHALL NOT be shown.

A check that cannot answer SHALL be treated as "no newer release known": when the release cannot be fetched (no
network, a rate limit, no published release) or either version cannot be read as a version, the control SHALL NOT be
shown, nothing SHALL be drawn in its place, and the failure SHALL be logged. The check SHALL NOT delay or block
anything else the window shows, and SHALL NOT be retried during the same launch.

The check SHALL run in every build, including development builds.

Pressing the control SHALL open the project's releases page in the system's default browser, never in the application
window. If the page cannot be opened, the window SHALL stay as it was, the control SHALL stay shown, and the failure
SHALL be logged.

#### Scenario: A newer release is published

- **WHEN** the running build's version is older than the latest published release
- **THEN** the navbar shows "Update Available" after the version, pulsing

#### Scenario: The running build is current

- **WHEN** the running build's version equals the latest published release's, or is newer
- **THEN** the navbar shows no "Update Available" control

#### Scenario: The check fails

- **WHEN** the latest release cannot be fetched, or its version cannot be read
- **THEN** the navbar shows no "Update Available" control and the failure is logged

#### Scenario: The check runs once

- **WHEN** the window has been open for any length of time
- **THEN** the release has been asked for at most once since launch

#### Scenario: Opening the releases page

- **WHEN** the user presses "Update Available"
- **THEN** the project's releases page opens in the system's default browser
- **AND** the application window is unchanged

### Requirement: The navbar's About control opens a dialog identifying the application

The navbar's About control SHALL open the About dialog, whether or not an image is open. The dialog SHALL show, in
order: the application icon, the application name, the running build's version (the same value the navbar shows), the
copyright line, and two links - the project's repository and the author's website.

The version and the copyright line SHALL be selectable, so they can be copied into a bug report. The rest of the
dialog SHALL behave as the rest of the window does and not be selectable.

The dialog SHALL be dismissable by its close control, by pressing Escape, and by clicking outside it. Dismissing it
SHALL change nothing else.

#### Scenario: Opening About

- **WHEN** the user presses About in the navbar
- **THEN** the About dialog opens, showing the application icon, the application name, the running build's version,
  the copyright line and the two links

#### Scenario: About is available with nothing open

- **WHEN** no image is open and the user presses About
- **THEN** the About dialog opens

#### Scenario: Copying the version

- **WHEN** the user drags across the version line, or the copyright line, and copies
- **THEN** the copied text is that line

#### Scenario: Dismissing the dialog

- **WHEN** the About dialog is open and the user presses its close control, presses Escape, or clicks outside it
- **THEN** the dialog closes and the window is as it was before About was pressed

### Requirement: The About dialog's links open in the system browser

Each link in the About dialog SHALL open its page in the system's default browser. Neither SHALL navigate the
application window, however it is clicked.

The window SHALL be able to ask for only the pages the application links to - the repository, the author's website and
the releases page - to be opened: which address a link opens SHALL be decided outside the window, not by anything the
window sends.

If the page cannot be opened, the dialog SHALL stay open and unchanged, and the failure SHALL be logged.

#### Scenario: Opening the repository

- **WHEN** the user clicks the repository link
- **THEN** the project's repository opens in the system's default browser
- **AND** the application window still shows the About dialog

#### Scenario: Opening the website

- **WHEN** the user clicks the website link
- **THEN** the author's website opens in the system's default browser
- **AND** the application window still shows the About dialog

#### Scenario: The window cannot open an arbitrary address

- **WHEN** the window asks for a page to be opened that is not one of the pages the application links to
- **THEN** nothing is opened

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

The one exception is the control that opens the Crop/Rotate surface, which is drawn over the sidebar's
miniature and is absent along with it. The rule exists to keep the frame's shape, and that area is not part
of the frame's shape: with no image open it is not a control strip with an empty box in it but a sentence
reporting that there is no preview.

#### Scenario: No image open

- **WHEN** the application window is shown with no image open
- **THEN** the drawer's fold control, the select-all control, the preview-comparison control, the
  zoom control, the add-enhancement control and the export control are all present and all disabled

#### Scenario: The Crop/Rotate control with no image open

- **WHEN** the application window is shown with no image open
- **THEN** no control for opening Crop/Rotate is drawn, and the sidebar's preview area reports that there is
  no preview

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

### Requirement: With an image open the canvas draws it

While an image is open, the preview canvas SHALL draw the current image in place of the invitation to
supply one, and SHALL draw it whole and in proportion within the space it has - never cropped and
never stretched.

Where the current image carries a crop, the photograph the canvas draws SHALL be the framed one: the
canvas draws that framing whole and in proportion, at the framing's own shape rather than the file's.
"Never cropped" is a statement about the canvas, which shows the whole of whatever photograph it is
given; it is not a statement about the user's own framing.

The canvas SHALL draw the photograph the application decodes from the file rather than the file's own
bytes, so that a camera RAW, a TIFF and a HEIF are drawn like any other photograph.

#### Scenario: An image is opened

- **WHEN** the user opens an image
- **THEN** the canvas draws it in place of the drag-and-drop invitation

#### Scenario: A format the interface cannot decode on its own

- **WHEN** the current image is a camera RAW, a TIFF or a HEIF
- **THEN** the canvas draws it

#### Scenario: An image whose shape differs from the canvas's

- **WHEN** the current image is drawn on a canvas of a different aspect ratio
- **THEN** the whole photograph is visible and its proportions are unchanged

#### Scenario: An image carrying a crop

- **WHEN** the current image carries a crop
- **THEN** the canvas draws the framed photograph whole and in proportion, at the framing's shape and
  not the file's

### Requirement: The preview canvas draws the chosen background behind everything else

The preview canvas SHALL draw the background the user has chosen, and SHALL draw it behind
everything else the canvas shows. The invitation to supply an image and the photograph itself are
both drawn over it, whichever surface is in effect.

The background SHALL be drawn in both of the canvas's states - while no image is open and while one
is. It is the surface the canvas is made of rather than something the empty canvas carries, so
loading a photograph SHALL NOT change it.

A saved choice SHALL take effect without restarting the application.

#### Scenario: The canvas with no image open

- **WHEN** the application window is shown with no image open
- **THEN** the canvas draws the chosen background
- **AND** the invitation to drag and drop images is legible over it

#### Scenario: The canvas with an image open

- **WHEN** an image is opened
- **THEN** the canvas still draws the chosen background
- **AND** the photograph is drawn over it

#### Scenario: Changing the background while the application is running

- **WHEN** the user chooses the other background and saves
- **THEN** the preview canvas draws the newly chosen surface without the application being restarted

### Requirement: The particle background is decorative and costs nothing while unwatched

While the particle field is the chosen background, it SHALL be decorative only. It SHALL NOT respond
to the pointer, SHALL NOT accept any interaction, and SHALL NOT be announced to assistive technology
or reachable by keyboard - it carries no information, and everything the canvas has to say is said
by what is drawn over it.

The field SHALL stop animating while the application window does not have focus, and SHALL resume
when it regains focus. An unfocused window is the normal state of a desktop application that is not
being used, and an animation nobody is looking at is cost with nothing bought by it.

Stopping and resuming SHALL be invisible in effect: the field SHALL still be drawn while the window
is unfocused, and regaining focus SHALL NOT restart it from an empty canvas.

#### Scenario: The pointer moves across the particle field

- **WHEN** the user moves the pointer across the canvas while the particle field is drawn
- **THEN** the particles are unaffected by where the pointer is
- **AND** nothing under the pointer becomes interactive

#### Scenario: The window loses and regains focus

- **WHEN** the application window loses focus while the particle field is drawn
- **THEN** the field stops animating but is still drawn
- **AND** it resumes animating from where it stopped when the window regains focus

#### Scenario: The particle field and assistive technology

- **WHEN** the canvas is examined by assistive technology while the particle field is drawn
- **THEN** the field is not announced, and the invitation or the photograph drawn over it is

### Requirement: The canvas compares the image in one of three ways

The frame SHALL carry a control offering three ways of drawing the current image, and the canvas SHALL
draw it the chosen way:

- the enhanced image alone, filling the canvas;
- the original and the enhanced image side by side, each in half the canvas;
- the two superimposed, split by a divider with the original on one side of it and the enhanced on the
  other.

Each side SHALL be labelled with which of the two it is, so that two panes showing the same photograph
are not two photographs the user has to tell apart by eye. While nothing has enhanced the current
image, the two sides SHALL show the same photograph, which is what the application has for both.

The control SHALL become available once an image is open, and the chosen way SHALL survive a restart -
it is a standing preference about how the user reads a comparison, not a property of one photograph.

#### Scenario: The comparison control with an image open

- **WHEN** an image is open
- **THEN** the comparison control can be used and reports which of the three is chosen

#### Scenario: Side by side

- **WHEN** the side-by-side comparison is chosen
- **THEN** the canvas draws two panes, one labelled as the original and one as the enhanced image

#### Scenario: The whole canvas

- **WHEN** the full comparison is chosen
- **THEN** the canvas draws the enhanced image alone, across the whole canvas

#### Scenario: Split

- **WHEN** the split comparison is chosen
- **THEN** the canvas draws the two superimposed, divided so that the original is shown on one side of
  the divider and the enhanced image on the other

#### Scenario: The choice survives a restart

- **WHEN** the user chooses a comparison and the application is restarted
- **THEN** the same comparison is chosen

### Requirement: The navbar reports the current image

While an image is open, the navbar SHALL show the current image's file name and its pixel dimensions,
labelled as such, and SHALL offer a control beside the name that opens the file options menu for that
image. While no image is open it SHALL show none of the three.

The dimensions shown SHALL describe what the user is working on rather than the file alone: where the
image carries a crop, the framing's dimensions; where the image's enhancements enlarge it, the
dimensions the enhanced result will have.

Resting on the dimensions SHALL show the three sizes side by side — the file's own, the framing's where
one is set, and the enhanced result's — so a user can see what a crop and an enlargement each cost
without applying either. The comparison SHALL be absent while no image is open.

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

#### Scenario: An image carrying a crop

- **WHEN** the current image carries a crop and its enhancements do not enlarge it
- **THEN** the navbar shows the framing's dimensions rather than the file's

#### Scenario: An image whose enhancements enlarge it

- **WHEN** the current image carries an enhancement that enlarges it
- **THEN** the navbar shows the dimensions the enhanced result will have

#### Scenario: Resting on the dimensions

- **WHEN** the user rests the pointer on the dimensions
- **THEN** the file's own dimensions and the enhanced result's are shown together, with the framing's
  between them where the image carries a crop and absent where it does not

### Requirement: The sidebar shows a miniature of the current image

While an image is open, the sidebar's preview area SHALL draw the current image, whole and in
proportion, in place of the words that report there is no preview.

Where the current image carries a crop, the miniature SHALL draw the framed photograph, so the
miniature and the canvas show the same picture at two sizes.

The miniature SHALL carry the control that opens the Crop/Rotate surface, drawn over its lower trailing
corner. That control SHALL be drawn only while the miniature is, since it belongs to the picture rather than
to the panel around it.

#### Scenario: An image is open

- **WHEN** an image is open
- **THEN** the sidebar's preview area draws it and no longer says there is no preview

#### Scenario: The image the sidebar draws

- **WHEN** the current image changes
- **THEN** the sidebar draws the image the canvas is drawing

#### Scenario: An image carrying a crop

- **WHEN** the current image carries a crop
- **THEN** the sidebar's miniature draws the framed photograph

#### Scenario: Opening Crop/Rotate from the miniature

- **WHEN** the user operates the control drawn over the miniature
- **THEN** the Crop/Rotate surface is opened for the current image

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

### Requirement: The strip of open images is drawn uncropped

The drawer's strip SHALL draw each open image as its file is, whatever crop that image carries.

The strip answers "which photographs are open", and a thumbnail that changes shape as a crop is dragged
answers it worse. The reference implementation behaves the same way, which is worth stating because the
difference between a cropped miniature and an uncropped strip beside it reads as an oversight.

#### Scenario: A cropped image in the strip

- **WHEN** an open image carries a crop
- **THEN** its thumbnail in the strip draws the whole photograph, while the sidebar's miniature draws
  the framing

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
