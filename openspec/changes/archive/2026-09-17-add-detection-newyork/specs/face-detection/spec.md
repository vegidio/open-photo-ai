## Purpose

What a face-detection run does to an image and what comes back from it: the fixed-size view of the photograph the
detector is given whatever its shape, which of the detector's many candidate detections survive, and the coordinate
space the faces are reported in — because the detector sees a square of one size and the caller has a photograph of
another, and every face it finds has to travel back across that difference correctly.

## ADDED Requirements

### Requirement: An image of any size and aspect is presented to the detector at its fixed size

The detector accepts one fixed square size. The system SHALL fit an image of any size and any aspect ratio into it by
scaling the whole image down until its longer side reaches that size, **preserving the aspect ratio**, and filling the
remainder of the square with content the detector reads as nothing rather than as picture.

Scaling each axis independently to fill the square would distort every face in the image, and a detector trained on
undistorted faces would find fewer of them and place their landmarks wrongly. Filling the remainder with a repetition
or a mirror of the picture's own edge would present the detector with content that is not in the photograph, which it
may find a face in.

The scaled image SHALL be placed at one fixed corner of the square rather than centred, so that the relationship
between a coordinate the detector reports and a coordinate in the source image is a single scale factor with no offset
to undo.

An image with no area SHALL be refused rather than run. There is no scaling of it into the detector's square, and a
run over a square of nothing but padding is not a detection of anything. This is the same refusal the enhancement path
makes for an image it cannot partition, for the same reason.

#### Scenario: A landscape photograph

- **WHEN** an image wider than it is tall is detected on
- **THEN** its width is scaled to the detector's size, its height is scaled by the same factor, and the rest of the square carries no picture content

#### Scenario: A portrait photograph

- **WHEN** an image taller than it is wide is detected on
- **THEN** its height is scaled to the detector's size, its width is scaled by the same factor, and the rest of the square carries no picture content

#### Scenario: An image already square

- **WHEN** a square image is detected on
- **THEN** it fills the detector's square with no padding at all

#### Scenario: An image with no area

- **WHEN** a detection run is asked for an image with no width or no height
- **THEN** the run is refused and no faces are produced

### Requirement: Only confident, non-overlapping detections are reported

The detector produces one candidate for every position, scale and shape it considers, the overwhelming majority of
which are not faces, and a real face is typically found several times over by neighbouring candidates. The system
SHALL reduce those candidates to one entry per face.

A candidate whose confidence does not exceed the detector's threshold SHALL be discarded. Of two surviving candidates
that overlap more than the permitted amount, the system SHALL keep the one with the higher confidence and discard the
other — so a face found four times is reported once, at the detector's best account of where it is.

Where no candidate survives, the result SHALL be an empty set of faces rather than an error. An image with no face in
it is a successful detection that found nothing.

#### Scenario: A face found by several neighbouring candidates

- **WHEN** several overlapping candidates above the threshold describe the same face
- **THEN** one entry is reported, the one with the highest confidence

#### Scenario: Two faces close together

- **WHEN** two candidates above the threshold overlap by less than the permitted amount
- **THEN** both are reported, because they are two faces rather than two accounts of one

#### Scenario: An image with no face

- **WHEN** no candidate exceeds the threshold
- **THEN** the result is an empty set of faces and is not an error

### Requirement: Faces are reported in the source image's own coordinate space

Every coordinate reported — the corners of the box and each of the five landmarks — SHALL be expressed in pixels of
the image the caller supplied, not of the detector's square, and not as a fraction of anything.

The caller supplied a photograph and will draw on that photograph, select regions of it, and hand those regions to
other models. A coordinate in the detector's square is meaningless to every one of those, and a caller left to undo
the scaling itself would have to know the detector's fixed size and how the image was fitted into it — which is the
whole of what this path exists to hide.

Coordinates SHALL be reported as the detector placed them, carried back across the scaling without being rounded to
whole pixels, clamped to the image's bounds or otherwise adjusted. A face may legitimately extend past the edge of
the photograph, and a box rounded to whole pixels loses the sub-pixel placement that is the only thing distinguishing
two runs of the same detector at different precisions.

The confidence SHALL be reported as the detector produced it, unrounded.

#### Scenario: A face in a large photograph

- **WHEN** a face is found in an image much larger than the detector's square
- **THEN** its box and landmarks are in pixels of that image, at the scale the caller can draw at directly

#### Scenario: A face at the edge of the frame

- **WHEN** a detected face extends past the edge of the image
- **THEN** the coordinates reported extend past it too, rather than being clamped to the image's bounds

#### Scenario: The same image at two detector precisions

- **WHEN** the same image is detected on at each precision the detector is published in
- **THEN** the same faces are found in the same order, differing only in sub-pixel placement
