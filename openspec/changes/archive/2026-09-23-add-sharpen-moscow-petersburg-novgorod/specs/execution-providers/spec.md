## ADDED Requirements

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
