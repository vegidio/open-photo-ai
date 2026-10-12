## ADDED Requirements

### Requirement: A colorization is asked for by its model alone

The interface SHALL be able to ask for a colorization by naming its model and the precision to run it at, and SHALL NOT
be required to send any value with it: a colorization takes none, so a value sent for it would describe a choice the
user cannot make.

#### Scenario: Every published colorization model can be run

- **WHEN** a chain names any colorization model the library publishes, at any precision it publishes that model at
- **THEN** the chain runs that model at that precision

#### Scenario: A model of another enhancement

- **WHEN** a colorization names a model the application publishes only for another enhancement
- **THEN** the operation is refused before anything is fetched

### Requirement: A colorization in the list reports its model

A colorization's row SHALL report the model it will use and the quality that model is being run at, and nothing else:
there is no amount to report.

#### Scenario: A colorization in the list

- **WHEN** a colorization carrying Delhi at the higher quality is in the list
- **THEN** its row reads `Delhi, HD`

#### Scenario: The line follows the model

- **WHEN** the colorization's model is changed to Jaipur at the lower quality
- **THEN** its row reads `Jaipur, SD`

### Requirement: A colorization's model can be changed from its options

A colorization's options SHALL offer every colorization model the application publishes, each at every quality it is
published at, with the one in use shown as chosen, and SHALL offer no other control.

#### Scenario: Only the model is offered

- **WHEN** a colorization's options are opened
- **THEN** they offer Delhi, Mumbai and Jaipur at each of their qualities, and no amount, scale or other control

#### Scenario: Choosing another model

- **WHEN** the user picks Jaipur at the higher quality in a colorization's options
- **THEN** the colorization carries Jaipur at that quality, and the preview is re-run once

#### Scenario: Opening the options

- **WHEN** a colorization's options are opened and closed without a model being chosen
- **THEN** the colorization is unchanged and nothing is re-run

### Requirement: An operation the application does not publish is refused before anything is fetched

A request naming a model, or a model at a precision, that the application does not publish SHALL be refused, and the
refusal SHALL name what could not be served and where in the chain it is.

The whole chain SHALL be judged before any of it begins, so a chain whose later operation cannot be served does not
first spend minutes fetching a model for its earlier one.

What the application publishes SHALL be decided by the library alone. The application SHALL NOT keep its own record of
which models exist or which precisions each is published at, so a model the library publishes SHALL be runnable rather
than refused by a second record that has not been kept in step.

#### Scenario: A model and precision that were never published together

- **WHEN** a chain names a model at a precision it is not published at
- **THEN** the request is refused rather than a model being fetched that does not exist

#### Scenario: A later operation that cannot be served

- **WHEN** a chain's first operation can be served and its second names a model the application does not publish
- **THEN** the request is refused, the refusal names the second operation, and no model is fetched for either

#### Scenario: A model the library publishes

- **WHEN** a chain names any model the library publishes, at a precision it publishes that model at
- **THEN** the chain runs, whether or not the application has been changed since that model was added

## MODIFIED Requirements

### Requirement: An enhancement is added from a menu of what can be run

While an image is open, the interface SHALL offer a control that lists the enhancements this
application can run, and choosing one SHALL add it to that image's list.

The menu SHALL offer every enhancement the application presents, and SHALL offer each of them once:
an enhancement already in the list SHALL be shown as unavailable rather than removed from the menu,
so the menu keeps its shape as a list is built up.

An enhancement SHALL be added carrying the model the user chose as that enhancement's default, and an
upscale SHALL additionally be added at a scale suited to the image's size rather than at a fixed one -
a small photograph can afford more enlargement than a large one. A light adjustment and a colour balance
SHALL each be added at a bias of 50%. A denoise and a sharpen SHALL each be added at a strength of 100%,
the model's own output. A colorization SHALL be added carrying its model alone.

Adding an enhancement SHALL NOT open its options: the user opens them from its row.

The list SHALL be shown in the order the enhancements are applied, whatever order they were added in.

#### Scenario: Adding an enhancement

- **WHEN** the user chooses an enhancement from the menu
- **THEN** it is added to the current image's list, carrying that enhancement's default model, and its options stay
  closed

#### Scenario: Every enhancement in the menu

- **WHEN** the menu is opened
- **THEN** it offers Denoise, Face Recovery, Colorization, Light Adjustment, Color Balance, Sharpen and Upscale, in that
  order

#### Scenario: An enhancement already added

- **WHEN** the menu is opened while an enhancement is already in the list
- **THEN** that entry is shown as unavailable and cannot be chosen a second time

#### Scenario: The scale a new upscale arrives at

- **WHEN** an upscale is added to a small image and to a large one
- **THEN** the smaller image's upscale arrives at a larger scale than the larger image's

#### Scenario: The bias a new light adjustment arrives at

- **WHEN** a light adjustment is added
- **THEN** it carries a bias of 50%

#### Scenario: The bias a new colour balance arrives at

- **WHEN** a colour balance is added
- **THEN** it carries a bias of 50%

#### Scenario: The strength a new denoise arrives at

- **WHEN** a denoise is added
- **THEN** it carries a strength of 100%

#### Scenario: The strength a new sharpen arrives at

- **WHEN** a sharpen is added
- **THEN** it carries a strength of 100%

#### Scenario: The model a new colorization arrives with

- **WHEN** a colorization is added by a user who has not chosen a default colorization model
- **THEN** it carries Delhi

#### Scenario: Added between the enhancements already there

- **WHEN** a light adjustment is added to a list holding a face recovery and an upscale
- **THEN** it is shown between the two, whatever order the three were added in

#### Scenario: Colour balance in pipeline order

- **WHEN** a colour balance is added to a list holding a light adjustment and an upscale
- **THEN** it is shown between the two, whatever order the three were added in

#### Scenario: Denoise first in pipeline order

- **WHEN** a denoise is added to a list holding a face recovery and an upscale
- **THEN** it is shown above both, whatever order the three were added in

#### Scenario: Sharpen in pipeline order

- **WHEN** a sharpen is added to a list holding a colour balance and an upscale
- **THEN** it is shown between the two, whatever order the three were added in

#### Scenario: Colorization in pipeline order

- **WHEN** a colorization is added to a list holding a face recovery and a light adjustment
- **THEN** it is shown between the two, whatever order the three were added in

#### Scenario: Denoise first in the menu

- **WHEN** the menu is opened
- **THEN** Denoise is its first entry

#### Scenario: Sharpen in the menu

- **WHEN** the menu is opened
- **THEN** Sharpen is offered after Color Balance and before Upscale

#### Scenario: Colorization in the menu

- **WHEN** the menu is opened
- **THEN** Colorization is offered after Face Recovery and before Light Adjustment

#### Scenario: No image open

- **WHEN** no image is open
- **THEN** the control that adds an enhancement is unavailable

## REMOVED Requirements

### Requirement: An operation the application cannot run is refused before anything is fetched

**Reason**: Its scenario "A family that cannot be run" describes a state that no longer exists. Once colorization has a
wire arm, every enhancement family the window presents can be asked for, and nothing the window sends names a family
it cannot run. A MODIFIED block cannot drop scenarios, so the requirement is replaced whole by "An operation the
application does not publish is refused before anything is fetched". That requirement keeps every rule that still
holds: an unpublished model or precision is refused and named, the whole chain is judged before any of it begins, and
the library alone decides what is published.

**Migration**: None. No interface sends a family without an arm, and a request that did would still fail to parse.
