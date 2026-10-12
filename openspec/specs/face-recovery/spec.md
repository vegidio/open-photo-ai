# face-recovery Specification

## Purpose

What a face-recovery run does to a photograph: the faces it is given rather than finds, the canonical frame each one
is restored in, how closely the restoration is held to the face that was there, and how the result is feathered back
into the picture so that everything outside a face is the photograph the caller handed over, untouched.

## Requirements

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

**Where the model accepts no fidelity weight, the run SHALL present the model with the face alone.** It SHALL NOT
substitute a default in place of the value such a model does not take: there is nothing for one to bind to, and a
run that sent one anyway would be refused by the model rather than quietly ignored. Such a model's runs over one
photograph and one selection at one set of settings are therefore **one** result, and the second is served from the
first.

Whether a model accepts a weight SHALL be a property of that model rather than of the operation that reaches the
run, so that neither an operation restored from storage nor one built without the value can send a model down the
wrong one of the two paths.

#### Scenario: Two fidelities over one photograph

- **WHEN** one selection is restored at maximum fidelity and then at half
- **THEN** the second run is not served the first's stored result, and the two results differ

#### Scenario: The same fidelity twice

- **WHEN** one selection is restored twice at the same fidelity
- **THEN** the second run is served the first's stored result

#### Scenario: An operation carrying no fidelity

- **WHEN** a run is asked for an operation whose model accepts a fidelity but which carries none
- **THEN** it restores at maximum fidelity rather than being refused

#### Scenario: A model that accepts no fidelity

- **WHEN** a run is asked for an operation whose model accepts no fidelity weight
- **THEN** the model is presented with the aligned face alone and no weight of any value
- **AND** the run is not refused

#### Scenario: One selection twice through a model that accepts no fidelity

- **WHEN** one selection is restored twice by a model that accepts no fidelity, at the same settings
- **THEN** the second run is served the first's stored result

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

### Requirement: Every model of the family frames and feathers a face the same way

Where the family offers several restoration models, each SHALL align a face onto the same canonical template, feather
it back through a mask of the same shape, and write within the same bound around the face's own box. What a caller
chooses between the models SHALL be the restoration itself — how the pixels inside the face are rebuilt, and what
the model accepts to control that — and not where the face is framed, how much of the surrounding picture is swept
into the aligned square, or how visible the seam is where the result meets the photograph.

A model SHALL be presented with the face at the square its own graph accepts, and the template SHALL be expressed at
a reference size and scaled to that square, so that two models accepting different squares still align against one
template rather than against one each.

This is what makes the choice of model a choice about quality rather than about framing: a user switching between
them on the same photograph sees the same faces in the same places with the same blending, differing only in the
restoration. Two models that framed differently would move a face's crop when the user only meant to try the other
one, and a per-model mask would put a different seam on the same photograph for a reason the user cannot see.

#### Scenario: One photograph through both models

- **WHEN** the same selection is restored by one model and then by the other, at the same settings
- **THEN** each face is framed by the same alignment and blended by a mask of the same shape
- **AND** the picture outside each face's neighbourhood is the photograph the caller handed over in both results

#### Scenario: A model accepting a different square

- **WHEN** a restoration model accepts a square other than the one another model of the family accepts
- **THEN** the canonical template is scaled to that model's square rather than presented at another model's

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

### Requirement: A chain's detection and the chain itself are reported as one run

The library SHALL let a caller report a chain's face detection and the chain that follows it as one run, whose figure
for the whole run moves forward once from start to exactly 1, rather than as two runs each filling from the start.
Every front end that runs a face recovery SHALL get the same mapping from the library, not reimplement it.

Every enhancement of the chain SHALL own an equal share of the run's figure: with `N` enhancements, each owns `1/N`,
in the order they are applied. The detection SHALL NOT count as an enhancement of its own. Where the detection did work
(its model was fetched or run), its reports SHALL occupy the first fifth of the first enhancement's share, and that
enhancement's own reports SHALL occupy the rest of its share. Every other enhancement's reports SHALL keep their share
unchanged. Where the face recovery is the first enhancement applied, the face recovery's share therefore covers the
detection and the recovery together. Where the detection's result was already known, the detection SHALL report
nothing and the chain's reports SHALL be passed on unchanged, so that re-running a chain over a photograph whose faces
are already known does not jump forward at its start.

A report from the detection SHALL be attributed to the face recovery it was made for. It SHALL carry that face
recovery's position in the chain as it is applied, and the chain's length, not the position and length of the
detection's own single-operation run. The chain's own reports SHALL keep the position and length they already carry.

#### Scenario: A detection that did work, then the chain

- **WHEN** a chain of a face recovery and an upscale is reported as one run, and the detection runs its model
- **THEN** the detection's reports go from the start to one tenth of the run, the face recovery's reports go from one
  tenth to one half, and the upscale's reports go from one half to exactly 1
- **AND** the figure never decreases across the handover

#### Scenario: A detection whose result was already known

- **WHEN** a chain with a face recovery and an upscale is reported as one run, and the detection's result was already
  known
- **THEN** nothing is reported for the detection, the face recovery's reports go from the start to one half, and the
  upscale's reports go from one half to exactly 1

#### Scenario: Equal shares whatever the chain's length

- **WHEN** a chain of a denoise, a face recovery and an upscale is reported as one run
- **THEN** each enhancement's reports end at its own third of the run, whether or not the detection did work

#### Scenario: A face recovery applied after another enhancement

- **WHEN** a chain of a denoise and a face recovery is reported as one run, and the detection runs its model
- **THEN** the detection's reports occupy the first fifth of the denoise's share, since the detection runs before the
  whole chain, and the figure never decreases

#### Scenario: The detection is counted as the face recovery's step

- **WHEN** a chain of a face recovery and an upscale is reported as one run, and the detection runs its model
- **THEN** every report from the detection says it concerns the first of two operations, the position of the face
  recovery in that chain
- **AND** the chain's reports say the face recovery is the first of two and the upscale the second of two
