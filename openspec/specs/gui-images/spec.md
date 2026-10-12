# gui-images Specification

## Purpose

How the application takes image files from the user, what it records about each one without decoding
it, and how an opened image's pixels reach the interface at whatever size the interface is drawing
them - the preview canvas, a drawer thumbnail and the sidebar miniature being the same request at
three bounds.

## Requirements

### Requirement: Asking the user for image files

The application SHALL be able to open the platform's own file picker for images, allowing more than
one file to be chosen at a time, and SHALL offer it filtered to the formats the application can
decode.

Dismissing the picker SHALL yield no files and SHALL NOT be reported as a failure: choosing nothing
is a choice.

A picker that cannot be opened SHALL also yield no files, and the application SHALL NOT claim to tell
that apart from a dismissal. The platform's picker reports both as the same answer - no selection -
so a distinction drawn here would be invented rather than observed. Nothing the interface does
differs between the two in any case: the user has added no images. The ambiguity SHALL be recorded in
the log, where it is a diagnostic for whoever is debugging rather than something the interface acts
on.

The picker's wording SHALL be supplied by the interface, which owns the application's translations,
rather than written into the layer that opens it - so the picker speaks the language the rest of the
window is speaking.

#### Scenario: Files are chosen

- **WHEN** the user selects one or more image files in the picker
- **THEN** a record is reported for each chosen file

#### Scenario: The picker is dismissed

- **WHEN** the user closes the picker without choosing anything
- **THEN** no files are reported and no failure is raised

#### Scenario: The picker cannot be opened

- **WHEN** the platform's picker fails to open at all
- **THEN** no files are reported, indistinguishably from a dismissal, and the reason such a selection
  may be empty is recorded in the log

#### Scenario: Only decodable formats are offered

- **WHEN** the picker is opened
- **THEN** its filter names every format the application can decode, camera RAW included, and no
  format it cannot

### Requirement: What the application records about a file

For each file admitted, the application SHALL report its path, an identity derived from its bytes,
its pixel dimensions, its extension in lower case, and its size on disk.

The identity SHALL be the XXH3-64 of the whole of the file's bytes as lowercase hexadecimal - the
same value a loaded image carries - so that a file described here and the same file loaded later are
recognised as one photograph.

The dimensions SHALL be read from the file's header rather than by decoding its pixels, so that
describing a large selection costs a header per file.

#### Scenario: A file is described

- **WHEN** a readable image file is admitted
- **THEN** its path, identity, width, height, lowercase extension and byte size are reported

#### Scenario: The identity matches the loaded image

- **WHEN** a file is described and the same file is then loaded
- **THEN** the identity reported by each is the same value

### Requirement: One unreadable file does not lose the selection

A file whose header cannot be read, or whose size cannot be determined, SHALL still be admitted with
that fact reported as unknown rather than as a value, and the remaining files in the selection SHALL
be described normally.

A selection is a batch the user assembled; discarding all of it because one file has gone missing or
carries a truncated header costs them the other files for no gain, and the file that failed is still
one they named.

#### Scenario: A file with an unreadable header

- **WHEN** a selection contains one file whose dimensions cannot be read and several that are sound
- **THEN** every file in the selection is reported, and the one that failed reports its dimensions as
  unknown

### Requirement: Files supplied without the picker are described identically

The application SHALL be able to describe image files from paths supplied by any other route, and
those records SHALL be indistinguishable from the picker's for the same files.

The picker is one way a file arrives and not the definition of one. A file dragged onto the window
is the same photograph as a file chosen from a dialog, and an interface that had to tell them apart
would be carrying a difference that does not exist.

#### Scenario: The same file by two routes

- **WHEN** one file is described from a supplied path and the same file is chosen through the picker
- **THEN** the two records are equal

### Requirement: A described selection has a stable order

The records for a selection SHALL be reported ordered by path, so that describing the same set of
files twice reports them in the same order.

#### Scenario: The same selection twice

- **WHEN** the same set of files is described on two occasions
- **THEN** the records are reported in the same order both times

### Requirement: An opened image's pixels are served to the interface at a requested size

The application SHALL serve the pixels of an opened image to the interface, addressed by that
image's identity together with a bound on the image's longest edge, and optionally a crop to apply
before the bound.

A bound SHALL cap the longest edge while preserving the image's aspect ratio. A request carrying no
bound SHALL serve the image at its own dimensions. A bound at or above the image's longest edge
SHALL serve the image at its own dimensions rather than enlarging it.

A request carrying a crop SHALL serve the framed pixels rather than the whole photograph, and the
bound SHALL cap the longest edge of what is served — the framing, not the photograph it was taken
from. A request carrying no crop SHALL be served exactly as it is today: the crop is an addition to
what may be asked for, not a change to what an existing request means.

A bounded request carrying a crop SHALL show the same framing as the unbounded request carrying that
crop, at the smaller size. What is served SHALL be the framing the crop describes, whether or not the
application reduced the photograph first to avoid doing full-resolution work for a thumbnail.

