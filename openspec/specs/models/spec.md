# models Specification

## Purpose

Names the AI models the application can run, validates the parameters each one accepts, and resolves
a requested operation to the model artifacts it needs, so that a front end can offer a model and the
inference pipeline can load it without either restating what the catalogue already knows.

## Requirements

### Requirement: Upscale variants and their precisions

The system SHALL offer four upscale variants - Tokyo, Kyoto, Saitama and Osaka - and SHALL admit only
the precisions each one is published in. Tokyo, Kyoto and Saitama are published as FP32 and FP16.
Osaka is published as FP16 and INT8, and is not published as FP32.

A request pairing a variant with a precision it is not published in SHALL be refused rather than
resolved to an artifact that does not exist.

#### Scenario: A convolutional variant accepts both of its precisions
- **WHEN** Tokyo, Kyoto or Saitama is requested at FP32 or at FP16
- **THEN** the operation is produced

#### Scenario: Osaka accepts FP16 and INT8
- **WHEN** Osaka is requested at FP16 or at INT8
- **THEN** the operation is produced

#### Scenario: Osaka cannot be requested at FP32
- **WHEN** Osaka is paired with FP32
- **THEN** the request is refused, and no artifact name containing `up_osaka_fp32` is ever produced

### Requirement: Upscale scale range

Every upscale operation SHALL carry a scale factor bounded to 1x through 8x inclusive. A scale of 1x
is meaningful and SHALL be accepted: the model restores detail at the image's original size.

The system SHALL offer both a rejecting and a clamping way to supply a scale, so that a caller which
has already bounded its input (a slider) and a caller which has not (a command-line flag) each get
the behaviour that suits it. Whichever is used, a scale that has been accepted SHALL be within range.

#### Scenario: A scale inside the range is taken as given
- **WHEN** a scale of 1, 1.65, 2, 2.5 or 8 is supplied
- **THEN** it is accepted and carried at that value

#### Scenario: An out-of-range scale is refused when rejection was asked for
- **WHEN** a scale of 0.5 or 12 is supplied through the rejecting constructor
- **THEN** an error is returned naming the permitted range, and no operation is produced

#### Scenario: An out-of-range scale is bounded when clamping was asked for
- **WHEN** a scale of 0.5 or 12 is supplied through the clamping constructor
- **THEN** it is bounded to 1 or 8 respectively, and an operation is produced

### Requirement: An operation is the request it describes

An operation SHALL be a value that can be compared for equality and used as a lookup key directly. It
SHALL NOT be reduced to a string in order to be compared, looked up or dispatched on.

Two operations SHALL be equal exactly when they describe the same request - the same model, at the same precision,
with the same per-run parameters. A consumer comparing two operations is therefore told whether a user has changed
anything, which is the question a consumer holding two of them actually has.

Equality SHALL NOT decide which model is loaded. What a run needs on disk is the artifacts it reports, and a loaded
model SHALL be shared by every operation needing that artifact - so a scale changed on a variant whose weights serve
every scale, or a parameter dragged through its range, never loads a second copy of weights already resident.

#### Scenario: Two identical requests are the same operation
- **WHEN** Kyoto at 2.5x in FP32 is constructed twice
- **THEN** the two compare equal and hash alike

#### Scenario: Two scales of a convolutional variant are different operations
- **WHEN** Kyoto at 2x in FP32 and Kyoto at 4x in FP32 are compared
- **THEN** they are not equal, because they describe different requests

#### Scenario: Two scales of Osaka are different operations
- **WHEN** Osaka at 2x in FP16 and Osaka at 6x in FP16 are compared
- **THEN** they are not equal, although one set of weights serves both

#### Scenario: Two precisions are different operations
- **WHEN** Kyoto at 4x in FP32 and Kyoto at 4x in FP16 are compared
- **THEN** they are not equal, because different artifacts serve them

#### Scenario: Weights are shared across operations that differ
- **WHEN** Osaka at 2x in FP16 and Osaka at 6x in FP16 are run in turn
- **THEN** the second reuses the weights the first loaded, because they require the same artifacts

### Requirement: A scale compares exactly

A scale SHALL be quantized to three decimal places when it is accepted, and SHALL compare exactly
thereafter. Two scales that are accepted as the same value SHALL be indistinguishable in every later
respect - equality, use as a lookup key, and the run cache.

