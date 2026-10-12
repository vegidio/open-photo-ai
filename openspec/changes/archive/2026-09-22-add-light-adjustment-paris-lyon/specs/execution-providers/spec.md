## ADDED Requirements

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
