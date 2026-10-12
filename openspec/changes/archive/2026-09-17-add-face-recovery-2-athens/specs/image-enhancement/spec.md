## MODIFIED Requirements

### Requirement: A run is refused rather than served wrongly where no pipeline exists

Where the system has no pipeline for an operation in the chain, the run SHALL fail with an error
naming that operation, and SHALL produce no image.

It SHALL NOT fall back to returning the image unchanged, nor to any result produced without the model
the operation names: a caller handed an unenhanced image with no error has no way to tell it from an
enhancement, and would show it to a user as one. This is the same refusal the tiling of a zero-area
image makes, for the same reason.

A refusal SHALL happen before any model is installed or opened, so that a chain naming an operation
that cannot be served does not first transfer gigabytes on its behalf.

The refusal SHALL say **which** of two different things is true of the operation, because a caller can
act on one of them and not on the other:

- **This operation cannot be run yet.** A deferral. The operation would produce an image, the system
  will run it in a later version, and a front end can reasonably offer it greyed out or say "not yet".
  It is a fact about the **operation**, not about its family: a family may run one of its variants and
  refuse another while the second one's pipeline is being written, and a caller told only that the
  family cannot run would grey out a model that works.
- **The operation's result is not an image.** Permanent, and not a deferral at all. A face-detection
  run produces a set of faces, so there is no enhanced picture for it to contribute to a chain and
  there never will be. The refusal SHALL name the path such an operation is run on instead, so that a
  caller that reached the chain by mistake is told where the operation belongs rather than being told
  to wait for a version that will never add it.

A caller SHALL NOT have to read the error's text to tell the two apart.

The system SHALL NOT publish a third answer distinguishing a family with no pipeline from a variant
with none. Both are the same fact to a caller — this one cannot be run yet — and the distinction
between them exists only while a family is partly written, which is a state every family is on its way
out of.

#### Scenario: An operation whose family has no pipeline

- **WHEN** a chain names an operation from a family the system cannot yet run
- **THEN** the run fails with an error naming that operation
- **AND** the refusal says the operation cannot be run yet
- **AND** no image is produced and no model is transferred

#### Scenario: An operation whose family runs its other variant

- **WHEN** a chain names a variant whose pipeline is not written, of a family whose other variant runs
- **THEN** the run fails with an error naming that operation
- **AND** the refusal says the operation cannot be run yet, exactly as it does for a family with no pipeline at all
- **AND** no image is produced and no model is transferred

#### Scenario: An operation whose result is not an image

- **WHEN** a chain names an operation whose result is a set of faces rather than an image
- **THEN** the run fails with an error naming that operation
- **AND** the refusal says the operation's result is not an image, and is distinguishable from an operation whose pipeline is merely unwritten
- **AND** no image is produced and no model is transferred

#### Scenario: An unservable operation later in the chain

- **WHEN** a chain's first operation can be served and its second cannot
- **THEN** the run fails before the first operation's model is installed or opened
- **AND** no image is produced