A request whose crop cannot be read SHALL be refused rather than served without it, for the reason a
malformed bound is refused: serving the whole photograph where a framing was asked for is a wrong
answer that looks like a right one.

The pixels served SHALL be those the application decodes from the file, not the file's own bytes -
so what is drawn is what the enhancement pipeline will later operate on, including a camera RAW's
development and the narrowing of a deeper image to what the transport can carry.

#### Scenario: A bounded rendition

- **WHEN** the pixels of a 3000 x 2000 opened image are requested with a bound of 100
- **THEN** an image 100 pixels on its longest edge is served, with the aspect ratio of the original

#### Scenario: No bound

- **WHEN** the pixels of an opened image are requested with no bound
- **THEN** the image is served at its own dimensions

#### Scenario: A bound larger than the image

- **WHEN** the pixels of a 640 x 480 opened image are requested with a bound of 2000
- **THEN** the image is served at 640 x 480 and is not enlarged

#### Scenario: A crop with no bound

- **WHEN** the pixels of an opened image are requested with a crop and no bound
- **THEN** the framed pixels are served, at the dimensions the crop's rectangle describes

#### Scenario: A crop with a bound

- **WHEN** the pixels of an opened image are requested with a crop and a bound smaller than the
  framing
- **THEN** the framing is served with its longest edge at the bound, showing what the unbounded
  request with the same crop shows

#### Scenario: A crop that cannot be read

- **WHEN** a request carries a crop the application cannot parse
- **THEN** it is refused, rather than the whole photograph being served

#### Scenario: A format the interface cannot decode

- **WHEN** the pixels of an opened camera RAW, TIFF or HEIF image are requested
- **THEN** they are served, in a form the interface can draw

#### Scenario: An image with transparency

- **WHEN** the pixels of an opened image carrying an alpha channel are requested
- **THEN** they are served in a form that keeps the transparency, rather than composited away

### Requirement: Only files the user has opened can be served

The application SHALL serve pixels only for files it has admitted through one of the two routes
above, or for an enhanced result it has itself produced from such a file, and SHALL address both by
identity rather than by location.

A request naming an identity the application has not admitted SHALL be refused, and refused
distinguishably from a file that was admitted and has since become unreadable.

No file on the machine outside that set SHALL be reachable through this route, whatever is named in
the request. The set of images the user has opened, together with what this application has produced
from them, is the whole of what the interface is entitled to read.

An enhanced result is reachable only while the application still holds it. It is produced from
admitted files and never read from disk, so it widens what the interface may draw without widening
what the interface may reach on the machine.

#### Scenario: An unknown identity

- **WHEN** pixels are requested for an identity no admitted file carries
- **THEN** the request is refused, and reported as unknown rather than as unreadable

#### Scenario: An admitted file that has gone away

- **WHEN** pixels are requested for an admitted file that has since been deleted or become unreadable
- **THEN** the request is refused, and reported as unreadable rather than as unknown

#### Scenario: A location outside the opened set

- **WHEN** a request names a location on the machine rather than an admitted identity
- **THEN** nothing is served

#### Scenario: An enhanced result the application holds

- **WHEN** pixels are requested for the identity of an enhanced result the application still holds
- **THEN** they are served, by the same route and in the same terms as an admitted file's

#### Scenario: An enhanced result the application no longer holds

- **WHEN** pixels are requested for the identity of an enhanced result a later run has displaced
- **THEN** the request is refused rather than the file behind it being read

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

### Requirement: An identity and a bound always denote the same pixels

Pixels served for a given identity, bound and crop SHALL be the same on every request, and the
interface SHALL be told it may retain them.

An identity describes bytes, so what it addresses cannot change without the identity changing, and a
bound and a crop are carried in the request rather than held by the application — so what a request
denotes cannot go stale while the request stays the same. That is what lets the interface draw the
same thumbnail repeatedly, and switch back to an image it has already shown, without the application
decoding the file again.

Two requests differing only in their crop SHALL be treated as two different requests, so a crop that
changes never shows the framing the previous one asked for.

#### Scenario: The same request twice

- **WHEN** the pixels for one identity and bound are requested twice
- **THEN** the same image is served both times, and the interface is told the response may be reused

#### Scenario: The same crop twice

- **WHEN** the pixels for one identity, bound and crop are requested twice
- **THEN** the same framing is served both times

#### Scenario: A crop that changes

- **WHEN** the pixels for one identity and bound are requested with two different crops
- **THEN** each request is served its own framing

### Requirement: How a crop frames a photograph

A crop SHALL carry a horizontal flip, a vertical flip, an angle of rotation and the rectangle to
frame, and the application SHALL apply them in that order: flip horizontally, flip vertically,
rotate, cut. The rectangle is expressed in the coordinate space of the rotated photograph, which is
why the order is fixed rather than chosen per request.

A positive angle SHALL turn the photograph clockwise, matching the direction an on-screen rotation
control reports, and the rotated photograph SHALL occupy the canvas its turned corners need rather
than the original's dimensions. Any area of that canvas no source pixel covers SHALL be transparent,
so a corner a rotation exposes is honest rather than filled with an invented colour, and what is
served SHALL keep that transparency rather than compositing it away.

