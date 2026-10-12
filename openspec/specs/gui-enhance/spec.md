# gui-enhance Specification

## Purpose

Running enhancements over an open image from the window: asking for a chain, stopping one that is
still running, what the application reports while it works, and how the enhanced pixels reach the
interface that asked for them.

## Requirements

### Requirement: The interface can ask for an open image to be enhanced

The application SHALL accept a request to run an ordered chain of operations over one open image, and
SHALL answer with the enhanced result's own identity together with its dimensions.

The source SHALL be named by the identity the application already addresses its pixels by, so the
interface names a photograph the same way whatever it is asking for. A request naming an identity the
application has not admitted SHALL be refused.

Each operation SHALL name its family, the model within that family, the precision to run it at, and
the values that model takes. The interface SHALL NOT be required to compose those into a single
encoded string: every part is published by the application's own catalogue, and naming them
separately is what keeps the two from drifting.

An empty chain SHALL be accepted and SHALL answer with the source unchanged. A user who has toggled
every enhancement off is asking a meaningful question, not making a mistake.

#### Scenario: A chain is run

- **WHEN** the interface asks for an admitted image to be enhanced by a chain of one or more
  operations
- **THEN** the chain is run in the order given, each operation over the result of the one before it,
  and the enhanced result's identity and dimensions are answered

#### Scenario: The result is identified by what produced it

- **WHEN** two different chains are run over the same source image
- **THEN** the two results carry different identities, and neither carries the source's own

#### Scenario: A source that was never opened

- **WHEN** a request names a source identity the application has not admitted
- **THEN** it is refused and nothing is run

#### Scenario: An empty chain

- **WHEN** the interface asks for an admitted image to be enhanced by no operations at all
- **THEN** the request succeeds and the source's pixels are what the result denotes

### Requirement: A run enhances the image as the user has framed it

A request to enhance an open image SHALL be able to carry that image's crop, and the chain SHALL run
over the framed pixels rather than over the whole photograph.

The result of such a run SHALL be identified by what produced it — the framed pixels and the chain —
so that the same chain run over two different framings of one photograph answers two different
results, and neither can be served where the other was asked for.

Work already done for one framing SHALL NOT be reused for another. A run over a newly framed
photograph SHALL produce the enhancement of that framing, whatever the application has already
computed for the same chain over the same photograph framed differently.

A request carrying no crop SHALL run exactly as it does today, over the whole photograph, and so SHALL
a request carrying a crop that frames the whole photograph unturned and unflipped: a framing that
changes no pixel SHALL cost no work, so opening the framing controls and changing nothing does not
discard everything already computed for that photograph.

#### Scenario: A chain over a framed image

- **WHEN** the interface asks for an admitted image to be enhanced and carries a crop with the request
- **THEN** the chain runs over the framed pixels, and the result's dimensions are those of the
  enhanced framing

#### Scenario: Two framings of one photograph

- **WHEN** one chain is run over an image with one crop and then over the same image with another
- **THEN** the two results carry different identities and each answers its own framing

#### Scenario: A repeated chain over one framing

- **WHEN** a chain that has already been run over a framed image is asked for again with the same crop
- **THEN** the same result is answered

#### Scenario: No crop

- **WHEN** the interface asks for an admitted image to be enhanced and carries no crop
- **THEN** the chain runs over the whole photograph

#### Scenario: A framing that changes nothing

- **WHEN** a chain that has already been run over a photograph is asked for again carrying a crop with
  no flip, no rotation and a rectangle covering the whole photograph
- **THEN** the result already computed is answered, rather than the chain being run a second time

### Requirement: A requested scale is brought inside what the application can run

An operation carrying a scale SHALL be run at that scale where the application permits it, and at the
nearest permitted value otherwise, rather than being refused.

The control the user drives is already bounded, so a value outside the range can only reach the
application through a fault. Refusing it would turn that fault into a failed enhancement in front of
the user; bringing it inside the range produces a picture and leaves the fault visible in the logs.

#### Scenario: A scale within range

- **WHEN** a scale the application permits is requested
- **THEN** the operation runs at exactly that scale

#### Scenario: A scale outside the range

