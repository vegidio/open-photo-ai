## ADDED Requirements

### Requirement: The faces found in a photograph can be chosen among

The window SHALL let the user choose which of the faces found in a photograph a face recovery
restores.

A face SHALL start out chosen or skipped according to **the area its bounding box covers** in the
framed photograph: a face covering more than 512x512 pixels SHALL start skipped, and one covering
that or less SHALL start chosen. The recovery models run every face through a fixed 512-pixel tile,
so a face larger than the tile is reduced into it, restored, and enlarged back again - which on a
face that was already sharper than the model works at can only soften it. A user who never opens the
chooser therefore gets the faces restored that a restoration helps.

The rule SHALL decide only what a face starts as. Nothing about turning a face on or off by hand
SHALL follow from it, and a face the user has decided SHALL keep what the user made of it.

A face found where the window has already recorded one SHALL keep whatever it is, so finding the
faces again at the framing in force SHALL change nothing. Where a face is found that was not
recorded before - at a framing nobody has been at, or in a photograph opened again - the rule SHALL
decide it.

"Already recorded" SHALL mean at **any** framing the photograph has been looked at since it was
opened, not merely the one it was last found at. A user who turns a large face on, re-frames the
photograph and returns to the framing they turned it on at is looking at the face they decided, and
deciding it a second time would skip it again without their asking - which is the same rule undoing
their choice rather than supplying a default in its absence.

The choice SHALL be offered from the face-recovery enhancement's own options, and the control that
offers it SHALL report how many faces are currently chosen. A photograph in which no faces were
found offers nothing to choose among, so that control SHALL be unavailable rather than opening on
an empty picture.

The choice SHALL belong to the photograph rather than to the enhancement: removing a face recovery
and adding another SHALL leave the choice as it was, because the faces it names are a property of
the pixels and not of the operation.

#### Scenario: A face small enough to restore

- **WHEN** a face covering 512x512 pixels or less is found in a photograph for the first time
- **THEN** it is chosen

#### Scenario: A face too large to restore

- **WHEN** a face covering more than 512x512 pixels is found in a photograph for the first time
- **THEN** it is skipped

#### Scenario: The faces are found again at the framing in force

- **WHEN** a user turns a skipped face on, and the faces in that photograph are found again at the
  same framing
- **THEN** that face is still on, and no face's state is decided again

#### Scenario: A face turned on, across a framing left and returned to

- **WHEN** a user turns a large face on, re-frames the photograph so that its faces are found afresh,
  and then returns to the framing they turned that face on at
- **THEN** that face is still on, and the size rule does not decide it a second time

#### Scenario: The control reports the chosen count

- **WHEN** one of two faces has been skipped
- **THEN** the control that opens the chooser reports that one face is chosen

#### Scenario: A photograph with nobody in it

- **WHEN** a face recovery's options are opened for a photograph in which no faces were found
- **THEN** the control that would open the chooser is unavailable

#### Scenario: The enhancement is replaced

- **WHEN** a face recovery over a photograph with a skipped face is removed and another is added
- **THEN** the same face is still skipped

### Requirement: Faces are chosen on the photograph itself, one box per face

The chooser SHALL draw the photograph **as it is framed**, fitted to its own proportions, with one
box over each face found in it. A chosen face and a skipped one SHALL be told apart by the
appearance of the box rather than by a list beside the picture, and operating a box SHALL turn that
face from chosen to skipped and back.

The chooser SHALL say how many of the faces found are chosen, so the user can read what the run
will do without counting boxes.

Each box SHALL be reachable and operable from the keyboard, and SHALL name which face it is, so a
user who cannot see the picture can still tell one box from another.

#### Scenario: The photograph and its boxes

- **WHEN** the chooser is opened for a photograph in which two faces were found
- **THEN** it draws the framed photograph with two boxes over it, both marked as chosen

#### Scenario: A box is operated

- **WHEN** a box over a chosen face is operated
- **THEN** that face is marked as skipped, and operating it again marks it chosen

#### Scenario: The chooser reports the count

- **WHEN** one of two faces has been skipped in the chooser
- **THEN** the chooser reports that one of two faces will be restored

#### Scenario: A box from the keyboard

- **WHEN** the chooser is opened
- **THEN** each box can be reached and operated from the keyboard, and each carries the number of
  the face it is over

### Requirement: A choice takes effect only when it is applied, and dismissing discards it

The chooser SHALL edit a working copy. The choice SHALL take effect only when the user applies it,
and dismissing the chooser - by its close control, by the keyboard's cancel, or by acting outside it
- SHALL leave the choice as it was before the chooser was opened.

Turning several faces off and on again SHALL cost at most one new run of the enhancements, made when
the choice is applied. A run per box operated would enhance a photograph the user is still deciding
about, and would do so once per click.

Applying a choice that changes nothing SHALL start no run, so opening the chooser to read what is
selected and closing it again leaves the work in flight alone.

#### Scenario: A choice is applied

- **WHEN** a face is skipped in the chooser and the choice is applied
- **THEN** that face is skipped for the photograph, and the enhancements are run again without it

#### Scenario: The chooser is dismissed

- **WHEN** faces are turned off in the chooser and the chooser is dismissed rather than applied
- **THEN** the choice is what it was before the chooser was opened, and no run is started

#### Scenario: Several faces are turned off before applying

- **WHEN** three faces are turned off in the chooser and the choice is then applied
- **THEN** the enhancements are run again once

#### Scenario: Nothing was changed

- **WHEN** the chooser is opened and applied without any box being operated
- **THEN** no run is started

