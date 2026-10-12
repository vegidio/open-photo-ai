## ADDED Requirements
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

## MODIFIED Requirements
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

## REMOVED Requirements
### Requirement: A per-run parameter is outside the model's identity

**Reason**: The property existed to serve a model registry keyed on an operation value. No such registry was ever
built: a loaded model is keyed on the artifact it was opened from and the execution provider it was opened on, so two
operations needing one graph already share a session whatever their parameters and however they compare. Keeping the
property meant two different requests comparing equal, which is wrong for the consumer that actually compares them -
a front end asking whether a user has changed anything.

**Migration**: Equality now means "the same request"; see the added requirement *A per-run parameter is inside the
model's identity*. A consumer that needs "would produce the same image" continues to use the run cache tag, which is
unchanged in role and still reports identical tags for identical requests.

### Requirement: A set of faces is outside the model's identity and inside its run cache tag

**Reason**: The same property as above, stated for the one family whose parameter is a set rather than a number, and
removed for the same reason: no registry keys on an operation, so nothing was gained by two selections comparing
equal, and a front end comparing two face-recovery operations was told that changing the selection changed nothing.

**Migration**: See the added requirement *A set of faces is inside the model's identity and inside its run cache
tag*. The cache-tag half is carried over unchanged, including its two-decimal resolution, its order sensitivity and
its distinct tag for an empty selection.

### Requirement: An operation is its own identity

**Reason**: The property that two operations compare equal when they would load the same model existed to serve a model registry keyed on an operation value. No such registry was ever built: a loaded model is keyed on the artifact it was opened from and the execution provider it was opened on. Keeping the property meant two different requests comparing equal, which is wrong for the consumer that actually compares them.

**Migration**: Equality now means "the same request"; see the added requirement *An operation is the request it describes*, which keeps unchanged the part of this that still holds - that an operation is a value rather than a string. A consumer that needs "would produce the same image" continues to use the run cache tag.

### Requirement: A detected face is a box, five landmarks and a confidence

**Reason**: The requirement stated that coordinates are not rounded or rescaled on acceptance. Once a face is part of an operation's value equality, a coordinate carried at full float precision makes that operation uncomparable without distinguishing anything a consumer can observe - the run cache has always folded these coordinates in at two decimal places.

**Migration**: See the added requirement *A detected face is a box, five landmarks and a confidence at a fixed resolution*. Everything else is carried over unchanged: the five landmarks, their fixed order, and the confidence range.

### Requirement: The catalogue publishes every family

**Reason**: The requirement stated that detection and face recovery each report no parameters. Face recovery cannot be constructed without a set of faces, so reporting nothing made its catalogue entry byte-identical to colorization's, which genuinely takes nothing - leaving a front end to know from elsewhere that choosing Athens means running a detection first.

**Migration**: See the added requirement *The catalogue publishes every family and the parameters each variant takes*. Detection still reports no parameters; face recovery now reports the faces it requires, as a parameter whose kind says it is supplied by another operation rather than by a control.

### Requirement: A published catalogue row can be turned back into a runnable operation

**Reason**: The requirement described two entry points - one for families taking a per-run parameter and one for families taking none - with the catalogue saying which a row needed. With a constructor per model, each taking exactly what that model takes, there is nothing for a second entry point to be the other of, and no pairing left for either to refuse.

**Migration**: See the added requirement *A published catalogue row names the operation it builds*. The inverse still resolves a codename and precision to a variant, or reports that the pairing names none; what follows it is the model's own constructor rather than a choice between two.
