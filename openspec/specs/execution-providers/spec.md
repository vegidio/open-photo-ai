# execution-providers Specification

## Purpose

Deciding what a model is asked to run on and the settings a session is built with: naming the
execution providers a user can choose between, working out what `Auto` means on the machine in front
of it, deciding what happens to a request that machine cannot honour, and carrying the measured
settings each provider and each model needs - because the difference between the right provider
options and the defaults is a factor of ten on a run, and the difference between the right ones and
the wrong ones is sometimes a silently incorrect image.

## Requirements

### Requirement: The execution providers a run can be asked for

The application SHALL name exactly five choices a run can be asked for: `Auto`, `CPU`, `CoreML`,
`CUDA` and `TensorRT`. Their spelling is user-facing - it is what a settings file stores and what a
command line accepts - and SHALL be stable.

A choice built from text the application did not produce SHALL be parsed rather than assumed: the
match SHALL be case-insensitive, and text naming none of the five SHALL be refused with an error
naming what was given. A choice SHALL survive a round trip through the application's own
serialization unchanged.

The reference implementation additionally names OpenVINO, whose only behaviour is to decline to
attach on every platform. It is deliberately not named here.

#### Scenario: A provider named in a settings file

- **WHEN** the text `coreml`, `CoreML` or `COREML` is parsed
- **THEN** all three yield the CoreML choice

#### Scenario: Text naming no published provider

- **WHEN** the text `openvino` or `metal` is parsed
- **THEN** the parse fails with an error naming the text that was given
- **AND** no choice is produced

#### Scenario: A stored choice is read back

- **WHEN** a choice is serialized and read back
- **THEN** it is the same choice
- **AND** the stored text is the user-facing spelling of that provider

### Requirement: The machine's provider report answers for a named provider

The report of which providers the machine supports SHALL answer, for any one of the five choices,
whether this machine supports it. `Auto` and `CPU` SHALL always be answered as supported, because
there is no machine on which the application cannot run on the CPU.

#### Scenario: A provider whose library was never installed

- **WHEN** the report of a machine with no NVIDIA libraries installed is asked about CUDA
- **THEN** it answers that the machine does not support it

#### Scenario: Auto on any machine

- **WHEN** any machine's report is asked about `Auto` or `CPU`
- **THEN** it answers that the machine supports it

### Requirement: Auto runs on the best provider this machine supports

`Auto` SHALL resolve to the providers the machine supports, in a fixed preference order: TensorRT,
then CUDA, then CoreML. Every supported provider from that order SHALL be attached, in that order, so
that one declining at session-build time leaves the next to run the graph. The CPU SHALL NOT appear in
that list: it needs no configuration and is what the runtime falls back to once every provider above
it has declined a node.

The order is a property of the providers, not of the operating system: a machine reports CoreML
unsupported off macOS and reports the NVIDIA providers unsupported until their libraries are
installed, so no per-platform list is consulted.

#### Scenario: A machine with the NVIDIA libraries installed

- **WHEN** `Auto` is resolved on a machine reporting CUDA and TensorRT supported
- **THEN** TensorRT is attached first and CUDA second

#### Scenario: A Mac

- **WHEN** `Auto` is resolved on a machine reporting only CoreML supported
- **THEN** CoreML is the only provider attached

#### Scenario: A machine with no accelerator

- **WHEN** `Auto` is resolved on a machine reporting no provider beyond the CPU
- **THEN** no provider is attached
- **AND** the run is a CPU run

### Requirement: A named provider the machine supports is the only one used

A request for a specific supported provider SHALL attach that provider and no other, so that a
deliberate choice is not quietly widened into the `Auto` chain. A request for the CPU SHALL attach no
provider at all and SHALL carry no provider configuration.

#### Scenario: CUDA requested on a machine that supports CUDA and TensorRT

- **WHEN** CUDA is requested
- **THEN** CUDA is the only provider attached
- **AND** TensorRT is not attached

#### Scenario: The CPU requested

- **WHEN** the CPU is requested
- **THEN** no provider is attached
- **AND** no provider options are produced

### Requirement: A request the machine cannot honour becomes a CPU run and is reported

A request for a provider the machine does not support SHALL NOT fail the run and SHALL NOT be
silently honoured. It SHALL resolve to a CPU run, and what was requested SHALL be reported alongside
what was resolved, so that the downgrade can be stated once rather than surfacing later as an
unexplained slow run.

This unifies two outcomes the reference implementation keeps apart: there, a provider its platform
does not list fails the run outright, while one listed but uninstalled is attached, declines
natively, and is logged. Both are the same fact - the machine cannot run this - and both are answered
the same way here.

#### Scenario: CoreML requested on a machine that is not a Mac

- **WHEN** CoreML is requested on a machine reporting CoreML unsupported
- **THEN** no provider is attached
- **AND** the result reports that CoreML was requested and a CPU run was resolved

#### Scenario: A provider whose libraries are absent

- **WHEN** TensorRT is requested on a machine reporting TensorRT unsupported
- **THEN** the run proceeds on the CPU rather than failing
- **AND** the result reports the requested provider beside the resolved one

### Requirement: Each provider is configured with the settings it was measured with

Every provider attached SHALL carry the settings measured for it, not the runtime's defaults:

- **TensorRT** SHALL run at builder optimization level 5, with its engine cache and its timing cache
  enabled, with hardware-compatible engines off, with INT8 off, and with a 4 GiB workspace ceiling.