This is what lets an operation be used as a key at all: a scale that compared approximately, or that
had values comparing unequal to themselves, could not soundly identify anything.

#### Scenario: A scale finer than three decimals is quantized on acceptance
- **WHEN** a scale of 1.66661 is supplied
- **THEN** it is accepted as 1.667 and the operation reports its scale as 1.667

#### Scenario: Two scales that quantize alike are one value
- **WHEN** scales of 1.66661 and 1.66669 are both supplied
- **THEN** both are accepted as 1.667, and operations built from them compare equal and hash alike

#### Scenario: Every accepted scale is comparable to itself
- **WHEN** any accepted scale is compared with itself
- **THEN** it compares equal, with no value behaving otherwise

### Requirement: Convolutional upscalers resolve to a pass sequence

A convolutional upscale operation SHALL resolve to an ordered sequence of native scaling passes that
covers the requested scale, chosen by the smallest native scale bound that is at least the request.
Each pass names the artifact that serves it. Where the sequence overshoots the requested scale, the
overshoot is corrected after inference and is not a property of this resolution.

Kyoto ships native 2x and 4x weights: a scale up to 2x is one 2x pass, up to 4x is one 4x pass, and
up to 8x is a 4x pass followed by a 2x pass. Tokyo and Saitama ship only native 4x weights: a scale
up to 4x is one 4x pass, and up to 8x is two 4x passes.

An operation SHALL also report the distinct artifacts its passes require, which is what determines
what must be present on disk before it can run. A sequence that runs one artifact twice requires that
artifact once.

#### Scenario: Kyoto covers a small scale with its 2x weights
- **WHEN** Kyoto is requested at 1.5x in FP32
- **THEN** the pass sequence is a single 2x pass served by `up_kyoto_2x_fp32`

#### Scenario: Kyoto covers a large scale with two passes of different weights
- **WHEN** Kyoto is requested at 8x in FP32
- **THEN** the pass sequence is a 4x pass served by `up_kyoto_4x_fp32` followed by a 2x pass served by
  `up_kyoto_2x_fp32`, and both artifacts are required

#### Scenario: Tokyo covers a large scale by repeating one set of weights
- **WHEN** Tokyo is requested at 8x in FP16
- **THEN** the pass sequence is two 4x passes, both served by `up_tokyo_4x_fp16`, and
  `up_tokyo_4x_fp16` is required exactly once

#### Scenario: Every scale in range resolves to at least one pass
- **WHEN** any variant is requested at any scale the system accepts
- **THEN** the pass sequence is never empty, so an operation can never silently degrade to a plain
  resize presented as a successful upscale

### Requirement: Osaka resolves to its three named graphs


An Osaka operation SHALL resolve to three graphs addressed by role - a VAE encoder, a diffusion
transformer, and a VAE decoder - rather than to a sequence of scaling passes. All three SHALL be
required together, since they are stages of one pass.

Only the diffusion transformer SHALL follow the operation's precision. The two VAE halves are
published only in FP16 and SHALL resolve to their FP16 artifacts whatever precision the operation
asks for, because no INT8 build of them exists.

Graphs SHALL be addressed by role rather than by position, so that reordering them cannot silently
exchange the encoder for the decoder.

Each resolved graph SHALL carry the execution-provider settings measured for that graph alongside the
artifact serving it. The two travel together for the same reason the role and the artifact do: a
graph opened under another graph's configuration is not an error anything downstream can detect - it
produces a working session and a slower or wronger result - so the settings must not be something a
caller looks up beside the artifact and could look up wrongly.

#### Scenario: An FP16 Osaka operation requires three FP16 artifacts
- **WHEN** Osaka is requested at FP16
- **THEN** the required artifacts are `up_osaka_fp16`, `up_osaka_vae_encoder_fp16` and
  `up_osaka_vae_decoder_fp16`

#### Scenario: An INT8 Osaka operation quantizes only the transformer
- **WHEN** Osaka is requested at INT8
- **THEN** the required artifacts are `up_osaka_int8`, `up_osaka_vae_encoder_fp16` and
  `up_osaka_vae_decoder_fp16` - the VAE halves staying at FP16

#### Scenario: A role that was never declared is reported rather than ignored
- **WHEN** a graph is requested by a role the variant does not declare
- **THEN** an error naming the role is returned, rather than an absent graph that would fail later

