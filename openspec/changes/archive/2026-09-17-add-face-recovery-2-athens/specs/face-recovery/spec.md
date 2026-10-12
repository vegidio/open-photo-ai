## Purpose

What a face-recovery run does to a photograph: the faces it is given rather than finds, the canonical frame each one
is restored in, how closely the restoration is held to the face that was there, and how the result is feathered back
into the picture so that everything outside a face is the photograph the caller handed over, untouched.

## ADDED Requirements

### Requirement: A run restores the faces it is given, and finds none of its own

A face-recovery run SHALL restore exactly the faces the operation carries, in the order they are carried, and SHALL
NOT detect faces itself. The faces come from a detection run the caller made, which is a separate operation with its
own model and its own report.

This is what keeps "what must be on disk before this can run" true of a face-recovery operation: it names one model,
never two, so a caller that already has a selection — because a user toggled individual faces in it — does not pay for
a detection it is not going to use, and no model is pinned as *the* detector from inside this family.

Restoring the same photograph with a different selection SHALL produce a different result, and a result stored for one
selection SHALL NOT be served for another.

#### Scenario: A photograph with two faces, both selected

- **WHEN** a run carries two faces
- **THEN** both are restored, in the order the operation carries them

#### Scenario: A photograph with two faces, one deselected

- **WHEN** a run carries only the second of the two faces a detection found
- **THEN** only that face is restored and the other is left exactly as the photograph had it

#### Scenario: One selection is not served another's result

- **WHEN** the same photograph is run at the same settings over two different selections
- **THEN** the second run is not served the first run's stored result

### Requirement: Each face is restored in a canonical frame rather than where it lies in the photograph

The restoration model accepts one fixed square size and expects a face presented in a canonical position within it.
For each face, the system SHALL fit the face's five landmarks onto a canonical landmark template — expressed at a
fixed reference size and scaled to the model's own square — and SHALL present the model with the picture sampled
through the transform that fit produces.

The fit SHALL be a **similarity** transform — one scale, one rotation, one translation, no shear and no independent
scaling of the axes — computed as the least-squares best fit over all five landmarks rather than from any subset of
them. A face is rigid under a change of viewpoint to the accuracy this needs, and an unconstrained affine fit would
stretch a face to satisfy landmarks the detector placed a pixel or two out.

Sampling SHALL be bilinear, and where the transform reaches outside the photograph the system SHALL reflect the
picture's own edge back inwards rather than sampling a colour that is not in it. A face near a border is exactly the
case this covers: a constant fill would put a hard edge through the model's input, which it would restore as picture.

The template SHALL be scaled from its reference size to whatever square the model accepts, so that a model accepting
a different square aligns against the template in its own coordinate space rather than against one in another.

Where all five landmarks coincide, no scale or rotation can be fit, and the system SHALL fall back to the translation
that maps the landmarks' centre onto the template's rather than producing an undefined transform.

#### Scenario: A face away from the centre of the photograph

- **WHEN** a face is restored whose landmarks sit far from the image centre
- **THEN** the model is given the face in the template's canonical position, not the corner of the photograph it occupies

#### Scenario: A rotated face

- **WHEN** a face is restored whose eye landmarks are not level
- **THEN** the transform rotates it upright, and does so without shearing or stretching it

#### Scenario: A face at the edge of the photograph

- **WHEN** the aligned square reaches past the photograph's border
- **THEN** the picture's own edge pixels are reflected back into it, and no constant-coloured region is presented to the model

#### Scenario: Landmarks that carry no orientation

- **WHEN** a face's five landmarks all coincide
- **THEN** the run translates it onto the template's centre rather than failing or producing a degenerate result

### Requirement: How closely a restoration keeps to the face that was there is the caller's to choose

Where the model accepts a fidelity weight, the system SHALL bind the value the operation carries to it, and SHALL
treat that value as part of what the run produces: the same photograph and the same selection at two fidelities are
two results, and neither is served from the other's stored result.

Maximum fidelity SHALL mean maximum faithfulness to the original face and the least restoration, which is the value
the reference implementation fixes for every run and therefore the value an untouched control is expected to carry.

Where an operation reaches the run carrying no fidelity at all — which a stored operation can, since the value is
absent rather than zero when it was never given — the run SHALL restore at maximum fidelity rather than refusing, so
that a selection persisted before a fidelity was ever chosen still runs and runs the way the reference does.

#### Scenario: Two fidelities over one photograph

- **WHEN** one selection is restored at maximum fidelity and then at half
- **THEN** the second run is not served the first's stored result, and the two results differ

