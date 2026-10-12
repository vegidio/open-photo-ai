## ADDED Requirements

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
