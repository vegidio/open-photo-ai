## Purpose

What the window does about the faces in an open photograph: when it asks for them and when it does
not, what they are expressed in, what a change of framing does to them, what it shows while the
detector is being fetched, and what a detection that finds nothing or fails means for the
enhancement that needed it.

## ADDED Requirements

### Requirement: Faces are found only for a photograph whose enhancements need them

The window SHALL ask for the faces in a photograph when, and only when, that photograph's list of
enhancements carries one that restores faces. It SHALL NOT detect faces in a photograph merely
because it is open, has been selected, or is being looked at.

Detection costs a model in memory, and on a first run it costs transferring one. A window that
detected in every photograph it opened would pay that for photographs nobody is going to restore a
face in, and would pay it again for every image in a folder the user is paging through.

Where the faces for a photograph as it is currently framed are already known, adding a second
enhancement, changing a model, or changing the processor SHALL NOT cause them to be found again.

The same SHALL hold while they are still being found: a detection already under way for a
photograph at the framing in force answers every one of those changes, and asking again would run a
second detector over the same pixels alongside the first - and, on a first use, transfer it twice.
The window SHALL go on reporting the detection it is waiting for across such a change.

#### Scenario: A photograph with no face-recovery enhancement

- **WHEN** a photograph is opened and enhancements that do not restore faces are added to it
- **THEN** no detection is asked for

#### Scenario: A face-recovery enhancement is added

- **WHEN** an enhancement that restores faces is added to a photograph whose faces are not known
- **THEN** the faces in that photograph are found

#### Scenario: Faces already known

- **WHEN** a second enhancement is added, or a model or the processor is changed, while the faces
  for the framing in force are already known
- **THEN** no further detection is asked for

#### Scenario: Faces still being found

- **WHEN** a second enhancement is added, or a model or the processor is changed, while the faces
  for the framing in force are still being found
- **THEN** no further detection is asked for, the one under way is still reported, and its answer
  serves the enhancements as they stand when it lands

### Requirement: Faces are found in the photograph as it is framed, and are expressed in its pixels

Faces SHALL be found in the photograph as the user has framed it - flipped, turned and cut - and
every coordinate reported SHALL be in the pixels of that framed photograph rather than of the file
on disk.

The framed photograph is what a face-recovery run is applied to and what the window draws, so a
coordinate in the unframed photograph's space would name a place in neither.

#### Scenario: An unframed photograph

- **WHEN** faces are found in a photograph the user has not framed
- **THEN** the coordinates are in the pixels of the photograph as it was opened

#### Scenario: A framed photograph

- **WHEN** faces are found in a photograph that has been turned and cut
- **THEN** the coordinates are in the pixels of the framed photograph, and a face the cut removed is
  not among them

### Requirement: Changing the framing discards the faces found for the previous one

When a photograph's framing changes, the window SHALL discard the faces it holds for that
photograph and, where an enhancement still needs them, find them again against the new framing.

A flip, a turn or a cut moves every face and can add or remove one, so the previous answer
describes a photograph nobody is looking at any more. This is the same rule the enhanced result
already follows when a framing changes.

Where a framing is returned to one the photograph has already been detected at, the faces SHALL be
the ones found then, and the detector SHALL NOT be run a second time. A photograph turned to one
angle, then another, then back to the first is at a framing that has already been answered.

#### Scenario: A photograph is cut

- **WHEN** a framing is applied to a photograph whose faces are known and whose enhancements
  restore faces
- **THEN** the faces held for the previous framing are discarded and the faces in the new framing
  are found

#### Scenario: A framing is returned to

- **WHEN** a photograph is turned to one angle, then to another, and then back to the first
- **THEN** the faces at the first angle are the ones found the first time, and the detector is not
  run again for them

### Requirement: Finding faces is reported as part of the enhancement that needed them

While faces are being found, the window SHALL report progress under the name of the enhancement
that asked for them, on the same indicator an enhancement run is reported on - including the
transfer of the detector when it is not yet on disk.

The detector is not an enhancement a user adds and is offered by no menu in this application, so
naming it would name a model nobody chose. What the user did ask for is the enhancement that needs
it, and from their side the transfer and the detection are that enhancement beginning its work.

#### Scenario: The detector is not yet on disk

- **WHEN** an enhancement that restores faces is added and the detector has to be transferred first
- **THEN** the indicator reports that transfer under that enhancement's name, rather than leaving
  the window with nothing on screen

#### Scenario: Detection runs

- **WHEN** faces are being found
- **THEN** the indicator is shown under the name of the enhancement that asked for them

### Requirement: A photograph with no faces, and a detection that fails

A detection that finds no face SHALL be an answer rather than a failure: the photograph is recorded
as having no faces, nothing is reported to the user, and the enhancement that asked stays in the
list.

A detection that fails SHALL be reported to the user, and SHALL leave the photograph recorded as
having no faces rather than in a state where the question has not been answered. The enhancements
in the list SHALL still be run: a failed detection means the faces are not restored, and it must
not also mean that an upscale the user asked for in the same list does not happen.

#### Scenario: A photograph with nobody in it

- **WHEN** a detection finds no face
- **THEN** the photograph is recorded as having no faces, nothing is reported, and the enhancements
  in the list are run

#### Scenario: A detection that fails

- **WHEN** a detection fails
- **THEN** it is reported to the user
- **AND** the enhancements in the list are still run

### Requirement: Faces are forgotten when the photograph is closed

The faces held for a photograph SHALL be released when that photograph is closed, and all of them
when every photograph is closed. Faces belong to a photograph that is open, and a window that kept
them would answer a later question about a different photograph from a record nobody can see.

A detection that answers after the photograph it was about has been closed SHALL be discarded rather
than held. The release has already happened, so an answer recorded after it would outlive the close
it belongs to. Moving to another photograph is not closing one: an answer for a photograph that is
still open is kept, whether or not it is the one being looked at.

#### Scenario: One photograph is closed

- **WHEN** a photograph whose faces are known is closed
- **THEN** the faces held for it are released, and those held for the photographs still open are not

#### Scenario: Every photograph is closed

- **WHEN** every open photograph is closed
- **THEN** no faces are held for any of them

#### Scenario: A detection answers after its photograph was closed

- **WHEN** a photograph is closed while the faces in it are being found, and the detection then
  answers
- **THEN** nothing is held for it

#### Scenario: A detection answers after the window moved to another photograph

- **WHEN** the window moves to another photograph while the faces in the first are being found, and
  the detection then answers
- **THEN** the faces are held for the photograph they were found in
