# autopilot Specification

## Purpose

What the application proposes for a photograph before anyone has touched it: which signals are read from the image,
what each one concludes, and how a conclusion that could not be reached is reported rather than mistaken for a
conclusion that nothing is needed. This is the first thing a user sees happen to a picture they just opened, so a
suggestion that is wrong, missing or unexplained is the application's first impression.

## Requirements

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

### Requirement: A photograph whose exposure is wrong calls for light adjustment

The system SHALL read a light signal from the distribution of the image's brightness, and SHALL suggest light
adjustment when that distribution shows the photograph is **underexposed** or **overexposed**:

- **Underexposed:** the whole tonal range is compressed into the shadows, so even the brightest part of the
  photograph never gets bright.
- **Overexposed:** the whole tonal range is lifted off the shadows, so even the darkest part of the photograph never
  gets dark.

The system SHALL NOT suggest light adjustment only because a photograph is dark or bright overall. A night scene or a
low-key portrait that still reaches its highlights is exposed by intent, and so is a high-key scene that still has
shadows. A large area of pure black or pure white is a composition, not evidence of an exposure error.

Where the evidence is borderline, the system SHALL prefer not suggesting. An analysis runs unprompted on a photograph
the user has just opened, and a wrong suggestion there does more harm than a missed one.

The reference implementation suggests light adjustment when the mean brightness is extreme *and* over a third of the
pixels are clipped to near black or near white. That rule is not reproduced. It misses ordinary underexposed
photographs, which are compressed rather than clipped, and it fires on correctly exposed low-key and high-key ones,
which are clipped by intent.

#### Scenario: An underexposed photograph

- **WHEN** a correctly exposed photograph is darkened by two stops and analysed
- **THEN** light adjustment is among the suggestions

#### Scenario: An overexposed photograph

- **WHEN** a correctly exposed photograph is brightened by two stops and analysed
- **THEN** light adjustment is among the suggestions

#### Scenario: A correctly exposed photograph

- **WHEN** a correctly exposed photograph is analysed
- **THEN** light adjustment is not among the suggestions

#### Scenario: A night scene with lights in it

- **WHEN** a photograph that is mostly black but contains bright light sources is analysed
- **THEN** light adjustment is not among the suggestions

#### Scenario: A subject on a white background

- **WHEN** a photograph that is mostly pure white but has a subject with its own shadows is analysed
- **THEN** light adjustment is not among the suggestions

### Requirement: A photograph with a colour cast calls for colour balance

The system SHALL read a colour signal and SHALL suggest colour balance when the photograph's colours are shifted
**as a whole** towards one hue, as estimated from the colour of the light the photograph was taken under.

The system SHALL NOT suggest colour balance only because a photograph is colourful, or because one colour dominates
its content. A forest, a sunset or a red car is not a cast.

The signal SHALL NOT count a pixel that has no hue — equal in all three channels — as belonging to any hue. It SHALL
NOT take its evidence from pixels too dark or too clipped to carry any. Its estimate of the light SHALL move with the
cast being measured, so that a stronger cast moves it further from neutral, never less.

Where the evidence is borderline, the system SHALL prefer not suggesting, for the same reason light adjustment does.

The reference implementation's colour heuristic is not reproduced. It counts pixels with no hue as red, so it flags
greyscale, overexposed and white-background photographs as skewed. Its neutral-pixel test uses a fixed saturation
cutoff, so a strong cast leaves too few pixels to measure and is detected *less* often than a weak one: its evidence
moves the wrong way as the cast grows.

#### Scenario: A warm cast

- **WHEN** a correctly balanced photograph is re-rendered under a tungsten white balance and analysed
- **THEN** colour balance is among the suggestions

#### Scenario: A cool cast

- **WHEN** a correctly balanced photograph is re-rendered under a shade white balance and analysed
- **THEN** colour balance is among the suggestions

#### Scenario: A correctly balanced photograph

- **WHEN** a correctly balanced photograph is analysed
- **THEN** colour balance is not among the suggestions

#### Scenario: A greyscale photograph

