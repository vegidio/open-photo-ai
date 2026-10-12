## ADDED Requirements

### Requirement: The five enhancement families and their variants

The system SHALL offer five further families of model beyond upscale, each with the variants below,
and SHALL name each variant's artifact from the family's two-letter prefix, the variant's codename and
the precision.

| Family | Prefix | Variants |
|---|---|---|
| Denoise | `dn` | Stockholm, Gothenburg, Malmo |
| Sharpen | `sh` | Moscow, Petersburg, Novgorod |
| Light adjustment | `la` | Paris, Lyon |
| Colour balance | `cb` | Rio, São Paulo |
| Colorization | `cl` | Delhi, Mumbai, Jaipur |

Every one of these thirteen variants is published at FP32 and at FP16, and at no other precision. A
request pairing one of them with any other precision SHALL be refused rather than resolved to an
artifact that does not exist.

#### Scenario: Each denoise variant names its published artifact
- **WHEN** Stockholm, Gothenburg or Malmo is requested at FP32 or FP16
- **THEN** the artifact required is `dn_stockholm_fp32`, `dn_gothenburg_fp32`, `dn_malmo_fp32` or the
  FP16 spelling of each

#### Scenario: Each sharpen variant names its published artifact
- **WHEN** Moscow, Petersburg or Novgorod is requested at FP32 or FP16
- **THEN** the artifact required is `sh_moscow_fp32`, `sh_petersburg_fp32`, `sh_novgorod_fp32` or the
  FP16 spelling of each

#### Scenario: Each light adjustment and colour balance variant names its published artifact
- **WHEN** Paris, Lyon, Rio or São Paulo is requested at FP32 or FP16
- **THEN** the artifact required is `la_paris_fp32`, `la_lyon_fp32`, `cb_rio_fp32`, `cb_saopaulo_fp32`
  or the FP16 spelling of each

#### Scenario: Each colorization variant names its published artifact
- **WHEN** Delhi, Mumbai or Jaipur is requested at FP32 or FP16
- **THEN** the artifact required is `cl_delhi_fp32`, `cl_mumbai_fp32`, `cl_jaipur_fp32` or the FP16
  spelling of each

#### Scenario: No enhancement variant can be requested at INT8
- **WHEN** any of the thirteen variants is paired with INT8
- **THEN** the request is refused, and no artifact name ending in `_int8` is ever produced for these
  five families

### Requirement: An enhancement operation resolves to exactly one artifact

Each of the thirteen variants is published as a single model graph, so an operation from these five
families SHALL resolve to exactly one artifact and SHALL require that one artifact and no other. This
is what the download path reads to decide what must be present on disk before the operation can run.

#### Scenario: An operation requires one artifact
- **WHEN** any of the thirteen variants is requested at either of its precisions
- **THEN** exactly one artifact is required

#### Scenario: The artifact name carries no scale segment
- **WHEN** any of the thirteen variants resolves to its artifact
- **THEN** the name is the family prefix, the codename and the precision alone, with no scale segment -
  these families do not publish one set of weights per scale

### Requirement: Denoise and sharpen carry a strength

A denoise or sharpen operation SHALL carry a strength bounded to 0.0 through 3.0 inclusive. A strength
of 1.0 is the model's own output; below it the effect is softened towards the original image and above
it the effect is amplified past the model's output.

As with the upscale scale, the system SHALL offer both a rejecting and a clamping way to supply a
strength, so that a caller which has already bounded its input and one which has not each get the
behaviour that suits it. Whichever is used, a strength that has been accepted SHALL be within range.

The reference implementation bounds this value only in its user interface and never in its model code,
so a caller that bypassed the interface could supply any value at all; bounding it here is deliberate
and closes that gap without changing what the interface offers.

#### Scenario: A strength inside the range is taken as given
- **WHEN** a strength of 0, 0.5, 1, 2.25 or 3 is supplied
- **THEN** it is accepted and carried at that value

#### Scenario: An out-of-range strength is refused when rejection was asked for
- **WHEN** a strength of -0.5 or 4 is supplied through the rejecting constructor
- **THEN** an error is returned naming the permitted range, and no operation is produced

