## ADDED Requirements

### Requirement: A run is refused rather than served wrongly where an operation cannot be served

Where the system has no pipeline for an operation in the chain, the run SHALL fail with an error
naming that operation, and SHALL produce no image.

It SHALL NOT fall back to returning the image unchanged, nor to any result produced without the model
the operation names: a caller handed an unenhanced image with no error has no way to tell it from an
enhancement, and would show it to a user as one. This is the same refusal the tiling of a zero-area
image makes, for the same reason.

A refusal SHALL happen before any model is installed or opened, so that a chain naming an operation
that cannot be served does not first transfer gigabytes on its behalf.

Every enhancement family the system publishes has a pipeline for every variant it publishes. The
system SHALL NOT publish an answer saying that an operation cannot be run **yet**. No operation
produces that answer, and an answer that nothing produces tells a caller nothing it can act on. Where
a later version publishes a model before its pipeline exists, that version adds the answer back.

Where the refusal is because the operation's result is not an image, it SHALL say so. That refusal is
permanent. A face-detection run produces a set of faces, so there is no enhanced picture for it to
contribute to a chain, and there never will be. The refusal SHALL name the path such an operation is
run on instead, so that a caller that reached the chain by mistake is told where the operation
belongs. A caller SHALL NOT have to read the error's text to recognise this refusal.

#### Scenario: Every published enhancement is served

- **WHEN** a chain names any variant of any enhancement family the system publishes, at any precision
  it is published in
- **THEN** the run is not refused for want of a pipeline

#### Scenario: An operation whose result is not an image

- **WHEN** a chain names an operation whose result is a set of faces rather than an image
- **THEN** the run fails with an error naming that operation
- **AND** the refusal says the operation's result is not an image
- **AND** no image is produced and no model is transferred

#### Scenario: An unservable operation later in the chain

- **WHEN** a chain's first operation can be served and its second cannot
- **THEN** the run fails before the first operation's model is installed or opened
- **AND** no image is produced

## REMOVED Requirements

### Requirement: A run is refused rather than served wrongly where no pipeline exists

**Reason**: Its answer that an operation "cannot be run yet", the two scenarios that describe that answer, and the
rule against a third answer distinguishing a family with no pipeline from a variant with none all describe a state
that no longer exists. Once colorization runs, every published variant of every enhancement family has a pipeline, and
nothing produces the deferral. A MODIFIED block cannot drop scenarios, so the requirement is replaced whole by "A run is
refused rather than served wrongly where an operation cannot be served". That requirement keeps every rule that still
holds: the refusal names the operation, there is no fallback to the unchanged image, the refusal comes before anything
installs, and the result-is-not-an-image answer stays.

**Migration**: A caller that matched `UnsupportedReason::FamilyNotImplemented` removes that arm. The enum is
`#[non_exhaustive]`, so its catch-all arm already covers every remaining reason. A front end that greyed out an
enhancement on that reason has nothing to grey out.
