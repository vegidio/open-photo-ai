## ADDED Requirements

### Requirement: A model's measured settings may differ between its own graphs

A model built from several graphs MAY declare settings for one graph that differ from the settings
its other graphs carry, and each graph SHALL be configured with the declaration that names it. Where
a graph is not named, it SHALL carry the model's own declaration unchanged.

This exists because the settings that suit a graph follow that graph's op mix, and a multi-stage
model does not have one. Osaka is the first: its two VAE halves are convolutional and gain from the
CUDA provider's NHWC layout, which is what its FP16 tensor-core kernels want - measured at -8.0% on
the encoder and -9.4% on the decoder. Its diffusion transformer contains no convolution whatsoever,
so there is no convolution kernel for that layout to be faster at and what remains is the layout
transform's own cost - measured at +1.3%. The choice is between two real numbers rather than between
one and a rounding error, which is why it is worth a per-graph declaration rather than rounding off.

A per-graph declaration SHALL NOT be a way to give one graph settings the model has not been measured
with. A setting that helps the graph it was chosen for can be severe on the others - on the reference
implementation, enabling TensorRT's INT8 mode for the transformer took the same model's VAE encoder
from 21.4ms to 124.6ms and its decoder from 45.5ms to 298.9ms - so a declaration reaching graphs it
was not measured against is the failure this requirement is shaped to prevent.

#### Scenario: A model whose graphs agree

- **WHEN** a model declares settings without naming any graph
- **THEN** every graph of that model is configured with that declaration

#### Scenario: Osaka's VAE halves

- **WHEN** Osaka's VAE encoder or VAE decoder is configured for the CUDA provider
- **THEN** its options select the NHWC convolution layout

#### Scenario: Osaka's diffusion transformer

- **WHEN** Osaka's diffusion transformer is configured for the CUDA provider
- **THEN** its options keep the runtime's default NCHW layout
- **AND** the two VAE halves of the same model are still configured with NHWC

#### Scenario: A graph that is not named

- **WHEN** a model declares settings for one of its graphs and another graph is configured
- **THEN** that other graph carries the model's own declaration rather than the named graph's

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
- **`SimplifiedLayerNormFusion` disabled, and it alone.** This is not a tuning choice. Under the
  pinned runtime that transformer rewrites the diffusion transformer so that it refers to a tensor it
  removed, and session creation **on the CPU provider** then fails outright naming a node it created.
  The model does not load on that provider without it.

  A model SHALL name at most **one** optimizer to disable. The pinned runtime takes its disable
  setting as a single transformer *name* rather than as a list, so a value naming two matches no
  transformer at all and disables neither - silently, in either order, and with or without a space.
  Osaka is where this was measured, and the measurement corrects the reference implementation, which
  names `ReshapeFusion` beside this one: on this runtime that pairing is indistinguishable from
  disabling nothing, and `ReshapeFusion` is not required for this graph to load in any case.
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

#### Scenario: Osaka configured for any provider

- **WHEN** Osaka is configured at FP16 or at INT8
- **THEN** its session settings run one node at a time, keep the memory planner off, and disable
  `SimplifiedLayerNormFusion`
- **AND** no second optimizer is named beside it, which would disable neither
- **AND** the same settings apply at both precisions

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

### Requirement: A model disables at most one graph optimizer

A model MAY name one graph transformer to switch off, and SHALL NOT name more than one. Where a model
names none, the runtime's optimization pipeline SHALL be left alone rather than handed an empty
setting.

This is the runtime's own contract rather than a simplification of it. ONNX Runtime 1.26 reads this
setting as a single transformer name, so a value naming two matches nothing and switches off neither -
and it reports nothing, so the only symptom is a model that fails to load exactly as though the
setting had never been written. Measured against Osaka's diffusion transformer on the pinned runtime:
`SimplifiedLayerNormFusion` alone opens the graph, while that same name beside `ReshapeFusion` fails
identically to naming neither - as does the reverse ordering, a space after the comma, and a pair
whose second name is not a transformer at all.

A model that genuinely needs two has found a runtime limitation rather than a setting to express, and
SHALL raise it: the honest answer is a runtime version, not a value the runtime will ignore.

#### Scenario: A model that disables one optimizer

- **WHEN** a model names one graph transformer to switch off
- **THEN** its session is configured with that name

#### Scenario: A model that disables none

- **WHEN** a model names no graph transformer
- **THEN** the disable setting is not written at all, rather than written empty

## MODIFIED Requirements

### Requirement: A model's measured settings include how the graph itself is run

A model's declaration SHALL be able to change settings that belong to the session rather than to one
provider: whether independent branches of the graph may run on separate threads, whether the memory
planner is used, and which individual graph optimizer is switched off. These SHALL apply on every
provider the model runs on.

The optimizer is singular, and that is the runtime's contract rather than a simplification of it -
see *A model disables at most one graph optimizer* for the measurement behind it and for what naming
a second one silently costs.

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