- **WHEN** a scale above or below what the application permits is requested
- **THEN** the operation runs at the nearest permitted scale rather than failing

### Requirement: A requested bias is brought inside what the application can run

A light-adjustment or colour-balance operation SHALL carry a bias, and SHALL be run at that bias where the
application permits it and at the nearest permitted value otherwise, rather than being refused.

The bias SHALL be expressed as the unit value the library takes - negative values reversing the adjustment, zero
leaving the photograph unchanged, one being the model's own output - so that the range the interface bounds its
control by is the range the application publishes, with nothing converted between them.

#### Scenario: A bias within range

- **WHEN** a light adjustment or a colour balance is requested at a bias the application permits
- **THEN** it runs at exactly that bias

#### Scenario: A bias outside the range

- **WHEN** a light adjustment or a colour balance is requested at a bias above or below what the application permits
- **THEN** it runs at the nearest permitted bias rather than failing

#### Scenario: Every published light-adjustment model can be run

- **WHEN** a light adjustment names any model the application publishes for that enhancement, at any precision it
  publishes that model at
- **THEN** the operation is accepted and run

#### Scenario: Every published colour-balance model can be run

- **WHEN** a colour balance names any model the application publishes for that enhancement, at any precision it
  publishes that model at
- **THEN** the operation is accepted and run

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

### Requirement: A run in flight can be stopped from the window

The interface SHALL be able to stop a run it has asked for, and a stopped run SHALL produce no image
at all - neither the operations that had already finished, nor the partly-covered result the stop
landed in.

The interface SHALL identify the run it means to stop, and a request naming a run that is no longer
the one in flight SHALL do nothing. A stop that arrives before its run has begun SHALL still take
effect on it, since the two cross the boundary independently and either may arrive first.

Stopping SHALL be reported distinguishably from failing. A user who has changed their mind has not
been told their enhancement broke.

#### Scenario: Stopping a run

- **WHEN** the interface stops a run it asked for while that run is still working
- **THEN** the run stops, no image is produced, and the outcome is reported as stopped rather than as
  failed

#### Scenario: Stopping a run that has already been replaced

- **WHEN** the interface asks to stop a run that is no longer the one in flight
- **THEN** nothing is stopped and the run that is in flight continues

#### Scenario: A stop that overtakes its own run

- **WHEN** a stop for a run arrives before the request that starts it
- **THEN** that run does not go on to do work that nobody is waiting for

### Requirement: The application runs one enhancement at a time

Asking for a run while another is in flight SHALL stop the one in flight before the new one begins.

The window draws one enhanced image, so a second run is always a statement that the first is no
longer wanted - the user has changed the enhancements, or moved to another photograph. Making the
application enforce that, rather than trusting the interface to ask, is what keeps an abandoned run
from holding models in memory and occupying the processor it was given.

#### Scenario: A second run replaces the first

- **WHEN** the interface asks for a run while an earlier run is still working
- **THEN** the earlier run is stopped and produces no image, and the new run proceeds

#### Scenario: The interface never asked for the stop

- **WHEN** an earlier run is displaced without the interface having asked for it to be stopped
- **THEN** it still stops, and still holds no models once it has

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

### Requirement: The enhanced result's pixels are served to the interface

The application SHALL serve the pixels of the enhanced result to the interface by the same route and
in the same terms as an open image's own: addressed by the result's identity, bounded on the longest
edge where a bound is asked for, and served at the result's own dimensions where none is.

The interface SHALL NOT receive the pixels as part of the answer to the run itself. A full-resolution
enlargement is tens of megabytes and the window has a way to fetch pixels already; carrying them
through the answer would put them on the message channel and make the interface responsible for
holding them.

#### Scenario: Drawing the enhanced result

- **WHEN** the interface requests the pixels of an enhanced result by its identity
- **THEN** they are served, at the requested bound or at the result's own dimensions

#### Scenario: The answer to a run carries no pixels

- **WHEN** a run finishes
- **THEN** what is answered describes the result and does not contain it

### Requirement: Only the most recent enhanced result is held

The application SHALL hold the result of the most recent completed run, and SHALL stop serving any
result a later run has displaced.

