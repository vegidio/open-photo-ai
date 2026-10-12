## ADDED Requirements

### Requirement: A detected face is a box, five landmarks and a confidence

The system SHALL provide a value describing one face found in an image, carrying exactly three things:
the bounding box enclosing the face, the five facial landmark points the detection model predicts, and
the confidence that what was found is a face.

The landmark count SHALL be exactly five and SHALL be fixed rather than variable, and the five SHALL be
in a stated order - left eye, right eye, nose, left mouth corner, right mouth corner - because a
consumer that aligns a face reads them positionally and a reordering would misalign every face
silently.

A confidence SHALL be a value from 0.0 to 1.0 inclusive. A confidence outside that range, or one that
is not a number, SHALL be refused rather than carried, so that a face which exists is a face whose
confidence is meaningful.

Coordinates SHALL be carried as the detection model reports them, in the pixel coordinate space of the
image that was detected on, and SHALL NOT be rounded or rescaled on acceptance.

#### Scenario: A face carries exactly five landmarks
- **WHEN** a face is constructed
- **THEN** it carries five landmark points, and there is no way to construct one carrying four or six

#### Scenario: A confidence inside the range is taken as given
- **WHEN** a confidence of 0.0, 0.5 or 1.0 is supplied
- **THEN** it is accepted and carried at that value

#### Scenario: A confidence outside the range is refused
- **WHEN** a confidence of -0.1, 1.5 or a value that is not a number is supplied
- **THEN** an error naming the permitted range is returned, and no confidence - and therefore no face -
  is produced

#### Scenario: Coordinates survive unrounded
- **WHEN** a face is constructed from sub-pixel box and landmark coordinates
- **THEN** it reports back exactly the coordinates it was given

### Requirement: The detection family and its variant

The system SHALL offer one face-detection variant - New York - published at FP32 and FP16 and at no
other precision, resolving to the artifacts `dt_newyork_fp32` and `dt_newyork_fp16`.

A detection run SHALL take no per-run parameter: the variant and its precision are the whole of what
one is. A request pairing New York with INT8 SHALL be refused rather than resolved to an artifact that
does not exist.

#### Scenario: New York names its published artifact
- **WHEN** New York is requested at FP32 and at FP16
- **THEN** the operations resolve to `dt_newyork_fp32` and `dt_newyork_fp16` respectively

#### Scenario: A detection operation resolves to exactly one artifact
- **WHEN** a detection operation is asked what must be on disk before it can run
- **THEN** it reports exactly one artifact, its own graph

#### Scenario: Detection cannot be requested at INT8
- **WHEN** New York is paired with INT8
- **THEN** the request is refused, and no artifact name containing `dt_newyork_int8` is ever produced

#### Scenario: A detection operation is fully described by its variant and precision
- **WHEN** a detection operation is constructed
- **THEN** there is no amount, strength, bias or scale to supply, and none is accepted

### Requirement: Detection produces faces rather than an image

Detection SHALL be the one family whose result is not an image. A detection run SHALL produce the set
of faces found in the input image, in the order the model reports them, and SHALL produce an empty set
rather than an error when the image contains no face.

The order SHALL be preserved as reported, because a consumer that lets a user pick faces refers to them
by position, and a reordering would silently re-target that selection.

#### Scenario: A detection result is a set of faces
- **WHEN** the library is asked what a detection run produces
- **THEN** it is a set of faces, not an image

#### Scenario: An image with no face yields an empty set
- **WHEN** a detection run finds no face
- **THEN** the result is an empty set of faces, and is not an error

### Requirement: The face recovery family and its variants

The system SHALL offer two face-recovery variants - Athens and Santorini - each published at FP32 and
FP16 and at no other precision, resolving to `fr_athens_fp32`, `fr_athens_fp16`, `fr_santorini_fp32`
and `fr_santorini_fp16`.

A face-recovery operation SHALL report exactly one artifact as required: its own graph. It SHALL NOT
report the detection graph, because the faces are supplied to it already detected and it never opens
that graph - whoever detected them asked the detection operation, which reported that artifact itself.