#### Scenario: Each resolved graph carries its own settings
- **WHEN** an Osaka operation is resolved
- **THEN** each of the three graphs reports the settings measured for it together with its artifact
- **AND** the transformer's settings differ from the two VAE halves' where the measurements differ

### Requirement: Every operation reports a run cache tag

The run cache stores results on disk and outlives the process, so it needs a stable textual key
where equality of values alone will not do. Every operation SHALL therefore report a cache tag.

Two operations that produce the same image from the same input SHALL report the same tag, and two
that do not SHALL report different tags. In particular the tag SHALL distinguish everything that
changes the output, including every per-run parameter the operation carries.

The tag SHALL be defined deliberately rather than derived from the serialized form, so that renaming
a field cannot silently invalidate every cached image.

#### Scenario: Two Osaka scales do not collide in the cache
- **WHEN** Osaka at FP16 is run at 2x and then at 6x over the same image
- **THEN** their cache tags differ, and the second run is not served the first one's image

#### Scenario: A repeated operation hits the cache
- **WHEN** Kyoto at 2x in FP32 is run twice over the same image
- **THEN** both runs report the same cache tag and the second may be served from cache

#### Scenario: Every distinguishable operation has a distinct tag
- **WHEN** any two upscale operations that would produce different images are compared
- **THEN** their cache tags differ

### Requirement: An artifact name is built from the native pass scale

An artifact's name SHALL be built from the native scale of the pass it serves, not from the scale that
was requested. The two differ whenever a request falls between native scales, and naming an artifact
from the request would name a file that was never published.

#### Scenario: A requested scale does not appear in an artifact name
- **WHEN** Kyoto is requested at 2.5x in FP32
- **THEN** the artifact required is named `up_kyoto_4x_fp32`, and no artifact named
  `up_kyoto_2.5x_fp32` is ever requested

#### Scenario: An artifact name carries no decimal part for a whole native scale
- **WHEN** any convolutional variant resolves a pass at native scale 4
- **THEN** the artifact name contains `_4x_`, not `_4.0x_`

### Requirement: Models report a display name

Every operation SHALL report a display name for a user interface to show, formed as the variant's
label followed by the precision in upper case in parentheses. A convolutional upscale operation, whose
scale selects its weights, SHALL include that scale in the label; Osaka, whose scale does not, SHALL
NOT - a name fixed at whichever scale happened to load the weights first would be wrong thereafter.

#### Scenario: A convolutional display name carries the scale
- **WHEN** Kyoto at 4x in FP16 is asked for its display name
- **THEN** it is `Kyoto 4x (FP16)`

#### Scenario: Osaka's display name carries no scale
- **WHEN** Osaka at INT8 is asked for its display name
- **THEN** it is `Osaka (INT8)`

### Requirement: The catalogue publishes variants and parameter ranges

The system SHALL publish a catalogue describing, for each family it supports, the variants available,
each variant's display label, the precisions each variant admits, and the parameters each variant takes.

Parameters SHALL be published per variant rather than per family, because two variants of one family may take
different parameters, and a control built from a family-wide list would offer one variant a value its sibling
refuses.

A published parameter SHALL describe what kind of value it is. A parameter a user adjusts through a control SHALL
publish the range that control may offer. A parameter that is not a control - one supplied by the result of another
operation - SHALL say so, so that a front end learns from the catalogue that an operation needs it rather than from
knowledge of its own.

A front end SHALL be able to populate a model chooser and configure a parameter control from the
catalogue alone, without restating any variant name, precision or bound of its own. This is what
keeps a control from offering a value the library will refuse: in the reference implementation the
bounds are literals in the user interface with nothing connecting them to the model code.

#### Scenario: The catalogue lists the upscale variants and their labels
- **WHEN** the catalogue is asked for the upscale family
- **THEN** it reports Tokyo, Kyoto, Saitama and Osaka with their display labels

#### Scenario: The catalogue reports each variant's published precisions
- **WHEN** the catalogue is asked which precisions each upscale variant admits
- **THEN** it reports FP32 and FP16 for Tokyo, Kyoto and Saitama, and FP16 and INT8 for Osaka

