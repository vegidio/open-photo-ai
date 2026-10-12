## Purpose

How the application takes image files from the user, what it records about each one without decoding
it, and how an opened image's pixels reach the interface at whatever size the interface is drawing
them - the preview canvas, a drawer thumbnail and the sidebar miniature being the same request at
three bounds.

## ADDED Requirements

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
image's identity together with a bound on the image's longest edge.

A bound SHALL cap the longest edge while preserving the image's aspect ratio. A request carrying no
bound SHALL serve the image at its own dimensions. A bound at or above the image's longest edge
SHALL serve the image at its own dimensions rather than enlarging it.

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

#### Scenario: A format the interface cannot decode

- **WHEN** the pixels of an opened camera RAW, TIFF or HEIF image are requested
- **THEN** they are served, in a form the interface can draw

#### Scenario: An image with transparency

- **WHEN** the pixels of an opened image carrying an alpha channel are requested
- **THEN** they are served in a form that keeps the transparency, rather than composited away

### Requirement: Only files the user has opened can be served

The application SHALL serve pixels only for files it has admitted through one of the two routes
above, and SHALL address them by identity rather than by location.

A request naming an identity the application has not admitted SHALL be refused, and refused
distinguishably from a file that was admitted and has since become unreadable.

No file on the machine outside that set SHALL be reachable through this route, whatever is named in
the request. The set of images the user has opened is the whole of what the interface is entitled to
read.

#### Scenario: An unknown identity

- **WHEN** pixels are requested for an identity no admitted file carries
- **THEN** the request is refused, and reported as unknown rather than as unreadable

#### Scenario: An admitted file that has gone away

- **WHEN** pixels are requested for an admitted file that has since been deleted or become unreadable
- **THEN** the request is refused, and reported as unreadable rather than as unknown

#### Scenario: A location outside the opened set

- **WHEN** a request names a location on the machine rather than an admitted identity
- **THEN** nothing is served

### Requirement: An identity and a bound always denote the same pixels

Pixels served for a given identity and bound SHALL be the same on every request, and the interface
SHALL be told it may retain them.

An identity describes bytes, so what it addresses cannot change without the identity changing. That
is what lets the interface draw the same thumbnail repeatedly, and switch back to an image it has
already shown, without the application decoding the file again.

#### Scenario: The same request twice

- **WHEN** the pixels for one identity and bound are requested twice
- **THEN** the same image is served both times, and the interface is told the response may be reused
