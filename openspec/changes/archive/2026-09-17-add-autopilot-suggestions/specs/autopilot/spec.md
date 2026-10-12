## Purpose

What the application proposes for a photograph before anyone has touched it: which signals are read from the image,
what each one concludes, and how a conclusion that could not be reached is reported rather than mistaken for a
conclusion that nothing is needed. This is the first thing a user sees happen to a picture they just opened, so a
suggestion that is wrong, missing or unexplained is the application's first impression.

## ADDED Requirements

### Requirement: An image is analysed and what it calls for comes back

The system SHALL analyse a picture and return the set of enhancements that picture calls for. Each suggestion SHALL
name the **family** of enhancement and carry only what the image itself determines about it.

A suggestion SHALL NOT name a model variant or a precision. Which model to run is a standing user preference that
this library does not hold, and a suggestion that named one would either invent a default or force the preference
into the library to be stored twice.

An image that calls for nothing SHALL produce an empty set rather than a refusal. A photograph that needs no work is
the answer, not the absence of one.

#### Scenario: A photograph that calls for work

- **WHEN** an image is analysed and one or more signals conclude it needs an enhancement
- **THEN** a suggestion naming each of those families comes back
- **AND** no suggestion names a model variant or a precision

#### Scenario: A photograph that calls for nothing

- **WHEN** an image is analysed and no signal concludes it needs anything
- **THEN** an empty set of suggestions comes back, and the analysis is not a failure

### Requirement: Nothing is suggested that the application cannot run

The system SHALL only suggest an enhancement whose family it can actually apply. A family the enhancement path would
refuse SHALL NOT appear in a suggestion, whatever an image-derived signal for it would conclude.

A suggestion is an instruction the user is invited to accept with one gesture. One that is refused the moment it is
accepted is worse than no suggestion at all: the user is told the photograph needs something and then told the
application will not do it.

This is a standing rule over whatever the application can run at the time, not a fixed list. A family whose pipeline
is added later becomes suggestible by that change without this requirement changing.

#### Scenario: A family with no pipeline

- **WHEN** an image is analysed and a family the enhancement path refuses would otherwise have been indicated
- **THEN** no suggestion for that family comes back

#### Scenario: A family whose pipeline is added later

- **WHEN** a family that could not previously be applied gains the ability to be applied
- **THEN** a suggestion for it may come back from an analysis, with no change to this rule

### Requirement: A photograph with faces in it calls for face recovery

The system SHALL read a face signal by detecting the faces in the image. Face recovery SHALL be suggested when at
least one face is found, and SHALL NOT be suggested when none is.

The detection SHALL run at the precision a suggested face-recovery enhancement would itself be added at. A suggestion
is accepted at its model's highest quality tier, so detecting at any other precision would install and open a second
detection graph to re-answer the question the first one has already answered, on the same image, moments earlier.

Reading this signal SHALL go through the same path a caller uses to detect faces directly, so that the detection an
analysis performs and the detection the accepted suggestion then needs are one run rather than two.

#### Scenario: A portrait

- **WHEN** an image containing at least one detectable face is analysed
- **THEN** face recovery is among the suggestions

#### Scenario: A landscape with nobody in it

- **WHEN** an image containing no detectable face is analysed
- **THEN** face recovery is not among the suggestions, and the analysis is not a failure

#### Scenario: The same image analysed again

- **WHEN** an image that has already been analysed is analysed again
- **THEN** the face signal is answered from the kept result of the first detection rather than by running the model
  again

#### Scenario: The suggestion is accepted

- **WHEN** a face-recovery suggestion is accepted and the faces it needs are asked for
- **THEN** they are the faces the analysis already found, rather than a second detection at a second precision

### Requirement: An image's size decides whether it calls for upscaling and by how much

The system SHALL read a geometry signal from the image's pixel count alone, and SHALL suggest upscaling with the
scale that pixel count calls for:

| Pixel count                      | Suggestion      |
|----------------------------------|-----------------|
| at most 1,048,576 (1 MP)         | upscale at 4x   |
| 1,048,577 to 4,194,304 (4 MP)    | upscale at 2x   |
| above 4,194,304                  | none            |

Both boundaries are inclusive of the lower band, and are exact: 1 MP and 4 MP are common photograph sizes, so a
boundary off by one pixel changes what a great many images are told they need.

The scale SHALL be part of the suggestion rather than left to the caller. Whether to upscale and how much are one
decision read off one number, and splitting them across the library and each front end is how two front ends come to
disagree about what the same photograph needs.

#### Scenario: A small photograph

- **WHEN** an image of at most 1,048,576 pixels is analysed
- **THEN** upscaling at 4x is among the suggestions

#### Scenario: A photograph exactly at the lower boundary

- **WHEN** an image of exactly 1,048,576 pixels is analysed
- **THEN** upscaling at 4x is suggested, not 2x

#### Scenario: A mid-sized photograph

- **WHEN** an image of more than 1,048,576 and at most 4,194,304 pixels is analysed
- **THEN** upscaling at 2x is among the suggestions

#### Scenario: A photograph exactly at the upper boundary

