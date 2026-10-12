## ADDED Requirements

### Requirement: A sharpen in the list reports its strength

A sharpen in the list SHALL report, beneath its name, the model it will use, its strength as a whole-number percentage,
and the quality the model is being run at - `Moscow, 100%, HD`.

#### Scenario: A sharpen in the list

- **WHEN** a sharpen carrying a model and a strength of 1 is in the list
- **THEN** its row reports that model, `100%` and that model's quality

#### Scenario: A strength past the model's output

- **WHEN** a sharpen carrying a strength of 2.5 is in the list
- **THEN** its row reports `250%`

### Requirement: A sharpen's strength can be changed from its options

A sharpen's options SHALL offer, beneath the model choice, its strength: a percentage the user can type and a slider
over the same value, labelled "Strength". The range both offer SHALL be the one the application publishes for the
chosen model, presented as a percentage (0% to 300%), and the slider SHALL mark 100, the value at which the model's
own output is applied unchanged.

A typed value outside the range SHALL be brought inside it. An empty field SHALL change nothing, and leaving the field
SHALL restore what the enhancement carries.

The slider SHALL change the enhancement when it is released, not at every step of a drag, so moving it asks for one
run. Its value SHALL be shown in the typed field while it moves.

Choosing another model SHALL keep the strength the sharpen carries.

The options SHALL be laid out as a denoise's are: the model choice, a separator, and the typed percentage above the
slider.

#### Scenario: The control is labelled Strength

- **WHEN** a sharpen's options are opened
- **THEN** the amount beneath the model choice is labelled "Strength"

#### Scenario: Typing a strength

- **WHEN** the user types 150 into the strength of a sharpen's options
- **THEN** the sharpen carries a strength of 1.5, and the slider stands at 150%

#### Scenario: A typed strength beyond the range

- **WHEN** the user types 400
- **THEN** the sharpen carries the largest strength the application permits

#### Scenario: A typed negative strength

- **WHEN** the user types -20
- **THEN** the sharpen carries a strength of 0

#### Scenario: Dragging the slider

- **WHEN** the user drags the strength slider across several values and releases it
- **THEN** the typed field follows the drag, and the sharpen is changed once, to the value it was released at

#### Scenario: Choosing another model

- **WHEN** a sharpen carrying a strength of 1.5 is switched from Moscow to Novgorod
- **THEN** it carries Novgorod and a strength of 1.5

#### Scenario: Opening the options

- **WHEN** a sharpen's options are opened and closed without anything being changed
- **THEN** the sharpen is unchanged and nothing is re-run

## MODIFIED Requirements

### Requirement: A requested strength is brought inside what the application can run

A denoise or sharpen operation SHALL carry a strength, and SHALL be run at that strength where the application permits
it and at the nearest permitted value otherwise, rather than being refused.

The strength SHALL be expressed as the unit value the library takes - zero leaving the photograph unchanged, one being
the model's own output, and values above one amplifying it - so that the range the interface bounds its control by is
the range the application publishes, with nothing converted between them.

#### Scenario: A strength within range

- **WHEN** a denoise or a sharpen is requested at a strength the application permits
- **THEN** it runs at exactly that strength

#### Scenario: A strength outside the range

- **WHEN** a denoise or a sharpen is requested at a strength above or below what the application permits
- **THEN** it runs at the nearest permitted strength rather than failing

#### Scenario: Every published denoise model can be run

- **WHEN** a denoise names any model the application publishes for that enhancement, at any precision it publishes
  that model at
- **THEN** the operation is accepted and run

#### Scenario: Every published sharpen model can be run

- **WHEN** a sharpen names any model the application publishes for that enhancement, at any precision it publishes
  that model at
- **THEN** the operation is accepted and run

#### Scenario: A model of another enhancement

- **WHEN** a denoise or a sharpen names a model the application publishes only for another enhancement
- **THEN** the operation is refused before anything is fetched

### Requirement: An enhancement is added from a menu of what can be run

While an image is open, the interface SHALL offer a control that lists the enhancements this
application can run, and choosing one SHALL add it to that image's list.

The menu SHALL offer only enhancements that can actually be run, and SHALL offer each of them once:
an enhancement already in the list SHALL be shown as unavailable rather than removed from the menu,
so the menu keeps its shape as a list is built up.

An enhancement SHALL be added carrying the model the user chose as that enhancement's default, and an
upscale SHALL additionally be added at a scale suited to the image's size rather than at a fixed one -
a small photograph can afford more enlargement than a large one. A light adjustment and a colour balance
SHALL each be added at a bias of 50%. A denoise and a sharpen SHALL each be added at a strength of 100%,
the model's own output.

Adding an enhancement SHALL NOT open its options: the user opens them from its row.

The list SHALL be shown in the order the enhancements are applied, whatever order they were added in.

#### Scenario: Adding an enhancement

- **WHEN** the user chooses an enhancement from the menu
- **THEN** it is added to the current image's list, carrying that enhancement's default model, and its options stay
  closed

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

#### Scenario: Denoise first in the menu

- **WHEN** the menu is opened
- **THEN** Denoise is its first entry

#### Scenario: Sharpen in the menu

- **WHEN** the menu is opened
- **THEN** Sharpen is offered after Color Balance and before Upscale

#### Scenario: No image open

- **WHEN** no image is open
- **THEN** the control that adds an enhancement is unavailable