It SHALL also stop holding a result once the image it was made from is no longer open, so a result is
released by the user finishing with the photograph as well as by a later run replacing it. Holding
only until the next run is what left a large result resident for the whole session when no next run
came.

A request for a result that is no longer held SHALL be refused distinguishably from one naming a
result that never existed, so an interface drawing a stale address can tell the two apart.

Holding one is what bounds the cost: a fourfold enlargement of a large photograph is over a gigabyte
of pixels, and holding every result a user had passed through would grow without limit over a session.

#### Scenario: A superseded result

- **WHEN** the pixels of an enhanced result are requested after a later run has completed
- **THEN** the request is refused, and reported as no longer held rather than as unknown

#### Scenario: A result whose image has been closed

- **WHEN** the pixels of an enhanced result are requested after the image it was made from was closed
- **THEN** the request is refused, and reported as no longer held rather than as unknown

#### Scenario: A result that never existed

- **WHEN** pixels are requested for an identity no run has produced
- **THEN** the request is refused and reported as unknown

### Requirement: Changing the framing starts a new run and lets go of the previous result

While an image is open, changing its crop SHALL start a new run of that image's chain over the new
framing, and SHALL stop whatever run was in flight.

The result the previous framing produced SHALL be released rather than left on screen while the new
run works: the window draws a photograph at the shape of its own dimensions, and a result made from
other pixels drawn into the new framing's shape would be drawn to the wrong proportions. Until the new
run lands, the window SHALL draw the newly framed source.

This is a deliberate divergence from the reference implementation, which keeps the previous result on
screen. It differs from changing an enhancement, where the previous result is held until a newer one
lands, because an enhancement does not change the shape of what is drawn and a crop does.

#### Scenario: The crop changes while a result is on screen

- **WHEN** the crop of an image carrying an enhanced result is changed
- **THEN** the previous result is released, the window draws the newly framed source, and a run over
  the new framing starts

#### Scenario: The crop changes while a run is in flight

- **WHEN** the crop of an image is changed while a run over the previous framing is running
- **THEN** that run is stopped and a run over the new framing is started

#### Scenario: An image with no enhancements

- **WHEN** the crop of an image carrying no enhancements is changed
- **THEN** no run is asked for, and the window draws the newly framed source

### Requirement: An enhanced result is released when its image is closed

The application SHALL stop holding an enhanced result's pixels once the image it was made from is no
longer open.

Closing one image SHALL release only what was enhanced from that image, and SHALL leave a result
belonging to any other image untouched - including one produced by a run still in flight. Closing
every open image SHALL release every held result.

An image with no identity has produced nothing and SHALL release nothing.

A run that was working on the closed image when it was closed SHALL NOT go on to have its result held.
Stopping a run is cooperative, so one can finish without noticing it was asked to stop; holding what it
produced would leave a closed image's result resident for the session, which is the whole of what
releasing prevents.

#### Scenario: The enhanced image is closed

- **WHEN** an image that has been enhanced is closed
- **THEN** the application no longer holds the enhanced pixels

#### Scenario: Another image is closed while a result is held

- **WHEN** an image is closed and the held result belongs to a different image
- **THEN** the held result is kept

#### Scenario: Every image is closed

- **WHEN** the user closes all open images at once
- **THEN** no enhanced result is held

#### Scenario: A run finishes after the image it was working on was closed

- **WHEN** a run that was enhancing an image finishes after that image was closed
- **THEN** its result is not held, and the run is reported as stopped

### Requirement: The record of produced results is bounded

The record the application keeps of results it has produced SHALL NOT grow without limit for the life
of the process.

An identity old enough to have left that record SHALL be refused as unknown. Nothing can still be
drawing it: an address is held because a pane is drawing it, and a pane draws a result because the run
that produced it was recent.

#### Scenario: A long session producing many results

- **WHEN** the application has produced more results than the record holds
- **THEN** the record has not grown past its limit, and the most recent results are still reported as
  no longer held once released

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

### Requirement: A face-recovery enhancement carries the faces it will restore

The application SHALL accept a face-recovery operation naming a model, a precision and the faces to
restore, and SHALL run it over exactly those faces.

The faces SHALL travel with the operation rather than being found by the run. The window has
already found them, shows them, and decides which of them this run is about; a run that detected its
own would find a set the window cannot see and restore faces the user turned off.

