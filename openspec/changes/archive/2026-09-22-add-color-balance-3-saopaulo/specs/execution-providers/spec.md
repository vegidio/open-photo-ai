## ADDED Requirements

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