- **WHEN** a photograph whose every pixel is equal in all three channels is analysed
- **THEN** colour balance is not among the suggestions

#### Scenario: A stronger cast is not missed where a weaker one is found

- **GIVEN** a correctly balanced photograph that is not suggested colour balance
- **WHEN** it is re-rendered with a cast and that rendering is suggested colour balance
- **THEN** the same photograph re-rendered with a stronger cast of the same hue is suggested colour balance too

### Requirement: The light and colour signals read the pixels and nothing else

The system SHALL read the light and colour signals from the image's own pixels. It SHALL NOT run a model, install
anything or touch the network to read them, and neither signal SHALL be a reason an analysis is reported incomplete.

The two signals SHALL be read from one pass over the image. The pass SHALL cost no more for a large photograph than
for a photograph of a bounded size: the signals are statistics of the whole frame, and they do not need every pixel of
a large one.

A photograph that carries 16 bits per channel SHALL be measured at 16 bits. Transparent pixels SHALL NOT contribute to
either signal: a subject cut out onto a transparent background is not a dark photograph.

A cancelled analysis SHALL still return nothing, whichever signal it was reading when it was cancelled.

#### Scenario: No model is available

- **WHEN** an image is analysed and no model can be installed or opened
- **THEN** the light and colour signals are still read and their suggestions still come back

#### Scenario: A very large photograph

- **WHEN** a photograph of tens of megapixels is analysed
- **THEN** the light and colour signals reach the same conclusions they would reach for the same photograph at a
  bounded size

#### Scenario: A cut-out on a transparent background

- **WHEN** a correctly exposed subject surrounded by fully transparent pixels is analysed
- **THEN** light adjustment is not among the suggestions

### Requirement: A photograph with visible grain calls for denoise

The system SHALL read a noise signal and SHALL suggest denoise when the photograph shows **visible grain**: random
variation from pixel to pixel, in its brightness or in its colour, strong enough to see where the photograph ought to
be smooth.

The system SHALL judge grain by the parts of the photograph with the least detail of their own. Where a photograph is
detailed, fine texture and grain look alike, and a photograph full of grass, fabric or foliage is not noisy.

Grain that is only in the colour — blotches of hue over surfaces of even brightness — SHALL count as grain. It is a
common form of high-sensitivity sensor noise, and the denoise models remove it.

The system SHALL NOT suggest denoise for noise too faint to see. Every photograph carries some measurable noise, and a
suggestion on all of them would say nothing.

Where the evidence is borderline, the system SHALL prefer not suggesting, for the same reason light adjustment does.

The reference implementation reads no noise signal and never suggests denoise. This signal is new rather than ported.

#### Scenario: A grainy photograph

- **WHEN** a clean photograph with strong sensor-like noise added to it is analysed
- **THEN** denoise is among the suggestions

#### Scenario: Grain in the colour alone

- **WHEN** a clean photograph with strong noise added to its colour but not to its brightness is analysed
- **THEN** denoise is among the suggestions

#### Scenario: A clean photograph

- **WHEN** a clean photograph is analysed
- **THEN** denoise is not among the suggestions

#### Scenario: A clean photograph full of fine texture

- **WHEN** a clean photograph whose surfaces are covered in fine, high-contrast texture is analysed
- **THEN** denoise is not among the suggestions

#### Scenario: Stronger grain is not missed where weaker grain is found

- **GIVEN** a clean photograph that is not suggested denoise
- **WHEN** it has noise added and that version is suggested denoise
- **THEN** the same photograph with stronger noise of the same kind added is suggested denoise too

### Requirement: A genuinely blurred photograph calls for sharpen

The system SHALL read a sharpness signal and SHALL suggest sharpen when the photograph is **genuinely blurred**: soft
across the whole frame, so that **even its sharpest part** lacks the detail a focused, steady exposure would have
recorded. Camera shake, subject motion and missed focus all produce this, and the sharpen models repair all three.

The system SHALL NOT suggest sharpen only because much of a photograph is blurred. A portrait or a close-up whose
background is out of focus around a sharp subject is blurred by intent, as a night scene is dark by intent.

