## ADDED Requirements

### Requirement: The preview canvas draws the chosen background behind everything else

The preview canvas SHALL draw the background the user has chosen, and SHALL draw it behind everything else the
canvas shows. The invitation to supply an image and the photograph itself are both drawn over it, whichever surface
is in effect.

The background SHALL be drawn in both of the canvas's states - while no image is open and while one is. It is the
surface the canvas is made of rather than something the empty canvas carries, so loading a photograph SHALL NOT
change it.

A saved choice SHALL take effect without restarting the application.

#### Scenario: The canvas with no image open

- **WHEN** the application window is shown with no image open
- **THEN** the canvas draws the chosen background
- **AND** the invitation to drag and drop images is legible over it

#### Scenario: The canvas with an image open

- **WHEN** an image is opened
- **THEN** the canvas still draws the chosen background
- **AND** the photograph is drawn over it

#### Scenario: Changing the background while the application is running

- **WHEN** the user chooses the other background and saves
- **THEN** the preview canvas draws the newly chosen surface without the application being restarted

### Requirement: The particle background is decorative and costs nothing while unwatched

While the particle field is the chosen background, it SHALL be decorative only. It SHALL NOT respond to the pointer,
SHALL NOT accept any interaction, and SHALL NOT be announced to assistive technology or reachable by keyboard - it
carries no information, and everything the canvas has to say is said by what is drawn over it.

The field SHALL stop animating while the application window does not have focus, and SHALL resume when it regains
focus. An unfocused window is the normal state of a desktop application that is not being used, and an animation
nobody is looking at is cost with nothing bought by it.

Stopping and resuming SHALL be invisible in effect: the field SHALL still be drawn while the window is unfocused,
and regaining focus SHALL NOT restart it from an empty canvas.

#### Scenario: The pointer moves across the particle field

- **WHEN** the user moves the pointer across the canvas while the particle field is drawn
- **THEN** the particles are unaffected by where the pointer is
- **AND** nothing under the pointer becomes interactive

#### Scenario: The window loses and regains focus

- **WHEN** the application window loses focus while the particle field is drawn
- **THEN** the field stops animating but is still drawn
- **AND** it resumes animating from where it stopped when the window regains focus

#### Scenario: The particle field and assistive technology

- **WHEN** the canvas is examined by assistive technology while the particle field is drawn
- **THEN** the field is not announced, and the invitation or the photograph drawn over it is
