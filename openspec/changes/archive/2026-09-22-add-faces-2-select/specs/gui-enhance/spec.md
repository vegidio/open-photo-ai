## MODIFIED Requirements

### Requirement: A face-recovery enhancement carries the faces it will restore

The application SHALL accept a face-recovery operation naming a model, a precision and the faces to
restore, and SHALL run it over exactly those faces.

The faces SHALL travel with the operation rather than being found by the run. The window has
already found them, shows them, and decides which of them this run is about; a run that detected its
own would find a set the window cannot see and restore faces the user turned off.

The faces carried SHALL be the ones the user has chosen, and no others. A face the user has skipped
SHALL NOT be restored, SHALL NOT be sent, and SHALL NOT be folded into the identity of the result -
so that the same photograph restored over one selection and then over another is two results rather
than one served twice.

A chain carrying a face-recovery operation SHALL NOT be run until the faces for the framing in
force are known. A run started before they arrive would restore nothing, would be stored as a
result for an empty selection, and would have to be run again the moment the faces landed.

#### Scenario: A face-recovery operation is run

- **WHEN** the interface asks for an admitted image to be enhanced by a face-recovery operation
  carrying two faces
- **THEN** both are restored, and the result's identity and dimensions are answered

#### Scenario: A face is skipped

- **WHEN** one of two faces is skipped and the enhancements are run
- **THEN** the operation carries only the chosen face, and only that face is restored

#### Scenario: The same photograph over two selections

- **WHEN** a photograph is restored over both faces and then over one of them
- **THEN** the second run is not served the first run's result

#### Scenario: A chain whose faces are not known yet

- **WHEN** a face-recovery enhancement is added to a photograph whose faces have not been found yet
- **THEN** no run is started until they have been

#### Scenario: A face-recovery operation carrying no faces

- **WHEN** a face-recovery operation carrying no faces is run
- **THEN** the request succeeds, nothing is restored, and any other operation in the chain is still
  applied

### Requirement: A face-recovery enhancement in the list reports how many faces it will restore

A face-recovery enhancement in the list SHALL report, beneath its name, the model it will use, how
many faces it will restore, and which quality that model is being run at.

Where every face found is chosen, the line SHALL report how many faces there are. Where some have
been skipped, it SHALL report how many of how many will be restored, so that a row cannot say `2
Faces` while the run restores one.

The total SHALL be the faces actually found in the photograph as it is framed, so that framing a
photograph so that a face falls outside it changes what the line reports.

#### Scenario: A face recovery in the list

- **WHEN** a face-recovery enhancement is in the list for a photograph in which two faces were
  found
- **THEN** its row names the enhancement, and reports that model, two faces, and that quality

#### Scenario: Some faces are skipped

- **WHEN** one of the two faces found has been skipped
- **THEN** the row reports that one of two faces will be restored

#### Scenario: Every face is skipped

- **WHEN** both of the two faces found have been skipped
- **THEN** the row reports that none of two faces will be restored

#### Scenario: The count follows the framing

- **WHEN** the photograph is framed so that one of the two faces falls outside it
- **THEN** the row reports one face
