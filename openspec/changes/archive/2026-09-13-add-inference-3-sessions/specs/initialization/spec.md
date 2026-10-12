## ADDED Requirements

### Requirement: Compiled provider artifacts are discarded when the runtime version changes

Initialization SHALL discard everything the execution providers have compiled from model weights,
together with the measurements shared across models, whenever the ONNX Runtime version in force
differs from the one those artifacts were compiled under. What a provider compiles is valid only for
the runtime that built it, and a stale artifact is at best a wasted rebuild and at worst loaded
against a runtime that cannot use it.

The decision SHALL be made against a durable record of the version the artifacts were compiled under,
rather than inferred from the runtime having just been replaced: an application that stopped between
replacing the runtime and discarding the artifacts would otherwise find a matching runtime on its
next start, skip the reinstall, and never discard them.

Where that record already names the version in force, initialization SHALL discard nothing, so this
costs a launch with nothing to do no work beyond the check.

Discarding them SHALL NOT remove any model's weights, and SHALL NOT fail initialization on a machine
that has never compiled anything.

#### Scenario: The pinned runtime version is bumped

- **WHEN** initialization runs and the compiled artifacts were compiled under a different runtime
  version
- **THEN** every compiled artifact and the shared measurements are discarded
- **AND** the version now in force is recorded

#### Scenario: A launch with the same runtime

- **WHEN** initialization runs and the recorded version is the one in force
- **THEN** nothing is discarded

#### Scenario: A machine that has never run a model

- **WHEN** initialization runs on a machine where nothing has ever been compiled
- **THEN** the version in force is recorded
- **AND** initialization completes

#### Scenario: Model weights are not touched

- **WHEN** the compiled artifacts are discarded because the runtime version changed
- **THEN** every installed model's files remain in place
- **AND** no model is re-downloaded on their account
