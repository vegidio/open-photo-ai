## ADDED Requirements

### Requirement: A model may disable several graph optimizers

A model MAY name one or more graph transformers to switch off while leaving the rest of the
optimization pipeline on - the scalpel to the optimization level's hammer, so that a model one
transformer miscompiles keeps every other fusion instead of giving them all up. Where a model names
none, the runtime's optimization pipeline SHALL be left alone rather than handed an empty setting.

Where a model names several, they SHALL be written using the separator the runtime actually parses
them with. Getting that separator wrong is **silent**: the runtime ignores an unrecognised name
rather than reporting it, so a wrongly joined list is read as one name that matches no transformer,
disables nothing, and leaves the model failing to load exactly as though the setting had never been
written. There is no error to read and no log line to find - the only symptom is a model that does
not open.

Because the failure is silent, the separator SHALL be established by measurement against a model that
genuinely needs a transformer disabled, rather than taken from a binding's documentation or from a
plausible assumption. A model that loads only with one of two named transformers disabled is the
instrument: with the correct separator it loads, and with any other it fails.

#### Scenario: A model that disables one optimizer

- **WHEN** a model names one graph transformer to switch off
- **THEN** its session is configured with that name

#### Scenario: A model that disables several optimizers

- **WHEN** a model names more than one graph transformer to switch off
- **THEN** its session is configured with all of them, joined the way the runtime parses them
- **AND** each named transformer is actually switched off, rather than the value matching none

#### Scenario: A model that disables none

- **WHEN** a model names no graph transformer
- **THEN** the disable setting is not written at all, rather than written empty

## REMOVED Requirements

### Requirement: A model disables at most one graph optimizer

**Reason**: It describes a runtime limitation that does not exist. The requirement was drawn from a
sweep in which the separator joining the names was hardcoded to a comma, so every multi-name case
tested one unrecognised name rather than a list, and each failed identically to naming nothing. The
reference implementation joins the same setting's entries with a semicolon and documents that it does.
The evidence was consistent with the separator being wrong and was read instead as the runtime
accepting only one name.

**Migration**: Replaced by *A model may disable several graph optimizers*, which restores the ability
to name more than one and requires the separator to be established by measurement rather than
assumed. No model other than Osaka declared an optimizer while this requirement stood, and Osaka's
declaration is restored to the reference's two names by this change.

## MODIFIED Requirements

### Requirement: A model's measured settings include how the graph itself is run

A model's declaration SHALL be able to change settings that belong to the session rather than to one
provider: whether independent branches of the graph may run on separate threads, whether the memory
planner is used, and which graph optimizers are switched off. These SHALL apply on every provider the
model runs on.

A model MAY switch off more than one optimizer - see *A model may disable several graph optimizers*
for how they are written and for why getting that wrong cannot be observed except as a model failing
to load.

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

### Requirement: Osaka's measured settings

The Osaka upscaler SHALL be configured with the settings measured for it at both of its precisions:

- **Sequential execution.** Its transformer is 12,940 nodes and close to a linear backbone, so the
  inter-op thread pool is charged a handoff at every one of them with no branch wide enough to use
  it - measured at -13.5% at FP16 and -10.1% at INT8 on the region, with output identical to the
  parallel run.
- **The memory planner off.** The shapes never vary, so the planner's own assumption holds; it is
  simply a loss on activations this large, measured at +22% on the VAE encoder. On the CUDA provider
  it is a tie on average but becomes bimodal once execution is sequential, with individual runs as
  bad as 2.5x.
- **`ReshapeFusion` and `SimplifiedLayerNormFusion` disabled.** This is not a tuning choice. Under the
  pinned runtime these transformers rewrite the diffusion transformer so that it refers to a tensor
  they removed, and session creation **on the CPU provider** then fails outright naming a node they
  created. The model does not load on that provider without the setting. Both names are declared,
  as the reference declares them; `SimplifiedLayerNormFusion` is the one measured to be sufficient on
  its own, and `ReshapeFusion` costs nothing but a fusion this graph was not relying on.

  The two SHALL be written with the separator the runtime parses, which is the thing that has to be
  right: a wrongly joined pair is read as a single unrecognised name and disables **neither**,
  silently, leaving the model failing to load exactly as though nothing had been set.
- **CoreML restricted to the CPU and GPU.** The default lets CoreML dispatch to the Neural Engine,
  which these graphs are consistently worse on: measured at 4.2x on the VAE encoder and 3.9x on the
  decoder, while pinning the Neural Engine costs 2.5x on the transformer.
- **TensorRT's builder optimization level at 3** rather than the 5 every other model carries. Runtime
  is flat across levels 1, 3 and 5 while the engine build is 86s, 116s and 203s - so the higher level
  buys this graph nothing and costs a minute and a half of the first run a user ever makes.

Osaka SHALL NOT be configured with TensorRT's FP16 mode at either precision. On the FP16 export it is
already honoured by the export's own typing and changes nothing; on the INT8 export it is not a no-op
at all and takes the transformer from 220.3ms to 78.4ms - but that speed is bought with precision the
caller did not ask for, the transformer drifting to a cosine similarity of 0.9827 against the same
graph on another provider where every other configuration stays at 0.9999. A user who asked for INT8
must not be given something further from the reference result than INT8 is.

Every one of these settings is the reference implementation's, field for field. Osaka's declaration
carries no deliberate divergence from it.

#### Scenario: Osaka configured for any provider

- **WHEN** Osaka is configured at FP16 or at INT8
- **THEN** its session settings run one node at a time, keep the memory planner off, and disable both
  `ReshapeFusion` and `SimplifiedLayerNormFusion`
- **AND** the two names are joined so that each is actually switched off
- **AND** the same settings apply at both precisions

#### Scenario: Osaka opened on the CPU provider

- **WHEN** Osaka's diffusion transformer is opened on the CPU provider under the pinned runtime
- **THEN** the session is created rather than failing on a node the optimization pipeline invented

#### Scenario: Osaka attached to CoreML

- **WHEN** Osaka is configured for CoreML
- **THEN** its options keep it off the Neural Engine and on the CPU and GPU

#### Scenario: Osaka attached to TensorRT

- **WHEN** Osaka is configured for TensorRT
- **THEN** its options carry builder optimization level 3
- **AND** its options carry TensorRT's FP16 mode disabled and its INT8 mode disabled

#### Scenario: A model that is not Osaka

- **WHEN** any other model is configured for TensorRT
- **THEN** its options carry builder optimization level 5, unchanged by Osaka's declaration