- **CUDA** SHALL search convolution algorithms exhaustively, SHALL be allowed the maximum convolution
  workspace, SHALL have no GPU memory ceiling imposed, and SHALL use the runtime's NCHW layout unless
  the model asks otherwise.
- **CoreML** SHALL use the MLProgram model format, SHALL require static input shapes, SHALL NOT be
  enabled on control-flow subgraphs, and SHALL dispatch to every compute unit unless the model asks
  otherwise.

The CoreML model format is fixed rather than offered as a per-model setting: the older format has no
typed execution, so an FP32 graph placed on the Neural Engine runs in half precision - measured on the
reference implementation as a 39% speed-up landing on the accuracy of that model's FP16 export. A
user who asked for FP32 must not be given FP16 by a provider setting.

#### Scenario: TensorRT attached

- **WHEN** TensorRT is attached for any model
- **THEN** its options carry the engine cache enabled, the timing cache enabled, hardware-compatible
  engines off, INT8 off, builder optimization level 5 and a 4 GiB workspace

#### Scenario: CoreML attached

- **WHEN** CoreML is attached for any model
- **THEN** its options select the MLProgram format and require static input shapes

### Requirement: Graph capture is disabled on both NVIDIA providers

The application SHALL leave CUDA graph capture disabled on the CUDA provider and on TensorRT, and
SHALL NOT offer it as a per-model setting.

It is measured and rejected rather than unexamined. On TensorRT the capture run is correct and every
subsequent run on that session returns an all-zero output with no error and plausible timings - which
in the application is no faces detected from the second detection onwards, or a blank recovered face.
On the CUDA provider the capture run fails outright because the BLAS library initializes lazily inside
the captured stream. Anyone revisiting it must compare the **second** run on the **same** session
against a CPU result; a single-run comparison reports a speed-up and ships a broken model.

#### Scenario: Any NVIDIA provider attached

- **WHEN** CUDA or TensorRT is attached for any model
- **THEN** its options carry graph capture disabled

### Requirement: Provider caches are pointed at the directories supplied for that model

What a provider compiles from a model - a TensorRT engine, a CoreML MLProgram - SHALL be cached in a
directory belonging to that model alone, and TensorRT's timing cache SHALL be pointed at a separate
directory shared by every model. Both directories SHALL be supplied to the configuration rather than
decided by it.

The two must not be conflated. The compiled result is per-model because replacing a model's weights
has to invalidate it. The timing cache is shared because kernel timings carry between graphs: the
reference implementation measures a cold build against a cache other models seeded at up to 60%
faster than against an empty one, and pointing TensorRT at a per-model directory for it silently gives
that back with nothing reporting the loss.

#### Scenario: Two models configured for TensorRT

- **WHEN** two different models are configured with their own engine directories and one shared
  timing directory
- **THEN** each carries its own engine cache path
- **AND** both carry the same timing cache path

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

A declaration MAY also change **how a provider compiles the model for the device**, which is a trade
rather than a win: the default strategy compiles once into a plan that is good across input sizes,
while specializing for latency spends longer at session build on a plan for the shapes it was given.
It is only worth declaring on a fixed-shape graph that stays resident, and even then only on the
evidence of a measurement — the same hint is a tie on some graphs and pure cold-start cost on others,
and it changes no pixel either way. Where a model declares it, the system SHALL apply it on every
machine, and where a model declares nothing the provider's own default strategy SHALL be used.

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

#### Scenario: A model measured as worth specializing

- **WHEN** a model declaring that its graph be specialized for latency is configured for CoreML
- **THEN** its options carry that strategy rather than the provider's default one
- **AND** the same setting applies at both of its precisions

#### Scenario: A model with nothing measured

- **WHEN** a model that declares no settings is configured for any provider
- **THEN** its options are the provider defaults, with nothing added or removed

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

### Requirement: New York's measured settings

The New York face detector SHALL be configured with the settings measured for it, at **both** of its
precisions:

- **The CUDA provider's NHWC convolution layout.** RetinaFace is a ResNet-34 backbone, a feature
  pyramid and three convolutional heads - dense convolution end to end - so the layout removes a
  transpose pair around every convolution. Measured on the graph alone at -13.9% at FP32 for the
  layout on its own.
- **Sequential execution.** The backbone is 176 nodes with no branch wide enough to pay for the
  inter-op handoff, so the parallel mode can only charge one per node. It is not per-provider, so it
  reaches CoreML too, where it is worth -6.5% at FP16 and a tie at FP32 - the largest CoreML win that
  mode has anywhere in the catalogue.

The two are independent and compose: together they are -17.5% at FP32 and -35.1% at FP16 on the graph,
and -11.6% and -23.4% end to end. The output is unaffected, checked across twenty consecutive runs on
one session - which is the check that matters here, because the failure this graph has actually shown
was a session that went wrong only from its second run onwards.

**That the layout is declared at FP32 as well as FP16 is the load-bearing part of this requirement.**
The only other model in the catalogue that asks for NHWC, Athens, declares it for FP16 **only**,
because at FP32 it measured +8% there. The layout is not a property of the precision; it is a property
of what the provider has kernels for on the shapes each graph actually asks for, so it has to be
measured per graph at each precision, and here the answer is the opposite of the answer next door. A
reader tidying this for symmetry with Athens would remove exactly the half worth 17.5%, and nothing
would report it: the model still loads and still returns the same faces, only slower.