#### Scenario: Each face recovery variant names its published artifact
- **WHEN** Athens and Santorini are requested at FP32 and at FP16
- **THEN** the four operations resolve to `fr_athens_fp32`, `fr_athens_fp16`, `fr_santorini_fp32` and
  `fr_santorini_fp16`

#### Scenario: A face recovery operation resolves to exactly one artifact
- **WHEN** any face-recovery operation is asked what must be on disk before it can run
- **THEN** it reports exactly one artifact, its own graph, and no detection artifact

#### Scenario: Face recovery cannot be requested at INT8
- **WHEN** Athens or Santorini is paired with INT8
- **THEN** the request is refused, and no artifact name containing `_int8` is produced for either

### Requirement: Face recovery carries the faces it is to recover

A face-recovery operation SHALL carry the set of faces the run is to restore, supplied by the caller
rather than detected by the operation itself.

An empty set SHALL be a legitimate value rather than an error: an image with no face, or one where the
user has deselected every face, is a run that recovers nothing, not a request the system refuses.

The set SHALL preserve the order it was given, and SHALL be carried unchanged - a face handed in is the
face handed back, so that a front end which stores a user's per-face selection by position is never
re-targeted by the library.

#### Scenario: An operation carries the faces it was given
- **WHEN** a face-recovery operation is built over two faces
- **THEN** it reports those two faces, in that order, unchanged

#### Scenario: An empty set of faces is accepted
- **WHEN** a face-recovery operation is built over no faces
- **THEN** the operation is produced, reports an empty set, and is not an error

### Requirement: A set of faces is outside the model's identity and inside its image cache tag

Two face-recovery operations SHALL be equal exactly when they would load the same model - that is, when
their variant and precision match - regardless of which faces each carries. One set of weights serves
every selection, so a user toggling a face must reuse the session already loaded rather than being read
as a different model.

The faces SHALL nonetheless distinguish two operations in the image cache, because a different
selection produces a different image. The cache tag SHALL therefore fold in the faces the run carries.

The tag SHALL be order-sensitive, SHALL be identical across repeated runs of the same selection, and
SHALL distinguish every distinct selection. It SHALL be derived from the faces' bounding boxes at a
resolution of two decimal places - finer than any difference a repeated detection of the same image
produces, and the same resolution the reference implementation folds in. An operation carrying no faces
SHALL report a tag that says so and that no non-empty selection can produce.

As for every other family, a cache tag SHALL NOT be spelled the way a published artifact name is, and
no two distinguishable operations across all eight families SHALL share one.

#### Scenario: Two face selections are the same operation
- **WHEN** Athens at FP32 over two faces and Athens at FP32 over one face are compared
- **THEN** they are equal and hash alike, so a registry keyed on them loads one model

#### Scenario: Two face selections do not collide in the cache
- **WHEN** the same two operations are asked for their cache tags
- **THEN** the tags differ, so the two runs cannot be served one another's image

#### Scenario: A reordered selection is a different cache entry
- **WHEN** two operations carry the same faces in a different order
- **THEN** their cache tags differ

#### Scenario: A repeated selection hits the cache
- **WHEN** an operation is rebuilt over an equal set of faces
- **THEN** it reports the same cache tag as before

#### Scenario: An empty selection has a tag of its own
- **WHEN** a face-recovery operation carrying no faces is asked for its cache tag
- **THEN** it reports a tag that no operation carrying faces reports

#### Scenario: A difference below the tag's resolution is one selection
- **WHEN** two selections differ only in a bounding-box coordinate below the second decimal place
- **THEN** they report the same cache tag, so a re-detection that moved a box by a hundredth of a pixel
  does not discard the cached image

### Requirement: Detection and face recovery report a display name

Both families SHALL report a display name built from the variant's published label and its precision -
`New York (FP32)`, `Athens (FP16)`. The faces a face-recovery operation carries SHALL NOT appear in its
display name: they do not select the weights, so a name fixed at whichever selection happened to load
them would be wrong from the second run onwards.