### Requirement: A choice names the faces themselves, not their place in the photograph

A skipped face SHALL be recorded as the face it is rather than as its position among the faces
found. Where the faces in a photograph are found again and the same face is among them, it SHALL
still be skipped; where it is not among them, nothing SHALL be skipped on its account; and a face
found that was not there before SHALL start as its size says it should.

This is what makes a framing change need no rule of its own. A flip, a turn or a cut moves every
face, so the faces found in the new framing are not the faces that were skipped in the old one, and
every face in the re-framed photograph starts as its size says. A framing returned to is answered by
the faces found there the first time, so a choice made at that framing is in force again.

This is a **deliberate divergence from the reference**, which records a skipped face by its position
and therefore carries a choice across a framing change by number - skipping whichever face now
happens to be second when the second face the user skipped may have been cut out of the picture.

#### Scenario: A photograph is re-framed

- **WHEN** a photograph with a skipped face is cut so that the faces are found again at new
  positions
- **THEN** every face found in the new framing starts as its size says it should, and nothing is
  carried over from the old framing

#### Scenario: A framing is returned to

- **WHEN** a photograph with a skipped face is turned to another angle and then back
- **THEN** that face is skipped again

### Requirement: The boxes are drawn where the choice is made and nowhere else

The window SHALL draw the face boxes only in the chooser. It SHALL NOT draw them over the
photograph on the canvas, over the enhanced result, or over the sidebar's miniature.

The canvas is where a user judges the photograph, and boxes standing over the faces are exactly what
stops them from seeing the restoration they asked for. The reference draws them only in its own
chooser, for the same reason.

#### Scenario: A photograph whose faces are known

- **WHEN** the faces in the photograph on the canvas are known and one of them is skipped
- **THEN** no box is drawn on the canvas

#### Scenario: The chooser is open

- **WHEN** the chooser is open
- **THEN** the boxes are drawn in it

### Requirement: The detection and the run it was for are one movement of the indicator

A detection and the enhancement run that asked for it SHALL share a single range on the indicator:
the detection SHALL occupy the head of it and the run SHALL occupy the rest, so that the indicator
moves forward once over the pair and reaches completion exactly once.

The two are separate runs, each reporting its own progress from nothing to complete, and drawing
them as they arrive filled the indicator, emptied it and filled it again - which reads as the
enhancement having been applied twice. To the person watching, the detection is the enhancement
they asked for beginning its work, and one piece of work is one movement of the bar.

A run that needed no detection SHALL own the whole range, including a run over faces that are
already known - only the run a detection was just made for begins where that detection ended.

While the run that follows a detection has yet to report anything, the indicator SHALL go on
showing where the detection left it, under the same name. The run spends its first moments loading
its model and says nothing while it does, and an indicator emptied at that moment is the second
sweep in everything but name.

#### Scenario: A face recovery from the first report to the last

- **WHEN** an enhancement that restores faces is added, the faces are found, and the enhancement
  then runs
- **THEN** the indicator advances through the detection, continues through the run from where the
  detection left it, and never returns to the start or completes more than once

#### Scenario: The run that follows has not reported yet

- **WHEN** the faces have been found and the run that asked for them is loading its model
- **THEN** the indicator stays where the detection left it, still named after that enhancement

#### Scenario: A run over faces already known

- **WHEN** an enhancement is changed, or the processor is, and the faces for the framing in force
  are already known
- **THEN** nothing is detected and the run owns the whole of the indicator

## MODIFIED Requirements

### Requirement: Faces are forgotten when the photograph is closed

The faces held for a photograph, and the choice made among them, SHALL be released when that
photograph is closed, and all of them when every photograph is closed. Faces belong to a photograph
that is open, and a window that kept them would answer a later question about a different photograph
from a record nobody can see. A choice kept past the close would then skip a face in a photograph
opened later that happened to have one in the same place.

A detection that answers after the photograph it was about has been closed SHALL be discarded rather
than held. The release has already happened, so an answer recorded after it would outlive the close
it belongs to. Moving to another photograph is not closing one: an answer for a photograph that is
still open is kept, whether or not it is the one being looked at.

#### Scenario: One photograph is closed

- **WHEN** a photograph whose faces are known is closed
- **THEN** the faces held for it are released, and those held for the photographs still open are not

#### Scenario: A photograph with a skipped face is closed

- **WHEN** a photograph in which a face was skipped is closed and opened again
- **THEN** nothing is held from before, and every face found in it starts as its size says it
  should

#### Scenario: Every photograph is closed

- **WHEN** every open photograph is closed
- **THEN** no faces are held for any of them, and no choice is held for any of them

#### Scenario: A detection answers after its photograph was closed

- **WHEN** a photograph is closed while the faces in it are being found, and the detection then
  answers
- **THEN** nothing is held for it

#### Scenario: A detection answers after the window moved to another photograph

- **WHEN** the window moves to another photograph while the faces in the first are being found, and
  the detection then answers
- **THEN** the faces are held for the photograph they were found in

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

A detection that answers for a photograph whose enhancements no longer restore faces SHALL be kept
for the framing it was found at and SHALL NOT disturb the run in flight. Nobody asked that question
of the list as it now stands, and stopping a run that is already working to ask it again would
throw away the work done so far and take the indicator back to the start for an answer that changes
nothing about what is being produced.

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

#### Scenario: The face recovery is removed while its faces are still being found

- **WHEN** the enhancement that restores faces is removed while its detection is in flight, and the
  detection then answers
- **THEN** the answer is kept for the framing it was found at, and the run working on the remaining
  enhancements is neither stopped nor started again
