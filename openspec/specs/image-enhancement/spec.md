# image-enhancement Specification

## Purpose

Running a chosen enhancement over an image end to end: what a run accepts and what it refuses, the
bit depth the result carries, how a requested scale that no single set of weights serves is covered by
a sequence of passes and corrected afterwards, what a run reports while it works, and what happens
when a caller stops one - because naming a model, putting it on disk, opening it and tiling an image
are four things this application can already do separately and none of them is an enhancement.

## Requirements

### Requirement: An image and an ordered chain of operations produce an enhanced image

The system SHALL accept an image together with an ordered sequence of operations and produce one
enhanced image, each operation applied to the result of the one before it, together with a report of
what the run was executed on — the contract for which is the requirement below.

An empty sequence SHALL return the image unchanged rather than being refused: applying nothing is a
meaningful request, and it is what a caller with a user who has toggled every enhancement off sends.

The result SHALL carry an identity composed from the input's identity, every operation applied, and
the bits per channel the result carries, so that two runs differing only in which operations were
applied are distinguishable, two runs differing only in the depth asked for are distinguishable, and
a second run over the result of a first is not mistaken for the first. The result SHALL NOT carry the
input's own identity, which would make a chained pair of runs indistinguishable from the first run
alone.

Where two runs would produce the same pixels they SHALL carry the same identity, and where they would
produce different pixels they SHALL NOT. An identity that did not distinguish depth would name two
different images: the same chain at 8 and at 16 bits per channel produces two results that are not
interchangeable, and anything keyed on that identity would serve one where the other was asked for.

The identity SHALL be composed from the image and the operations alone. What the run was executed on
SHALL NOT enter it: the same chain over the same image produces the same pixels whether it ran on a
GPU or on the CPU, and an identity that moved with the provider would make a result computed on one
machine useless to another.

Where an operation's result is already known - because an identical operation over an identical image
has already been carried out and its result kept - the system MAY produce that result without running
the model, provided the result is the one the model would have produced. What is kept and for how long
is the run cache's own contract.

#### Scenario: One operation over an image

- **WHEN** an image is run through a chain of one supported operation
- **THEN** an enhanced image is produced
- **AND** it carries an identity that is neither the input's nor that of the same image run through a
  different operation

#### Scenario: An empty chain

- **WHEN** an image is run through a chain of no operations
- **THEN** the image is returned unchanged
- **AND** the run succeeds

#### Scenario: A run chained onto the result of another

- **WHEN** an image is run through one operation and its result is then run through a second, both at
  the same depth
- **THEN** the final identity is the same as running both operations in one chain at that depth
- **AND** it differs from the identity of either single run

#### Scenario: The same chain at two different depths

- **WHEN** an image is run through the same chain twice, once asking for 8 bits per channel and once
  for 16
- **THEN** the two results carry different identities

#### Scenario: Two ways of asking for the same depth

- **WHEN** a 16-bit image is run through a chain once asking for 16 bits per channel and once asking
  for whatever the source carried
- **THEN** the two results carry the same identity, because they are the same pixels

#### Scenario: The same chain run on two different providers

- **WHEN** one image is run through one chain on a GPU provider, and the same image through the same
  chain on the CPU
- **THEN** the two results carry the same identity

### Requirement: A run is refused rather than served wrongly where an operation cannot be served

Where the system has no pipeline for an operation in the chain, the run SHALL fail with an error
naming that operation, and SHALL produce no image.

It SHALL NOT fall back to returning the image unchanged, nor to any result produced without the model
the operation names: a caller handed an unenhanced image with no error has no way to tell it from an
enhancement, and would show it to a user as one. This is the same refusal the tiling of a zero-area
image makes, for the same reason.

A refusal SHALL happen before any model is installed or opened, so that a chain naming an operation
that cannot be served does not first transfer gigabytes on its behalf.

Every enhancement family the system publishes has a pipeline for every variant it publishes. The
system SHALL NOT publish an answer saying that an operation cannot be run **yet**. No operation
produces that answer, and an answer that nothing produces tells a caller nothing it can act on. Where
a later version publishes a model before its pipeline exists, that version adds the answer back.

Where the refusal is because the operation's result is not an image, it SHALL say so. That refusal is
permanent. A face-detection run produces a set of faces, so there is no enhanced picture for it to
contribute to a chain, and there never will be. The refusal SHALL name the path such an operation is
run on instead, so that a caller that reached the chain by mistake is told where the operation
belongs. A caller SHALL NOT have to read the error's text to recognise this refusal.

#### Scenario: Every published enhancement is served

