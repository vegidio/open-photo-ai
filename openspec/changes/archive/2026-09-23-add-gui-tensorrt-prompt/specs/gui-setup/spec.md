## ADDED Requirements

### Requirement: A machine that supports TensorRT is asked once whether to use it

Once initialization has succeeded, if the provider report says this machine offers TensorRT and the user has never
answered the question, the application SHALL ask whether to enable TensorRT.

The question SHALL say what TensorRT is, that its first run of a model spends minutes optimizing it and later runs are
much faster, and that the answer can be changed at any time in the settings surface, under the AI processor. It SHALL
offer exactly two answers, **No** and **Yes**, with Yes as the primary one.

The question SHALL NOT appear while the setup dialog or its failure state is on screen, and SHALL NOT appear on a
launch whose initialization failed. It follows a retry that succeeds exactly as it follows a first attempt that does.

The question SHALL NOT be dismissable without an answer: pressing Escape or clicking outside it SHALL leave it in
place, and it SHALL carry no close control. An unanswered question is one the application would have to ask again,
which is the repetition this question exists to avoid.

Answering SHALL take the question down, and the question SHALL NOT be asked again on any later launch - see
`gui-settings` for what each answer records.

A machine whose report does not offer TensorRT SHALL never be asked, whatever else it offers.

#### Scenario: A first launch on a machine with an RTX card

- **WHEN** initialization succeeds on a machine reporting TensorRT supported, and the question has never been answered
- **THEN** the TensorRT question is shown once the setup dialog has gone away

#### Scenario: A launch after the question was answered

- **WHEN** initialization succeeds on a machine reporting TensorRT supported, and the question was answered on an
  earlier launch
- **THEN** the question is not shown

#### Scenario: A machine without TensorRT

- **WHEN** initialization succeeds on a machine whose report does not offer TensorRT
- **THEN** the question is not shown

#### Scenario: A launch whose setup failed

- **WHEN** initialization fails on a machine with an RTX card
- **THEN** the failure dialog is shown and the question is not

#### Scenario: A retry that succeeds

- **WHEN** a failed initialization is retried and succeeds on a machine reporting TensorRT supported, and the question
  has never been answered
- **THEN** the question is shown once the setup dialog has gone away

#### Scenario: Trying to dismiss the question

- **WHEN** the question is shown and the user presses Escape or clicks outside it
- **THEN** the question stays on screen