- **WHEN** an image of exactly 4,194,304 pixels is analysed
- **THEN** upscaling at 2x is suggested

#### Scenario: A large photograph

- **WHEN** an image of more than 4,194,304 pixels is analysed
- **THEN** upscaling is not among the suggestions

### Requirement: An image with no area calls for nothing

The system SHALL suggest nothing for an image with no area, and SHALL NOT attempt to read any signal that would run a
model over it.

There is no enhancement of nothing: the enhancement path already refuses such an image and the detection path already
refuses to run over one. Suggesting work that would be refused the moment it was accepted is the failure the
suggestibility rule above exists to prevent, and running a detection only to be refused would report the refusal as a
signal that could not be read, which would describe a degenerate input as a fault.

The reference implementation suggests upscaling for an image with no area, because its size test is a comparison
against an upper bound that zero passes. Its two other heuristics each guard against an empty image explicitly, so
that outcome is incidental rather than intended, and is not reproduced.

#### Scenario: An image with no pixels

- **WHEN** an image with no area is analysed
- **THEN** no suggestion comes back
- **AND** no detection is run over it
- **AND** the analysis is not reported as a failure or as incomplete

### Requirement: Suggestions come back in one canonical family order

The system SHALL return suggestions in the single canonical order of families the library names, rather than in the
order the signals happen to be read in.

Ordering SHALL be a property of the family, so that a family added later takes its place in that one order without
the ordering having to be revisited.

This is deliberately *not* stated as "the order the enhancement path applies them". The enhancement path applies
operations in the order its caller supplies them and imposes no family order at all, so there is no such order in
the library to defer to. The reference implementation does hold one list that is both the order enhancements are
presented in and the order they are applied in — because an add menu offering them in one order while the pipeline
ran them in another is a silent inconsistency — and that list agrees with the library's canonical order on every
family that can currently be suggested. Where the two disagree about a family that cannot yet be suggested, the
disagreement SHALL be recorded as a checked fact rather than described in prose, so that the change which makes that
family suggestible has to settle it rather than discover it.

#### Scenario: Several suggestions at once

- **WHEN** an image is analysed and more than one signal concludes it needs an enhancement
- **THEN** the suggestions come back in the library's canonical family order, whatever order their signals were read
  in

#### Scenario: A family whose place the reference disagrees about

- **WHEN** the library's canonical family order and the reference's present-and-apply order disagree about where a
  family belongs
- **THEN** that disagreement is pinned by a check that fails if either order moves
- **AND** every family that can currently be suggested is in the same relative order in both

### Requirement: A signal that could not be read is reported rather than left to look like a conclusion

Where a signal cannot be read at all — the model it needs cannot be installed or opened, or the run fails — the
system SHALL still return the suggestions from the signals it could read, and SHALL report alongside them that the
analysis was incomplete, naming what went wrong.

A signal that cannot be read and a signal that concluded "nothing needed" produce the same absence from the
suggestion set, and only one of them means the photograph genuinely needs nothing. A caller that cannot tell them
apart cannot explain to a user why a portrait was given no face-recovery suggestion.

Failing the whole analysis instead SHALL NOT be done: the analysis runs unprompted when a picture is opened, so a
failure raised to the user is an error about something they never asked for, while the signals that did succeed are
still worth acting on.

#### Scenario: The detection model cannot be obtained

- **WHEN** an image is analysed and the face signal's model cannot be installed or opened
- **THEN** the suggestions from every other signal still come back
- **AND** the result reports that the analysis was incomplete, and why
- **AND** the call is not a failure

#### Scenario: Every signal was read

- **WHEN** an image is analysed and every signal is read successfully
- **THEN** the result reports no incompleteness, whether or not any enhancement was suggested

### Requirement: A cancelled analysis returns nothing

Where the caller cancels the analysis, or the application is shutting down, the system SHALL fail the call rather
than return the suggestions it had already gathered.

This is the one failure that is not an incomplete analysis. A signal that could not be read leaves a question
unanswered; a cancellation withdraws the question, and answering a withdrawn question partially is worse than not
answering it — the picture the suggestions describe is the one the user has already moved on from.

#### Scenario: The caller cancels mid-analysis

- **WHEN** an analysis is cancelled while the face signal is running
- **THEN** the call fails with a cancellation
- **AND** no suggestions come back, including the ones already determined

### Requirement: An analysis says what it suggested

An analysis installs models, opens sessions and runs inference without the user having asked for anything, so an
account of it SHALL be in the log a user attaches without being asked to reproduce anything.

The system SHALL record, at the default verbosity, what an analysis concluded — how many suggestions it produced and
which families they name. Where a signal could not be read, the system SHALL record which signal it was and the
reason it gave, at a verbosity that reaches the same file.

#### Scenario: An analysis completes

- **WHEN** an image is analysed while a sink is installed
- **THEN** a record at the default verbosity names the families suggested

#### Scenario: An analysis is incomplete

- **WHEN** a signal cannot be read during an analysis
- **THEN** a record names the signal that could not be read and the reason
- **AND** it is present whether or not the caller reports the incompleteness to a user
