## ADDED Requirements

### Requirement: The catalogue builds the operations it describes

The library SHALL be able to turn a model it publishes - named by its family, its codename, the
precision it is to run at, and the values of the parameters it takes - into the enhancement operation
that model performs.

Every model the catalogue publishes SHALL be buildable at every precision the catalogue publishes it
at. A model that can be described but not built is a fault in the library, not a refusal for a caller
to report.

A caller SHALL NOT need to know which models exist, how a model's precision is spelled internally, or
which models admit which precisions. Those are the questions the catalogue already answers, and
answering them again is what lets two answers disagree.

#### Scenario: A published model is built

- **WHEN** a model the catalogue publishes is requested at a precision the catalogue publishes it at
- **THEN** the library answers with the operation that model performs

#### Scenario: Every published model

- **WHEN** every variant in the catalogue is requested at each precision it publishes
- **THEN** every one of them is built

#### Scenario: A model published only at some precisions

- **WHEN** a model is requested at a precision the catalogue does not publish it at
- **THEN** the request is refused, naming the model and the precision

#### Scenario: A codename no family publishes

- **WHEN** a codename the catalogue does not publish is requested
- **THEN** the request is refused, naming the family and the codename

### Requirement: A new model reaches the interface without a second registration

Adding a model to the library SHALL make it available to run without any other part of the project
naming it.

This is what the catalogue is for: a front end reads it rather than restating the vocabulary, and a
front end that had to restate it in order to *run* a model would be the same duplication in a less
visible place.

#### Scenario: A model is added to the library

- **WHEN** a new variant is published by the catalogue
- **THEN** it is offered by every chooser built from the catalogue and can be run, with no change
  outside the library
