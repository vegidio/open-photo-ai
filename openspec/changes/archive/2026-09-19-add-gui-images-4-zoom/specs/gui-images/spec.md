## ADDED Requirements

### Requirement: Each open image is looked at at its own zoom and position

Every open image SHALL carry how closely it is being looked at - a magnification and which part of
the photograph is in view - independently of every other open image.

An image SHALL begin fitted: drawn whole, in proportion, and centred in the space available to it.

The magnification SHALL be kept between fitting the image and eight times that size. There is no
zooming out past the whole photograph, because the space around it is not part of the photograph, and
no zooming in past eight times, which is the reference implementation's own ceiling.

How far into a photograph the user has gone is a property of that photograph and not of the window:
it is what they were examining, and coming back to it is coming back to what they were looking at.

#### Scenario: A newly opened image

- **WHEN** an image is opened and becomes the current one
- **THEN** it is drawn whole, in proportion and centred, at the size that fits

#### Scenario: Returning to an image

- **WHEN** the user magnifies one image, makes another the current one, and then returns to the first
- **THEN** the first image is shown at the same magnification and the same part of it as when they
  left it

#### Scenario: One image's view is its own

- **WHEN** the user magnifies the current image
- **THEN** no other open image's magnification or position changes

#### Scenario: Below the fitted size

- **WHEN** the user attempts to magnify the current image to less than the size that fits
- **THEN** it stays at the size that fits

#### Scenario: Above eight times

- **WHEN** the user attempts to magnify the current image beyond eight times the size that fits
- **THEN** it stays at eight times

#### Scenario: A new session

- **WHEN** the user magnifies an image, restarts the application and opens the same file again
- **THEN** the image is fitted again, as a newly opened image is