New York SHALL be left on the CoreML defaults, at both precisions. Every CoreML setting this system
can express was measured against this graph and none of them won, which nothing else in the catalogue
records and which is why it is written down rather than left to look like an oversight. The expensive
one is restricting CoreML to the CPU and GPU - the setting most likely to be copied over from Athens
for symmetry - which costs the FP16 graph 20%, because dense convolution is exactly what the Neural
Engine is built for. Asking for fast prediction measured as a tie.

Two of the CUDA provider's own defaults are load-bearing for this graph and SHALL NOT be changed for
it. Disabling TF32 costs 114% at FP32 and 29% at FP16. Convolution-bias fusion is broken here: at FP32
it decodes zero faces from intact confidence scores, and at FP16 it fails on the first convolution.

#### Scenario: New York configured for CUDA

- **WHEN** New York is configured for the CUDA provider at FP32 or at FP16
- **THEN** its options select the NHWC convolution layout
- **AND** the same setting applies at both precisions, unlike Athens

#### Scenario: New York configured for any provider

- **WHEN** New York is configured at either precision, for any provider
- **THEN** its session settings run one node at a time

#### Scenario: New York attached to CoreML

- **WHEN** New York is configured for CoreML at either precision
- **THEN** its options carry CoreML's default compute units rather than restricting it to the CPU and GPU
- **AND** its options carry CoreML's default specialization strategy

#### Scenario: New York's output is unaffected by its settings

- **WHEN** New York is run with its measured settings and with the provider defaults
- **THEN** both runs report the same faces in the same order

### Requirement: Athens' measured settings

The Athens face-restoration model SHALL be configured with the settings measured for it:

- **CoreML restricted to the CPU and GPU, at both precisions.** Left to choose, CoreML gives most of the
  FP16 graph to the Neural Engine and the transitions on and off it cost more than the engine saves:
  restricting it is worth 1.57x on an M2 Max. The op mix is why — this is a CodeFormer, 72 GroupNorms,
  19 LayerNorms held at FP32 for precision, and several hundred reshapes and transposes through the
  transformer. Asking for the CPU and the Neural Engine together is the tell: 10-14x slower is not a
  slower engine, it is an engine rejecting most of the graph and thrashing on what is left.
- **Sequential execution.** Worth -20.6% at FP32 and -13.2% at FP16 on the graph alone under CUDA,
  diluted to -8.5% and -4.6% end to end because most of a face-recovery run is the alignment and the
  composite around each face rather than the graph. The output is bit-identical.
- **The CUDA provider's NHWC convolution layout, at FP16 only.** On an RTX 5090 it is worth -7.7% at
  FP16 and costs +5.9% at FP32 on the graph, which is -4% and +8% end to end. The split is the tensor
  cores: cuDNN's FP16 kernels are written for NHWC, so at FP16 the layout removes a transpose pair
  around every convolution and reaches them, while at FP32 there are no such kernels to reach and the
  conversion is pure cost.

**That the CoreML restriction is declared at FP32 as well as FP16 is deliberate**, even though only FP16
moves: an FP32 MLProgram cannot reach the Neural Engine at all, so the FP32 declaration states the
intent rather than changing a measurement, and a future export that shifted what the Neural Engine will
accept cannot silently re-enable it.

**That the NHWC layout is declared at FP16 only is the load-bearing part of this requirement**, and it is
the opposite of the answer for New York, which declares it at both. The layout is a property of what the
provider has kernels for on the shapes each graph asks for, not a property of the precision, so it is
measured per graph at each precision. Declaring it at FP32 here costs 8% end to end and nothing reports
it: the model still loads and still restores the same faces, only slower. The output is unaffected either
way — through the pipeline the NHWC result lands 67.1 dB from the NCHW one, where both sit 56.6 dB from
the FP32 model's.

Athens SHALL be the one model in the catalogue whose execution mode is set **against** its own CoreML
measurement. On CoreML sequential execution leans 0.1-0.3% the other way here, measured in both build
orders, where Tokyo, Santorini and New York each gain 3.5-6.5% from it. The execution mode is a session
setting rather than a per-provider one, so there is one answer to give, and 0.1-0.3% is not a reason to
make this the one model that runs a different mode from the rest. On TensorRT it is a tie, as it is for
New York.

Two of the CUDA provider's own defaults are load-bearing for this graph and SHALL NOT be changed for it.
Disabling TF32 costs 37%. Convolution-bias fusion returns a **different answer on every run** here, which
is the worst failure in this catalogue: not an error, not a slower run, but a restoration that is not
reproducible and would make one image's cached result disagree with the same image recomputed.

The convolution algorithm search, the default-stream copy, the arena extension strategy, the EP-level
unified stream, the tunable-op switch and the attention kernel selection were all measured on this graph
and are all within noise; the search SHALL NOT be set to its default setting, which costs 70%.

#### Scenario: Athens attached to CoreML

- **WHEN** Athens is configured for CoreML at FP32 or at FP16
- **THEN** its options restrict CoreML to the CPU and the GPU
- **AND** the same setting applies at both precisions

#### Scenario: Athens configured for CUDA at FP16

- **WHEN** Athens is configured for the CUDA provider at FP16
- **THEN** its options select the NHWC convolution layout

#### Scenario: Athens configured for CUDA at FP32

- **WHEN** Athens is configured for the CUDA provider at FP32
- **THEN** its options do not select the NHWC convolution layout, unlike New York at the same precision

#### Scenario: Athens configured for any provider

