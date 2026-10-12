## Purpose

Deciding what a model is asked to run on and the settings a session is built with: naming the
execution providers a user can choose between, working out what `Auto` means on the machine in front
of it, deciding what happens to a request that machine cannot honour, and carrying the measured
settings each provider and each model needs - because the difference between the right provider
options and the defaults is a factor of ten on a run, and the difference between the right ones and
the wrong ones is sometimes a silently incorrect image.

## ADDED Requirements

### Requirement: The execution providers a run can be asked for

The application SHALL name exactly five choices a run can be asked for: `Auto`, `CPU`, `CoreML`,
`CUDA` and `TensorRT`. Their spelling is user-facing - it is what a settings file stores and what a
command line accepts - and SHALL be stable.

A choice built from text the application did not produce SHALL be parsed rather than assumed: the
match SHALL be case-insensitive, and text naming none of the five SHALL be refused with an error
naming what was given. A choice SHALL survive a round trip through the application's own
serialization unchanged.

The reference implementation additionally names OpenVINO, whose only behaviour is to decline to
attach on every platform. It is deliberately not named here.

#### Scenario: A provider named in a settings file

- **WHEN** the text `coreml`, `CoreML` or `COREML` is parsed
- **THEN** all three yield the CoreML choice

#### Scenario: Text naming no published provider

- **WHEN** the text `openvino` or `metal` is parsed
- **THEN** the parse fails with an error naming the text that was given
- **AND** no choice is produced

#### Scenario: A stored choice is read back

- **WHEN** a choice is serialized and read back
- **THEN** it is the same choice
- **AND** the stored text is the user-facing spelling of that provider

### Requirement: The machine's provider report answers for a named provider

The report of which providers the machine supports SHALL answer, for any one of the five choices,
whether this machine supports it. `Auto` and `CPU` SHALL always be answered as supported, because
there is no machine on which the application cannot run on the CPU.

#### Scenario: A provider whose library was never installed

- **WHEN** the report of a machine with no NVIDIA libraries installed is asked about CUDA
- **THEN** it answers that the machine does not support it

#### Scenario: Auto on any machine

- **WHEN** any machine's report is asked about `Auto` or `CPU`
- **THEN** it answers that the machine supports it

### Requirement: Auto runs on the best provider this machine supports

`Auto` SHALL resolve to the providers the machine supports, in a fixed preference order: TensorRT,
then CUDA, then CoreML. Every supported provider from that order SHALL be attached, in that order, so
that one declining at session-build time leaves the next to run the graph. The CPU SHALL NOT appear in
that list: it needs no configuration and is what the runtime falls back to once every provider above
it has declined a node.

The order is a property of the providers, not of the operating system: a machine reports CoreML
unsupported off macOS and reports the NVIDIA providers unsupported until their libraries are
installed, so no per-platform list is consulted.

#### Scenario: A machine with the NVIDIA libraries installed

- **WHEN** `Auto` is resolved on a machine reporting CUDA and TensorRT supported
- **THEN** TensorRT is attached first and CUDA second

#### Scenario: A Mac

- **WHEN** `Auto` is resolved on a machine reporting only CoreML supported
- **THEN** CoreML is the only provider attached

#### Scenario: A machine with no accelerator

- **WHEN** `Auto` is resolved on a machine reporting no provider beyond the CPU
- **THEN** no provider is attached
- **AND** the run is a CPU run

### Requirement: A named provider the machine supports is the only one used

A request for a specific supported provider SHALL attach that provider and no other, so that a
deliberate choice is not quietly widened into the `Auto` chain. A request for the CPU SHALL attach no
provider at all and SHALL carry no provider configuration.

#### Scenario: CUDA requested on a machine that supports CUDA and TensorRT

- **WHEN** CUDA is requested
- **THEN** CUDA is the only provider attached
- **AND** TensorRT is not attached

#### Scenario: The CPU requested

- **WHEN** the CPU is requested
- **THEN** no provider is attached
- **AND** no provider options are produced

### Requirement: A request the machine cannot honour becomes a CPU run and is reported

A request for a provider the machine does not support SHALL NOT fail the run and SHALL NOT be
silently honoured. It SHALL resolve to a CPU run, and what was requested SHALL be reported alongside
what was resolved, so that the downgrade can be stated once rather than surfacing later as an
unexplained slow run.

This unifies two outcomes the reference implementation keeps apart: there, a provider its platform
does not list fails the run outright, while one listed but uninstalled is attached, declines
natively, and is logged. Both are the same fact - the machine cannot run this - and both are answered
the same way here.

#### Scenario: CoreML requested on a machine that is not a Mac

- **WHEN** CoreML is requested on a machine reporting CoreML unsupported
- **THEN** no provider is attached
- **AND** the result reports that CoreML was requested and a CPU run was resolved

#### Scenario: A provider whose libraries are absent

- **WHEN** TensorRT is requested on a machine reporting TensorRT unsupported
- **THEN** the run proceeds on the CPU rather than failing
- **AND** the result reports the requested provider beside the resolved one

### Requirement: Each provider is configured with the settings it was measured with

Every provider attached SHALL carry the settings measured for it, not the runtime's defaults:

- **TensorRT** SHALL run at builder optimization level 5, with its engine cache and its timing cache
  enabled, with hardware-compatible engines off, with INT8 off, and with a 4 GiB workspace ceiling.
