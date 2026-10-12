## MODIFIED Requirements

### Requirement: A result that did not come from a model says so

A cached result is indistinguishable from a computed one in everything a user sees except the time it
took, which is the whole point of it and also what makes a cache that is silently doing nothing
impossible to notice. Whether a run's work was served from the store or computed SHALL be recorded.

This SHALL hold wherever the store is read, rather than on the enhancement path alone: an analysis
whose result is not a picture is served from the same store and is subject to the same silence, and a
record written for one path and not the other would make "the store is doing nothing" answerable for
half the application. A served analysis and a computed one SHALL each be recorded, and both records
SHALL name the operation and the entry, so a run that missed and the later run it serves can be read
as one sequence rather than as two unrelated records.

These records repeat per operation, so they SHALL be written below the default verbosity — available
to a user who has been asked for more detail, absent from an ordinary session's file.

#### Scenario: A step is served from the store

- **WHEN** a step of a run is served from the run cache at raised verbosity
- **THEN** a record identifies the operation and says it was served from the store

#### Scenario: A step is computed

- **WHEN** a step of a run is not in the store and is computed at raised verbosity
- **THEN** a record identifies the operation and says it was computed

#### Scenario: An analysis is served from the store

- **WHEN** a run whose result is not a picture is served from the run cache at raised verbosity
- **THEN** a record identifies the operation and the entry, and says it was served from the store

#### Scenario: An analysis is computed

- **WHEN** a run whose result is not a picture is not in the store and executes its model at raised verbosity
- **THEN** a record identifies the operation and says it was computed

#### Scenario: An ordinary session

- **WHEN** a run is made at the default verbosity
- **THEN** the per-step records are absent from the file
- **AND** the run's own beginning and outcome are still present
