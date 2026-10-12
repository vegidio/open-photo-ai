## MODIFIED Requirements

### Requirement: The result type is the operation's own, not the caller's to choose

Each operation whose result is not an image SHALL name exactly one result type, and the system SHALL bind a run of
that operation to that type. A caller SHALL NOT be able to ask such a run for a result type the operation does not
produce, and SHALL NOT be able to put an operation that produces an image onto this path.

Operations whose result is not an image SHALL be carried in a value of their own, distinct from the one carrying
operations whose result is an image. Which of the two an operation is carried in SHALL follow from the model being
named rather than being stated separately, so a caller naming a model is never also asked which path it runs on and
can never answer wrongly.

Neither of those is a refusal the system reports at run time. They are pairings that cannot be expressed: the type a
run produces follows from the operation being run, so there is no second place for a caller to state it and nothing to
state it wrongly. A system that decided this while the model was already loaded would discover a mismatch only after
transferring and opening it, and would report as an error what was never a legitimate request.

#### Scenario: A detection run produces faces

- **WHEN** a detection operation is run on this path
- **THEN** the result is the set of faces that operation produces, and no other type can be asked for

#### Scenario: An operation that produces an image

- **WHEN** an operation whose result is an image is named for this path
- **THEN** the request cannot be expressed, rather than being accepted and then refused

#### Scenario: Naming a model decides its path

- **WHEN** a caller names a model whose result is not an image
- **THEN** the operation produced is carried in the value this path takes, with no further statement from the caller
  about which path it runs on