- **WHEN** a chain names any variant of any enhancement family the system publishes, at any precision
  it is published in
- **THEN** the run is not refused for want of a pipeline

#### Scenario: An operation whose result is not an image

- **WHEN** a chain names an operation whose result is a set of faces rather than an image
- **THEN** the run fails with an error naming that operation
- **AND** the refusal says the operation's result is not an image
- **AND** no image is produced and no model is transferred

#### Scenario: An unservable operation later in the chain

- **WHEN** a chain's first operation can be served and its second cannot
- **THEN** the run fails before the first operation's model is installed or opened
- **AND** no image is produced

### Requirement: The result carries 8 bits per channel unless the caller asks otherwise

A caller that states no depth SHALL receive a result carrying 8 bits per channel. A model's output is
floating point and carries more precision than any channel depth, so no depth is the natural one and
one has to be chosen; 8 is chosen because it is what every enhancement produced before a choice
existed, so a caller that says nothing is not silently given something new.

A caller MAY instead state that the result is to carry 16 bits per channel, or whatever the source
image carried, and the system SHALL then produce that depth. Carrying more than 8 bits is therefore a
deliberate request and never an accident.

Where the caller asks for the source's own depth, it SHALL be resolved against the image actually
loaded: a source carrying more than 8 bits per channel resolves to 16, and any other source resolves
to 8.

Where a chain applies several operations, every operation SHALL produce the same depth, so that the
depth of the result does not depend on how many enhancements a user happened to select.

#### Scenario: A caller that states no depth

- **WHEN** an image is enhanced with no depth stated
- **THEN** the result carries 8 bits per channel

#### Scenario: A 16-bit source with no depth stated

- **WHEN** a 16-bit image is enhanced with no depth stated
- **THEN** the result carries 8 bits per channel, because carrying more is a request rather than a
  default

#### Scenario: A 16-bit source at the source's own depth

- **WHEN** a 16-bit image is enhanced with the result asked to carry whatever the source carried
- **THEN** the result carries 16 bits per channel

#### Scenario: An 8-bit source at the source's own depth

- **WHEN** an 8-bit image is enhanced with the result asked to carry whatever the source carried
- **THEN** the result carries 8 bits per channel

#### Scenario: A depth asked for against the source's own

- **WHEN** an 8-bit image is enhanced with the result asked to carry 16 bits per channel
- **THEN** the result carries 16 bits per channel

### Requirement: A requested scale is covered by native passes and corrected afterwards

An upscale whose weights are published per whole factor SHALL cover a requested scale by running the
sequence of native passes that reaches or exceeds it, each pass taking the previous pass's result. A
user may ask for any scale in the supported range.

Where that sequence produces an image larger than the request - a request no whole factor matches -
the system SHALL resample the result to exactly the requested scale after the last pass, and SHALL
NOT resample between passes: a model's input is the previous model's output, and resampling in
between would feed the next pass an interpolation rather than a restoration.

The final result's dimensions SHALL be the input's, each multiplied by the requested scale and
rounded, whether or not any pass overshot.

Where no sequence of passes covers a requested scale, the run SHALL fail. It SHALL NOT return a plain
resample of the input presented as a successful enhancement.

This is not the only way an upscale reaches a requested scale, and it is stated for the contract it
belongs to rather than for every upscale. An upscaler whose model preserves resolution has no native
factor and no pass sequence at all; it reaches a scale by resampling to the requested size first and
restoring at that size.

#### Scenario: A scale a single pass serves exactly

- **WHEN** an image is upscaled by a factor one set of weights serves natively
- **THEN** one pass runs
- **AND** the result is that factor times the input's width and height, with no resample afterwards

#### Scenario: A scale no whole factor serves

- **WHEN** an image is upscaled by a factor between two native factors
- **THEN** the passes covering it run and the result is resampled once, after the last of them
- **AND** the final dimensions are the input's multiplied by the requested scale

#### Scenario: A scale needing several passes

- **WHEN** an image is upscaled by a factor larger than any single set of weights serves
- **THEN** the passes run in sequence, each over the previous one's result
- **AND** the final dimensions are the input's multiplied by the requested scale

#### Scenario: An upscaler with no native factor

- **WHEN** an image is upscaled by a variant whose model returns an image the size it was given
- **THEN** no pass sequence is resolved for it
- **AND** the requested scale is reached by resampling to the requested size before the model runs

### Requirement: An upscaler that preserves resolution reaches a scale by resampling first

An upscale variant whose model returns an image the size of what it was given SHALL reach a requested
scale by resampling the input to the requested size and then restoring detail at that size. The
resampled image SHALL be produced with the same high-quality filter the system corrects an overshoot
with, and SHALL be what the model is conditioned on.