#### Scenario: The same fidelity twice

- **WHEN** one selection is restored twice at the same fidelity
- **THEN** the second run is served the first's stored result

#### Scenario: An operation carrying no fidelity

- **WHEN** a run is asked for an operation whose model accepts a fidelity but which carries none
- **THEN** it restores at maximum fidelity rather than being refused

### Requirement: A restored face is feathered back into the photograph and nothing else is touched

The system SHALL composite each restored face back into the photograph through the same transform its alignment used,
weighting the restoration against the photograph by a mask that is fully opaque over the centre of the aligned square
and falls smoothly to nothing at its boundary.

The falloff SHALL be smooth rather than a hard edge: the restoration differs from the surrounding skin in colour and
in detail, and a mask that ended abruptly would draw a visible ellipse around every restored face. The mask SHALL be
elliptical rather than rectangular, so that the corners of the aligned square — which hold background, hair and
whatever else the transform swept in — contribute nothing.

Writing SHALL be bounded to the face's own neighbourhood: the face's bounding box expanded by a margin proportional to
its longest side, clipped to the photograph. Pixels whose mask weight is below a small threshold SHALL be left as they
were, rather than written with a contribution too small to see.

Where two faces overlap, each SHALL be composited over the result of the ones before it, in the order the operation
carries them, so that a run over one selection produces one deterministic image.

Everything outside the composited neighbourhoods SHALL be the photograph the caller handed over, carried across at the
run's own channel depth.

#### Scenario: A restored face in a photograph

- **WHEN** one face is restored
- **THEN** the picture more than a margin away from its bounding box is unchanged, and there is no visible edge where the restoration meets the photograph

#### Scenario: Two overlapping faces

- **WHEN** two faces whose neighbourhoods overlap are restored in one run
- **THEN** the second is composited over the first's result, and the run produces the same image every time

#### Scenario: A face at the edge of the photograph

- **WHEN** a restored face's neighbourhood extends past the photograph's border
- **THEN** the composite is clipped to the photograph and nothing outside it is written

### Requirement: A selection with no faces produces the photograph, at the depth that was asked for

Where a run carries no faces — an image in which none was found, or one in which the user deselected every face — the
system SHALL produce the photograph's own pixels with nothing restored, at the channel depth the caller asked for and
in the same form every other enhancement produces.

It is a legitimate request rather than an error: a chain applied across a batch will meet images with no face in them,
and refusing one would fail an export that is otherwise correct.

The result SHALL NOT be the source image handed straight back in whatever form it was decoded in. A caller that asked
for sixteen bits per channel receives sixteen from every operation in the chain or from none of them, and an operation
whose output form depended on whether the photograph happened to contain a face would be one a caller could not
reason about.

An image with no area SHALL be refused rather than run, which is the same refusal the enhancement path makes for an
image it cannot partition and the detection path makes for one it cannot present to the detector. An empty selection
over an image that exists is a request; an image that does not is not.

#### Scenario: A photograph with no faces selected

- **WHEN** a run carries an empty selection
- **THEN** the result is the photograph with nothing restored, and the run does not fail

#### Scenario: An empty selection at sixteen bits

- **WHEN** a run carrying an empty selection is asked for sixteen bits per channel
- **THEN** the result carries sixteen bits per channel, as an enhancement of the same image would

#### Scenario: An image with no area

- **WHEN** a face-recovery run is asked for an image with no width or no height
- **THEN** the run is refused and no image is produced

### Requirement: A run reports what it has finished and stops between faces

A face-recovery run SHALL report progress across the whole of its own range, divided evenly between the faces it
carries, and SHALL report a face's restoration and its composite as separate steps — the second being the slower of
the two on a large photograph, and invisible in the report if the two were folded together.

No part of the range SHALL be reserved for detection. The faces arrive already found, so a run that reserved a share
of its bar for work it does not do would sit at that fraction before its first face and never explain why.

A run SHALL check for cancellation before each face, between its restoration and its composite, and after it. A
cancelled run SHALL produce no image at all — not the faces already composited, and not the buffer a cancellation
landed in.

#### Scenario: Progress over several faces

- **WHEN** a run over three faces is watched
- **THEN** progress never decreases, reaches its end exactly once, and reports two steps per face

#### Scenario: Progress with no faces to restore

- **WHEN** a run carrying an empty selection is watched
- **THEN** progress reaches its end and never sits partway through the range waiting for work that will not happen

#### Scenario: A run cancelled partway

- **WHEN** a run over three faces is cancelled after the first has been composited
- **THEN** no image is produced, including the partly restored one
