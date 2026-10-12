## ADDED Requirements

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
