## MODIFIED Requirements

### Requirement: Provider caches are pointed at the directories supplied for that model

What a provider compiles from a model - a TensorRT engine, a CoreML MLProgram - SHALL be cached in a
directory belonging to that model and that provider alone, and TensorRT's timing cache SHALL be
pointed at a separate directory shared by every model. Every one of these directories SHALL be
supplied to the configuration rather than decided by it.

The two must not be conflated. The compiled result is per-model because replacing a model's weights
has to invalidate it, and per-provider so that one provider's output is never mistaken for or mixed
with another's. The timing cache is shared because kernel timings carry between graphs: the reference
implementation measures a cold build against a cache other models seeded at up to 60% faster than
against an empty one, and pointing TensorRT at a per-model directory for it silently gives that back
with nothing reporting the loss.

#### Scenario: Two models configured for TensorRT

- **WHEN** two different models are configured with their own engine directories and one shared
  timing directory
- **THEN** each carries its own engine cache path
- **AND** both carry the same timing cache path

#### Scenario: One model configured for TensorRT and CoreML

- **WHEN** one model is configured for TensorRT and for CoreML
- **THEN** TensorRT's engine cache path and CoreML's model cache path are different directories
- **AND** both are inside that model's own cache directory
