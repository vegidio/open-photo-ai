## MODIFIED Requirements

### Requirement: Faces are found only for a photograph whose enhancements need them

The window SHALL ask for the faces in a photograph in two cases, and no others:

- **The photograph's list of enhancements carries one that restores faces.**
- **An Autopilot analysis of the photograph has suggested face recovery**, and the window is deciding whether to keep
  that suggestion (see `gui-autopilot`).

An Autopilot analysis allowed to suggest face recovery SHALL look for faces as part of deciding whether to suggest
it. Apart from these, the window SHALL NOT detect faces in a photograph merely because it is open, has been selected,
or is being looked at.

Detection costs a model in memory, and on a first run it costs transferring one. A window that detected in every
photograph it opened would pay that for photographs nobody is going to restore a face in, and would pay it again for
every image in a folder the user is paging through. Autopilot is the user's standing request that the photographs
they look at be examined for what they need, and a face is one of the things examined. A user who switches face
recovery off for Autopilot, or switches Autopilot off, SHALL have no face detected in a photograph they have not asked
to restore.

Where the faces for a photograph as it is currently framed are already known, adding a second enhancement, changing a
model, or changing the processor SHALL NOT cause them to be found again. That holds however they came to be known,
including by Autopilot.

The same SHALL hold while they are still being found. A detection already under way for a photograph at the framing
in force answers every one of those changes. Asking again would run a second detector over the same pixels alongside
the first, and on a first use it would transfer the detector twice. The window SHALL go on reporting the detection it
is waiting for across such a change.

A detection that answers for a photograph whose enhancements no longer restore faces SHALL be kept for the framing it
was found at, and SHALL NOT disturb the run in flight. Nobody asked that question of the list as it now stands.
Stopping a run that is already working, to ask it again, would throw away the work done so far and take the
indicator back to the start, for an answer that changes nothing about what is being produced.

#### Scenario: A photograph with no face-recovery enhancement

- **WHEN** a photograph is opened with Autopilot off, and enhancements that do not restore faces are added to it
- **THEN** no detection is asked for

#### Scenario: Autopilot may not suggest face recovery

- **WHEN** a photograph with faces in it is analysed by Autopilot while face recovery is switched off for Autopilot
- **THEN** no detection is made, by the analysis or by the window

#### Scenario: Autopilot suggests face recovery

- **WHEN** an Autopilot analysis suggests face recovery for a photograph
- **THEN** the faces in that photograph are found at the framing the analysis was made at, and are recorded for it

#### Scenario: A face-recovery enhancement is added

- **WHEN** an enhancement that restores faces is added to a photograph whose faces are not known
- **THEN** the faces in that photograph are found

#### Scenario: Faces already known

- **WHEN** a second enhancement is added, or a model or the processor is changed, while the faces for the framing in
  force are already known
- **THEN** no further detection is asked for

#### Scenario: Faces still being found

- **WHEN** a second enhancement is added, or a model or the processor is changed, while the faces for the framing in
  force are still being found
- **THEN** no further detection is asked for, the one under way is still reported, and its answer serves the
  enhancements as they stand when it lands

#### Scenario: The face recovery is removed while its faces are still being found

- **WHEN** the enhancement that restores faces is removed while its detection is in flight, and the detection then
  answers
- **THEN** the answer is kept for the framing it was found at, and the run working on the remaining enhancements is
  neither stopped nor started again

### Requirement: Finding faces is reported as part of the enhancement that needed them

While faces are being found for an enhancement in the list, the window SHALL report progress on the same indicator an
enhancement run is reported on, under the name of the enhancement that asked for them. This includes the transfer of
the detector when it is not yet on disk.

The detector is not an enhancement a user adds, and no menu in this application offers it, so naming it would name a
model nobody chose. What the user did ask for is the enhancement that needs it. From their side, the transfer and the
detection are that enhancement beginning its work.

Faces found while Autopilot decides whether to suggest face recovery SHALL NOT be reported on that indicator. They
belong to the analysis, and the analysis is reported by its own row in the sidebar (see `gui-autopilot`). Reporting
them under the name "Face Recovery" would name an enhancement the user has not added and that Autopilot may yet leave
out.

#### Scenario: The detector is not yet on disk

- **WHEN** an enhancement that restores faces is added and the detector has to be transferred first
- **THEN** the indicator reports that transfer under that enhancement's name, rather than leaving the window with
  nothing on screen

#### Scenario: Detection runs

- **WHEN** faces are being found for an enhancement in the list
- **THEN** the indicator is shown under the name of the enhancement that asked for them

#### Scenario: Faces found for Autopilot

- **WHEN** faces are being found because an Autopilot analysis suggested face recovery
- **THEN** the canvas shows no indicator for it, and the sidebar shows the analysing row
