## ADDED Requirements

### Requirement: The user can choose what is drawn behind the image

The settings surface SHALL carry a choice of what the preview canvas draws behind the image, offering exactly two
surfaces: a field of drifting particles and a grid of dots. The choice SHALL be presented as a set of named options
of which exactly one is chosen at a time, so that both surfaces and which of them is in effect are visible without
opening anything.

The two SHALL be exclusive rather than cumulative. Choosing one SHALL replace the other entirely; neither is ever
drawn over the other, and nothing else about the canvas changes with the choice.

An application that has never been told SHALL draw the particle field. This is a preference with no wrong answer -
both surfaces are decorative, and neither affects what the application can do - so it is started at the one the
design's own screens open on rather than left unset.

The choice SHALL be a draft until it is saved, like every other preference the surface holds. Nothing outside the
surface observes a draft, so the canvas behind the dialog SHALL keep the surface it had until the user saves, and
cancelling SHALL leave it unchanged throughout. This is the same rule the chosen language already follows and for
the same reason: a background applied as the control was touched could not be taken back by Cancel.

A remembered choice naming neither surface SHALL read as the particle field, by the rule every other remembered
preference already follows.

#### Scenario: Choosing the dotted surface

- **WHEN** the user chooses the dotted surface and saves
- **THEN** the preview canvas draws a grid of dots and no particles
- **AND** the choice is still in effect after a restart

#### Scenario: An application that has never been told

- **WHEN** the application is started for the first time and the user has chosen no background
- **THEN** the preview canvas draws the particle field
- **AND** the settings surface shows the particle field as the chosen option

#### Scenario: A background picked but not saved is never applied

- **WHEN** the user picks the other background and then cancels
- **THEN** the preview canvas draws the same surface throughout, before and after
- **AND** the settings surface shows the original choice again

#### Scenario: A remembered choice naming neither surface

- **WHEN** the application starts and the remembered background names neither surface
- **THEN** the preview canvas draws the particle field
- **AND** every other remembered preference is left as it was