#### Scenario: The catalogue reports the scale bounds
- **WHEN** the catalogue is asked for the upscale scale parameter
- **THEN** every upscale variant reports it with a lower bound of 1 and an upper bound of 8, the same bounds that
  constructing a scale enforces

#### Scenario: Two variants of one family publish different parameters
- **WHEN** the catalogue is asked for Athens and for Santorini
- **THEN** Athens reports a fidelity parameter bounded 0 to 1 and Santorini reports none, so a control drawn from the
  catalogue is drawn for one and not the other

### Requirement: Operations survive a round trip through serialization

An operation SHALL be convertible to a serialized form and back, producing an equal operation, so
that a front end can carry it across a process boundary and persist a user's chosen enhancements
between sessions.

A serialized form that names a variant and precision pairing the system does not publish, or a
parameter outside its permitted range, SHALL be refused on the way back in rather than producing an
operation that could not have been constructed directly.

#### Scenario: An operation round-trips unchanged
- **WHEN** any upscale operation is serialized and then deserialized
- **THEN** the result equals the original, including its variant, precision and scale

#### Scenario: A serialized form that could not have been constructed is refused
- **WHEN** a serialized operation pairs Osaka with FP32, or carries a scale of 12
- **THEN** deserialization fails rather than producing an operation that violates the rules above

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

A strength, a bias and a fidelity SHALL each be quantized to three decimal places when accepted, and SHALL compare
exactly thereafter. Two values that are accepted as the same value SHALL be indistinguishable in every later respect,
including in equality, in use as a lookup key, and in the run cache.

In the reference implementation this value reaches the run cache key formatted to three significant
figures, so two values that round alike share a key and the second run is served the first one's
image. Quantizing on acceptance makes the two one value before anything is rendered, so they are one
run rather than two colliding.

#### Scenario: A value finer than three decimals is quantized on acceptance
- **WHEN** a strength of 1.23456 is supplied
- **THEN** it is accepted as 1.235 and the operation reports that value

#### Scenario: Two values that quantize alike are one value
- **WHEN** biases of 0.12341 and 0.12339 are both supplied
- **THEN** both are accepted as 0.123, and operations built from them compare equal and report the same cache tag

#### Scenario: A fidelity finer than three decimals is quantized on acceptance
- **WHEN** fidelities of 0.50001 and 0.49999 are both supplied
- **THEN** both are accepted as 0.5, and Athens operations built from them compare equal and report the same cache tag

#### Scenario: Every accepted value is comparable to itself
- **WHEN** any accepted strength, bias or fidelity is compared with itself
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

### Requirement: A per-run parameter is inside the model's identity

Two operations SHALL be equal exactly when they name the same model at the same precision and carry the same per-run
parameters. A strength, a bias, a scale, a fidelity and a set of faces SHALL each distinguish two operations, so that
a consumer comparing two operations is told whether they describe the same request.

Equality SHALL NOT be used to decide which model to load. What a run needs on disk is the artifacts the operation
reports, and a loaded model SHALL be reused by every operation needing that artifact whatever else those operations
carry - so two scales of one convolutional variant that share a set of weights load them once, and a parameter changed
between runs never loads a second copy of weights already resident.

#### Scenario: Two strengths of one denoise variant are different operations
- **WHEN** Stockholm at FP32 with a strength of 0.5 and Stockholm at FP32 with a strength of 2.5 are compared
- **THEN** they are not equal, because they describe different requests

#### Scenario: Two scales of Osaka are different operations
- **WHEN** Osaka at FP16 at 2x and Osaka at FP16 at 6x are compared
- **THEN** they are not equal

#### Scenario: Two face selections are different operations
- **WHEN** Athens at FP32 over two faces and Athens at FP32 over one face are compared
- **THEN** they are not equal

#### Scenario: Two identical requests are the same operation
- **WHEN** any operation is constructed twice from the same model, precision and parameters
- **THEN** the two compare equal and hash alike

#### Scenario: A parameter change does not reload weights
- **WHEN** two operations naming one model at one precision, differing only in a per-run parameter, are run in turn
- **THEN** the second reuses the weights the first loaded, because what a run loads is decided by the artifacts it
  reports rather than by comparing operations

### Requirement: A per-run parameter is inside the run cache tag

Every enhancement operation SHALL report a run cache tag, and that tag SHALL distinguish everything
that changes the resulting image, including every per-run parameter the operation carries.