This is not a fallback for a model that cannot enlarge. Such a model is resolution-preserving by
construction, so there is no scale it could apply; resampling first is how it is meant to be driven,
and the reference implementation drives it the same way for the same reason.

At a requested scale of 1 the resample SHALL leave the image at its own dimensions, and the run SHALL
still restore detail at that size - a scale of 1 is a restoration request, not a no-op.

Where the model accepts only one region size, or accepts only dimensions that are a multiple of some
figure, the system SHALL extend the resampled image to satisfy both before running, by mirroring its
own edge pixels outwards rather than by adding a border of any colour, and SHALL remove that extension
from the result. Mirroring keeps the edge pixels the user asked for; a flat border is treated by the
model as picture content, and whatever it makes of it bleeds back across the edge into pixels the
image actually has. Cropping instead of extending would discard pixels the user asked for.

The final result's dimensions SHALL be the input's, each multiplied by the requested scale and
rounded - the same arithmetic every other upscale contract produces, so which contract served a
request is not observable in the size of what comes back.

#### Scenario: A scale larger than 1

- **WHEN** an image is upscaled by a resolution-preserving upscaler at a factor above 1
- **THEN** the result's dimensions are the input's multiplied by that factor and rounded
- **AND** the result carries restored detail rather than only the resample's interpolation

#### Scenario: A restoration at a scale of 1

- **WHEN** an image is run through a resolution-preserving upscaler at a scale of 1
- **THEN** the result has the input's own dimensions
- **AND** the image has still been through the model rather than returned unchanged

#### Scenario: An image smaller than the one region the model accepts

- **WHEN** the resampled image is smaller in either dimension than the single region size the model
  accepts
- **THEN** it is extended to at least that size by mirroring its own edge pixels
- **AND** the extension does not appear in the result, whose dimensions are the requested ones

#### Scenario: An image whose size is not a multiple the model requires

- **WHEN** the resampled image's width or height is not a multiple of the figure the model requires
- **THEN** it is extended to the next such multiple by mirroring its own edge pixels
- **AND** the extension does not appear in the result

### Requirement: A diffusion upscaler's colour drift is corrected against what it was conditioned on

A single diffusion step drifts in overall colour and brightness. The system SHALL correct that drift
by separating the restored image into low-frequency and high-frequency content, replacing the
low-frequency content with that of the image the model was conditioned on, and keeping the restored
image's own high-frequency detail. The correction SHALL preserve the detail the model synthesized: it
changes colour and illumination, not texture.

The correction SHALL be applied **once, to the assembled image**, and SHALL NOT be applied to each
region separately. Each region drifts independently, so correcting a region against its own crop bakes
that region's drift in as a step change at its boundary rather than removing it - which reads as a
seam that no amount of combining removes, because the regions genuinely disagree about the colour of
what they share.

The reference for the correction SHALL be the resampled image the model was conditioned on, and not
the original input, so that the correction undoes only what the model changed rather than also undoing
the resample.

#### Scenario: A colour cast across the whole result

- **WHEN** a diffusion upscale produces a result whose overall colour and brightness have drifted from
  the image it was conditioned on
- **THEN** the drift is corrected towards that image
- **AND** the detail the model synthesized is still present in the result

#### Scenario: Regions that drifted differently from one another

- **WHEN** adjacent regions of one image drifted in different directions
- **THEN** the correction is applied once to the assembled image rather than per region
- **AND** no step in colour appears at the boundary between them

### Requirement: A run reports what it is doing and how far it has got

The system SHALL report progress to a caller that asks for it, and a caller that does not ask SHALL
not be charged for reporting it.

Each report SHALL name the operation it concerns and SHALL say whether the model is being put on disk,
being run, or was not run at all because the result was already known - so that a caller can
distinguish a transfer from an enhancement and either from neither. The three take different amounts
of time, for different reasons, and a bar that cannot tell them apart cannot explain either a long
wait or an instant one.

Each report SHALL carry how far the whole chain has got. That figure SHALL never decrease over one
run, and SHALL reach exactly 1 when the run finishes rather than stopping just short of it. In
particular it SHALL NOT return to the start when one operation ends and the next begins, when a
transfer hands over to a run, or when one pass of a multi-pass upscale hands over to the next.

An operation whose result was already known SHALL still contribute its full share of that figure, so
that a chain of operations that all had known results still fills the bar rather than leaving it
stalled at the start.

Where an operation is served by several passes over progressively larger images, the passes SHALL
contribute to that figure in proportion to the work they do rather than equally, so that the reported
progress does not stall on whichever pass has the most pixels to cover.

