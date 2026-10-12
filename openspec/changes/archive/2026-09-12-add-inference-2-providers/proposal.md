## Why

Slice 1 put a verified `.onnx` on disk. Nothing in this repository can yet say what a session built
from it should be asked to run on: `providers.rs` reports what the *machine* supports — four booleans
folded out of the GPU install plan — and there is no type a caller can name a provider with, no order
`Auto` means anything in, and no settings a session would be built with.

That is the whole of what stands between an installed model and a session. Slice 3 builds sessions;
it cannot start until "which provider, configured how" is a value it can be handed. This slice makes
it one, and does it without opening a graph: everything here is decided before the runtime is
touched, which is what lets the whole of it be tested on a CI runner with no GPU and no ONNX Runtime
present.

## What Changes

Task estimate: **13**. The inference feature estimates far past 15 and is split into six slices (see
design.md - Slices); this proposal covers slice 2 only.

**In `opai`:**

- **An `ExecutionProvider` type** — `Auto`, `CPU`, `CoreML`, `CUDA`, `TensorRT` — public, because it
  is a value a front end offers in a settings pane and hands back, exactly as `Precision` is. Parsed
  case-insensitively from an untrusted string and refused when unknown, so a stale settings file is
  rejected at the boundary rather than reaching a session build as a silent no-op.
- **`SupportedProviders` answers for a named provider.** The report initialization already produces
  gains the `supports(ExecutionProvider)` method its own doc comment anticipated.
- **What `Auto` resolves to**: the machine's supported providers in a fixed preference order —
  TensorRT, then CUDA, then CoreML — with the CPU implicit beneath them. There is no per-platform
  table: CoreML is false off macOS and the two NVIDIA providers are false until their libraries are
  installed, so the platform falls out of the report the machine already made.
- **What an unsupported request resolves to**: the CPU, reported rather than refused. A provider the
  user picked on another machine, or before a driver was removed, downgrades the run instead of
  failing it — and the plan carries what was asked for beside what was resolved, so slice 3 can say
  so in one line rather than leaving a 10x slowdown unexplained.
- **Per-provider default options**, measured by the reference implementation and carried over whole:
  TensorRT's engine and timing caches, its hardware-compatible engines off, its builder optimization
  level; CUDA's convolution algorithm search and NCHW default; CoreML's MLProgram format and static
  input shapes. CUDA graph capture stays off on both providers, and the reasoning for that — it
  returns silent all-zero output on one and fails the first run on the other — is carried across with
  it, because it is the single most expensive thing to rediscover here.
- **A per-model profile that overrides those defaults**, declared by the variant it was measured
  against and resolved per precision. The precision is already inside the Rust variant, so what is a
  `func(Precision) EPProfile` in the reference implementation is a plain method here and the
  `Fp16Only` helper it needs disappears into a match arm.
- **The session-level settings a profile carries**: execution mode, the memory-pattern planner, and
  individually disabled graph optimizers.
- **Kyoto's measured profile**, and no other. The mechanism lands here with the one declaration slice
  5 will exercise; the other eleven measured profiles land with the changes that first run those
  families, so each arrives beside a pipeline that can verify it rather than as tuning nothing runs.

**What this slice does not do:** it never calls ONNX Runtime. It produces the plan a session is built
from — the providers to attach in order, each with its resolved options, plus the session settings —
and slice 3 applies it to an `ort::SessionBuilder`. That seam is the reference implementation's own
(*"each provider's settings are built by a pure function so the mapping from profile to options can
be tested without an ONNX Runtime present"*), and it is what makes every requirement here testable in
CI.

**Not exposed to a front end in this slice.** The type is public because it is vocabulary, but
nothing takes an `ExecutionProvider` as a parameter until slice 5 gives the chain a public entry
point, so there is no Tauri command, no i18n key and no component here.

## Capabilities

### New Capabilities

- `execution-providers`: deciding what a model is asked to run on and the settings a session is built
  with — naming the providers, what `Auto` means on a given machine, what happens to a request the
  machine cannot honour, the defaults each provider is configured with, and the per-model tuning that
  overrides them.

  Distinct from `initialization`, which owns *installing* the GPU libraries and reporting which
  providers the machine can offer. That report is this capability's input; what is done with it —
  choosing one, configuring it, refusing to configure one that was never installed — is new here and
  belongs to neither `initialization` nor `models`.

### Modified Capabilities

None. `initialization` still reports exactly what it reported before; asking that report about a
named provider is a new question over an unchanged answer, and the requirement covering it lives with
the type that names the provider.

## Impact

**Crates touched:** `opai` only. `gui` and `cli` are untouched, and no logic is placed in a front end
— a provider choice is a setting both front ends will offer and neither can act on, which is the test
for library code.

**`rust-sak` - what was found before anything was proposed.** Nothing, and nothing was needed. Its
seven modules cover hashing, HTTP, filesystem work, image codecs, memoization, telemetry and hardware
probes; this slice does no I/O of any kind — it resolves a value and builds two maps of strings. The
one adjacent thing it does touch, the config directory the provider caches live under, is
`fs::mk_user_config_dir` and already in use, and the directories themselves are slice 3's. No
`rust-sak` change is proposed, and none is expected.

**New third-party crates:** none. `ort` is already a dependency (pinned `=2.0.0-rc.13`, `load-dynamic`,
`api-26`) and this slice adds no call into it — its option keys are the vocabulary the resolved maps
are written in, and `ort` accepts exactly those keys through both its typed setters and
`with_arbitrary_config`, which is what slice 3 will use.

**Parity divergences from the Go implementation, deliberate:**

- **OpenVINO is not named.** The reference implementation publishes it in the enum and in every
  platform's chain, and its appender returns *"the OpenVINO provider is disabled in this build"* — so
  a machine resolving to it runs on CPU kernels and logs a decline, every time, on every platform. It
  is a provider that exists only to fail. This project names the four that work plus `Auto`, and adds
  a fifth when one is actually built.
- **An unsupported provider downgrades to the CPU instead of erroring or silently declining.** The
  reference implementation splits the case in two: a provider absent from the platform's chain is an
  error that fails the run, while one present but uninstalled is attached, declines, and is logged.
  Here both are one answer — the machine does not support it, so the run is CPU and the plan says so.
  CoreML requested on Linux stops being a failed run; CUDA requested without the libraries stops
  being a warning buried under a native decline.
- **Provider exclusion is not carried over.** `EPProfile.ExcludeEPs` and the chain substitution built
  for it have had no setter since Osaka's TensorRT exclusion was measured as a 2.5x win, so the
  mechanism serves nothing. It returns, with tests, when a model needs it.
- **The profile carries seven fields rather than fifteen.** The eight the reference implementation
  keeps unset — `Fp16`, `TrtShapes`, `TrtWorkspaceBytes`, `CudaOptions`, `Extra`, `GraphOptimization`,
  `DynamicShapes`, `ExcludeEPs` — are omitted; the raw TensorRT overlay stays, so per-model tuning
  that has no typed field still has somewhere to go. The defaults those fields would have produced
  are unchanged, so no model behaves differently for their absence.

**Not in scope:** session construction, the session cache and CPU fallback after a failed build
(slice 3); the engine and timing cache directories, their layout and their invalidation when weights
are replaced (slice 3, as slice 1 already deferred — this slice takes the two paths as arguments and
decides nothing about where they are); tiling and tensor conversion (slice 4); the public entry point
and anything a front end can reach (slice 5); the image cache (slice 6). Also out: the eleven
measured profiles other than Kyoto's, the model memory budget, and any `gui` or `cli` change.
