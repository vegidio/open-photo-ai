## ADDED Requirements

### Requirement: A requested bias is brought inside what the application can run

A light-adjustment operation SHALL carry a bias, and SHALL be run at that bias where the application permits it and
at the nearest permitted value otherwise, rather than being refused.

The bias SHALL be expressed as the unit value the library takes - negative values reversing the adjustment, zero
leaving the photograph unchanged, one being the model's own output - so that the range the interface bounds its
control by is the range the application publishes, with nothing converted between them.

#### Scenario: A bias within range

- **WHEN** a light adjustment is requested at a bias the application permits
- **THEN** it runs at exactly that bias

#### Scenario: A bias outside the range

- **WHEN** a light adjustment is requested at a bias above or below what the application permits
- **THEN** it runs at the nearest permitted bias rather than failing

#### Scenario: Every published light-adjustment model can be run

- **WHEN** a light adjustment names any model the application publishes for that enhancement, at any precision it
  publishes that model at
- **THEN** the operation is accepted and run

### Requirement: A light adjustment in the list reports its bias

A light adjustment in the list SHALL report, beneath its name, the model it will use, its bias as a whole-number
percentage, and the quality the model is being run at - `Paris, 25%, HD`.

A negative bias SHALL be reported with its sign, since the direction is what a negative bias changes.

#### Scenario: A light adjustment in the list

- **WHEN** a light adjustment carrying a model and a bias of 0.25 is in the list
- **THEN** its row reports that model, `25%` and that model's quality

#### Scenario: A negative bias

- **WHEN** a light adjustment carrying a bias of -0.4 is in the list
- **THEN** its row reports `-40%`

### Requirement: A light adjustment's bias can be changed from its options

A light adjustment's options SHALL offer, beneath the model choice, its bias: a percentage the user can type and a
slider over the same value, labelled "Bias". The range both offer SHALL be the one the application publishes for the
chosen model, presented as a percentage, and the slider SHALL mark the neutral value of zero.

A typed value outside the range SHALL be brought inside it. An empty field, or one holding only a minus sign, SHALL
change nothing: both are states on the way to a number rather than a number, and leaving the field SHALL restore what
the enhancement carries.

The slider SHALL change the enhancement when it is released, not at every step of a drag, so moving it asks for one
run. Its value SHALL be shown in the typed field while it moves.

#### Scenario: Typing a bias

- **WHEN** the user types 25 into the bias of a light adjustment's options
- **THEN** the light adjustment carries a bias of 0.25, and the slider stands at 25%

#### Scenario: Typing a negative bias

- **WHEN** the user types `-`, then `-30`
- **THEN** nothing is changed or re-run on `-`, and the light adjustment carries a bias of -0.3 once `-30` is typed

#### Scenario: A typed bias beyond the range

- **WHEN** the user types 150
- **THEN** the light adjustment carries the largest bias the application permits

#### Scenario: Dragging the slider

- **WHEN** the user drags the bias slider across several values and releases it
- **THEN** the typed field follows the drag, and the light adjustment is changed once, to the value it was released at

#### Scenario: An emptied field

- **WHEN** the user clears the bias field and then leaves it
- **THEN** the light adjustment is unchanged and the field shows the bias it still carries

## MODIFIED Requirements

### Requirement: An enhancement is added from a menu of what can be run

While an image is open, the interface SHALL offer a control that lists the enhancements this
application can run, and choosing one SHALL add it to that image's list.

The menu SHALL offer only enhancements that can actually be run, and SHALL offer each of them once:
an enhancement already in the list SHALL be shown as unavailable rather than removed from the menu,
so the menu keeps its shape as a list is built up.

An enhancement SHALL be added carrying the model the user chose as that enhancement's default, and an
upscale SHALL additionally be added at a scale suited to the image's size rather than at a fixed one -
a small photograph can afford more enlargement than a large one. A light adjustment SHALL be added at a
bias of 50%.

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

#### Scenario: Added between the enhancements already there

- **WHEN** a light adjustment is added to a list holding a face recovery and an upscale
- **THEN** it is shown between the two, whatever order the three were added in

#### Scenario: No image open

- **WHEN** no image is open
- **THEN** the control that adds an enhancement is unavailable