Two operations that produce the same image from the same input SHALL report the same tag, and two that
do not SHALL report different tags. As with the upscale families, the tag SHALL be defined deliberately
rather than derived from the serialized form, so that renaming a field cannot silently invalidate every
cached image.

#### Scenario: Two strengths do not collide in the cache
- **WHEN** Stockholm at FP32 is run at a strength of 0.5 and then at 2.5 over the same image
- **THEN** their cache tags differ, and the second run is not served the first one's image

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

### Requirement: The catalogue publishes every family and the parameters each variant takes

The catalogue SHALL describe all eight families - detection and face recovery alongside the six named
before them - reporting for each the variants available, each variant's display label, the precisions
each variant admits, and the parameters each of those variants takes.

A front end SHALL be able to populate every model chooser and configure every parameter control from
the catalogue alone, without restating a variant name, precision or bound of its own. This includes
which model detects faces: a front end that pre-detects reads the detection variant and its precisions
from the catalogue rather than naming that model itself.

A variant that takes no parameter SHALL report that it takes none, rather than being absent or reporting
a parameter with an empty range. Detection SHALL report no parameters, because a run is its variant and precision
alone. Face recovery SHALL report the faces it requires as a parameter of a kind that is not a control, so that
reporting nothing - which is what a family with nothing to supply reports - is not also what a family requiring a
detection run's output reports.

#### Scenario: The catalogue lists every family
- **WHEN** the catalogue is asked what the library offers
- **THEN** it reports eight families - denoise, sharpen, light adjustment, colour balance,
  colorization, upscale, detection and face recovery

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

#### Scenario: Face recovery and colorization are told apart
- **WHEN** the catalogue is asked for the face recovery and colorization families
- **THEN** colorization reports no parameters while both face recovery variants report a parameter naming the faces
  they require, so a front end learns that choosing one means running a detection first

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

### Requirement: A detected face is a box, five landmarks and a confidence at a fixed resolution

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

Coordinates SHALL be expressed in the pixel coordinate space of the image that was detected on, and SHALL be
quantized to two decimal places on acceptance, comparing exactly thereafter. Two faces accepted as the same value
SHALL be indistinguishable in every later respect - equality, use as a lookup key, and the run cache tag of any
operation carrying them. A hundredth of a pixel is finer than any difference two runs of one detector produce and
finer than the resolution the run cache has always folded these coordinates in at; carrying more than that would make
an operation uncomparable without distinguishing anything a consumer can observe.

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

#### Scenario: A coordinate finer than two decimals is quantized on acceptance
- **WHEN** a face is constructed from a box corner at 12.3456
- **THEN** it reports that corner at 12.35

#### Scenario: Two coordinates that quantize alike are one face
- **WHEN** two faces are constructed differing only in coordinates below the second decimal place
- **THEN** they compare equal and hash alike, and operations carrying them report the same cache tag

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

The set SHALL preserve the order it was given, and SHALL be carried as accepted - a face handed in is the face
handed back, so that a front end which stores a user's per-face selection by position is never re-targeted by the
library.

Where the model also takes a fidelity weight, the operation SHALL carry that beside the faces, and where it does not,
the operation SHALL have nowhere to put one.

#### Scenario: An operation carries the faces it was given
- **WHEN** a face-recovery operation is built over two faces
- **THEN** it reports those two faces, in that order

#### Scenario: An empty set of faces is accepted
- **WHEN** a face-recovery operation is built over no faces
- **THEN** the operation is produced, reports an empty set, and is not an error

#### Scenario: A fidelity travels beside the faces where the model takes one
- **WHEN** an Athens operation is built over two faces at a fidelity of 0.5
- **THEN** it reports both the faces and the fidelity

### Requirement: Athens carries a fidelity weight and Santorini carries none

Athens SHALL carry a fidelity weight, a value from 0.0 to 1.0 inclusive, where 1.0 is maximum fidelity to the face
that was detected and lower values permit greater restoration. A fidelity outside that range, or one that is not a
number, SHALL be refused rather than carried.

Santorini SHALL NOT carry a fidelity, and SHALL have nowhere to put one. Its graph takes the image alone, so a value
supplied for it would be a value nothing reads. This SHALL be a pairing that cannot be expressed rather than one that
is accepted and ignored.