- **WHEN** Athens is configured at either precision, for any provider
- **THEN** its session settings run one node at a time

### Requirement: Santorini's measured settings

The Santorini face-restoration model SHALL be configured with the settings measured for it:

- **CoreML asked to specialize the graph for latency.** It is worth 1.04x at FP16 on an M2 Max and a
  tie at FP32 in steady state, at the price of 20-30ms of cold start in every FP32 round — which is
  the specialization time Apple names as this hint's price. That cost is paid once per session build
  and models stay resident, so it is set for the graph rather than for one precision.
- **Sequential execution, at both precisions.** This graph is a decoder hanging off a U-Net — a
  backbone rather than a branchy graph — so the inter-op pool never has two independent nodes to hand
  out and only charges the handoff. Worth 1.09x at FP16 and 1.04x at FP32 on CUDA with bit-identical
  output, and the CPU provider gains too, which is what shows it to be the graph's shape rather than
  anything about CUDA. CoreML agrees at FP16 (-4.8%) and is a tie at FP32.

**Santorini SHALL NOT restrict CoreML's compute units**, and this is the load-bearing half of the
requirement. It is the setting most likely to be copied over from Athens by someone tidying the two
models of one family into agreement, and doing so costs about 9%: Santorini's FP16 graph runs FP16
through its upsampling path, so the Neural Engine takes it whole, where the other model's
normalization and transpose mix thrashes on it. **The difference is the export rather than the
architecture** — before these weights were re-exported their resize nodes were held at FP32 with 84
cast nodes wrapping 42 upsamples, and the other model's rule looked right for them too. If the weights
are re-exported again, this is the first thing to re-measure.

The end-to-end margin from sequential execution at FP16 is much wider than the graph alone accounts
for — -9.8% over a two-face sample, where twice the graph's own saving is a quarter of that — because
the inter-op pool is process-wide and in parallel mode competes for the same cores as the alignment
and compositing work around each face. Both figures are recorded because either one alone is
misleading.

Two of the CUDA provider's options that would otherwise look like free wins **SHALL NOT** be set for
this graph, because each breaks it rather than slowing it:

- **The NHWC convolution layout fails at run time**, reporting that the input channels do not equal
  the kernel channels times the group. This is exactly the modulated-convolution hazard that setting
  carries: 23 of this graph's 95 convolutions take a weight computed by a reshape rather than by a
  stored initializer.
- **Convolution-bias fusion** costs time at FP32 and fails inside the provider's convolution frontend
  at FP16.

Every other CUDA option measured on this graph lands within noise, except two that are not options at
all here: the convolution algorithm search SHALL NOT be set to its default setting, which costs 2.1x
because all 95 convolutions then fall back, and TF32 SHALL NOT be disabled, which costs 1.5x.

#### Scenario: Santorini attached to CoreML

- **WHEN** Santorini is configured for CoreML at FP32 or at FP16
- **THEN** its options ask CoreML to specialize the graph for latency
- **AND** its options carry CoreML's default compute units rather than restricting it to the CPU and GPU, unlike Athens

#### Scenario: Santorini configured for any provider

- **WHEN** Santorini is configured at either precision, for any provider
- **THEN** its session settings run one node at a time

#### Scenario: Santorini configured for CUDA

- **WHEN** Santorini is configured for the CUDA provider at either precision
- **THEN** its options keep the runtime's default NCHW layout, unlike Athens at FP16 and New York at both

### Requirement: Paris' measured settings

The Paris light adjustment model SHALL be configured with the settings measured for it, and SHALL declare them at
**FP16 only**:

- **CoreML restricted to the CPU and GPU.** On an M2 Max at Paris' 1024 square, CoreML's default compute units cost
  roughly twice what the GPU alone does — 18.3ms against 9.2ms — and the restriction is also what turns this model's
  FP16 graph from slower than its FP32 one into a tie with it. The Neural Engine loses in both precisions and loses
  badly: +872% at FP32 and +72% at FP16.
- **Sequential execution.** A much smaller figure than the compute units, and declared on the strength of its
  consistency rather than its size: -2.3% and -2.9% on CoreML across two build orders, -0.4% on the CPU provider, and
  the lowest minimum of any configuration measured for this graph.

**Paris SHALL NOT declare either setting at FP32.** The sequential execution mode measures +1.2% on CoreML against
-2.7% on the CPU provider there, which is a wash rather than a setting, and a declaration that is free at one precision
and worth something at the other is not worth carrying to a precision that did not earn it. This is the same
precision-split rule Saitama and Kyoto already follow, reached from a different measurement.

CoreML's latency specialization hint and its low-precision GPU accumulation both measured within 0.8% in both
precisions for this graph and SHALL NOT be set.

#### Scenario: Paris attached to CoreML at FP16

- **WHEN** Paris is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run one node at a time

#### Scenario: Paris attached to CoreML at FP32

- **WHEN** Paris is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units and the provider's default execution mode

### Requirement: Lyon's measured settings

The Lyon light adjustment model SHALL be configured with the setting measured for it, and SHALL declare it at **FP16
only**:

- **CoreML restricted to the CPU and GPU.** On an M2 Max this is worth 2.6x — 1770ms against 687ms — and the mechanism
  is recorded rather than inferred, because CoreML's own compute plan reports it: under the default compute units 363
  of this graph's operations are placed on the Neural Engine among 1626 placed on the GPU, so every run pays hundreds
  of transitions between the two.
