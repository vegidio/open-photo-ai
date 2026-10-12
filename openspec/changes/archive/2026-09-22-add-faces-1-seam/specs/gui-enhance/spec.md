## ADDED Requirements

### Requirement: A face-recovery enhancement carries the faces it will restore

The application SHALL accept a face-recovery operation naming a model, a precision and the faces to
restore, and SHALL run it over exactly those faces.

The faces SHALL travel with the operation rather than being found by the run. The window has
already found them, shows them, and - once a user can choose among them - decides which of them
this run is about; a run that detected its own would find a set the window cannot see and restore
faces the user turned off.

A chain carrying a face-recovery operation SHALL NOT be run until the faces for the framing in
force are known. A run started before they arrive would restore nothing, would be stored as a
result for an empty selection, and would have to be run again the moment the faces landed.

#### Scenario: A face-recovery operation is run

- **WHEN** the interface asks for an admitted image to be enhanced by a face-recovery operation
  carrying two faces
- **THEN** both are restored, and the result's identity and dimensions are answered

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

The count SHALL be the faces actually found in the photograph as it is framed, so that framing a
photograph so that a face falls outside it changes what the line reports.

#### Scenario: A face recovery in the list

- **WHEN** a face-recovery enhancement is in the list for a photograph in which two faces were
  found
- **THEN** its row names the enhancement, and reports that model, two faces, and that quality

#### Scenario: The count follows the framing

- **WHEN** the photograph is framed so that one of the two faces falls outside it
- **THEN** the row reports one face