#### Scenario: Progress across a chain of several operations

- **WHEN** an image is run through several operations with a progress callback
- **THEN** the reported progress never decreases
- **AND** it does not return to the start when one operation hands over to the next
- **AND** the final value reported is exactly 1

#### Scenario: A model that has to be installed first

- **WHEN** an operation's model is not yet on disk
- **THEN** the reports during the transfer say the model is being installed and name the operation
- **AND** the reports after it say it is being run
- **AND** progress does not return to the start between the two

#### Scenario: A multi-pass upscale

- **WHEN** an upscale is served by two passes, the second covering far more pixels than the first
- **THEN** progress does not return to the start when the first pass hands over to the second
- **AND** the second pass accounts for the larger share of the operation's progress

#### Scenario: An operation whose result was already known

- **WHEN** an operation of a chain is not run because its result was already known
- **THEN** a report names that operation and says its result was already known
- **AND** progress advances by that operation's full share

#### Scenario: A chain every operation of which was already known

- **WHEN** every operation of a chain has a result that is already known
- **THEN** the run reports progress reaching exactly 1
- **AND** it does not finish with the bar still at the start

### Requirement: A run reports the execution provider it was asked for beside the one it ran on

A run SHALL report, to the caller that asked for it, both the execution provider that was requested
and the execution provider each session it built was actually built on.

This exists because the two can differ without anything failing. A provider the machine does not
support, or one that cannot open a particular model, resolves to a CPU run rather than an error —
which is the correct outcome and is required elsewhere in this project. What makes it dangerous is
that it is invisible: a run that was downgraded returns the same image, in the same way, only
slower. Without this report, a benchmark attributes the slowness to the model and a user attributes
it to the application, and neither of them can find out otherwise from anything the run produced.

The report SHALL be part of what a successful run returns, not something a caller has to register
interest in beforehand. A caller that asked for no progress reporting and registered no handler
SHALL still receive it: a downgrade is a property of the run's outcome, not an event in its
progress.

The report SHALL cover every session the run built, not only the first to be downgraded. Whether a
provider can open a model is decided per model, so one chain can legitimately run one operation on a
GPU and the next on the CPU, and a report naming only one of them would describe a run that did not
happen. Where a run built several sessions on one provider, that provider SHALL be reported once
rather than repeated per session.

A run that built no session at all — an empty chain, or one every operation of which was served from
the run cache — SHALL report the provider it was asked for and an empty account of what it ran on,
which is distinguishable from having run on the requested provider. Reporting the requested provider
as though it had been used would claim a measurement that was never taken.

#### Scenario: A run on the provider that was asked for

- **WHEN** a chain is run asking for a provider the machine supports and every model opens on it
- **THEN** the run reports that provider as the one requested
- **AND** it reports that provider, and only it, as the one the run was executed on

#### Scenario: A run the requested provider could not serve

- **WHEN** a chain is run asking for a provider that cannot open the model, and the run is completed
  on the CPU instead
- **THEN** the run succeeds and produces the enhanced image
- **AND** it reports the requested provider beside the CPU, so the downgrade is legible from the
  result alone

#### Scenario: A chain downgraded for one model and not another

- **WHEN** a chain of two operations is run asking for one provider, the first model opens on it and
  the second does not
- **THEN** the run reports both the provider the first ran on and the CPU the second ran on

#### Scenario: A downgrade with no progress handler registered

- **WHEN** a downgraded run is asked for by a caller that registered no progress handler
- **THEN** the downgrade is reported on the result exactly as it is for a caller that registered one

#### Scenario: A run that built no session

- **WHEN** a chain is run whose every operation is served from the run cache, or which names no
  operations at all
- **THEN** the run reports the provider that was requested
- **AND** it reports that nothing was executed, rather than reporting the requested provider as
  having been used

### Requirement: A run stops when the caller asks, and fails as a whole

The system SHALL stop a run when the caller signals that it is no longer wanted, and SHALL report
that it stopped rather than returning a partly-finished picture.

A run SHALL be stoppable while a model is being run, not only between operations: an upscale of a
large photograph is the longest single wait this application asks a user to sit through, and a cancel
that took effect only at the end of it would not be a cancel.

A run SHALL also be stoppable while a model it needs is being **transferred**, and the stop SHALL
reach the transfer itself rather than only the waiting: a model is gigabytes, it is the longest wait
of all, and a download that carried on after the last run wanting it had gone would be the
application spending a user's bandwidth on something nobody asked for. A transfer stopped this way
SHALL leave what it has already moved on disk, so a run that asks for the same model again resumes
rather than starting over.