- The same setting is also **more accurate**, which is unusual enough to be part of the requirement rather than a note
  beside it. The operations placed on the Neural Engine run at its reduced internal precision, so against an FP32 CPU
  reference the default compute units score 69.3 dB where the restriction scores 74.0 dB. Faster and closer to the
  reference, and it turns this model's FP16 graph from slower than its FP32 one into faster than it.

**Lyon SHALL NOT be configured for CoreML's Neural Engine**, which will not take this graph at all — the compiler
reports that it failed to compile the model — so what such a configuration measures is the fallback path, at 5.7s at
FP32 and 16.2s at FP16.

**Lyon SHALL NOT declare a sequential execution mode**, and this is the load-bearing contrast with Paris. The two
models sit in one family and reach opposite answers on the same hardware: a sequential mode, CoreML's latency
specialization hint and its low-precision GPU accumulation all land within 0.3% of doing nothing for Lyon, with
bit-identical output, because this graph is compiled into a **single fused CoreML operation** and is already
fixed-shape and resident — which is the case the specialization hint exists to buy and which leaves the inter-op pool
nothing to schedule. Copying Paris' settings across on the grounds that both adjust light would be applying a
measurement to a graph it was never taken against.

Anyone re-measuring Lyon SHALL first confirm that its graph is still compiled as a single CoreML partition. Before the
rewrites that made it one it compiled as 207, and CoreML was then slower than the CPU provider — which would make every
figure above a measurement of partition handoff rather than of compute units.

#### Scenario: Lyon attached to CoreML at FP16

- **WHEN** Lyon is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run nodes in the provider's default execution mode, unlike Paris at the same precision

#### Scenario: Lyon attached to CoreML at FP32

- **WHEN** Lyon is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units

#### Scenario: Two models of one family disagreeing

- **WHEN** Paris and Lyon are each configured for CoreML at FP16
- **THEN** both restrict CoreML to the CPU and GPU
- **AND** only Paris asks for one node at a time

### Requirement: Rio's measured settings

The Rio colour balance model SHALL be configured with the setting measured for it, and SHALL declare it at **FP16
only**:

- **CoreML's fast-prediction specialization.** Worth -3.0% at FP16 on an M2 Max at Rio's 656 square. It is the
  smallest figure any model in this catalogue has declared a setting on, and it is declared because the *mechanism* is
  understood rather than because the number is large: the specialization trades compile time for prediction latency
  and is only worth paying for on a fixed-shape graph that stays resident, which is what this graph became when it was
  re-exported at a fixed square. Santorini is the only other model here that earns it.

**Rio SHALL NOT declare the setting at FP32.** Both configurations bottom out at 14.48 ms there, so what separates
them is variance rather than compute — and a declaration that is worth something at one precision and nothing at the
other is not carried to the precision that did not earn it. This is the same precision-split rule Paris, Lyon, Saitama
and Kyoto already follow.

**Rio SHALL NOT be restricted off CoreML's Neural Engine, at either precision**, and this is the load-bearing
contrast in this requirement. Paris and Lyon both pin the CPU and GPU because CoreML scatters their graphs across the
Neural Engine and the GPU and they pay for every transition. Rio is the opposite case: its graph is 52 convolutional
nodes, which is what the Neural Engine is built for, and it lands there whole rather than in pieces — so restricting
it to the CPU and GPU **costs +91%**, at no accuracy gain. Neither family's compute-unit answer is evidence for the
other's, and neither SHALL be carried across on the grounds that both correct colour.

FP16 is worth having on this model in a way it is not on most, and the reason is a property of the pipeline rather
than of the graph: the model's output is only ever sampled into a global colour fit, so half precision is averaged
away rather than shown. It scores 84.5 dB against FP32 through CoreML, with a largest DC shift of 0.058 levels.

#### Scenario: Rio attached to CoreML at FP16

- **WHEN** Rio is configured for CoreML at FP16
- **THEN** its options ask CoreML to specialize the graph for prediction latency
- **AND** its options carry CoreML's default compute units, leaving the Neural Engine available

#### Scenario: Rio attached to CoreML at FP32

- **WHEN** Rio is configured for CoreML at FP32
- **THEN** its options carry the provider's defaults throughout

#### Scenario: Two families disagreeing about the Neural Engine

- **WHEN** Rio and Paris are each configured for CoreML at FP16
- **THEN** Paris restricts CoreML to the CPU and GPU
- **AND** Rio does not

### Requirement: São Paulo's measured settings

The São Paulo colour balance model SHALL be configured with **the provider defaults, unchanged, at both precisions**.

**That is a measurement rather than an omission**, and it is why this requirement exists at all: a model that
declares nothing and a model that was swept and found to want nothing are indistinguishable in the configuration and
are not the same fact about the catalogue. Swept on an M2 Max at the 656 square, nothing beats the CoreML defaults.
The fast-prediction specialization is the only row that even looks like a win — 3.3% at FP16 — and it sits inside
that row's own 6-7% run-to-run spread. Execution mode is likewise a wash, which is worth recording because this
application runs the parallel mode rather than ONNX Runtime's sequential one: on a graph that is a single fused
CoreML node the inter-op pool has nothing to schedule either way.

**São Paulo SHALL NOT carry Rio's settings, and this is the point of stating it.** The two are the same family, the
same canvas and the same precisions, and Rio's fast-prediction specialization was measured on the editing network
alone. São Paulo's graph is that network plus a weight predictor plus two internal resamples — a different op mix,
which is exactly what a profile is a measurement of. A setting SHALL NOT be carried from one model to another on the
grounds that both correct colour, in either direction.

