## ADDED Requirements

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