The system SHALL NOT suggest sharpen for a photograph with too little detail to judge — a clear sky, a plain wall, a
fog — since it has no edge that could be sharp or soft. It SHALL NOT suggest sharpen for a photograph that is only
slightly soft when viewed pixel for pixel, as most photographs from high-resolution cameras are.

Grain SHALL NOT be read as sharpness. A blurred photograph with visible grain SHALL be judged on its blur, so that it
is suggested sharpen as well as denoise rather than denoise alone.

Where the evidence is borderline, the system SHALL prefer not suggesting.

The reference implementation reads no sharpness signal and never suggests sharpen. This signal is new rather than
ported.

#### Scenario: An out-of-focus photograph

- **WHEN** a sharp photograph blurred as a lens out of focus would blur it is analysed
- **THEN** sharpen is among the suggestions

#### Scenario: A shaken photograph

- **WHEN** a sharp photograph blurred along one direction, as camera shake would blur it, is analysed
- **THEN** sharpen is among the suggestions

#### Scenario: A sharp photograph

- **WHEN** a sharp photograph is analysed
- **THEN** sharpen is not among the suggestions

#### Scenario: A sharp subject against a blurred background

- **WHEN** a photograph whose centre is sharp and whose surroundings are heavily out of focus is analysed
- **THEN** sharpen is not among the suggestions

#### Scenario: A photograph with no detail

- **WHEN** a photograph of smooth gradients with no edges in it is analysed
- **THEN** sharpen is not among the suggestions, and the analysis is not a failure

#### Scenario: A blurred photograph with grain in it

- **WHEN** a sharp photograph is blurred out of focus, strong noise is then added to it, and it is analysed
- **THEN** sharpen and denoise are both among the suggestions

#### Scenario: Stronger blur is not missed where weaker blur is found

- **GIVEN** a sharp photograph that is not suggested sharpen
- **WHEN** it is blurred and that version is suggested sharpen
- **THEN** the same photograph blurred more strongly in the same way is suggested sharpen too

### Requirement: The noise and sharpness signals read the pixels at full resolution and nothing else

The system SHALL read the noise and sharpness signals from the image's own pixels. It SHALL NOT run a model, install
anything or touch the network to read them, and neither signal SHALL be a reason an analysis is reported incomplete.

Both signals SHALL be read at the photograph's **own resolution**. Grain and blur exist only between neighbouring
pixels: a reduced copy of a grainy or blurred photograph looks clean and sharp, so a conclusion drawn from one is a
conclusion about a different photograph.

The two signals SHALL be read from one pass, and that pass SHALL cost no more for a large photograph than for a
photograph of a bounded size: it reads parts of a large photograph at full resolution rather than all of it.

A photograph that carries 16 bits per channel SHALL be measured at 16 bits. Transparent pixels SHALL NOT contribute to
either signal.

A photograph too small to hold the smallest region either signal measures SHALL conclude nothing for that signal,
rather than a guess.

A cancelled analysis SHALL still return nothing, whichever signal it was reading when it was cancelled.

#### Scenario: No model is available

- **WHEN** an image is analysed and no model can be installed or opened
- **THEN** the noise and sharpness signals are still read and their suggestions still come back

#### Scenario: A very large photograph

- **WHEN** a grainy, or a blurred, photograph of tens of megapixels is analysed
- **THEN** it is suggested denoise, or sharpen, as a crop of it at its own resolution is

#### Scenario: A cut-out on a transparent background

- **WHEN** a clean, sharp subject surrounded by fully transparent pixels is analysed
- **THEN** neither denoise nor sharpen is among the suggestions

#### Scenario: A photograph too small to measure

- **WHEN** a photograph smaller than the region the noise and sharpness signals measure is analysed
- **THEN** neither denoise nor sharpen is among the suggestions, and the analysis is not a failure or incomplete

### Requirement: A grayscale photograph calls for colorization