**São Paulo SHALL NOT be restricted off CoreML's GPU, at either precision.** Restricting it to the CPU and Neural
Engine costs **+62% at FP16 and +732% at FP32**, at no accuracy gain. That is the same answer Rio reaches from the
opposite direction and for the same underlying reason: Rio is 52 convolutional nodes that the Neural Engine takes
whole, while this is 511 nodes including resamples and a softmax, which it cannot.

Both precisions SHALL be published. They are bit-identical between the CPU and CoreML providers, so which provider a
machine picks cannot change what the user sees, and FP16 measures 66.8 dB against FP32 on the same provider — a
largest single-channel error of one 8-bit level — at half the download, 16.7 MB against 33.2 MB.

#### Scenario: São Paulo attached to CoreML at either precision

- **WHEN** São Paulo is configured for CoreML at FP16 or at FP32
- **THEN** its options carry the provider's defaults throughout, with nothing added or removed
- **AND** its options leave CoreML's GPU available

#### Scenario: Two models of one family disagreeing

- **WHEN** Rio and São Paulo are each configured for CoreML at FP16
- **THEN** Rio's options ask CoreML to specialize the graph for prediction latency
- **AND** São Paulo's do not

#### Scenario: São Paulo on any other provider

- **WHEN** São Paulo is configured for a provider other than CoreML
- **THEN** its options are that provider's defaults

### Requirement: Gothenburg's measured settings

The Gothenburg denoise model SHALL be configured with the setting measured for it, and SHALL declare it at **FP16
only**:

- **CoreML restricted to the CPU and GPU.** Gothenburg is a Restormer: its 44 blocks are mostly layer normalization,
  reshape and transpose around a channel-attention matmul, which is not what the Neural Engine is built for, and CoreML
  takes the graph there whenever its default compute units permit. Measured per 256-square tile on an M2 Max, macOS
  26.6, ONNX Runtime 1.26, the restriction costs 133.3ms at FP16 against the default's 179.4ms — **-25.7%** — and is
  what makes FP16 faster than FP32 on this model at all. The Neural Engine alone is slower in both precisions: +571% at
  FP32 and +46.7% at FP16.

**Gothenburg SHALL NOT declare the setting at FP32.** There it measured +0.1%, because CoreML bars an FP32 program from
the Neural Engine, so the default and the restriction compile to the same session. The precision split is kept so that
a setting is never carried to a precision it was not measured to help.

**Gothenburg SHALL NOT declare a sequential execution mode**: it measured -0.4% at FP32 and +2.9% at FP16, because
CoreML fuses the graph into a single node and leaves the inter-op pool nothing to schedule. CoreML's fast-prediction
hint and its low-precision GPU accumulation measured within run-to-run spread and SHALL NOT be set.

These figures are the reference implementation's, **ported without being re-measured on this project's build**, and
were taken against the re-exported graph that CoreML compiles as one partition rather than 48. Anyone re-measuring
Gothenburg SHALL first confirm that its graph is still a single CoreML partition, which would otherwise make every figure
a measurement of partition handoff.

#### Scenario: Gothenburg attached to CoreML at FP16

- **WHEN** Gothenburg is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run nodes in the provider's default execution mode

#### Scenario: Gothenburg attached to CoreML at FP32

- **WHEN** Gothenburg is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units and the provider's default execution mode

### Requirement: Malmö's measured settings

The Malmö denoise model SHALL be configured with the setting measured for it, and SHALL declare it at **FP16 only**:

- **CoreML restricted to the CPU and GPU**, for the same architectural reason as Gothenburg — the two are the same
  Restormer backbone. Measured per 256-square tile on the same M2 Max, macOS 26.6, ONNX Runtime 1.26, the restriction
  costs 138.6ms at FP16 against the default's 197.9ms — **-29.9%**. The Neural Engine alone is slower in both
  precisions: +630% at FP32 and +52.6% at FP16.

**Malmö SHALL NOT declare the setting at FP32**, where it measured -0.1%. CoreML's fast-prediction hint and a
sequential execution mode measured within run-to-run spread and SHALL NOT be set.

Malmö's figures were measured separately from Gothenburg's and are not evidence for one another: two checkpoints of one
architecture reaching the same answer is a finding, not a licence to copy one's settings to the next.

These figures are the reference implementation's, **ported without being re-measured on this project's build**, and
were taken against the re-exported graph that CoreML compiles as one partition rather than 92. Anyone re-measuring Malmö
SHALL first confirm that its graph is still a single CoreML partition.

#### Scenario: Malmö attached to CoreML at FP16

- **WHEN** Malmö is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run nodes in the provider's default execution mode

#### Scenario: Malmö attached to CoreML at FP32

- **WHEN** Malmö is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units and the provider's default execution mode

### Requirement: Stockholm's measured settings

The Stockholm denoise model SHALL be configured with the provider defaults at **both FP32 and FP16**, and SHALL
declare no setting at either precision. This is a **measured** outcome, not an unmeasured one: every candidate was swept
on this project's build and none beat the defaults outside run-to-run spread.

Stockholm is NAFNet rather than Restormer, so Gothenburg's and Malmö's findings SHALL NOT be applied to it. Measured
with `perf` on an M2 Max (64 GB, macOS 26.6), ONNX Runtime 1.26, over the embedded 640x640 sample, as whole-pipeline
medians of 15 runs (nine 256-square tiles and the blend), one candidate per build:

