## Purpose

Names the AI models the application can run, validates the parameters each one accepts, and resolves
a requested operation to the model artifacts it needs, so that a front end can offer a model and the
inference pipeline can load it without either restating what the catalogue already knows.

## ADDED Requirements

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

### Requirement: An operation is its own identity

An operation SHALL be a value that can be compared for equality and used as a lookup key directly. It
SHALL NOT be reduced to a string in order to be compared, looked up or dispatched on.

Two operations SHALL be equal exactly when they would load the same model and produce the same image
from the same input. This is what a model registry keys on, so that requesting the same model twice
reuses what is already loaded.

The scale SHALL distinguish two convolutional operations, because each scale is served by a different
set of weights. The scale SHALL NOT distinguish two Osaka operations, because one set of weights
serves every scale: a scale change must not read as a different model and force those weights - which
are the largest the application loads - to be loaded again.

#### Scenario: Two identical requests are the same operation
- **WHEN** Kyoto at 2.5x in FP32 is constructed twice
- **THEN** the two compare equal and hash alike, so a registry keyed on them loads one model

#### Scenario: Two scales of a convolutional variant are different operations
- **WHEN** Kyoto at 2x in FP32 and Kyoto at 4x in FP32 are compared
- **THEN** they are not equal, because different weights serve them

#### Scenario: Two scales of Osaka are the same operation
- **WHEN** Osaka at 2x in FP16 and Osaka at 6x in FP16 are compared
- **THEN** they are equal, so a registry keyed on them loads its weights once

#### Scenario: Two precisions are different operations
- **WHEN** Kyoto at 4x in FP32 and Kyoto at 4x in FP16 are compared
- **THEN** they are not equal, because different artifacts serve them

### Requirement: A scale compares exactly

A scale SHALL be quantized to three decimal places when it is accepted, and SHALL compare exactly
thereafter. Two scales that are accepted as the same value SHALL be indistinguishable in every later
respect - equality, use as a lookup key, and the image cache.

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

### Requirement: Every operation reports an image cache tag

The image cache stores results on disk and outlives the process, so it needs a stable textual key
where equality of values alone will not do. Every operation SHALL therefore report a cache tag.

Two operations that produce the same image from the same input SHALL report the same tag, and two
that do not SHALL report different tags. In particular the tag SHALL distinguish everything that
changes the output, including a parameter that does not distinguish the operation itself - Osaka's
scale is such a parameter, since one set of weights serves every scale.

The tag SHALL be defined deliberately rather than derived from the serialized form, so that renaming
a field cannot silently invalidate every cached image.

#### Scenario: Two Osaka scales do not collide in the cache
- **WHEN** Osaka at FP16 is run at 2x and then at 6x over the same image
- **THEN** the two operations are equal but their cache tags differ, and the second run is not served
  the first one's image

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
each variant's display label, the precisions each variant admits, and the permitted range of each
parameter the family takes.

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
- **THEN** it reports a lower bound of 1 and an upper bound of 8, the same bounds that constructing a
  scale enforces

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
