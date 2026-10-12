## MODIFIED Requirements

### Requirement: A model's measured settings override the provider defaults

A model MAY declare the settings measured for it, and those SHALL override the corresponding
defaults for that model alone. A declaration SHALL be specific to the precision the model is run at,
because tuning does not survive a precision change - the same graph exported at FP16 and at FP32
wants opposite answers from the CUDA layout preference, and applying the FP16 answer to both is a
measured regression rather than a neutral choice.

A declaration MAY restrict which of a machine's compute units a model is dispatched to in order to
**exclude** one, and not only to prefer one. A compute unit that cannot compile a model at all is not
a slower choice than the alternatives: the attempt is paid for in full at session build and the model
then runs on whatever the failure left it with. Where a model declares such a restriction, the system
SHALL apply it on every machine it runs that model on rather than treating it as a hint the provider
may reconsider.

A model that declares nothing SHALL run on the provider defaults unchanged.

#### Scenario: Kyoto at FP16

- **WHEN** the Kyoto upscaler at FP16 is configured for CoreML
- **THEN** its options keep it off the GPU and on the CPU and Neural Engine

#### Scenario: Kyoto at FP32

- **WHEN** the Kyoto upscaler at FP32 is configured for CoreML
- **THEN** its options carry the default compute units

#### Scenario: Saitama at FP16

- **WHEN** the Saitama upscaler at FP16 is configured for CoreML
- **THEN** its options keep it off the GPU and on the CPU and Neural Engine

#### Scenario: Saitama at FP32

- **WHEN** the Saitama upscaler at FP32 is configured for CoreML
- **THEN** its options carry the default compute units

#### Scenario: Tokyo at either precision

- **WHEN** the Tokyo upscaler is configured for CoreML at FP16 or at FP32
- **THEN** its options keep it off the Neural Engine and on the CPU and GPU
- **AND** the two precisions carry the same compute units, because what excludes the unit is the
  graph's shape rather than the precision it was exported at

#### Scenario: A model with nothing measured

- **WHEN** a model that declares no settings is configured for any provider
- **THEN** its options are the provider defaults, with nothing added or removed

### Requirement: A model's measured settings include how the graph itself is run

A model's declaration SHALL be able to change settings that belong to the session rather than to one
provider: whether independent branches of the graph may run on separate threads, whether the memory
planner is used, and which individual graph optimizers are switched off. These SHALL apply on every
provider the model runs on.

A declaration that independent branches may not run on separate threads SHALL NOT change the pixels
the model produces. It is a setting rather than a trade: a graph whose nodes form a chain has no two
independent nodes to hand out, so the only thing the parallel mode can do on one is charge a thread
handoff per node, and the result is identical either way.

Unless a model says otherwise, independent branches MAY run on separate threads, the memory planner
SHALL be used, and every optimizer SHALL be left on - which is what every model in the reference
implementation runs with before it is measured.

#### Scenario: A model that has not been measured

- **WHEN** a model declaring nothing is configured
- **THEN** the session settings allow parallel execution, keep the memory planner on and disable no
  optimizer

#### Scenario: A model measured as sequential

- **WHEN** a model declaring sequential execution is configured for any provider
- **THEN** the session settings run one node at a time
- **AND** the same setting applies whichever provider was resolved

#### Scenario: Tokyo, whose graph is a chain

- **WHEN** the Tokyo upscaler is configured at either precision
- **THEN** the session settings run one node at a time
- **AND** the setting applies on every provider it is resolved to, including the ones on which it
  makes no measurable difference

#### Scenario: A sequential model's output

- **WHEN** a model declaring sequential execution is run and the same model is run with independent
  branches allowed on separate threads
- **THEN** the two runs produce the same image
