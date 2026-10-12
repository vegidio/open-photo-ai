## ADDED Requirements

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
