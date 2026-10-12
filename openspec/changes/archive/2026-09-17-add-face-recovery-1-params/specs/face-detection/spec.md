## MODIFIED Requirements

### Requirement: Faces are reported in the source image's own coordinate space

Every coordinate reported — the corners of the box and each of the five landmarks — SHALL be expressed in pixels of
the image the caller supplied, not of the detector's square, and not as a fraction of anything.

The caller supplied a photograph and will draw on that photograph, select regions of it, and hand those regions to
other models. A coordinate in the detector's square is meaningless to every one of those, and a caller left to undo
the scaling itself would have to know the detector's fixed size and how the image was fitted into it — which is the
whole of what this path exists to hide.

Coordinates SHALL be reported where the detector placed them, carried back across the scaling without being rounded
to whole pixels or clamped to the image's bounds. A face may legitimately extend past the edge of the photograph, and
a box rounded to whole pixels would lose sub-pixel placement entirely.

Coordinates SHALL be carried at a resolution of a hundredth of a pixel, which is the resolution the values are
accepted and compared at throughout the library. That is finer than the difference between two runs of one detector
and finer than any difference a consumer can act on; it is what lets a selection be compared and cached as a value
rather than only as a string written from one.

The confidence SHALL be reported as the detector produced it, at the same resolution.

#### Scenario: A face in a large photograph

- **WHEN** a face is found in an image much larger than the detector's square
- **THEN** its box and landmarks are in pixels of that image, at the scale the caller can draw at directly

#### Scenario: A face at the edge of the frame

- **WHEN** a detected face extends past the edge of the image
- **THEN** the coordinates reported extend past it too, rather than being clamped to the image's bounds

#### Scenario: Sub-pixel placement survives

- **WHEN** a face is found whose box falls between whole pixels
- **THEN** the coordinates reported are not whole numbers, and are carried to a hundredth of a pixel

#### Scenario: The same image at two detector precisions

- **WHEN** the same image is detected on at each precision the detector is published in
- **THEN** the same faces are found in the same order, differing at most in sub-pixel placement and comparing equal
  wherever that difference falls below a hundredth of a pixel