| Candidate                      | CoreML FP32 | CoreML FP16 | CPU FP32 | CPU FP16 |
|--------------------------------|-------------|-------------|----------|----------|
| Default                        | 343.3ms     | 327.0ms     | 4888.3ms | 4889.4ms |
| Default, again after the sweep | 344.6ms     | 327.6ms     | 4849.0ms | 5116.6ms |
| CoreML CPUAndGPU               | 452.4ms     | 341.3ms     |          |          |
| CoreML CPUAndNeuralEngine      | 2704.4ms    | 444.4ms     |          |          |
| Sequential execution           | 354.6ms     | 337.7ms     | 4825.2ms | 4820.2ms |
| CoreML FastPrediction          | 417.2ms     | 330.7ms     |          |          |

The two default runs bracket run-to-run spread: under 0.5% on CoreML and up to 4.6% on the CPU provider. On CoreML every
candidate is slower than the default at both precisions. On the CPU provider sequential execution is 1.3-1.4% faster,
which is inside that spread, so it SHALL NOT be declared. CUDA and TensorRT were not measured.

#### Scenario: Stockholm attached to a provider

- **WHEN** Stockholm is configured for CoreML or the CPU provider at FP32 or at FP16
- **THEN** its options carry CoreML's default compute units, no specialization hint, and the provider's default
  execution mode

### Requirement: Moscow's measured settings

The Moscow sharpen model SHALL be configured with the setting measured for it, and SHALL declare it at **FP16 only**:

- **CoreML restricted to the CPU and GPU.** Moscow is a Restormer, the same backbone as Novgorod, Gothenburg and Malmö.
  Its 44 blocks are mostly layer normalization, reshape and transpose around a channel-attention matmul, which is not
  what the Neural Engine is built for, and CoreML takes the graph there whenever its default compute units permit. The
  reference measured the FP16 graph end to end over its 640x640 sample on an Apple Silicon Mac at 5.067s under the
  default against 2.803s under the restriction. Taking the GPU away instead (CPU and Neural Engine) measured +59% per
  tile, which confirms which half of the default costs.

**Moscow SHALL NOT declare the setting at FP32.** The reference declares it at FP16 only and records no FP32 figure, and
CoreML keeps an FP32 program off the Neural Engine in any case, so a setting at FP32 would restate a choice CoreML has
already made. CoreML's fast-prediction hint and a sequential execution mode measured
within run-to-run spread and SHALL NOT be set.

The size of the win depends on the export. The reference records it as −22% per tile on a single-partition build of
this graph, against −45% on the export behind the figures above: the same direction either way. These figures are the
reference implementation's, **ported without being re-measured on this project's build**. Anyone re-measuring Moscow
SHALL first establish how many CoreML partitions the published graph compiles to, or the figures cannot be compared.

#### Scenario: Moscow attached to CoreML at FP16

- **WHEN** Moscow is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run nodes in the provider's default execution mode

#### Scenario: Moscow attached to CoreML at FP32

- **WHEN** Moscow is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units and the provider's default execution mode

### Requirement: Novgorod's measured settings

The Novgorod sharpen model SHALL be configured with the setting measured for it, and SHALL declare it at **FP16 only**:

- **CoreML restricted to the CPU and GPU**, for the same architectural reason as Moscow. Measured per 256-square tile
  on an M2 Max, the restriction costs 143ms at FP16 against the default's 180ms. That difference decides which
  precision is faster: at 143ms FP16 is the fastest way to run this model, and at 180ms it would be slower than FP32's
  152ms, so a user picking FP16 for speed would get the opposite.

**Novgorod SHALL NOT declare the setting at FP32.** Novgorod's figures were measured separately from Moscow's and are
not evidence for one another: two checkpoints of one architecture reaching the same answer is a finding, not a licence
to copy one's settings to the next.

These figures are the reference implementation's, **ported without being re-measured on this project's build**.

#### Scenario: Novgorod attached to CoreML at FP16

- **WHEN** Novgorod is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run nodes in the provider's default execution mode

#### Scenario: Novgorod attached to CoreML at FP32

- **WHEN** Novgorod is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units and the provider's default execution mode

### Requirement: Petersburg's measured settings

The Petersburg sharpen model SHALL be configured with the provider defaults at **both FP32 and FP16**, and SHALL declare
no setting at either precision. This is a **measured** outcome, not an unmeasured one: the reference swept every
candidate and none beat the defaults outside run-to-run spread.

Petersburg is NAFNet, a plain convolutional U-Net, not a Restormer, so Moscow's and Novgorod's findings SHALL NOT be
applied to it. CoreML compiles it as one partition and places all of it on the GPU at both precisions without being
told to. Its default and the CPU-and-GPU restriction are therefore the same session. Measured per 256-square tile on an
M2 Max, macOS 26.6, ONNX Runtime 1.26:

| Compute units                     | FP32              | FP16             |
|-----------------------------------|-------------------|------------------|
| Default                           | 34.4ms            | 34.3ms           |
| CPU and GPU                       | 34.2ms (−0.5%)    | 34.4ms (+0.2%)   |
| CPU and Neural Engine             | 248.1ms (+622%)   | 37.0ms (+7.7%)   |
| CPU only / the CPU provider       | 471.9ms           | 468.6ms          |

CoreML's fast-prediction hint, a sequential execution mode and low-precision GPU accumulation measured within
run-to-run spread at both precisions and SHALL NOT be set. The FP16 export is no faster than FP32 on CoreML, because
both compile to the same GPU program. On this model FP16 buys download size and nothing else.

