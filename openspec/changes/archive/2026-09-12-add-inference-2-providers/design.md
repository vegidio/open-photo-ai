## Slices

Running an image through a chain of models estimates far past 15 tasks. It is divided into six
changes, each leaving the project buildable and testable on its own. **This change is slice 2.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-inference-1-models` | A named model's files on disk, verified against the published manifest | the initialization series | **done** |
| 2 | `add-inference-2-providers` | The `ExecutionProvider` type including `Auto`, per-provider default options, and the per-model profile that overrides them | 1 | **current** |
| 3 | `add-inference-3-sessions` | Session construction, the in-memory session cache with its 30-minute idle TTL, and CPU fallback | 2 | pending |
| 4 | `add-inference-4-tiling` | The shared inference primitives: the tile grid, CHW tensor conversion, seam blending | none (parallel with 1-3) | pending |
| 5 | `add-inference-5-kyoto` | The upscale pipeline and the public chain entry point, with progress, cancellation and `OutputDepth` — Kyoto runs end to end | 3, 4 | pending |
| 6 | `add-inference-6-cache` | The 24-hour per-step image cache, keyed on the input identity plus the operations applied | 5 | pending |

Slice 4 depends on nothing in 1-3 and can be implemented alongside them. Everything else is a
straight chain.

Deferred past the series, each to its own change when something needs it: the model memory budget
with LRU eviction; alpha preservation through inference; the remaining seven families' pipelines; and
— added by this slice — the eleven measured execution-provider profiles other than Kyoto's, each
landing with the family pipeline that can verify it.

## Context

`providers.rs` today holds `SupportedProviders`, four booleans folded out of the GPU install plan, and
a crate-internal `Provider { Cuda, TensorRt }` that a pinned GPU-library row carries to say which
provider installing it unlocks. Its own doc comment names what is missing: *"`#[non_exhaustive]` keeps
a `supports(ExecutionProvider)` method additive when that arrives."* This slice is when it arrives.

`models/` names every model and resolves an operation to its artifacts. The precision is **inside**
the variant (`UpscaleVariant::Kyoto(FloatPrecision::Fp16)`), which is the one structural difference
from the reference implementation that matters here: its profiles are `func(types.Precision) EPProfile`
because a Go variant cannot carry its own precision.

`ort` is already a dependency at `=2.0.0-rc.13`. Its provider types take exactly the ONNX Runtime
option keys the reference implementation writes — `CoreML::with_model_format` sets `"ModelFormat"`,
`with_compute_units` sets `"MLComputeUnits"` — and every provider also implements
`ArbitrarilyConfigurableExecutionProvider::with_arbitrary_config`, so a key/value pair is applicable
whether or not `ort` has a typed setter for it. Nothing in this slice calls any of it.

See proposal.md - Why for motivation, and specs/execution-providers/spec.md for the behaviour.

## Goals / Non-Goals

**Goals:**

- One value answers "what will this run on, configured how", derived from the machine's own report.
- Every measured setting the reference implementation earned is carried over, with the measurement
  that justifies it, so none of it has to be rediscovered on hardware nobody here has.
- All of it testable on a CI runner with no GPU, no Mac and no ONNX Runtime.
- A model's tuning lives beside the model it was measured against.

**Non-Goals:**

- Calling ONNX Runtime. No session, no provider registration, no environment.
- Deciding where the provider caches live, or invalidating them. Slice 1 deferred that to the slice
  that first compiles one, which is slice 3.
- Thread-pool sizing (`intra_threads`, `inter_threads`). The reference implementation sets neither,
  and a setting nobody measured is not a setting worth porting.
- Per-model tuning for the eleven families this slice does not run.

## Decisions

### D1. One enum for the choice, kept apart from the "this library unlocks that provider" fact

`ExecutionProvider { Auto, Cpu, CoreMl, Cuda, TensorRt }` is public vocabulary, alongside `Precision`:
a front end offers it, stores it and hands it back. Parsing is case-insensitive with the user-facing
spellings (`Auto`, `CPU`, `CoreML`, `CUDA`, `TensorRT`) preserved on the way out, because they are
what a settings file and a command line already carry.