#### Scenario: A detection display name is the variant and precision
- **WHEN** New York at FP32 is asked for its display name
- **THEN** it reports `New York (FP32)`

#### Scenario: A face recovery display name carries no faces
- **WHEN** Athens at FP16 is asked for its display name over any selection of faces
- **THEN** it reports `Athens (FP16)`, the same name for every selection

### Requirement: Detection and face recovery operations survive a round trip through serialization

Both operations SHALL be convertible to a serialized form and back, producing an equal operation. A
face-recovery operation SHALL preserve the faces it carries across the round trip, including an empty
set, so that a front end can persist a user's per-face selection between sessions and carry it across a
process boundary.

A serialized form that could not have been constructed directly SHALL be refused on the way back in -
a variant paired with a precision it is not published in, or a face carrying a confidence outside 0.0
to 1.0 or a landmark count other than five.

#### Scenario: An operation round-trips unchanged
- **WHEN** any detection or face-recovery operation is serialized and then deserialized
- **THEN** the result equals the original, reports the same faces, and reports the same cache tag

#### Scenario: An empty selection round-trips as an empty selection
- **WHEN** a face-recovery operation carrying no faces is serialized and deserialized
- **THEN** the result carries no faces, rather than failing or acquiring one

#### Scenario: A serialized form that could not have been constructed is refused
- **WHEN** a serialized operation pairs New York, Athens or Santorini with INT8, or carries a face
  whose confidence is 1.5 or whose landmark count is four
- **THEN** deserialization fails rather than producing an operation that violates the rules above

## MODIFIED Requirements

### Requirement: The catalogue publishes every family

The catalogue SHALL describe all eight families - detection and face recovery alongside the six named
before them - reporting for each the variants available, each variant's display label, the precisions
each variant admits, and the permitted range of each parameter the family takes.

A front end SHALL be able to populate every model chooser and configure every parameter control from
the catalogue alone, without restating a variant name, precision or bound of its own. This includes
which model detects faces: a front end that pre-detects reads the detection variant and its precisions
from the catalogue rather than naming that model itself.

A family that takes no parameter SHALL report that it takes none, rather than being absent or reporting
a parameter with an empty range. Detection and face recovery SHALL each report no parameters -
detection because a run is its variant and precision alone, and face recovery because the faces it
carries are supplied by a detection run rather than adjusted through a control.

#### Scenario: The catalogue lists every family
- **WHEN** the catalogue is asked what the library offers
- **THEN** it reports eight families - denoise, sharpen, light adjustment, colour balance,
  colorization, upscale, detection and face recovery - the six already published in the order they were
  already published in, and the two added here after them

#### Scenario: The catalogue lists each family's variants and labels
- **WHEN** the catalogue is asked for the denoise family
- **THEN** it reports Stockholm, Gothenburg and Malmo with their display labels

#### Scenario: The catalogue names the model that detects faces
- **WHEN** the catalogue is asked for the detection family
- **THEN** it reports New York with its display label and the precisions it is published in, so a front
  end never has to name that model itself

#### Scenario: The catalogue reports the precisions every enhancement variant admits
- **WHEN** the catalogue is asked which precisions the thirteen enhancement variants admit
- **THEN** it reports FP32 and FP16 for each of them, and INT8 for none of them

#### Scenario: The catalogue reports the parameter bounds the constructors enforce
- **WHEN** the catalogue is asked for the denoise and light adjustment parameters
- **THEN** it reports a range of 0 to 3 for the first and -1 to 1 for the second, the same bounds that
  constructing those values enforces

#### Scenario: The catalogue reports that colorization takes no parameter
- **WHEN** the catalogue is asked for the colorization family
- **THEN** it lists the family and its variants and reports no parameters, so a front end knows to draw
  no control rather than having to know it itself

#### Scenario: The catalogue reports that detection and face recovery take no parameter
- **WHEN** the catalogue is asked for the detection and face recovery families
- **THEN** each lists its variants and reports no parameters

#### Scenario: Everything the catalogue offers can be built
- **WHEN** every variant and precision pairing the catalogue lists is constructed
- **THEN** each one is constructible and resolves to at least one artifact
