## ADDED Requirements

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

## MODIFIED Requirements

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