The existing `pub(crate) enum Provider { Cuda, TensorRt }` **stays**, rather than being replaced by
the new enum. It answers a different question — which provider a GPU library row unlocks — and its
narrowness is the point: a row that could name `Auto` or `Cpu` is a row that can be wrong, and the
existing `SupportedProviders::with(Provider)` folds it into the report precisely because one row sets
exactly one flag. Merging the two would buy one fewer type and lose that.

`supports(ExecutionProvider) -> bool` is added to `SupportedProviders`, answering `true` for `Auto`
and `Cpu` unconditionally.

### D2. Preference order, not a per-platform table

The reference implementation keys its chain on `runtime.GOOS`, with a `currentPlatform` variable
existing only so tests can pretend to be another platform. Here `Auto` is one order — TensorRT, CUDA,
CoreML — filtered by `SupportedProviders`.

That is not a simplification of the same data, it is the more accurate source. A platform table says
what *could* be published; the report says what is *installed on this machine*, which is what the
platform table was standing in for. CoreML is already false off macOS (the check is compiled out),
and CUDA and TensorRT are already false until their libraries are on disk. So filtering the order by
the report reproduces every platform's chain and additionally stops offering a provider whose library
failed to install — and the tests need no platform seam at all, because a `SupportedProviders` is four
booleans a test can write down.

CPU is deliberately absent from the list, as it is in the reference implementation: it takes no
configuration, and it is what the runtime falls back to once the providers above it decline a node.

### D3. An unhonourable request downgrades to the CPU and says so

The reference implementation answers this in two different ways and neither is right. A provider
absent from the platform's chain — CoreML on Linux — is an error that fails the run. One present but
uninstalled is attached anyway, declines natively, and is logged at Warn, which its own comment admits
is *"the single most common thing a user reports as 'the app got slow'"*.

Here both are the same fact and get one answer: the machine does not support it, so the resolution is
a CPU run, and the resolved plan carries the requested provider beside the resolved one. A stale
settings file costs speed, never the run — and because the downgrade is data rather than a log line,
slice 3 states it once and slice 5 could surface it in the UI without re-deriving anything.

What is *not* softened: parsing unknown text still fails (D1). An invalid name is a bug in whatever
produced it; a valid name this machine cannot serve is an ordinary Tuesday.

### D4. The output is a plan, and nothing here touches `ort`

`resolve(requested, &supported, &profile, &paths)` returns a `SessionPlan`:

```
SessionPlan {
    requested: ExecutionProvider,          // what was asked for
    resolved:  ExecutionProvider,          // Cpu where the request could not be honoured
    providers: Vec<ProviderOptions>,       // attach order; empty means a CPU run
    settings:  SessionSettings,            // parallel execution, memory pattern, disabled optimizers
}
ProviderOptions { provider: ExecutionProvider, options: BTreeMap<String, String> }
```

The options are ONNX Runtime's own key spelling (`trt_engine_cache_path`, `MLComputeUnits`), which is
what makes a row here comparable line-for-line with the provider's documentation, with the reference
implementation's measured table, and with an ORT log line. Slice 3 registers them — `ort`'s typed
setter where one exists, `with_arbitrary_config` otherwise, both of which write into the same map.

`BTreeMap` rather than `HashMap` so a test compares a whole map by equality and a failure prints in a
stable order.