#### Scenario: An out-of-range strength is bounded when clamping was asked for
- **WHEN** a strength of -0.5 or 4 is supplied through the clamping constructor
- **THEN** it is bounded to 0 or 3 respectively, and an operation is produced

### Requirement: Light adjustment and colour balance carry a bias

A light adjustment or colour balance operation SHALL carry a bias bounded to -1.0 through 1.0
inclusive. A bias of 0.0 leaves the image as it was, 1.0 is the model's own output, and a negative
bias applies the adjustment in the opposite direction.

The same rejecting and clamping ways to supply it SHALL be offered, with the same guarantee that an
accepted value is in range.

#### Scenario: A bias inside the range is taken as given
- **WHEN** a bias of -1, -0.35, 0, 0.5 or 1 is supplied
- **THEN** it is accepted and carried at that value

#### Scenario: An out-of-range bias is refused when rejection was asked for
- **WHEN** a bias of -2 or 1.5 is supplied through the rejecting constructor
- **THEN** an error is returned naming the permitted range, and no operation is produced

#### Scenario: An out-of-range bias is bounded when clamping was asked for
- **WHEN** a bias of -2 or 1.5 is supplied through the clamping constructor
- **THEN** it is bounded to -1 or 1 respectively, and an operation is produced

### Requirement: A per-run parameter compares exactly

A strength and a bias SHALL each be quantized to three decimal places when accepted, and SHALL compare
exactly thereafter. Two values that are accepted as the same value SHALL be indistinguishable in every
later respect, including in the image cache.

In the reference implementation this value reaches the image cache key formatted to three significant
figures, so two values that round alike share a key and the second run is served the first one's
image. Quantizing on acceptance makes the two one value before anything is rendered, so they are one
run rather than two colliding.

#### Scenario: A value finer than three decimals is quantized on acceptance
- **WHEN** a strength of 1.23456 is supplied
- **THEN** it is accepted as 1.235 and the operation reports that value

#### Scenario: Two values that quantize alike are one value
- **WHEN** biases of 0.12341 and 0.12339 are both supplied
- **THEN** both are accepted as 0.123, and operations built from them report the same cache tag

#### Scenario: Every accepted value is comparable to itself
- **WHEN** any accepted strength or bias is compared with itself
- **THEN** it compares equal, with no value behaving otherwise

### Requirement: Colorization takes no per-run parameter

A colorization operation SHALL take no per-run parameter. It SHALL NOT carry an unused parameter
defaulted to some neutral value, and its identity and its cache tag SHALL be determined by its variant
and precision alone.

#### Scenario: A colorization operation is fully described by its variant and precision
- **WHEN** two colorization operations name the same variant at the same precision
- **THEN** they are equal and report the same cache tag, with no further input able to distinguish them

#### Scenario: No colorization request accepts an amount
- **WHEN** a colorization operation is constructed
- **THEN** there is no strength, bias or other per-run amount to supply

### Requirement: A per-run parameter is outside the model's identity

For all four parameterized enhancement families, the strength or bias SHALL NOT distinguish two
operations. Two operations naming the same variant at the same precision SHALL be equal and hash alike
whatever their parameter, because one set of weights serves every value of it.

This is what a model registry keys on: dragging a slider must reuse the session that is already loaded
rather than read as a different model and load a second copy of the same weights.

#### Scenario: Two strengths of one denoise variant are the same operation
- **WHEN** Stockholm at FP32 with a strength of 0.5 and Stockholm at FP32 with a strength of 2.5 are
  compared
- **THEN** they are equal and hash alike, so a registry keyed on them loads one model

#### Scenario: Two biases of one colour balance variant are the same operation
- **WHEN** Rio at FP16 with a bias of -0.5 and Rio at FP16 with a bias of 1.0 are compared
- **THEN** they are equal and hash alike

#### Scenario: Two precisions are different operations
- **WHEN** Moscow at FP32 and Moscow at FP16 are compared at the same strength
- **THEN** they are not equal, because different artifacts serve them

#### Scenario: Two variants of one family are different operations
- **WHEN** Paris and Lyon are compared at the same precision and bias
- **THEN** they are not equal