The rectangle SHALL be brought inside the rotated photograph's bounds before it is cut, so a
rectangle that runs past an edge frames what exists rather than failing the request, and what is
served SHALL always be at least one pixel on each edge. The interface cannot ask for pixels that were
never there.

A framing SHALL keep the bit depth of the photograph it was taken from, so a developed camera RAW is
not flattened to eight bits by being framed.

#### Scenario: The three parts are applied in order

- **WHEN** a request carries a crop with a flip, a rotation and a rectangle
- **THEN** the photograph is flipped, then rotated, then cut, and the pixels served are those the
  rectangle frames in the rotated photograph

#### Scenario: The direction a positive angle turns

- **WHEN** a request carries a crop with a positive angle
- **THEN** the photograph served has turned clockwise

#### Scenario: A rectangle that runs past the edge

- **WHEN** a request carries a crop whose rectangle extends beyond the rotated photograph
- **THEN** the rectangle is brought inside those bounds and the framed pixels are served, rather than
  the request being refused

#### Scenario: A rotated photograph's uncovered corners

- **WHEN** a request carries a crop that rotates the photograph and frames a corner no source pixel
  covers
- **THEN** that area is served as transparent rather than as a colour

#### Scenario: A sixteen-bit photograph

- **WHEN** a request carries a crop of a photograph holding sixteen bits per channel
- **THEN** the framing is served without its depth being flattened

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

### Requirement: The region that accepts a drop identifies itself while a drag is over it

While files are being dragged over the preview canvas, the canvas SHALL draw a visible region
marking where they will be accepted. The mark SHALL appear when the drag arrives over the canvas and
SHALL disappear when the drag leaves it or the files are let go of, whether or not anything was
opened.

The region drawn SHALL be the region that accepts the drop. The canvas already accepts a drop and
nowhere else does, and drawing one rectangle while accepting another would teach the user a boundary
the application does not honour - so what is offered and what is taken up stay the same answer, told
twice.

The mark SHALL appear for any drag over the canvas, without regard to what is being dragged. It
states that the region takes files, not that these files are acceptable: what the application will
not open is already reported by name after the drop, in the user's own language, and a mark that had
to resolve the question first would appear late or not at all on a drag passed quickly over the
window.

The mark SHALL be drawn in both of the canvas's states, whether or not an image is already open,
because the canvas accepts a drop in both.

A drag over any other region of the window SHALL draw nothing, by the same rule that makes a drop
there open nothing and report nothing.

The mark SHALL be decorative: it SHALL carry no text, SHALL NOT be announced to assistive
technology, and SHALL NOT be reachable by keyboard. What the drop area means is already stated in
words by the empty canvas.

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

### Requirement: Each open image is looked at at its own zoom and position

Every open image SHALL carry how closely it is being looked at - a magnification and which part of
the photograph is in view - independently of every other open image.

An image SHALL begin fitted: drawn whole, in proportion, and centred in the space available to it.

The magnification SHALL be kept between fitting the image and eight times that size. There is no
zooming out past the whole photograph, because the space around it is not part of the photograph, and
no zooming in past eight times, which is the reference implementation's own ceiling.

How far into a photograph the user has gone is a property of that photograph and not of the window:
it is what they were examining, and coming back to it is coming back to what they were looking at.

An image whose framing changes SHALL be fitted again, as a newly opened one is. Where in a photograph the
user had gone is expressed against that photograph's shape, and a framing gives it another shape - the same
position names a different part of a picture that is no longer the same picture. **This diverges from the
reference implementation**, which leaves its magnification alone when a crop is applied; it can afford to,
because its magnification is one value shared by every open image rather than a property of each.

#### Scenario: A newly opened image

- **WHEN** an image is opened and becomes the current one
- **THEN** it is drawn whole, in proportion and centred, at the size that fits

#### Scenario: Returning to an image

- **WHEN** the user magnifies one image, makes another the current one, and then returns to the first
- **THEN** the first image is shown at the same magnification and the same part of it as when they
  left it

#### Scenario: One image's view is its own

- **WHEN** the user magnifies the current image
- **THEN** no other open image's magnification or position changes

#### Scenario: Below the fitted size

- **WHEN** the user attempts to magnify the current image to less than the size that fits
- **THEN** it stays at the size that fits

#### Scenario: Above eight times

- **WHEN** the user attempts to magnify the current image beyond eight times the size that fits
- **THEN** it stays at eight times

#### Scenario: A new session

- **WHEN** the user magnifies an image, restarts the application and opens the same file again
- **THEN** the image is fitted again, as a newly opened image is

#### Scenario: A framing is applied to a magnified image

- **WHEN** the user magnifies an image, frames it and applies that framing
- **THEN** the framed photograph is drawn whole, in proportion and centred, at the size that fits

#### Scenario: A framing is applied to another image

- **WHEN** the user magnifies one image and then applies a framing to a different one
- **THEN** the magnified image is still at the magnification and position it was left at