The alternative — building `ort::ep::CoreML` values here — was rejected for what it costs the suite:
those types are only meaningful once registered against a session, so every assertion about an option
would need a runtime present, which is exactly the position the reference implementation designed its
way out of (*"built by a pure function so the mapping from profile to options can be tested without an
ONNX Runtime present"*). It also drags provider construction across the slice boundary into a change
that owns no session.

Session-level settings stay typed rather than key/value, because ONNX Runtime takes them through
dedicated calls (`SetSessionExecutionMode`, `SetMemPattern`) rather than the config-entry map. The one
exception is the disabled-optimizer list, which *is* a config entry; `ort` exposes it as
`with_disabled_optimizers`, so the plan carries the names and slice 3 joins them.

### D5. Cache directories are arguments, never policy

`CachePaths { engine: PathBuf, timing: PathBuf }` is an input. This slice writes both into the option
map and decides nothing about where they are, which directory owns which, or when either is emptied —
slice 1 already deferred the engine cache and its invalidation to the slice that first compiles one.

The struct exists rather than two `&Path` parameters for the reason the reference implementation
spells out: the two have different lifetimes — one is per-model, one is installation-wide — and a pair
of bare paths at every call site is how they end up swapped. Handing TensorRT the per-model directory
as its timing cache costs up to 60% of a cold engine build and reports nothing at all.

### D6. The profile carries the seven fields a measurement actually uses

`EpProfile` (a `Default`-able struct whose zero value is "nobody has measured this graph"):

| Field | What it changes |
|---|---|
| `coreml_compute_units` | Which Mac engines CoreML may dispatch to |
| `coreml_specialization` | Whether CoreML specializes for the shapes it was given |
| `execution_mode` | Whether independent branches run on separate threads |
| `disable_mem_pattern` | Whether the static memory planner is used |
| `disabled_optimizers` | Individual graph transformers switched off |
| `cuda_prefer_nhwc` | The CUDA provider's convolution layout |
| `trt_options` | Raw TensorRT overlay, applied last, for settings with no typed field |

These are the seven the reference implementation's twelve measured profiles actually set. The eight it
keeps permanently unset — `Fp16`, `TrtShapes`, `TrtWorkspaceBytes`, `CudaOptions`, `Extra`,
`GraphOptimization`, `DynamicShapes`, `ExcludeEPs` — are omitted, along with the chain-substitution
logic `ExcludeEPs` alone justifies. Their default behaviour is preserved exactly (TensorRT FP16 off,
a 4 GiB workspace, `RequireStaticInputShapes=1`, every optimizer on), so no model runs differently for
their absence, and a typed field returns the day a model needs one.

`trt_options` is kept despite no profile in *this* slice setting it, because it is the escape hatch
that stops the next measured setting from needing a type change — and because the two things it is
most likely to carry are already known: Osaka's lower builder optimization level, and a model that
wants hardware-compatible engines back.

**Two settings are pinned rather than offered**, and both are correctness rather than tuning.
CoreML's `ModelFormat` stays `MLProgram`: the older format has no typed execution, so an FP32 graph
may run on the Neural Engine in half precision, which measured as the single largest win in the
reference implementation's sweep and is a precision downgrade the user never asked for. Graph capture
stays off on both NVIDIA providers: on TensorRT every run after the capture returns zeros with no
error, on CUDA the capture run itself dies. Both carry their measurement into the doc comment, because
a future sweep will find them again and needs to know why they were declined.

### D7. A profile is declared by the variant, and the precision is already in it

Each family's `variant.rs` gains `fn profile(self) -> EpProfile`, and `Operation` forwards it as it
forwards the questions it already forwards. The tuning then sits in the file that names the graph it
was measured against — where anyone re-measuring it will look, and where the project's module layout
convention already puts variant-level facts.

The reference implementation's `func(types.Precision) EPProfile` indirection and its `Fp16Only` helper
both disappear: a Rust variant carries its own precision, so `UpscaleVariant::Kyoto(FloatPrecision::Fp16)`
is one match arm and every other Kyoto arm is `EpProfile::default()`. A precision-specific profile
cannot be written precision-blind here, which is what `Fp16Only` existed to enforce by hand.

`EpProfile` lives in `providers/` and `models/` imports it, rather than the reverse. It is provider
vocabulary — it names CoreML compute units — and `providers/` must not depend on `models/`, since the
option builders take a profile and two paths and nothing else. The dependency is on a plain data type
that owns no resource, which is the same relationship `models/` already has with `Precision`.

Osaka's one per-graph override (NHWC off for the diffusion transformer alone, while both VAE halves
keep it) is **not** built now. It arrives with Osaka's profile, and the shape it will take is already
available: `GraphSet` reaches each graph by `GraphRole`, so the override is a match on the role rather
than a new mechanism.

### D8. Only Kyoto's profile is declared here

The mechanism ships with the one declaration slice 5 exercises. The other eleven — saitama, tokyo,
osaka, moscow, novgorod, paris, lyon, rio, athens, santorini, newyork — land with the change that
first runs that family, so each arrives beside a pipeline that can verify it rather than as tuning
nothing in this project can execute. Every variant still answers `profile()`, so the hook is exercised
across all eight families from the start and adding a declaration later is an edit to one match arm.

Kyoto's is `CoreMLComputeUnits::CpuAndNeuralEngine` at FP16 and the default at FP32, and the reason it
is FP16-only travels with it: CoreML's typed execution bars an FP32 MLProgram from the Neural Engine
altogether, so naming it there drops the graph onto the CPU instead — a 9x regression at 2x and 20x at
4x on an M2 Max.

