## ADDED Requirements

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
