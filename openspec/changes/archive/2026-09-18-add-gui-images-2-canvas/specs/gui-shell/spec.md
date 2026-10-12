## ADDED Requirements

### Requirement: With an image open the canvas draws it

While an image is open, the preview canvas SHALL draw the current image in place of the invitation to
supply one, and SHALL draw it whole and in proportion within the space it has - never cropped and
never stretched.

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
labelled as such. While no image is open it SHALL show neither.

The name shown SHALL be the file's own name rather than its location: the navbar answers "which
photograph am I looking at", and a path long enough to push the rest of the navbar off the window
answers it worse than the name does.

#### Scenario: An image is open

- **WHEN** an image is open
- **THEN** the navbar shows its file name and its width and height in pixels

#### Scenario: No image is open

- **WHEN** no image is open
- **THEN** the navbar shows no file name and no dimensions

#### Scenario: A file in a deep folder

- **WHEN** the current image's file lies several folders deep
- **THEN** the navbar shows the file's name and not the folders above it

### Requirement: The sidebar shows a miniature of the current image

While an image is open, the sidebar's preview area SHALL draw the current image, whole and in
proportion, in place of the words that report there is no preview.

#### Scenario: An image is open

- **WHEN** an image is open
- **THEN** the sidebar's preview area draws it and no longer says there is no preview

#### Scenario: The image the sidebar draws

- **WHEN** the current image changes
- **THEN** the sidebar draws the image the canvas is drawing