### Requirement: A per-run parameter is inside the image cache tag

Every enhancement operation SHALL report an image cache tag, and that tag SHALL distinguish everything
that changes the resulting image - including the strength or bias, which does not distinguish the
operation itself.

Two operations that produce the same image from the same input SHALL report the same tag, and two that
do not SHALL report different tags. As with the upscale families, the tag SHALL be defined deliberately
rather than derived from the serialized form, so that renaming a field cannot silently invalidate every
cached image.

#### Scenario: Two strengths do not collide in the cache
- **WHEN** Stockholm at FP32 is run at a strength of 0.5 and then at 2.5 over the same image
- **THEN** the two operations are equal but their cache tags differ, and the second run is not served
  the first one's image

#### Scenario: A repeated operation hits the cache
- **WHEN** Rio at FP16 with a bias of 0.5 is run twice over the same image
- **THEN** both runs report the same cache tag and the second may be served from cache

#### Scenario: No enhancement cache tag can be mistaken for an artifact name
- **WHEN** any enhancement operation reports its cache tag
- **THEN** the tag is distinguishable from every published artifact name, so the two namespaces cannot
  collide

#### Scenario: Every distinguishable operation has a distinct tag
- **WHEN** any two enhancement operations that would produce different images are compared
- **THEN** their cache tags differ

### Requirement: Enhancement models report a display name

Every enhancement operation SHALL report a display name for a user interface to show, formed as the
variant's label followed by the precision in upper case in parentheses. None of these five families
SHALL include its per-run parameter in that name: the parameter does not select the weights, so a name
fixed at whichever value happened to load them first would be wrong thereafter.

#### Scenario: A denoise display name carries no strength
- **WHEN** Stockholm at FP32 with any strength is asked for its display name
- **THEN** it is `Stockholm (FP32)`

#### Scenario: A colour balance display name uses the variant's published label
- **WHEN** São Paulo at FP16 is asked for its display name
- **THEN** it is `São Paulo (FP16)`, the label the reference application shows

#### Scenario: A colorization display name is the variant and precision
- **WHEN** Delhi at FP32 is asked for its display name
- **THEN** it is `Delhi (FP32)`

### Requirement: The catalogue publishes every family

The catalogue SHALL describe all six families - the five added here alongside upscale - reporting for
each the variants available, each variant's display label, the precisions each variant admits, and the
permitted range of each parameter the family takes.

A front end SHALL be able to populate every model chooser and configure every parameter control from
the catalogue alone, without restating a variant name, precision or bound of its own. A family that
takes no parameter SHALL report that it takes none, rather than being absent or reporting a parameter
with an empty range.

#### Scenario: The catalogue lists every family
- **WHEN** the catalogue is asked what the library offers
- **THEN** it reports six families - denoise, sharpen, light adjustment, colour balance, colorization
  and upscale

#### Scenario: The catalogue lists each family's variants and labels
- **WHEN** the catalogue is asked for the denoise family
- **THEN** it reports Stockholm, Gothenburg and Malmo with their display labels

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

#### Scenario: Everything the catalogue offers can be built
- **WHEN** every variant and precision pairing the catalogue lists is constructed
- **THEN** each one is constructible and resolves to at least one artifact

### Requirement: Enhancement operations survive a round trip through serialization

An enhancement operation SHALL be convertible to a serialized form and back, producing an equal
operation and preserving its per-run parameter, so that a front end can carry it across a process
boundary and persist a user's chosen enhancements between sessions.

A serialized form that names a variant and precision pairing the system does not publish, or a
parameter outside its permitted range, SHALL be refused on the way back in rather than producing an
operation that could not have been constructed directly.

#### Scenario: An operation round-trips unchanged
- **WHEN** any enhancement operation is serialized and then deserialized
- **THEN** the result equals the original and reports the same parameter and the same cache tag

#### Scenario: A serialized form that could not have been constructed is refused
- **WHEN** a serialized operation pairs any enhancement variant with INT8, or carries a strength of 4
  or a bias of 2
- **THEN** deserialization fails rather than producing an operation that violates the rules above
