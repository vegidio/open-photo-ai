## MODIFIED Requirements

### Requirement: The user can choose which enhancements Autopilot may suggest

The settings surface SHALL let the user choose, for each enhancement the application offers, whether Autopilot may
suggest it. The choice SHALL be shown beside that enhancement's default model, so each enhancement is listed once with
everything the surface holds about it.

An enhancement Autopilot may not suggest SHALL still be available to add by hand. The choice governs what Autopilot
proposes, not what the application can do.

Where the user has expressed no preference, Autopilot SHALL be allowed to suggest every enhancement, so that Autopilot
behaves as it did before the choice existed until the user opts an enhancement out. An enhancement the application
offers that the user has never been asked about - one added in a later version - SHALL likewise start allowed.

The choice SHALL be remembered across restarts and be a draft until saved, like every other preference the surface
holds. A remembered choice that cannot be read SHALL be treated as allowing every enhancement, by the rule every other
remembered preference already follows.

Whenever the application analyses an image for Autopilot, it SHALL ask for only the enhancements the user allows.
Switching every enhancement off SHALL leave Autopilot suggesting nothing, and SHALL NOT be an error. The choice in
force is the saved one at the moment an analysis is asked for. Saving a different choice SHALL NOT stop or change an
analysis already under way, and SHALL NOT analyse again a photograph that has already been analysed.

#### Scenario: A user who has never chosen

- **WHEN** the user opens the Enhancements page without having changed any Autopilot choice
- **THEN** every enhancement is shown as one Autopilot may suggest

#### Scenario: Opting an enhancement out

- **WHEN** the user switches Autopilot off for colorization and saves
- **THEN** colorization is recorded as an enhancement Autopilot may not suggest
- **AND** colorization can still be added by hand
- **AND** the choice is still in effect after a restart

#### Scenario: A choice made but not saved

- **WHEN** the user switches Autopilot off for an enhancement and cancels
- **THEN** that enhancement is still recorded as one Autopilot may suggest

#### Scenario: A remembered choice that cannot be read

- **WHEN** the application starts and the remembered Autopilot choice is not something it can read
- **THEN** every enhancement reads as one Autopilot may suggest
- **AND** every other remembered preference is left as it was

#### Scenario: The choice governs the next analysis

- **WHEN** the user switches colorization off for Autopilot and saves, then opens a grayscale photograph with
  Autopilot on
- **THEN** the photograph is analysed and no colorization is added to it

#### Scenario: Every enhancement switched off

- **WHEN** the user switches every enhancement off for Autopilot and saves, then opens a photograph with Autopilot on
- **THEN** nothing is added to it and nothing is reported