Where several runs need the same model, the transfer SHALL continue while any of them is still
waiting for it, and SHALL stop when the last one goes: one run's cancellation is not another run's.

Where any operation of a chain fails, the whole run SHALL fail reporting that failure, and SHALL NOT
return the results of the operations that had already succeeded. A failure SHALL be distinguishable
by a caller between a model that could not be put on disk, a model that could not be opened, and a
model that failed while running - the three are reported to a user differently and are acted on
differently.

A run stopped by the caller SHALL be distinguishable from a run that failed, so that a caller does not
tell a user their enhancement broke when they cancelled it themselves.

#### Scenario: A caller that cancels partway through a long run

- **WHEN** the caller signals cancellation while a model is running over an image
- **THEN** the run stops without finishing the remaining work
- **AND** it reports having been cancelled rather than returning a picture

#### Scenario: A caller that cancels while a model is being transferred

- **WHEN** the caller signals cancellation while the model an operation needs is being downloaded,
  and no other run is waiting for that model
- **THEN** the transfer stops rather than running to completion
- **AND** the run reports having been cancelled rather than reporting a failed install
- **AND** what had already been transferred is kept, so the next run for that model resumes onto it

#### Scenario: A second run waiting on the same transfer

- **WHEN** one run is cancelled while another is waiting for the same model to be transferred
- **THEN** the transfer continues
- **AND** the run still waiting is served the model when it arrives

#### Scenario: An operation that fails partway through a chain

- **WHEN** the second operation of a chain fails
- **THEN** the run fails reporting that failure
- **AND** the first operation's result is not returned

#### Scenario: A model that cannot be put on disk

- **WHEN** an operation's model cannot be installed
- **THEN** the run fails reporting that the model could not be installed
- **AND** the failure is distinguishable from one where the model was opened and failed while running

### Requirement: A run does not block the caller's event loop, and holds its models until it finishes

Running a model is seconds to minutes of uninterruptible computation. The system SHALL carry it out
without preventing the caller from continuing to run its own work - including the reporting of the
progress the run itself produces, which a caller that was blocked could not display.

Every model a chain uses SHALL stay open until the whole chain finishes, not merely until the
operation using it returns. Releasing one earlier would let it be reclaimed while a later operation is
still running, and the cost of opening it again would be charged to the run that is already in
progress.

A model in use SHALL NOT be reclaimed by anything, whatever else asks for it, for as long as the run
holds it.

#### Scenario: A long run alongside other work

- **WHEN** a chain is running over a large image
- **THEN** the caller continues to make progress on its own work
- **AND** the progress the run reports reaches the caller while the run is still going

#### Scenario: A model reclaimed while a later operation still needs it

- **WHEN** a chain's first operation finishes and something else asks for that model to be released
- **THEN** the chain's later operations still run against a usable model
- **AND** the model is not opened a second time on the running chain's account

### Requirement: The catalogue builds the operations it describes

The library SHALL be able to turn a model it publishes - named by its family, its codename, the
precision it is to run at, and the values of the parameters it takes - into the enhancement operation
that model performs.

Every model the catalogue publishes SHALL be buildable at every precision the catalogue publishes it
at. A model that can be described but not built is a fault in the library, not a refusal for a caller
to report.

A caller SHALL NOT need to know which models exist, how a model's precision is spelled internally, or
which models admit which precisions. Those are the questions the catalogue already answers, and
answering them again is what lets two answers disagree.

#### Scenario: A published model is built

- **WHEN** a model the catalogue publishes is requested at a precision the catalogue publishes it at
- **THEN** the library answers with the operation that model performs

#### Scenario: Every published model

- **WHEN** every variant in the catalogue is requested at each precision it publishes
- **THEN** every one of them is built

#### Scenario: A model published only at some precisions

- **WHEN** a model is requested at a precision the catalogue does not publish it at
- **THEN** the request is refused, naming the model and the precision

#### Scenario: A codename no family publishes

- **WHEN** a codename the catalogue does not publish is requested
- **THEN** the request is refused, naming the family and the codename

### Requirement: A new model reaches the interface without a second registration

Adding a model to the library SHALL make it available to run without any other part of the project
naming it.

This is what the catalogue is for: a front end reads it rather than restating the vocabulary, and a
front end that had to restate it in order to *run* a model would be the same duplication in a less
visible place.

#### Scenario: A model is added to the library

- **WHEN** a new variant is published by the catalogue
- **THEN** it is offered by every chooser built from the catalogue and can be run, with no change
  outside the library
