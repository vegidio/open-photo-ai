## MODIFIED Requirements

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

#### Scenario: An image is open

- **WHEN** an image is open
- **THEN** the sidebar's preview area draws it and no longer says there is no preview

#### Scenario: The image the sidebar draws

- **WHEN** the current image changes
- **THEN** the sidebar draws the image the canvas is drawing

#### Scenario: An image carrying a crop

- **WHEN** the current image carries a crop
- **THEN** the sidebar's miniature draws the framed photograph

## ADDED Requirements

### Requirement: The strip of open images is drawn uncropped

The drawer's strip SHALL draw each open image as its file is, whatever crop that image carries.

The strip answers "which photographs are open", and a thumbnail that changes shape as a crop is dragged
answers it worse. The reference implementation behaves the same way, which is worth stating because the
difference between a cropped miniature and an uncropped strip beside it reads as an oversight.

#### Scenario: A cropped image in the strip

- **WHEN** an open image carries a crop
- **THEN** its thumbnail in the strip draws the whole photograph, while the sidebar's miniature draws
  the framing
