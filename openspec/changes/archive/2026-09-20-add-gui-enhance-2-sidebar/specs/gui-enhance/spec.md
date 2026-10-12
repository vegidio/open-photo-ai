## ADDED Requirements

### Requirement: Each open image carries its own enhancements

Every open image SHALL have its own list of enhancements, and the interface SHALL show the list
belonging to the image currently being looked at.

Moving between images SHALL leave each list as its owner left it, and closing an image SHALL discard
the list belonging to it without touching any other. An image opened again after being closed SHALL
start with no enhancements, and no list SHALL survive a restart of the application: a stack is a
statement about the photograph on screen now, not a setting.

#### Scenario: Two images with different enhancements

- **WHEN** an enhancement is added to one open image and the user moves to another
- **THEN** the second image shows its own enhancements, and returning to the first shows the ones it
  was given

#### Scenario: An image is closed

- **WHEN** an image carrying enhancements is closed
- **THEN** its enhancements are discarded, every other open image keeps its own, and opening the same
  file again shows no enhancements

#### Scenario: The application is restarted

- **WHEN** the application is closed while an image carries enhancements and is started again
- **THEN** no enhancements are restored

### Requirement: An enhancement is added from a menu of what can be run

While an image is open, the interface SHALL offer a control that lists the enhancements this
application can run, and choosing one SHALL add it to that image's list.

The menu SHALL offer only enhancements that can actually be run, and SHALL offer each of them once:
an enhancement already in the list SHALL be shown as unavailable rather than removed from the menu,
so the menu keeps its shape as a list is built up.

An enhancement SHALL be added carrying the model the user chose as that enhancement's default, and an
upscale SHALL additionally be added at a scale suited to the image's size rather than at a fixed one -
a small photograph can afford more enlargement than a large one.

The list SHALL be shown in the order the enhancements are applied, whatever order they were added in.

#### Scenario: Adding an enhancement

- **WHEN** the user chooses an enhancement from the menu
- **THEN** it is added to the current image's list, carrying that enhancement's default model

#### Scenario: An enhancement already added

- **WHEN** the menu is opened while an enhancement is already in the list
- **THEN** that entry is shown as unavailable and cannot be chosen a second time

#### Scenario: The scale a new upscale arrives at

- **WHEN** an upscale is added to a small image and to a large one
- **THEN** the smaller image's upscale arrives at a larger scale than the larger image's

#### Scenario: No image open

- **WHEN** no image is open
- **THEN** the control that adds an enhancement is unavailable

### Requirement: An enhancement in the list says what it will do

Each enhancement in the list SHALL be shown with its own name and, beneath it, what it is set to do:
the model it will use, the amount where the enhancement takes one, and which quality the chosen model
is being run at.

Those values SHALL be the ones the enhancement actually carries, so that changing a model or an
amount changes what the line reports.

#### Scenario: An upscale in the list

- **WHEN** an upscale carrying a model and a scale is in the list
- **THEN** its row names the enhancement, and reports that model, that scale and that quality

#### Scenario: The line follows the enhancement

- **WHEN** the model or the scale of an enhancement in the list is changed
- **THEN** its row reports the new values

### Requirement: An enhancement can be removed

Each enhancement in the list SHALL offer a way to remove it, reachable both by pointer and by
keyboard, and removing one SHALL leave the rest of the list untouched.

The control SHALL be revealed when its own enhancement is under the pointer or holds focus rather
than being drawn on every row at once, so a list of several reads as a list rather than as a column of
controls.

#### Scenario: Removing one of several

- **WHEN** the user removes one enhancement from a list of several
- **THEN** that one is gone from the list and the others remain, in their order

#### Scenario: Reaching the control

- **WHEN** an enhancement's row is under the pointer, or reached with the keyboard
- **THEN** the control that removes it is available

### Requirement: An enhancement's model and amount can be changed

Choosing an enhancement in the list SHALL open its options, and the options SHALL offer every model
that enhancement publishes, each at every quality it is published at, with the one in use shown as
chosen.

Where a model carries an explanation of what it is for, the options SHALL make that explanation
reachable without it crowding the choice itself.

An upscale's options SHALL additionally offer the scale: a value the user can type, shortcuts for the
common multipliers and for the largest the application permits, and an indication of which of those
the current value is. A typed value outside what the application permits SHALL be brought inside it,
and the range offered SHALL be the one the application publishes rather than one the interface keeps
its own copy of.

Changing a model or a scale SHALL change the enhancement in the list. Merely opening the options
SHALL change nothing.

#### Scenario: Choosing another model

- **WHEN** the user picks a different model in an enhancement's options
- **THEN** that enhancement now carries the chosen model at the chosen quality

#### Scenario: Typing a scale

- **WHEN** the user types a scale within the permitted range
- **THEN** the upscale carries that scale, and the shortcut matching it is shown as chosen

#### Scenario: A scale beyond the range

- **WHEN** the user types a scale larger than the application permits
- **THEN** the value is brought down to the largest permitted one rather than being refused

#### Scenario: Opening the options changes nothing

- **WHEN** the options of an enhancement are opened and closed without a value being changed
- **THEN** the enhancement is unchanged and nothing is re-run

### Requirement: The enhancements decide what the enhanced view shows

Whenever the current image's enhancements change, the application SHALL run them over that image and
show the result as the enhanced view, leaving the original view showing the image as it was.

