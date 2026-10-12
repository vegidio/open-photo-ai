## ADDED Requirements

### Requirement: Replacing a model's weights invalidates what was compiled from them

Where a model's files are replaced, the application SHALL also remove whatever an execution provider
compiled from the previous ones. At best a stale compiled artifact is wasted disk; at worst it is
reused against weights it was never built for, which produces an image nothing reports as wrong.

The removal SHALL be part of the same replacement that removes the previous files, rather than
something a later step is trusted to do, so that an interruption cannot leave new weights beside an
old compiled artifact.

An install that is already current, and so transfers nothing, SHALL leave the compiled artifacts
alone - they were compiled from exactly those weights, and discarding them would cost a rebuild of
minutes for no reason.

Installing or replacing one model SHALL NOT remove what was compiled from any other model, and SHALL
NOT remove the timing measurements shared across models.

#### Scenario: A model whose published files have changed

- **WHEN** a model is installed whose published file set differs from the one previously recorded
- **THEN** whatever a provider compiled from the previous weights is removed
- **AND** the new files are installed

#### Scenario: A model that is already installed and intact

- **WHEN** an install runs for a model whose recorded files are all present at their recorded sizes
- **THEN** whatever a provider compiled from those weights is left in place

#### Scenario: Replacing one model beside another

- **WHEN** one model is replaced while another model's compiled artifacts are present
- **THEN** the other model's compiled artifacts remain in place
- **AND** the measurements shared across models remain in place
