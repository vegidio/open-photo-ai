# gui-autopilot Specification

## Purpose

How Autopilot runs in the window, from both sides of the boundary. On the window's side: when an analysis is asked
for (the current photograph, with Autopilot on, that has never had a list of enhancements), how one in flight is
shown (a row in place of the sidebar's list), how its answer becomes that photograph's enhancements (in one batch,
as the add menu would build them, with face recovery kept only where a face is small enough to restore), and what
stops one (closing the photograph, switching Autopilot off, or framing it differently). On the boundary: which
photograph and framing an analysis sees, what comes back across it, and how an analysis the window no longer wants
is stopped.

## Requirements

### Requirement: The interface can ask for an open photograph to be analysed

The application SHALL let its interface ask for an open photograph to be analysed, naming the enhancement families
the analysis may suggest. The answer SHALL be the enhancements the photograph calls for, among those families only.

Each suggestion SHALL name its family. A suggested upscale SHALL carry the scale the analysis chose for it. No
suggestion SHALL carry a model, a precision, or any other value: which model runs is the user's standing
preference, which the interface holds.

The suggestions SHALL come back in the order the application applies enhancements in, whatever order the interface
named the families in.

An empty set of families SHALL be answered with no suggestions and SHALL NOT be a failure. A photograph that calls
for nothing SHALL likewise be answered with no suggestions rather than a failure.

#### Scenario: A photograph that calls for work

- **WHEN** the interface asks for an open, small, grayscale photograph to be analysed, naming upscale and colorization
- **THEN** a colorization suggestion and an upscale suggestion come back, colorization first
- **AND** the upscale suggestion carries its scale
- **AND** neither names a model or a precision

#### Scenario: A family the interface did not name

- **WHEN** the interface asks for a photograph to be analysed without naming face recovery, and the photograph has
  faces in it
- **THEN** no face-recovery suggestion comes back

#### Scenario: No families named

- **WHEN** the interface asks for a photograph to be analysed and names no families
- **THEN** no suggestions come back, and the request is not a failure

#### Scenario: A photograph that needs nothing

- **WHEN** the interface asks for a photograph to be analysed and it calls for none of the families named
- **THEN** no suggestions come back, and the request is not a failure

### Requirement: An analysis sees the photograph as the user has framed it

The analysis SHALL be made on the photograph as it is framed at the time of the request: the crop, rotation and flip
the interface sends are applied before anything is read from it. A request that sends no framing SHALL be analysed on
the whole photograph.

Faces found by an analysis SHALL be the faces a request to detect faces in that photograph at that framing would
find. A request to detect faces made after an analysis, for the same photograph at the same framing, SHALL be
answered without running the detector a second time.

#### Scenario: A cropped photograph

- **WHEN** the interface asks for a photograph to be analysed with a crop that leaves a small region of it
- **THEN** the analysis is made on that region, and an upscale suggested for it carries the scale that region's size
  calls for

#### Scenario: Faces asked for after an analysis

- **WHEN** a photograph is analysed with face recovery among the families, and the interface then asks for the faces
  in that photograph at the same framing
- **THEN** the faces come back without the detector being run again

### Requirement: Only a photograph the user has opened can be analysed

A request to analyse SHALL name an image the user has opened. A request naming anything else SHALL be refused before
any file is read or any model is fetched. A request made before the application has finished starting SHALL be
refused, and so SHALL a request for an opened photograph whose file can no longer be read.

Each of these refusals SHALL be distinguishable from the others, and from an analysis that failed while it ran.

#### Scenario: An image that was never opened

- **WHEN** the interface asks for an analysis naming an identity the application never admitted
- **THEN** the request is refused as an unknown source, and nothing is read or fetched

#### Scenario: A file removed after it was opened

- **WHEN** the interface asks for an analysis of an opened photograph whose file is no longer on disk
- **THEN** the request is refused as an unreadable source

#### Scenario: Before start-up has finished

- **WHEN** the interface asks for an analysis before the application has finished starting
- **THEN** the request is refused as not ready

### Requirement: An analysis the interface no longer wants can be stopped

The interface SHALL name each analysis it asks for, and SHALL be able to stop an analysis by that name. A stopped
analysis SHALL answer with a refusal that says it was stopped, distinct from a failure, and SHALL NOT answer any
suggestions, including ones already determined.

A stop that reaches the application before the analysis it names has started SHALL still stop that analysis.

A stop naming an analysis that has already finished, or that the application has never heard of, SHALL do nothing
and SHALL NOT be a failure. Stopping one analysis SHALL NOT affect any other.

#### Scenario: Stopped while running

- **WHEN** the interface stops an analysis while its face detection is running
- **THEN** that analysis answers that it was stopped, with no suggestions

#### Scenario: The stop arrives first

- **WHEN** the interface asks for an analysis and stops it, and the stop reaches the application first
- **THEN** the analysis answers that it was stopped, and no model is run for it

#### Scenario: A stop for a finished analysis

- **WHEN** the interface stops an analysis that has already answered
- **THEN** nothing happens, and the stop is not a failure

#### Scenario: Two analyses in flight

- **WHEN** two photographs are being analysed and the interface stops one of them
- **THEN** the other goes on and answers its suggestions

### Requirement: Analysing does not disturb enhancing

An analysis SHALL NOT stop, replace or release an enhancement run or an enhanced result, and an enhancement run SHALL
NOT stop an analysis. More than one analysis SHALL be allowed to be in flight at once.

#### Scenario: An analysis during a run

- **WHEN** a photograph is being enhanced and the interface asks for another photograph to be analysed
- **THEN** the enhancement run goes on, and its result is still served when it finishes

### Requirement: An incomplete analysis answers what it could read

Where one of an analysis's signals could not be read, such as a face detector that could not be fetched, the request
SHALL still answer the suggestions from the signals that were read, and SHALL NOT be a failure. The reason SHALL be
recorded in the application's log. It SHALL NOT be part of the answer, which carries only what the photograph calls
for.

#### Scenario: The face detector cannot be fetched

- **WHEN** a small photograph is analysed with upscale and face recovery among the families, and the face detector
  cannot be fetched
- **THEN** the upscale suggestion comes back and the request is not a failure
- **AND** the log records that the face signal could not be read, and why

### Requirement: A photograph is analysed when it is looked at with Autopilot on and nothing done to it yet

The window SHALL ask for a photograph to be analysed when all three of these hold at once:

- the photograph is the current one
- Autopilot is on
- the photograph has never had a list of enhancements

An analysis SHALL be asked for whenever that becomes true, whichever of the three changed last. That includes the
user turning Autopilot on while looking at such a photograph.

A photograph that is open but not current SHALL NOT be analysed, with one exception: an export reaching it. When a
batch export reaches a photograph that has never had a list of enhancements, with Autopilot on, that photograph SHALL
be analysed before it is exported, whether or not it is current. What the analysis suggests SHALL be added to its
list by the same rules as any other analysis, and the export SHALL run what the list then holds. Opening a folder of
photographs SHALL NOT analyse every one of them: each is analysed when the user comes to it, or when an export
reaches it.

A photograph SHALL have at most one analysis in flight. Returning to a photograph whose analysis is still under way
SHALL NOT ask for a second one. An export that reaches a photograph whose analysis is already under way SHALL wait
for that analysis rather than ask for a second one.

#### Scenario: A photograph opened with Autopilot on

- **WHEN** a photograph is opened into an empty window and becomes the current one, with Autopilot on
- **THEN** it is analysed

#### Scenario: Several photographs opened at once

- **WHEN** five photographs are opened at once with Autopilot on
- **THEN** only the one that becomes current is analysed, and each of the others is analysed when the user moves to it

#### Scenario: Autopilot off

- **WHEN** a photograph becomes current with Autopilot off
- **THEN** it is not analysed

#### Scenario: Autopilot turned on over a photograph

- **WHEN** the user turns Autopilot on while looking at a photograph that has never had a list of enhancements
- **THEN** it is analysed

#### Scenario: Back to a photograph still being analysed

- **WHEN** the user moves off a photograph while it is being analysed and comes back before the analysis answers
- **THEN** no second analysis is asked for, and the first one's answer is applied when it arrives

#### Scenario: A never-visited photograph reached by an export

- **WHEN** Autopilot is on, and a batch export reaches a picked photograph the user never looked at
- **THEN** it is analysed, its suggestions are added to its list, and it is exported with them

#### Scenario: An export with Autopilot off

- **WHEN** Autopilot is off and a batch export reaches a photograph that has never had a list of enhancements
- **THEN** it is not analysed, and it is exported as it is framed

#### Scenario: An export reaching an analysis already under way

- **WHEN** a batch export reaches the current photograph while its analysis is still under way
- **THEN** no second analysis is asked for, and the export runs what the first one adds

### Requirement: An analysis asks only for the enhancements the user allows

An analysis SHALL name every enhancement the application offers, except the ones the user has switched off for
Autopilot in the settings. The switch-offs in force are those at the moment the analysis is asked for.

Where the user has switched every enhancement off, the analysis SHALL still be made, and it SHALL answer nothing.
This SHALL NOT be reported as a failure.

#### Scenario: One enhancement switched off

- **WHEN** the user has switched colorization off for Autopilot, and a grayscale photograph is analysed
- **THEN** no colorization is suggested for it

#### Scenario: Every enhancement switched off

- **WHEN** the user has switched every enhancement off for Autopilot, and a photograph is analysed
- **THEN** nothing is added to it and nothing is reported

### Requirement: A photograph is analysed once

An analysis that answers SHALL leave the photograph with a list of enhancements, even when the answer is that the
photograph needs nothing. The photograph SHALL NOT be analysed again while it stays open, even if the user later
removes every enhancement from it.

A photograph closed and opened again SHALL be a photograph that has never had a list, and SHALL be analysed afresh.

An analysis that fails SHALL leave the photograph as it was, with no list. The photograph SHALL be analysed again the
next time the trigger's conditions become true for it.

#### Scenario: A photograph that needs nothing

- **WHEN** an analysis answers that a photograph needs nothing, and the user moves away and back
- **THEN** it is not analysed again

#### Scenario: Every suggestion removed

- **WHEN** the user removes every enhancement Autopilot added
- **THEN** the photograph is not analysed again, and its list stays empty

#### Scenario: A failed analysis is tried again

- **WHEN** an analysis fails, and the user moves to another photograph and back
- **THEN** the photograph is analysed again

#### Scenario: Closed and reopened

- **WHEN** an analysed photograph is closed and opened again with Autopilot on
- **THEN** it is analysed again

### Requirement: A photograph being analysed says so where its enhancements are listed

While the current photograph is being analysed, the sidebar SHALL show, in place of its list of enhancements, a
single row with a spinning indicator and the words "Analysing image...", in the language the rest of the window is
using.

The row SHALL belong to the photograph being analysed. Moving to another photograph SHALL show that photograph's own
list, or its own row if it too is being analysed. Moving back SHALL show the row again while the analysis is still
under way.

The canvas's progress indicator SHALL NOT report an analysis.

#### Scenario: Analysing the current photograph

- **WHEN** the current photograph is being analysed
- **THEN** the sidebar shows the analysing row, and not the photograph's list

#### Scenario: Moving off a photograph being analysed

- **WHEN** the user moves from a photograph being analysed to one that is not
- **THEN** the sidebar shows the second photograph's list, with no analysing row

#### Scenario: The analysis answers

- **WHEN** the analysis of the current photograph answers
- **THEN** the analysing row is replaced by the photograph's list, carrying what was suggested

### Requirement: What an analysis suggests is added as the enhancements the menu would add

Each suggestion SHALL be added to the photograph it was made for as the enhancement the add menu would add for that
family: the user's default model, at its quality, with the family's starting amount. The one exception is an
upscale, which SHALL carry the scale the analysis chose for it rather than the scale the menu would choose.

The suggestions SHALL be added together, as one change to the list, and SHALL be shown in the order enhancements are
applied. A suggestion for a family already in the list, such as one the user added by hand while the analysis ran,
SHALL be left out, and the enhancement already there SHALL be kept as it is.

An answer SHALL be applied to the photograph it was made for, even if the user has since moved to another one.

#### Scenario: Suggestions arrive

- **WHEN** an analysis suggests colorization and an upscale at 4x, and the user's default colorization model is
  Mumbai
- **THEN** the photograph's list holds a colorization carrying Mumbai, followed by an upscale at 4x carrying the
  user's default upscale model

#### Scenario: A family added by hand during the analysis

- **WHEN** the user adds an upscale by hand at 2x while the photograph is being analysed, and the analysis then
  suggests an upscale and a light adjustment
- **THEN** the list holds the user's upscale at 2x and a light adjustment, and only one upscale

#### Scenario: An answer for a photograph the user moved off

- **WHEN** the user moves to another photograph while the first is being analysed, and the analysis then answers
- **THEN** the suggestions are added to the first photograph, and the second photograph's list is unchanged

### Requirement: Face recovery is suggested only where a face is small enough to restore

Where an analysis suggests face recovery, the window SHALL find the faces in the photograph at the framing the
analysis was made at, and SHALL keep the suggestion only if at least one face covers no more than the area a
restoration works at, 512x512 pixels. Where every face found is larger than that, face recovery SHALL be left out of
what is added, and the rest of the suggestions SHALL be added as usual.

The faces found this way SHALL be recorded for the photograph at that framing, exactly as faces found for an
enhancement are. A face recovery that then runs, whether Autopilot added it or the user adds one later at that
framing, SHALL NOT detect again.

This rule SHALL apply to Autopilot's suggestion only. A face recovery the user adds by hand SHALL be added whatever
size the faces are.

Where finding the faces fails, the face-recovery suggestion SHALL be kept and nothing SHALL be recorded. The face
recovery then finds its faces when it runs, as one added by hand does, and reports a failure there if there is one.

#### Scenario: A small face

- **WHEN** an analysis suggests face recovery for a photograph with one face of 300x300 pixels and one of 900x900
- **THEN** face recovery is added

#### Scenario: Only large faces

- **WHEN** an analysis suggests face recovery and an upscale for a photograph whose only face is 900x900 pixels
- **THEN** the upscale is added and face recovery is not

#### Scenario: The run that follows does not detect again

- **WHEN** Autopilot adds a face recovery to a photograph
- **THEN** the enhancement runs without the faces being detected again

#### Scenario: Added by hand after being left out

- **WHEN** face recovery was left out because every face was too large, and the user then adds it by hand
- **THEN** it is added, and no detection is asked for at the same framing

### Requirement: A failed analysis is reported, and a stopped one is not

An analysis that fails SHALL be reported to the user in a way that does not interrupt what they are doing. This
covers a photograph that cannot be read and an analysis that went wrong while it ran. The photograph's list SHALL be
left as it was.

An analysis asked for by an export that fails SHALL be reported on that export's row instead, which reads Failed and
carries the reason. It SHALL NOT also be reported the usual way. The photograph SHALL NOT be exported, and the batch
SHALL go on to the next row.

An analysis that answers only part of what it was asked, because one of its signals could not be read, SHALL NOT be
reported. What it did answer SHALL be applied.

An analysis that was stopped SHALL NOT be reported, and nothing it had found SHALL be applied.

#### Scenario: An analysis fails

- **WHEN** an analysis cannot be completed
- **THEN** the user is told that Autopilot failed, and the photograph's list is unchanged

#### Scenario: An analysis fails during an export

- **WHEN** an analysis asked for by an export cannot be completed
- **THEN** that export's row reads Failed with the reason, no other notice is shown, no file is written for that
  photograph, and the batch continues

#### Scenario: A signal could not be read

- **WHEN** an analysis answers an upscale but could not fetch the face detector
- **THEN** the upscale is added and nothing is reported

### Requirement: An analysis the user has moved past is stopped

An analysis in flight SHALL be stopped, and its answer discarded, when any of these happens:

- **The photograph is closed.** This includes being closed along with every other photograph. The analysis SHALL be
  stopped, and nothing SHALL be kept for the photograph.
- **Autopilot is switched off.** Every analysis in flight SHALL be stopped, for the current photograph and for any
  the user moved off. None of those photographs SHALL be given a list.
- **The photograph is framed differently**, by a crop, a rotation or a flip. The analysis SHALL be stopped, and a new
  one SHALL be asked for at the new framing, because the photograph still has no list.

Stopping one photograph's analysis SHALL NOT stop another's, and SHALL NOT disturb an enhancement run.

#### Scenario: Closed while being analysed

- **WHEN** a photograph is closed while it is being analysed
- **THEN** its analysis is stopped, nothing is reported, and nothing is added to any photograph

#### Scenario: Autopilot switched off during an analysis

- **WHEN** the user switches Autopilot off while two photographs are being analysed
- **THEN** both analyses are stopped, the analysing row goes away, and neither photograph is given a list

#### Scenario: Switched back on

- **WHEN** the user switches Autopilot off during an analysis and back on while still looking at that photograph
- **THEN** the photograph is analysed again

#### Scenario: Cropped while being analysed

- **WHEN** the user crops the current photograph while it is being analysed
- **THEN** the analysis is stopped, a new one is asked for at the new framing, and only the new one's suggestions are
  added

#### Scenario: Another photograph's analysis

- **WHEN** one of two photographs being analysed is closed
- **THEN** the other's analysis goes on and its suggestions are added
