## ADDED Requirements

### Requirement: The library finds the faces for a chain on the caller's behalf

The library SHALL offer one step that prepares a chain for running by finding the faces its face recoveries restore,
so that every front end feeds a face recovery the same way.

For a chain that carries at least one face recovery, the step SHALL run the library's one face-recovery detection over
the chain's input, exactly once however many face recoveries the chain carries. It SHALL then hand each face recovery
the faces the caller's selection keeps for that recovery, in the order the detector found them. It SHALL answer the
chain, every operation otherwise unchanged and in its original order, together with every face the detection found,
whether kept or not.

For a chain that carries no face recovery, the step SHALL detect nothing, SHALL answer the chain unchanged, and SHALL
report that faces were not asked for.

The detection is the same one the GUI's face picker and Autopilot run, so a photograph already examined by either is
served its faces from the run store rather than detected again.

#### Scenario: A chain with one face recovery and an upscale

- **WHEN** a chain of an upscale and a face recovery is prepared over a photograph with two faces, and the selection
  keeps both
- **THEN** one detection is made, the face recovery carries both faces in the order they were found, the upscale is
  unchanged, and both faces are reported as found

#### Scenario: A chain with no face recovery

- **WHEN** a chain of an upscale alone is prepared
- **THEN** no detection is made, the chain is answered unchanged, and the faces are reported as not asked for

#### Scenario: Two face recoveries in one chain

- **WHEN** a chain carrying two face recoveries is prepared
- **THEN** exactly one detection is made, and each recovery carries the faces the selection keeps for it

### Requirement: Without a choice from the user, a face recovery restores the faces small enough to gain detail

The library SHALL publish a default selection that keeps exactly the faces it already judges restorable: a face small
enough for a recovery to give back more detail than it holds. A larger face, which a recovery can only soften, is
left as the photograph has it.

This is the same rule the GUI applies to a face nobody has chosen about and that Autopilot suggests face recovery by.
A front end with no way to choose among faces therefore restores exactly the faces the GUI restores before the user
changes anything.

#### Scenario: A small face and a large face

- **WHEN** a chain with a face recovery is prepared with the default selection over a photograph with one face small
  enough to restore and one too large
- **THEN** the recovery carries only the small face, and both faces are reported as found

### Requirement: A detection that fails restores nobody, and the rest of the chain still runs

When the detection fails, the step SHALL NOT fail. Every face recovery in the chain SHALL be handed no faces, every
other operation SHALL be answered unchanged, and the step SHALL report the failure, with the library's own description
of it, for the caller to show. A failed detection means the faces are not restored. It SHALL NOT also cancel an
upscale asked for in the same chain.

A detection that finds no face is an answer, not a failure. It SHALL be reported as found, with no faces.

#### Scenario: The detection fails

- **WHEN** a chain of a face recovery and an upscale is prepared and the detection fails
- **THEN** the step answers the chain with the face recovery carrying no faces and the upscale unchanged, and reports
  the failure with its description

#### Scenario: Nobody in the photograph

- **WHEN** a chain with a face recovery is prepared over a photograph with no face in it
- **THEN** the recovery carries no faces, and the faces are reported as found, with none

### Requirement: Stopping the detection stops the chain

The detection SHALL run under the caller's stop, provider and progress reporting. A stop that reaches it SHALL end the
step as stopped, with no chain answered, so that a caller does not go on to run the chain the user stopped.

#### Scenario: Stopped while finding faces

- **WHEN** the caller stops the run while the detection for a chain is in progress
- **THEN** the step ends as stopped and answers no chain