A run SHALL also be started when the image being looked at changes, and when the processor the user
has chosen changes. Each SHALL stop the run it replaces, and the interface SHALL stop a run that
nothing is waiting for any more - the last enhancement removed, or an image with no enhancements moved
to - rather than leaving it working.

An image with no enhancements SHALL show the image itself as the enhanced view, so both views agree
and neither is blank.

A run that was stopped SHALL leave the view showing what it showed before, and SHALL NOT replace it
with a partly-enhanced image.

#### Scenario: An enhancement is added

- **WHEN** an enhancement is added to the current image
- **THEN** the enhancements are run over it and the enhanced view shows the result, with the original
  view unchanged

#### Scenario: An enhancement is changed while one is running

- **WHEN** the user changes a model or a scale while a run is still working
- **THEN** that run is stopped and a run of the new enhancements takes its place

#### Scenario: The last enhancement is removed

- **WHEN** the only enhancement is removed from the current image
- **THEN** any run in flight is stopped and the enhanced view shows the image itself

#### Scenario: Moving to another image

- **WHEN** the user moves to another image while a run is working
- **THEN** that run is stopped, and what is shown is the new image and its own enhancements' result

### Requirement: A run in progress is reported on the canvas

While a run is working, the application SHALL show over the preview how far it has got and which
enhancement it is on, named in the language the rest of the window is using.

The indicator SHALL appear as soon as a run is asked for, before that run has reported anything: a
run loads its model before it reports, and an indicator that waited for the first report appeared
long after the enhancement was added. Until the first report it SHALL show no progress and name the
work generically.

While a model is being fetched, that indicator SHALL instead report the fetch and how far it has got
on its own terms, since the whole run's figure barely moves during one.

The indicator SHALL be absent when no run is in flight.

#### Scenario: An enhancement is running

- **WHEN** a run is working on an enhancement whose model is on the machine
- **THEN** the preview shows how far the run has got, labelled with that enhancement's name in the
  interface's own language

#### Scenario: A run that has not reported yet

- **WHEN** a run has been asked for and has reported nothing
- **THEN** the preview shows the indicator at no progress, naming the work generically

#### Scenario: A model is being fetched

- **WHEN** a run is fetching a model it does not have
- **THEN** the preview reports the fetch and its own percentage

#### Scenario: Nothing is running

- **WHEN** no run is in flight
- **THEN** no indicator is shown over the preview

### Requirement: A run that fails is reported, and a run that was stopped is not

A run that fails SHALL be reported to the user in a way that does not interrupt what they are doing,
and the interface SHALL go on showing the image it was showing.

A run that was stopped - by the user changing the enhancements, by moving to another image, or by
anything else that replaces it - SHALL NOT be reported as a failure. The user asked for it to stop.

#### Scenario: A run fails

- **WHEN** a run cannot be completed
- **THEN** the user is told, and the view keeps showing what it was showing

#### Scenario: A run is replaced

- **WHEN** a run is stopped because the enhancements changed
- **THEN** nothing is reported to the user about it

## MODIFIED Requirements

### Requirement: A run reports what it is doing while it works

The application SHALL report a run's progress to the interface as it advances, and each report SHALL
carry which operation it is about, how far the whole chain has got, and whether that operation's
model is being fetched or being run.

An operation SHALL be named twice over: by the model it is running, spelled as the application spells
it, and by which enhancement it belongs to, in the terms the application's own catalogue uses. The
first is what a diagnostic needs; the second is what an interface needs to name the operation in the
language its user is reading, which it cannot do from a composed sentence.

A report about a model being fetched SHALL additionally carry how far that fetch has got on its own
terms. The two numbers differ and both are needed: a fetch occupies only the head of one operation's
share of the chain, so a bar tracking the chain barely moves during one, and without the fetch's own
figure a multi-gigabyte download is indistinguishable from a stall.

How far the whole chain has got SHALL never decrease over a run and SHALL reach completion exactly
once, however many operations the chain holds and however many fetches happen within it.

An operation whose result the application already knew SHALL advance the chain's figure by that
operation's whole share without being reported as either fetched or run. It did neither, and a run
made entirely of such operations therefore finishes without ever reporting work.

Reports SHALL be limited to a rate an interface can draw. A run over a large photograph produces
thousands of them and a progress indicator has about a hundred distinct positions.

#### Scenario: A model is being run

- **WHEN** an operation's model is running over the image
- **THEN** reports name that operation, say it is being run, and carry how far the whole chain has got

#### Scenario: A model is being fetched

- **WHEN** an operation's model is not yet on the machine and is being fetched
- **THEN** reports name that operation, say it is being fetched, and carry both how far the chain has
  got and how far the fetch itself has got

#### Scenario: Which enhancement a report is about

- **WHEN** a report is made about any operation, whatever its stage
- **THEN** it names the enhancement that operation belongs to, in the same terms the catalogue names
  it, as well as naming the model

#### Scenario: The chain only moves forward

- **WHEN** a chain of several operations runs, one of which fetches its model first
- **THEN** the figure for the whole chain never decreases, neither between operations nor when a fetch
  gives way to a run, and it reaches completion exactly once

#### Scenario: An operation whose result was already known

- **WHEN** an operation's result is already held from an earlier run
- **THEN** the chain's figure advances by that operation's whole share and the operation is reported
  as neither fetched nor run

#### Scenario: A run that is entirely already known

- **WHEN** every operation in a chain has a result already held
- **THEN** the run completes without reporting any work having been done
