## MODIFIED Requirements

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