The faces carried SHALL be the ones the user has chosen, and no others. A face the user has skipped
SHALL NOT be restored, SHALL NOT be sent, and SHALL NOT be folded into the identity of the result -
so that the same photograph restored over one selection and then over another is two results rather
than one served twice.

A chain carrying a face-recovery operation SHALL NOT be run until the faces for the framing in
force are known. A run started before they arrive would restore nothing, would be stored as a
result for an empty selection, and would have to be run again the moment the faces landed.

#### Scenario: A face-recovery operation is run

- **WHEN** the interface asks for an admitted image to be enhanced by a face-recovery operation
  carrying two faces
- **THEN** both are restored, and the result's identity and dimensions are answered

#### Scenario: A face is skipped

- **WHEN** one of two faces is skipped and the enhancements are run
- **THEN** the operation carries only the chosen face, and only that face is restored

#### Scenario: The same photograph over two selections

- **WHEN** a photograph is restored over both faces and then over one of them
- **THEN** the second run is not served the first run's result

#### Scenario: A chain whose faces are not known yet

- **WHEN** a face-recovery enhancement is added to a photograph whose faces have not been found yet
- **THEN** no run is started until they have been

#### Scenario: A face-recovery operation carrying no faces

- **WHEN** a face-recovery operation carrying no faces is run
- **THEN** the request succeeds, nothing is restored, and any other operation in the chain is still
  applied

### Requirement: How closely a restoration keeps to the original face is fixed by the application

The application SHALL run every face recovery at maximum fidelity - maximum faithfulness to the
face that was there, and the least restoration - and SHALL NOT offer a control for it.

Where the chosen model accepts no such value, the application SHALL send none rather than
substituting one. Whether a model takes it is the model's own property, so a request SHALL NOT be
able to name a model and a fidelity that do not go together.

This is the value the reference implementation fixes for every run, and it is what an untouched
control would carry. Putting it on the wire before there is a control to move it would describe a
choice the user cannot make.

#### Scenario: A model that accepts a fidelity

- **WHEN** a face recovery is run on a model that accepts a fidelity
- **THEN** it runs at maximum fidelity

#### Scenario: A model that accepts no fidelity

- **WHEN** a face recovery is run on a model that accepts no fidelity
- **THEN** no fidelity is sent, and the run is not refused

#### Scenario: The same photograph through both models

- **WHEN** one photograph is restored by each of the two models in turn, over the same faces
- **THEN** neither run is refused, and neither is served the other's stored result

### Requirement: A face-recovery enhancement in the list reports how many faces it will restore

A face-recovery enhancement in the list SHALL report, beneath its name, the model it will use, how
many faces it will restore, and which quality that model is being run at.

Where every face found is chosen, the line SHALL report how many faces there are. Where some have
been skipped, it SHALL report how many of how many will be restored, so that a row cannot say `2
Faces` while the run restores one.

The total SHALL be the faces actually found in the photograph as it is framed, so that framing a
photograph so that a face falls outside it changes what the line reports.

#### Scenario: A face recovery in the list

- **WHEN** a face-recovery enhancement is in the list for a photograph in which two faces were
  found
- **THEN** its row names the enhancement, and reports that model, two faces, and that quality

#### Scenario: Some faces are skipped

- **WHEN** one of the two faces found has been skipped
- **THEN** the row reports that one of two faces will be restored

#### Scenario: Every face is skipped

- **WHEN** both of the two faces found have been skipped
- **THEN** the row reports that none of two faces will be restored

#### Scenario: The count follows the framing

- **WHEN** the photograph is framed so that one of the two faces falls outside it
- **THEN** the row reports one face

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

### Requirement: A colour balance in the list reports its bias

A colour balance in the list SHALL report, beneath its name, the model it will use, its bias as a whole-number
percentage, and the quality the model is being run at - `Rio, 50%, HD`.

A negative bias SHALL be reported with its sign, since the direction is what a negative bias changes.

#### Scenario: A colour balance in the list

- **WHEN** a colour balance carrying a model and a bias of 0.5 is in the list
- **THEN** its row reports that model, `50%` and that model's quality

