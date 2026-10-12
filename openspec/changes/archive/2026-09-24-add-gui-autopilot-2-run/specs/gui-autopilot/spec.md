## ADDED Requirements

### Requirement: A photograph is analysed when it is looked at with Autopilot on and nothing done to it yet

The window SHALL ask for a photograph to be analysed when all three of these hold at once:

- the photograph is the current one
- Autopilot is on
- the photograph has never had a list of enhancements

An analysis SHALL be asked for whenever that becomes true, whichever of the three changed last. That includes the
user turning Autopilot on while looking at such a photograph.

A photograph that is open but not current SHALL NOT be analysed. Opening a folder of photographs SHALL NOT analyse
every one of them: each is analysed when the user comes to it.

A photograph SHALL have at most one analysis in flight. Returning to a photograph whose analysis is still under way
SHALL NOT ask for a second one.

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

An analysis that answers only part of what it was asked, because one of its signals could not be read, SHALL NOT be
reported. What it did answer SHALL be applied.

An analysis that was stopped SHALL NOT be reported, and nothing it had found SHALL be applied.

#### Scenario: An analysis fails

- **WHEN** an analysis cannot be completed
- **THEN** the user is told that Autopilot failed, and the photograph's list is unchanged

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