### D9. Module layout and errors

`providers.rs` becomes `providers/`: `mod.rs` for `ExecutionProvider`, `SupportedProviders` and the
existing detection, `profile.rs` for `EpProfile` and its enums, and `options.rs` for the per-provider
maps, the resolution and `SessionPlan`. Tests sit beside each, as the project's convention requires.

One error variant is added for an unparseable provider name, in the existing `thiserror` enum in
`error.rs`. Resolution itself cannot fail: every other outcome is a downgrade (D3), not an error.

### D10. Ownership and lifetime

Nothing here owns a native resource, and that is by construction rather than by luck — a plan is
owned data with no handle in it, and it is produced before the runtime is involved (D4). The session
ownership model the project's conventions call for is slice 3's to describe, because slice 3 is where
the first session exists. What this slice fixes for it is that the plan a session is built from can be
computed, compared and logged without holding anything that must be destroyed.

## Risks / Trade-offs

**An option key or value is only validated when a session is built, which this slice never does** →
The keys are transcribed from the reference implementation's measured table, which is running in
production, and the values are the same strings. The real exposure is a typo, and its failure mode is
nasty: ONNX Runtime rejects an option update wholesale, so one bad key costs the whole provider and
shows up as a slow run. Mitigated in slice 3 rather than here — it is the change that first sees a
provider attach, and it owns the log line naming which provider a session actually got.

**`Auto` filtered by the machine's report attaches fewer providers than the reference implementation
would** → Deliberate, and the difference is only ever a provider that would have declined natively.
The one behavioural edge: if `SupportedProviders` is ever wrong in the pessimistic direction — a
library present that the report does not know about — the provider is not attempted where the
reference implementation would have attempted it. That report is built from the install plan that put
the files there, so being wrong that way means the install bookkeeping is wrong, which is a louder
problem than this.

**Seven fields rather than fifteen means a future measurement may need a type change** → That is the
trade, taken knowingly: `trt_options` absorbs anything TensorRT, and adding a typed field to a
`pub(crate)` struct with one construction site is a small edit. The alternative is eight fields that
exist to be `Default` in every declaration for as long as the project lasts.

**A downgrade reported in data is only as visible as its consumer makes it** → The plan carries it;
nothing in this slice can log or surface it, because nothing here has a caller. Slice 3 is where that
becomes a log line, and slice 5 is where a front end could show it. Recorded here so it is not assumed
handled.

**Splitting `providers.rs` into a directory touches a module the initialization series owns** → The
existing type, its detection and its tests move unchanged; the suite that covers the GPU install plan
and its provider report is what proves the move was a move. No behaviour of `initialization` changes,
which is why this change declares no modified capability.

## Migration Plan

No migration. Nothing consumes an `ExecutionProvider` yet, no value is persisted in this slice, and
nothing on disk changes. Reverting the change leaves a `providers` module with the report it has
today.

## Validation

**Platforms:** every decision here is platform-independent, because the platform enters only through
`SupportedProviders`, which a test writes down as four booleans. The suite therefore exercises the
Mac, NVIDIA and no-accelerator resolutions on whichever runner CI provides, on all three of ubuntu,
macos and windows.

**Execution providers:** none are attached, built or run in this slice — the first time any of these
option maps reaches ONNX Runtime is slice 3, and the first time one is measured end to end is slice 5.
The values themselves were measured by the reference implementation on an RTX 5090 (driver 610.88,
ONNX Runtime 1.26, TensorRT 10) and on an M2 Max; they are carried over with their measurements and
are **unverified on this project's own stack** until slice 5. Nothing here has been validated against
an Intel Mac, an AMD GPU, or any NVIDIA card older than Ampere.

**What is untested until slice 3:** that ONNX Runtime accepts these keys through `ort`, and that a
provider configured this way attaches rather than declining.

## Open Questions

- **Whether the resolved plan should also carry a human-readable reason for a downgrade**, rather than
  just the requested/resolved pair. Slice 3 is the first caller with something to say, and it can
  compose the sentence from the pair; a reason string would only be worth adding if a second caller
  wanted a different one. Answerable then without changing the specs, the approach or the tasks.