A face-recovery operation SHALL report whether it carries a fidelity, and what it is where it does.

The fidelity SHALL be quantized to three decimal places on acceptance, as every other per-run parameter is, and SHALL
compare exactly thereafter.

#### Scenario: Athens is given a fidelity
- **WHEN** an Athens operation is built at a fidelity of 0.5
- **THEN** the operation reports a fidelity of 0.5

#### Scenario: A fidelity outside the range is refused
- **WHEN** a fidelity of -0.1, 1.5 or a value that is not a number is supplied
- **THEN** an error naming the permitted range is returned, and no operation is produced

#### Scenario: Santorini has no fidelity to give
- **WHEN** a Santorini operation is built
- **THEN** there is no fidelity to supply, and the operation reports that it carries none

#### Scenario: Two fidelities do not collide in the cache
- **WHEN** Athens at FP32 over one selection is run at a fidelity of 1.0 and then at 0.5 over the same image
- **THEN** their cache tags differ, and the second run is not served the first one's image

#### Scenario: A repeated fidelity hits the cache
- **WHEN** Athens at FP32 over one selection is run twice at a fidelity of 1.0 over the same image
- **THEN** both runs report the same cache tag and the second may be served from cache

#### Scenario: A fidelity cannot make a Santorini run miss
- **WHEN** two Santorini operations over the same selection at the same precision are compared
- **THEN** they report the same cache tag, with no fidelity able to distinguish them

### Requirement: A set of faces is inside the model's identity and inside its run cache tag

Two face-recovery operations SHALL be equal exactly when they name the same model at the same precision, carry the
same fidelity where they carry one, and carry the same faces in the same order.

The faces SHALL also distinguish two operations in the run cache, because a different selection produces a different
image. The cache tag SHALL therefore fold in the faces the run carries, at a resolution of two decimal places - the
same resolution the coordinates themselves are carried at, and the same the reference implementation folds in.

The tag SHALL be order-sensitive, SHALL be identical across repeated runs of the same selection, and SHALL distinguish
every distinct selection. An operation carrying no faces SHALL report a tag that says so and that no non-empty
selection can produce.

As for every other family, a cache tag SHALL NOT be spelled the way a published artifact name is, and no two
distinguishable operations across all eight families SHALL share one.

#### Scenario: Two face selections do not collide in the cache
- **WHEN** Athens at FP32 over two faces and Athens at FP32 over one face are asked for their cache tags
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

#### Scenario: A difference below the coordinates' resolution is one selection
- **WHEN** two detections of one image differ only in a coordinate below the second decimal place
- **THEN** the two selections are equal and report the same cache tag, so a re-detection that moved a box by a
  hundredth of a pixel neither reads as a different request nor discards the cached image

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

### Requirement: Every model is named by a constructor of its own

The system SHALL offer, for every model it publishes, a constructor naming that model and taking exactly what that
model takes: its precision, and each per-run parameter that model has. A model that takes no parameter SHALL have
nowhere to put one, and a model that takes one SHALL NOT be constructible without it.

A constructor SHALL accept only the precisions its model is published in. A precision a model does not publish SHALL
be unexpressible through that model's constructor rather than refused by it, so that a request which could never be
served is not a request that can be written.

A constructor SHALL produce the carrying value for the path its model runs on - the enhancement carrier for a model
whose result is an image, the analysis carrier for a model whose result is not - so that operations of different
families can be collected together and handed to the library without the caller naming a family or converting
anything.

#### Scenario: A model is named directly
- **WHEN** a caller names a model, its precision and its parameters
- **THEN** an operation is produced, carried in the form the library's entry points take, with no further wrapping or
  conversion required of the caller

#### Scenario: A precision a model does not publish cannot be named
- **WHEN** Osaka, which is published at FP16 and INT8 only, is named at FP32
- **THEN** the request cannot be expressed, rather than being accepted and then refused

#### Scenario: Operations of several families are collected together
- **WHEN** operations of different enhancement families are named and placed in one ordered chain
- **THEN** the chain is accepted as it stands, with no per-element conversion

### Requirement: An operation of any family is one value

The system SHALL provide, for each of its two run paths, a single value carrying an operation of any family that runs
on that path - one for the families whose result is an image and one for the families whose result is not - so that an
operation can be stored, serialized, sent across a process boundary and dispatched on without the holder knowing which
family it belongs to.