The system SHALL read a monochrome signal and SHALL suggest colorization for a **grayscale** photograph: one that
carries no colour at all. That includes a photograph stored with a single brightness channel, one stored as colour
with its three channels equal, and one of either kind that was saved as colour and compressed, whose encoder left a
trace of rounding off grey.

The system SHALL NOT be required to suggest colorization for a black-and-white print that was toned (sepia, selenium,
cyanotype), split-toned, yellowed with age or scanned in colour. Measured on real scans of such prints, the colour
their brightness does not explain (stains, uneven toning, the scanner's noise) is more than the haziest and darkest
colour photographs carry, so no statistic of this kind can find them without flagging those photographs. Leaving them
alone is the accepted cost.

Colorization replaces a photograph's colour rather than blending with it, so a false alarm destroys colour that was
really there. The system SHALL NOT suggest colorization for a colour photograph, **including a muted one**: fog, snow,
night, overcast, or a scene of one dominant hue. Where the evidence is borderline, it SHALL prefer not suggesting.

The system SHALL judge the photograph as a whole, and a small coloured subject on a grey scene SHALL NOT be a reason to
suggest colorization.

The system SHALL read this signal from the image's own pixels. It SHALL NOT run a model, install anything or touch the
network to read it, and it SHALL NOT be a reason an analysis is reported incomplete. It SHALL cost no more for a large
photograph than for a photograph of a bounded size. A photograph that carries 16 bits per channel SHALL be measured at
16 bits, and transparent pixels SHALL NOT contribute. A photograph with nothing left to measure SHALL conclude nothing
for this signal rather than a guess. A cancelled analysis SHALL still return nothing.

The reference implementation reads no monochrome signal and never suggests colorization. This signal is new rather
than ported.

#### Scenario: A neutral black-and-white photograph

- **WHEN** a photograph whose every pixel is equal in all three channels is analysed
- **THEN** colorization is among the suggestions

#### Scenario: A photograph stored with a single channel

- **WHEN** a photograph stored as grayscale, with one brightness channel and no colour channels, is analysed
- **THEN** colorization is among the suggestions

#### Scenario: A grayscale photograph saved as a colour JPEG

- **WHEN** a grayscale photograph that was saved as a colour JPEG is analysed
- **THEN** colorization is among the suggestions

#### Scenario: A colour photograph

- **WHEN** a colour photograph is analysed
- **THEN** colorization is not among the suggestions

#### Scenario: A muted colour photograph

- **WHEN** a colour photograph of a scene with little colour, such as fog, snow or an overcast street, is analysed
- **THEN** colorization is not among the suggestions

#### Scenario: A grayscale photograph that is also underexposed

- **WHEN** a grayscale photograph that the light signal concludes is underexposed is analysed
- **THEN** both colorization and light adjustment are among the suggestions, colorization first

#### Scenario: No model is available

- **WHEN** a grayscale photograph is analysed and no model can be installed or opened
- **THEN** colorization is still among the suggestions

#### Scenario: A cut-out on a transparent background

- **WHEN** a colour subject surrounded by fully transparent pixels is analysed
- **THEN** colorization is not among the suggestions

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

### Requirement: Suggestions come back in the reference's family order

The system SHALL return suggestions in the single canonical order of families the library names, rather than in the
order the signals happen to be read in.

Ordering SHALL be a property of the family, so that a family added later takes its place in that one order without
the ordering having to be revisited.

This is deliberately *not* stated as "the order the enhancement path applies them". The enhancement path applies
operations in the order its caller supplies them and imposes no family order at all, so there is no such order in
the library to defer to. The reference implementation does hold one list that is both the order enhancements are
presented in and the order they are applied in, because an add menu offering them in one order while the pipeline
ran them in another is a silent inconsistency: denoise, face recovery, colorization, light adjustment, colour balance,
sharpen, upscale.

The library's canonical order SHALL agree with that list on **every** family a user can add. Colorization comes before
light adjustment and colour balance: colorization replaces the photograph's colour, so exposure and colour are
corrected on the colour it produced rather than on colour it will discard. The agreement SHALL be a checked fact
rather than described in prose, so that a change moving either order has to settle the disagreement rather than
introduce it silently.

#### Scenario: Several suggestions at once

- **WHEN** an image is analysed and more than one signal concludes it needs an enhancement
- **THEN** the suggestions come back in the library's canonical family order, whatever order their signals were read
  in

#### Scenario: Colorization beside light adjustment and colour balance

- **WHEN** an image is analysed and colorization, light adjustment and colour balance are all suggested
- **THEN** colorization comes back before light adjustment, and light adjustment before colour balance

#### Scenario: The reference's order

- **WHEN** the library's canonical family order is compared with the reference's present-and-apply order, over the
  families a user can add
- **THEN** the two are the same order
- **AND** a check fails if either order moves

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

### Requirement: An analysis can be narrowed to a set of families

The system SHALL let the caller of an analysis name the enhancement families it wants checked, or name none.

Where no set is named, the system SHALL check every family, exactly as an analysis did before the set existed.

Where a set is named, the system SHALL suggest only families in that set. It SHALL NOT read any signal that serves only
families outside the set. The face signal SHALL NOT run unless face recovery is in the set. A signal that more than one
family needs SHALL be read when any one of them is in the set, and SHALL still suggest only the families in the set.

An empty set SHALL check nothing. It SHALL return no suggestions, report the analysis complete, and not be a failure.
An empty set does not mean "no preference": a caller that asks for no families is not charged for every one.

A family in the set that no signal can suggest, such as face detection, SHALL match nothing and SHALL NOT be an error.

A signal that was not read because its families were not asked for SHALL NOT make the analysis incomplete. Only a
signal the analysis tried to read and could not can make it incomplete.

Narrowing SHALL NOT change anything else: suggestions still come back in the canonical family order, a cancelled
analysis still returns nothing, an image with no area still calls for nothing, and a family the application cannot run
is still never suggested.

The record the system keeps of each analysis SHALL name the families it was asked to check, or record that it was not
narrowed. Without that, a user's log cannot tell a family that was never checked from one that was checked and needed
nothing.

The reference implementation has no way to narrow an analysis and always checks every family. Narrowing is new rather
than ported; an analysis that is not narrowed behaves as the reference does.

#### Scenario: No set is named

- **WHEN** an image is analysed without a set of families
- **THEN** every family is checked, and the suggestions are the same as for an analysis before narrowing existed

#### Scenario: A set of two families

- **WHEN** an image that calls for upscaling, colorization, light adjustment and face recovery is analysed with the
  set upscale and colorization
- **THEN** exactly upscale and colorization come back, in the canonical family order
- **AND** no face detection is run

#### Scenario: Face recovery is not asked for and the detection model cannot be obtained

- **WHEN** an image is analysed with a set that does not include face recovery, and the face signal's model cannot be
  installed or opened
- **THEN** the analysis is reported complete

#### Scenario: Face recovery is asked for and the detection model cannot be obtained

- **WHEN** an image is analysed with a set that includes face recovery, and the face signal's model cannot be
  installed or opened
- **THEN** the suggestions from the other families in the set still come back
- **AND** the result reports that the analysis was incomplete, and why

#### Scenario: A family whose signal also serves another family

- **WHEN** a blurred, grainy photograph is analysed with the set sharpen
- **THEN** sharpen comes back and denoise does not

#### Scenario: An empty set

- **WHEN** an image is analysed with an empty set of families
- **THEN** no suggestion comes back
- **AND** no signal is read and no detection is run
- **AND** the analysis is not reported as a failure or as incomplete

#### Scenario: A set naming only face detection

- **WHEN** an image is analysed with a set holding only face detection
- **THEN** no suggestion comes back, and the analysis is not reported as a failure or as incomplete

#### Scenario: A narrowed analysis is cancelled

- **WHEN** an analysis narrowed to face recovery is cancelled while the face signal is running
- **THEN** the call fails with a cancellation, and no suggestions come back

#### Scenario: The record of a narrowed analysis

- **WHEN** an image is analysed with a set of families while a sink is installed
- **THEN** a record at the default verbosity names the families asked for, beside the families suggested
