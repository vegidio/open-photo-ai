## ADDED Requirements

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