These figures are the reference implementation's, **ported without being re-measured on this project's build**. CUDA
and TensorRT were not measured.

#### Scenario: Petersburg attached to a provider

- **WHEN** Petersburg is configured for CoreML or the CPU provider at FP32 or at FP16
- **THEN** its options carry CoreML's default compute units, no specialization hint, and the provider's default
  execution mode

### Requirement: Mumbai's measured settings

The Mumbai colorization model SHALL be configured with the setting measured for it, and SHALL declare it at **FP16
only**:

- **CoreML restricted to the CPU and GPU.** Mumbai is DDColor: a ConvNeXt encoder that feeds a transformer colour
  decoder. Its operations are dominated by matrix products, transposes and normalizations, not by the dense
  convolution the Neural Engine is built for. On the published graph the restriction measured 34.9% faster at FP16
  (298.8ms to 194.5ms). On a graph re-exported to compile as a single CoreML partition, the default compute units make
  the FP16 result **wrong**, not merely slow: its a/b planes deviate from the FP32 result by 334 Lab units on planes
  whose whole range is about ±64. The restriction is the one setting that keeps the graph away from the compiler that
  does this.

**Mumbai SHALL NOT declare the setting at FP32.** A sequential execution mode, CoreML's fast-prediction hint and
low-precision GPU accumulation all measured within 0.7% and bit-identical, and SHALL NOT be set.

The published graph compiles to six CoreML partitions, because the provider declines five of its nodes. One of those
nodes crosses a partition boundary with a 268 MB feature map per run, which is why the FP16 graph is currently slower
than FP32 (246ms against 185ms). The reference records CoreML spending about five minutes compiling this graph on its
first run, whatever the partition count; a first run on this project's build took 8 s, which was not investigated. Neither is a provider setting, so neither is addressed here.

These figures are the reference implementation's, **ported without being re-measured on this project's build**. Anyone
re-measuring Mumbai SHALL first establish how many CoreML partitions the published graph compiles to, or the figures
cannot be compared.

#### Scenario: Mumbai attached to CoreML at FP16

- **WHEN** Mumbai is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run nodes in the provider's default execution mode

#### Scenario: Mumbai attached to CoreML at FP32

- **WHEN** Mumbai is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units and the provider's default execution mode

### Requirement: Delhi's measured settings

The Delhi colorization model SHALL be configured with CoreML restricted to the CPU and GPU at **FP16 only**, and with
the provider defaults at FP32.

**This is the reference's declaration, and the reference records no measurement of Delhi's own graph behind it.** It is
declared beside Mumbai's, and Delhi is the same DDColor architecture. This requirement states that provenance so the
setting is not mistaken for a measured one. Re-measuring Delhi is the way to confirm or revise it.

#### Scenario: Delhi attached to CoreML at FP16

- **WHEN** Delhi is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run nodes in the provider's default execution mode

#### Scenario: Delhi attached to CoreML at FP32

- **WHEN** Delhi is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units and the provider's default execution mode

### Requirement: Jaipur's measured settings

The Jaipur colorization model SHALL be configured with the setting measured for it, and SHALL declare it at **FP16
only**:

- **CoreML restricted to the CPU and GPU.** Measured on Jaipur's FP16 graph:

| Compute units            | FP16   |
|--------------------------|--------|
| Default                  | 955ms  |
| CPU and GPU              | 209ms  |
| CPU and Neural Engine    | 235ms  |

The CPU-and-Neural-Engine figure being four times **faster** than the default identifies the cost. The Neural Engine
is not what is slow. The default lets CoreML's planner split the graph across the GPU and the Neural Engine and pay a
transition at every crossing. Pinning either unit alone avoids that, and the default is the only setting that cannot.

Jaipur is DeOldify: a ResNet34 U-Net, as convolutional as a graph gets. So its measurements SHALL NOT be read as
following from Mumbai's and Delhi's, which are a different architecture. The two families of graphs reached the same
answer separately, and neither is evidence for the other.

**Jaipur SHALL NOT declare the setting at FP32.** A sequential execution mode and CoreML's fast-prediction hint
measured within run-to-run spread and bit-identical, and SHALL NOT be set.

These figures are the reference implementation's, **ported without being re-measured on this project's build**. They
are of the artistic generator's graph that Jaipur publishes today. Figures taken against the graph published before it
are not comparable.

#### Scenario: Jaipur attached to CoreML at FP16

- **WHEN** Jaipur is configured for CoreML at FP16
- **THEN** its options restrict CoreML to the CPU and GPU
- **AND** its session settings run nodes in the provider's default execution mode

#### Scenario: Jaipur attached to CoreML at FP32

- **WHEN** Jaipur is configured for CoreML at FP32
- **THEN** its options carry CoreML's default compute units and the provider's default execution mode

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

### Requirement: Choosing and configuring a provider needs no runtime and no model file

Resolving what a model runs on and the settings it runs with SHALL depend on nothing but the
machine's provider report, the model, and the cache directories supplied. It SHALL NOT require the
ONNX Runtime to be installed or loaded, SHALL NOT open or read a model file, and SHALL NOT create or
write to any directory.

#### Scenario: A machine with no runtime installed

- **WHEN** any provider is resolved and configured on a machine where no runtime has been installed
- **THEN** it succeeds
- **AND** nothing is read from or written to disk