- **CUDA** SHALL search convolution algorithms exhaustively, SHALL be allowed the maximum convolution
  workspace, SHALL have no GPU memory ceiling imposed, and SHALL use the runtime's NCHW layout unless
  the model asks otherwise.
- **CoreML** SHALL use the MLProgram model format, SHALL require static input shapes, SHALL NOT be
  enabled on control-flow subgraphs, and SHALL dispatch to every compute unit unless the model asks
  otherwise.

The CoreML model format is fixed rather than offered as a per-model setting: the older format has no
typed execution, so an FP32 graph placed on the Neural Engine runs in half precision - measured on the
reference implementation as a 39% speed-up landing on the accuracy of that model's FP16 export. A
user who asked for FP32 must not be given FP16 by a provider setting.

#### Scenario: TensorRT attached

- **WHEN** TensorRT is attached for any model
- **THEN** its options carry the engine cache enabled, the timing cache enabled, hardware-compatible
  engines off, INT8 off, builder optimization level 5 and a 4 GiB workspace

#### Scenario: CoreML attached

- **WHEN** CoreML is attached for any model
- **THEN** its options select the MLProgram format and require static input shapes

### Requirement: Graph capture is disabled on both NVIDIA providers

The application SHALL leave CUDA graph capture disabled on the CUDA provider and on TensorRT, and
SHALL NOT offer it as a per-model setting.

It is measured and rejected rather than unexamined. On TensorRT the capture run is correct and every
subsequent run on that session returns an all-zero output with no error and plausible timings - which
in the application is no faces detected from the second detection onwards, or a blank recovered face.
On the CUDA provider the capture run fails outright because the BLAS library initializes lazily inside
the captured stream. Anyone revisiting it must compare the **second** run on the **same** session
against a CPU result; a single-run comparison reports a speed-up and ships a broken model.

#### Scenario: Any NVIDIA provider attached

- **WHEN** CUDA or TensorRT is attached for any model
- **THEN** its options carry graph capture disabled

### Requirement: Provider caches are pointed at the directories supplied for that model

What a provider compiles from a model - a TensorRT engine, a CoreML MLProgram - SHALL be cached in a
directory belonging to that model alone, and TensorRT's timing cache SHALL be pointed at a separate
directory shared by every model. Both directories SHALL be supplied to the configuration rather than
decided by it.

The two must not be conflated. The compiled result is per-model because replacing a model's weights
has to invalidate it. The timing cache is shared because kernel timings carry between graphs: the
reference implementation measures a cold build against a cache other models seeded at up to 60%
faster than against an empty one, and pointing TensorRT at a per-model directory for it silently gives
that back with nothing reporting the loss.

#### Scenario: Two models configured for TensorRT

- **WHEN** two different models are configured with their own engine directories and one shared
  timing directory
- **THEN** each carries its own engine cache path
- **AND** both carry the same timing cache path

### Requirement: A model's measured settings override the provider defaults

A model MAY declare the settings measured for it, and those SHALL override the corresponding
defaults for that model alone. A declaration SHALL be specific to the precision the model is run at,
because tuning does not survive a precision change - the same graph exported at FP16 and at FP32
wants opposite answers from the CUDA layout preference, and applying the FP16 answer to both is a
measured regression rather than a neutral choice.

A model that declares nothing SHALL run on the provider defaults unchanged.

#### Scenario: Kyoto at FP16

- **WHEN** the Kyoto upscaler at FP16 is configured for CoreML
- **THEN** its options keep it off the GPU and on the CPU and Neural Engine

#### Scenario: Kyoto at FP32

- **WHEN** the Kyoto upscaler at FP32 is configured for CoreML
- **THEN** its options carry the default compute units

#### Scenario: A model with nothing measured

- **WHEN** a model that declares no settings is configured for any provider
- **THEN** its options are the provider defaults, with nothing added or removed

### Requirement: A model's measured settings include how the graph itself is run

A model's declaration SHALL be able to change settings that belong to the session rather than to one
provider: whether independent branches of the graph may run on separate threads, whether the memory
planner is used, and which individual graph optimizers are switched off. These SHALL apply on every
provider the model runs on.

Unless a model says otherwise, independent branches MAY run on separate threads, the memory planner
SHALL be used, and every optimizer SHALL be left on - which is what every model in the reference
implementation runs with before it is measured.

#### Scenario: A model that has not been measured

- **WHEN** a model declaring nothing is configured
- **THEN** the session settings allow parallel execution, keep the memory planner on and disable no
  optimizer

#### Scenario: A model measured as sequential

- **WHEN** a model declaring sequential execution is configured for any provider
- **THEN** the session settings run one node at a time
- **AND** the same setting applies whichever provider was resolved

### Requirement: Choosing and configuring a provider needs no runtime and no model file

Resolving what a model runs on and the settings it runs with SHALL depend on nothing but the
machine's provider report, the model, and the cache directories supplied. It SHALL NOT require the
ONNX Runtime to be installed or loaded, SHALL NOT open or read a model file, and SHALL NOT create or
write to any directory.

#### Scenario: A machine with no runtime installed

- **WHEN** any provider is resolved and configured on a machine where no runtime has been installed
- **THEN** it succeeds
- **AND** nothing is read from or written to disk
