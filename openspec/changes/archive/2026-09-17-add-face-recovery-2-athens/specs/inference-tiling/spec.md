## ADDED Requirements

### Requirement: A model may be fed a second input that is a value rather than an image

Where a model's graph declares a second input carrying a single value the caller chooses — a weight, rather than
pixels — the system SHALL supply that value alongside the image tensor in one run, and SHALL read the output back at
the shape the caller declared, with the same length check a single-input run already carries.

Every model the system ran before this took one input, so the run could be expressed as "the tensor" with nothing
naming which input it was. A restoration model that takes a fidelity weight inverts nothing about that: the image is
still the first input and is still supplied the same way. What is new is that the caller has a second thing to say,
and it is a scalar rather than a tensor of pixels, so it is stated as a value rather than assembled into a buffer with
a shape of its own.

The two inputs SHALL be supplied **by position**, in the order the graph declares them, and SHALL NOT require the
caller to name them. This is the opposite of the rule for a model returning several outputs, and deliberately so: the
outputs of a face detector are three tensors of comparable shape and interchangeable type, so an export that reordered
them would be read wrongly with nothing able to notice. An image tensor and a single value are not interchangeable —
they differ in rank and in extent by orders of magnitude — so an export that swapped them fails at the runtime rather
than producing a plausible wrong answer, and naming them would instead make the system fail on a graph that named its
inputs something else.

Where the model returns a tensor of a different length than the caller declared, or returns none at all, the system
SHALL report an error and SHALL NOT write a partial result — the same guarantee a single-input declared-shape run
already carries.

#### Scenario: A model taking an image and a weight

- **WHEN** a model declaring an image input and a single-value input is run with both supplied
- **THEN** it is fed the image tensor at the declared shape and the value beside it, and the result is read back at the declared output shape

#### Scenario: The value reaching the model

- **WHEN** the same image is run against the same model at two different values
- **THEN** each run supplies the value it was given, and the two runs are not equivalent

#### Scenario: A model that returns the wrong number of values

- **WHEN** a model given an image and a value returns a tensor whose length differs from the declared output shape
- **THEN** an error naming the expected and actual lengths is returned
- **AND** no partial result is written