Each carrying value SHALL report, for any family it carries: the family, the precision the operation runs at, the
display name a user interface shows, the artifacts that must be on disk before it can run, and the run cache tag its
result is stored under - each identical to what the family's own type reports for the same operation.

Carrying an operation in this form SHALL NOT weaken any guarantee the per-family types give. In
particular a variant of one family SHALL NOT be pairable with a different family, and a family that
takes no per-run parameter SHALL NOT gain one.

The two carriers SHALL NOT be interchangeable. An operation whose result is an image SHALL NOT be presentable to the
path for operations whose result is not, and the reverse SHALL equally not be presentable - and neither SHALL be a
refusal reported at run time.

#### Scenario: Every family can be carried
- **WHEN** an operation of each of the eight families is placed in the carrier for its path
- **THEN** each reports the same family, precision, display name, required artifacts and cache tag as
  the family's own type reports for that same operation

#### Scenario: A codename cannot cross families
- **WHEN** an operation is built for a family
- **THEN** only that family's variants can be supplied, and pairing another family's codename with it
  is not expressible

#### Scenario: A carrier cannot be handed to the other path
- **WHEN** an operation whose result is an image is named for the path that produces no image, or an operation whose
  result is not an image is placed in an ordered chain
- **THEN** the request cannot be expressed, rather than being accepted and then refused

#### Scenario: A carried operation survives a round trip
- **WHEN** an operation is serialized from its carrier and deserialized back
- **THEN** the result equals the original, including its family, its variant, its precision and any
  per-run parameter it carries

### Requirement: A published catalogue row names the operation it builds

The system SHALL provide, as public API, the inverse of the catalogue: given a family, a variant
codename and a precision as the catalogue publishes them, it SHALL produce the variant that pairing names, or report
that the pairing names none.

A pairing the catalogue does not publish SHALL report that it names no variant rather than producing one that
resolves to an artifact which was never published.

The variant SHALL then name the constructor that builds an operation from it, and the parameters that constructor
requires SHALL be exactly those the catalogue published for that variant - so a front end that read the catalogue has
already been told what it must collect from the user, and a parameter a variant does not take SHALL have nowhere to
be supplied.

This inverse SHALL be available to any consumer of the library, not only to its own tests, so that a
front end populating a chooser from the catalogue can turn the user's selection back into an
operation without restating any model vocabulary of its own.

#### Scenario: Every published row builds
- **WHEN** every variant and precision the catalogue publishes is resolved and then built with the parameters the
  catalogue published for it
- **THEN** each produces an operation, and each such operation reports at least one required artifact

#### Scenario: A variant's constructor takes exactly what the catalogue published
- **WHEN** a variant publishing a parameter and a variant publishing none are each built
- **THEN** the first cannot be built without that parameter and the second has nowhere to supply one

#### Scenario: An unpublished pairing names nothing
- **WHEN** a variant is paired with a precision the catalogue does not publish for it - Osaka at FP32,
  or any single-graph variant at INT8
- **THEN** the inverse reports that the pairing names no variant, and no operation is produced

#### Scenario: An unknown codename names nothing
- **WHEN** a codename no family publishes is supplied
- **THEN** the inverse reports that the pairing names no variant

### Requirement: A family added later cannot be silently skipped

The system SHALL name every family it offers in one place, and everything that iterates families -
the catalogue, the cross-family cache-tag checks, and the shared operation value - SHALL be checked
against that one list rather than against a transcribed copy of it, so that a family omitted from any
of them fails to compile or fails a check that names it. Where a list of families cannot be generated
from that one list - the arms of a type, which no list can write - the check SHALL be what holds it,
and a family the type cannot carry SHALL still be a compile error somewhere that reaches every
family.

A family added to the library and omitted from the catalogue SHALL cause a failure that names the
omission, rather than passing because both the expectation and the published list were missing it.

#### Scenario: The catalogue publishes every family the library names
- **WHEN** the set of families the catalogue publishes is compared against the set the library names
- **THEN** the two are equal, and a family present in one and absent from the other fails the check
  naming that family

#### Scenario: Completeness does not rest on a counted total
- **WHEN** a published variant row is removed
- **THEN** the checks fail by naming what is missing, rather than by a total that no longer matches a
  transcribed number