#### Scenario: A negative bias

- **WHEN** a colour balance carrying a bias of -0.4 is in the list
- **THEN** its row reports `-40%`

### Requirement: A colour balance's bias can be changed from its options

A colour balance's options SHALL offer, beneath the model choice, its bias: a percentage the user can type and a
slider over the same value, labelled "Bias". The range both offer SHALL be the one the application publishes for the
chosen model, presented as a percentage, and the slider SHALL mark the neutral value of zero.

A typed value outside the range SHALL be brought inside it. An empty field, or one holding only a minus sign, SHALL
change nothing, and leaving the field SHALL restore what the enhancement carries.

The slider SHALL change the enhancement when it is released, not at every step of a drag, so moving it asks for one
run. Its value SHALL be shown in the typed field while it moves.

Choosing another model SHALL keep the bias the colour balance carries.

#### Scenario: Typing a bias

- **WHEN** the user types 25 into the bias of a colour balance's options
- **THEN** the colour balance carries a bias of 0.25, and the slider stands at 25%

#### Scenario: A typed bias beyond the range

- **WHEN** the user types 150
- **THEN** the colour balance carries the largest bias the application permits

#### Scenario: Dragging the slider

- **WHEN** the user drags the bias slider across several values and releases it
- **THEN** the colour balance is changed once, to the value it was released at

#### Scenario: Choosing another model

- **WHEN** a colour balance carrying a bias of 0.3 is switched from Rio to São Paulo
- **THEN** it carries São Paulo and a bias of 0.3

#### Scenario: Opening the options

- **WHEN** a colour balance's options are opened and closed without anything being changed
- **THEN** the colour balance is unchanged and nothing is re-run

### Requirement: A denoise in the list reports its strength

A denoise in the list SHALL report, beneath its name, the model it will use, its strength as a whole-number percentage,
and the quality the model is being run at - `Stockholm, 100%, HD`.

#### Scenario: A denoise in the list

- **WHEN** a denoise carrying a model and a strength of 1 is in the list
- **THEN** its row reports that model, `100%` and that model's quality

#### Scenario: A strength past the model's output

- **WHEN** a denoise carrying a strength of 2.5 is in the list
- **THEN** its row reports `250%`

### Requirement: A denoise's strength can be changed from its options

A denoise's options SHALL offer, beneath the model choice, its strength: a percentage the user can type and a slider
over the same value, labelled "Strength". The range both offer SHALL be the one the application publishes for the
chosen model, presented as a percentage (0% to 300%), and the slider SHALL mark 100, the value at which the model's
own output is applied unchanged.

A typed value outside the range SHALL be brought inside it. An empty field SHALL change nothing, and leaving the field
SHALL restore what the enhancement carries.

The slider SHALL change the enhancement when it is released, not at every step of a drag, so moving it asks for one
run. Its value SHALL be shown in the typed field while it moves.

Choosing another model SHALL keep the strength the denoise carries.

The options SHALL be laid out as a light adjustment's and a colour balance's are: the model choice, a separator, and
the typed percentage above the slider.

#### Scenario: The control is labelled Strength

- **WHEN** a denoise's options are opened
- **THEN** the amount beneath the model choice is labelled "Strength"

#### Scenario: Typing a strength

- **WHEN** the user types 150 into the strength of a denoise's options
- **THEN** the denoise carries a strength of 1.5, and the slider stands at 150%

#### Scenario: A typed strength beyond the range

- **WHEN** the user types 400
- **THEN** the denoise carries the largest strength the application permits

#### Scenario: A typed negative strength

- **WHEN** the user types -20
- **THEN** the denoise carries a strength of 0

#### Scenario: Dragging the slider

- **WHEN** the user drags the strength slider across several values and releases it
- **THEN** the typed field follows the drag, and the denoise is changed once, to the value it was released at

#### Scenario: Choosing another model

- **WHEN** a denoise carrying a strength of 1.5 is switched from Stockholm to Malmö
- **THEN** it carries Malmö and a strength of 1.5

#### Scenario: Opening the options

- **WHEN** a denoise's options are opened and closed without anything being changed
- **THEN** the denoise is unchanged and nothing is re-run

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
