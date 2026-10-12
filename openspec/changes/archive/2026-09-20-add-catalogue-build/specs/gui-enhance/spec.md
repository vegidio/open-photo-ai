## MODIFIED Requirements

### Requirement: An operation the application cannot run is refused before anything is fetched

A request naming a family, model or precision the application does not publish SHALL be refused, and
the refusal SHALL name what could not be served.

The whole chain SHALL be judged before any of it begins, so a chain whose later operation cannot be
served does not first spend minutes fetching a model for its earlier one.

What the application publishes SHALL be decided by the library alone. The application SHALL NOT keep
its own record of which models exist or which precisions each is published at, so a model the library
publishes SHALL be runnable rather than refused by a second record that has not been kept in step.

#### Scenario: A family that cannot be run

- **WHEN** a chain names a family the application cannot run
- **THEN** the request is refused, the refusal names that family, and no model is fetched for any
  operation in the chain

#### Scenario: A model and precision that were never published together

- **WHEN** a chain names a model at a precision it is not published at
- **THEN** the request is refused rather than a model being fetched that does not exist

#### Scenario: A model the library publishes

- **WHEN** a chain names any model the library publishes, at a precision it publishes that model at
- **THEN** the chain runs, whether or not the application has been changed since that model was added
