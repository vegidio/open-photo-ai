## MODIFIED Requirements

### Requirement: Each open image carries its own enhancements

Every open image SHALL have its own list of enhancements, and the interface SHALL show the list belonging to the
image currently being looked at.

Moving between images SHALL leave each list as its owner left it. Closing an image SHALL discard the list belonging to
it without touching any other. No list SHALL survive a restart of the application: a stack is a statement about the
photograph on screen now, not a setting.

An image opened again after being closed SHALL start with nothing kept from before: no enhancement it carried when it
was closed SHALL come back. With Autopilot on it is then analysed like any photograph that has never had a list, and
whatever that analysis suggests is added (see `gui-autopilot`). With Autopilot off it SHALL start with no
enhancements.

#### Scenario: Two images with different enhancements

- **WHEN** an enhancement is added to one open image and the user moves to another
- **THEN** the second image shows its own enhancements, and returning to the first shows the ones it was given

#### Scenario: An image is closed

- **WHEN** an image carrying enhancements is closed
- **THEN** its enhancements are discarded, and every other open image keeps its own

#### Scenario: An image opened again with Autopilot off

- **WHEN** an image carrying enhancements is closed and the same file is opened again with Autopilot off
- **THEN** it shows no enhancements

#### Scenario: An image opened again with Autopilot on

- **WHEN** an image carrying a light adjustment the user set to 80% is closed and the same file is opened again with
  Autopilot on
- **THEN** it shows exactly what the new analysis suggested, and a light adjustment among those carries 50% rather
  than 80%

#### Scenario: The application is restarted

- **WHEN** the application is closed while an image carries enhancements and is started again
- **THEN** no enhancements are restored
