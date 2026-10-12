## ADDED Requirements

### Requirement: The answer to the TensorRT question sets the processor, and is remembered apart from the preferences

Answering the TensorRT question (see `gui-setup`) SHALL set the processor preference: **Yes** to the automatic choice,
**No** to CUDA. The answer SHALL be applied and recorded at once, exactly as a processor saved from the settings
surface is - there is no draft to save or cancel, because the question is not the settings surface.

That the question was answered SHALL be remembered across restarts, apart from the preferences the settings surface
holds. It is not a preference: the settings surface SHALL NOT show it, and resetting a page to its defaults SHALL NOT
clear it, since doing so would ask a user who has already answered to answer again.

An answer recorded by the application this one replaces, under the same stored preferences, SHALL be read as the
question having been answered, so a user upgrading is not asked a second time.

#### Scenario: Answering Yes

- **WHEN** the user answers Yes
- **THEN** the processor preference is the automatic choice
- **AND** the Performance page shows the automatic choice selected

#### Scenario: Answering No

- **WHEN** the user answers No
- **THEN** the processor preference is CUDA
- **AND** the Performance page shows CUDA selected

#### Scenario: The answer survives a restart

- **WHEN** the user answers the question and restarts the application
- **THEN** the processor preference is the one the answer set
- **AND** the question is not asked again

#### Scenario: Resetting the Performance page after answering

- **WHEN** the user answered No, then resets the Performance page and saves
- **THEN** the processor preference is the automatic choice
- **AND** the question is not asked on the next launch

#### Scenario: A user upgrading from the application this one replaces

- **WHEN** the application starts with stored preferences in which that application recorded the question as answered
- **THEN** the question is not asked
