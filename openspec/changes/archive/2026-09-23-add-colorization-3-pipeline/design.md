## Context

See proposal.md for why. The pieces this slice wires together already exist and are proven. What shapes the design is
where they sit and what the surrounding pipelines already do.

**Slices 1 and 2 left no signature to invent.** `models/colorization/` holds:

- `lab`: the Lab round trip;
- `stretch::stretched`;
- `ab::{SIDE, CHANNELS, FILTER, input, planes}`: the Ab contract;
- `jaipur::rgb::{SIDE, CHANNELS, FILTER, input, chroma}`: the Rgb contract;
- `compose::composed::<T: Encoded>(sampler, (a, b), side, (width, height))`.

Each module is declared under `#[cfg_attr(not(test), expect(dead_code, ...))]` naming this change. `ab::planes` borrows
two slices of the output, and `rgb::chroma` returns two owned planes. Slice 2 D5 decided that nothing reconciles the
two, and that each arm passes what it holds.

**Light adjustment is the pattern for a fixed-square, whole-image pipeline.** `light_adjustment/process.rs` has:

- a `SingleGraph` that holds the artifact, profile and name;
- `OnOneGraph` for the `Model` half, and `ImagePipeline::stages() == 1`;
- `run` dispatching the depth once over `adjusted::<T>`;
- a zero-area refusal as `InferenceError::Untileable`;
- `reporter` and `checkpoint` over counted `STEPS`;
- a test module with its own `Fake` backend.

Colorization follows it everywhere except the arithmetic.

**Sharpen is the pattern for per-model data on the variant.** `SharpenVariant::guard()` is an exhaustive `const` match
that reaches each model's directory. `LightAdjustmentVariant::profile()` and `canvas()` are the same shape.

**`FamilyNotImplemented` is produced in exactly one place**, `Operation::pipeline`'s `Colorization` arm. It is also
named in:

- one public-API pin;
- the `served` predicate and two traversal tests in `models/operation.rs`;
- the `unservable()` fixture and the four tests using it in `inference/process.rs`;
- one `perf` test;
- the prose of `lib.rs`, `operation.rs` and `autopilot.rs`.

## Goals / Non-Goals

**Goals:**

- A colorization run that satisfies the `colorization` spec, with each spec scenario either a fake-backend test or,
  where only real weights can show it, a property of the live check.
- No colorization arithmetic written in this slice. `process.rs` only orders calls into slices 1 and 2.
- `FamilyNotImplemented` gone, with nothing left behind that refers to it.

**Non-Goals:**

- **Re-measuring any profile.** Ported, as slice 1 decided. The live check prints timings for the record but does not
  tune.
- **The Mumbai re-export the reference describes** (single partition, −45%/−61%). That is a new published artifact,
  not a code change.
- **Correcting the `image-enhancement` spec's stale "result is not an image" scenario.** Detection has been carried
  in `Analysis` since the carrier split, so that refusal is now a type error rather than a runtime answer. This slice
  replaces the requirement (REMOVED plus ADDED, because a MODIFIED block cannot drop scenarios) only as far as removing
  the deferral forces it, and keeps that bullet's wording. Bringing
  it in line with the carrier split is its own change (see Open Questions).
- **GUI or CLI work.** The CLI runs no operations yet. The GUI's wire carries face recovery and upscale only, and
  each family is added to it by its own change (`gui/src/enhance/operation.rs`: "a family becomes runnable by gaining
  an arm here"). Colorization's GUI arm, and its options panel, belong in that series beside light adjustment's,
  colour balance's, denoise's and sharpen's, none of which is on the wire yet.

## Decisions

### D1. The three profiles live in each model's directory, reached by a match

```
  models/colorization/
      delhi/mod.rs      new    profile(Precision): CpuAndGpu at FP16, default at FP32
                               prose: the reference's declaration, beside Mumbai's; no measurement of Delhi's own
      mumbai/mod.rs     new    profile(Precision): the same settings
                               prose: mumbai.go's, all of it (the 34.9%, the 334-unit FP16 breakage, the six
                               partitions and the 268 MB crossing, the five-minute cold compile, the re-export note)
      jaipur/mod.rs     grows  profile(Precision): the same settings
                               prose: jaipur.go's (the 955/209/235 table, the planner-transition reading, the
                               artistic-generator export note)
      variant.rs               profile(): match { Delhi => delhi::profile, Mumbai => mumbai::profile,
                                                  Jaipur => jaipur::profile }
```

**Three files with the same settings, not one shared constant.** `light_adjustment/variant.rs` states the reason
already: carrying one model's answer to another because both do the same job is how a profile ends up hurting a graph
it was never measured against. Jaipur's prose says it reached the answer on its own graph. Delhi's says it did not
reach it at all and only borrowed Mumbai's. A shared constant would erase exactly that difference.

**Each file has two tests, as Paris's does:** FP16 is exactly `CpuAndGpu` over the default, and FP32 is the default.
Both are asked through the variant, so the match is what is tested.

**Delhi's prose is written, not left empty.** Slice 2 found that `delhi.go` has none. A blank file would read as
"unmeasured, so the default", and the setting is not the default. The prose says where the setting came from.

**Rejected: `EpProfile::default()` for Delhi until it is measured.** The reference ships the restriction for Delhi,
and parity is the target. Dropping it would be a behavioural divergence based on the absence of a comment.

### D2. `Graph::{Ab, Rgb}` in `process.rs`, answered by `ColorizationVariant::graph()`

```rust
// process.rs (family tier)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Graph { Ab, Rgb }

impl Graph {
    const fn side(self) -> u32;                             // ab::SIDE | rgb::SIDE
    const fn channels(self) -> usize;                       // ab::CHANNELS | rgb::CHANNELS
    fn input(self, source: &DynamicImage) -> Vec<f32>;      // ab::input | rgb::input
    fn composed<T: Encoded>(self, source: &Sampler<'_>, output: &[f32], extent: (u32, u32))
        -> ImageBuffer<Rgb<T>, Vec<T>>;                    // planes|chroma, then compose::composed
}

// variant.rs
pub(crate) const fn graph(self) -> Graph {
    match self { Self::Delhi(_) | Self::Mumbai(_) => Graph::Ab, Self::Jaipur(_) => Graph::Rgb }
}
```

**Each method is one match, and `composed` is where the ownership difference is absorbed.** Its `Ab` arm borrows
`ab::planes(output)`. Its `Rgb` arm binds `rgb::chroma(output)` and passes references to it. Both call
`compose::composed` with their own `SIDE`. The pipeline body never sees a contract-specific type, which is slice 2's
"neither arm reaches `process.rs`'s shared steps with anything contract-specific except those".

**Why `composed` takes the whole output rather than `chroma` returning planes:** a common return type for "two planes"
would be either an owned pair, which copies 2 MB out of the Ab output every run, or a `Cow`, which slice 2 D5 rejected.
Folding the chroma read and the compose into one method needs neither.

**In `process.rs`, not `variant.rs`.** `Graph` is pipeline vocabulary, and its methods call into `ab`, `jaipur::rgb`
and `compose`, which are all `process.rs`'s neighbours. `variant.rs` only names which arm a variant is, as sharpen's
`guard()` names a value.

**Rejected: a trait with two unit-struct implementors.** It is the same dispatch spread over three types, and it
would need generic dispatch in `Colorize` or a `dyn`. It gains nothing over a two-arm `Copy` enum that the compiler
checks exhaustively.

### D3. The pipeline

```
  Colorize { graph: SingleGraph, contract: Graph }            (stages() == 1)

  run(input, sessions, depth, progress, cancelled)
    match depth { Eight => colorized::<u8>, Sixteen => colorized::<u16> } .map(into_dynamic)

  colorized::<T>
    (w, h) == 0 ?                     -> Err(Untileable)      before anything is allocated
    cancelled()?                      -> Err(Cancelled)
    tensor = contract.input(input)                            stretch + gray/luma at the square
    checkpoint 1/3
    output = vec![0; channels * side²]
    B::run_graph(&sessions[0], &tensor, (3, side, side), &mut output, (channels, side, side))
    checkpoint 2/3
    contract.composed::<T>(&Sampler::new(input), &output, (w, h))
    report 1.0
```

**Three steps, counted.** The reference reports only 0 and 1. The input build is two resamples of the photograph plus
one pass at the square. The graph run is fixed. The compose is proportional to the photograph: slice 2 measured 86 ms
at 12 megapixels, which is small against a graph of 100 to 900 ms, but not zero. Three counted steps put a report on
each side of the graph run, which is what the spec's "advances after the model has run" needs. Weighting the steps by
measured cost is rejected, for light adjustment's reason: it is a constant correct for one photograph size and one
provider.

**Cancellation is checked where the reference checks its context**: before the stretch, after the input, and after the
graph. As in light adjustment, nothing is checked after the compose, because the image is finished by then.

**The zero-area refusal comes first.** `stretched` would hand `resize_exact` an empty source, and `composed` panics on
a zero-length axis (slice 2 D6 documented this and deferred the check to here). The error is `Untileable`, the same one
light adjustment and every tiled family return.

**`Sampler::new(input)` is built once, after the graph run.** It is the owned `Rgba16` conversion for layouts
`Sampler` cannot borrow. Built earlier, it would hold a photograph-sized buffer across the graph run for nothing.

**`colorization::pipeline::<B>()` builds it from `display_name`, `artifact`, `profile` and `variant.graph()`,** and
`Operation::pipeline`'s arm becomes `Ok(colorization.pipeline::<B>())`, beside light adjustment's, with the same
comment form: all three variants run through one pipeline, so there is nothing for the seam to refuse.

### D4. What the fake-backend suite pins

A `Fake` backend in `process.rs`'s tests, as light adjustment's is. Its session records the input tensor and shapes it
was run with, and writes an output chosen by the test. Every test runs through `ImagePipeline::run`, not through the
helpers, so the wiring is what is tested.

| Spec scenario | Test |
| --- | --- |
| run once at the square, large and small | each variant: one `run_graph` call, input `(3, side, side)`, output `(channels, side, side)`, for a photograph larger than the square and one smaller. Delhi/Mumbai are 512/2 and Jaipur is 560/3 |
| each model shown its own gray rendering | the recorded input equals `ab::input(photo)` for Delhi and Mumbai, and `rgb::input(photo)` for Jaipur, bit for bit. The two differ on a colour photograph, so a swapped contract fails |
| no colour predicted → the photograph's gray | an Ab fake writes zeros, and an Rgb fake echoes its gray input back as RGB. Each result pixel is within one level of the photograph's gray, and its channels within one level of each other, at both depths |
| a colour predicted → warm, lightness kept | an Ab fake writes a constant `a = 20, b = 20` over a mid-gray photograph. R exceeds B at every pixel, and `lab::lightness` of the result is within the rounding of the depth of the photograph's |
| a colour photograph is recoloured | a saturated blue photograph with an Ab fake writing a warm constant comes back warm |
| far from square | a 64×8 photograph, with an Ab fake whose left half of the square is warm and right half cool. The result's left half is warm and its right half cool. This catches a fit instead of a stretch, and a transposed axis |
| large photograph, own size and detail | a photograph several times the square comes back at its own size, and a one-pixel checkerboard in its lightness survives |
| 16 bits | a 16-bit gradient at `ChannelDepth::Sixteen` comes back `Rgb16` with more than 256 distinct levels |
| transparency | an `Rgba8` source comes back `Rgb8`, and its result equals the result of the source flattened against black |
| progress | the reports are 1/3, 2/3, 1, never decreasing |
| cancelled | a cancellation at each of the three boundaries gives `Err(Cancelled)` and no image. One observed before the stretch never runs the graph |
| no area | 0×N and N×0 give `Err(Untileable)`, and the graph is never run |

The "each variant over one photograph gives three different results" scenario can only be shown with real weights (the
fake is deterministic per contract), so it belongs to the live check (D6). The chained-run scenario needs no image:
`operation.rs` plans a colorization of every variant on both sides of an upscale, and both operations are planned
(D5).

### D5. Deleting `FamilyNotImplemented`, and what each test that named it becomes

| Where | Now | After |
| --- | --- | --- |
| `error.rs` | the variant and its `#[error]` | deleted. `UnsupportedReason`'s doc drops "this is coming later" and describes the one case left |
| `operation.rs` `pipeline` | the refusal arm and its doc | the hand-over (D3), and the doc says every family runs and names `IncompleteGraphSet` as the refusal left |
| `operation.rs` traversal test and `served()` | served or refused for the right reason | **"every operation the carrier can hold reaches a pipeline"**, which is the spec's "every published enhancement is served". `served()` is deleted |
| `operation.rs` plan test | a colorization plan is refused | the refusal half is deleted, and the "never refused as a category error" half stays |
| `inference/process.rs` | `unservable()` and four tests | deleted (decided 2026-09-23). The rule they tested stays in the spec. It remains true structurally, because `process::run` calls `plan` over the whole chain before it installs anything, but **no test exercises it end to end any more**: no refusable operation is reachable from a test |
| `tests/public_api.rs` | the deferral pin and the `Unsupported` rendering through it | the deferral pin is deleted. The rendering is pinned through `IncompleteGraphSet { missing: "decoder" }`, so the `no pipeline for {operation}: {reason}` format a front end shows is still pinned |
| `perf/src/sweep.rs` | a `FamilyNotImplemented` refusal classified as declined | the same test over `IncompleteGraphSet` |
| `perf/src/{main,json,report,viewport}.rs` | fixtures showing Delhi declined | the fixtures name Osaka at FP16 (Osaka publishes no FP32 build), declined with the `IncompleteGraphSet` rendering. It is the one model whose seam can still refuse, so the example stays one the library could produce |
| `lib.rs` `Opai::process` | "Colorization, the one family still refused…" | lists colorization with the other six, and says that no published enhancement is refused |
| `autopilot.rs` | the doc naming `FamilyNotImplemented` | says "a family the enhancement path refuses". The test is unchanged, as slice 1 decided |

**`IncompleteGraphSet` is kept.** It is the diffusion seam's guard against a variant that is declared without a graph,
and it is not a temporary gap.

### D6. The live check

`models/colorization/live.rs`, `#[cfg(test)]`, `#[ignore]`d, in the form of `sharpen/live.rs`. There are two tests,
`every_variant_colorizes_a_photograph_on_the_cpu` and `…_on_coreml`, and each runs all three variants at both
precisions. Weights install into `live_support::kept_dir("colorization")`, as Osaka's do: they total 4.0 GB, and a
temporary directory would download them again on every run.

The photograph is the committed fixture **reduced to gray first**, so that any colour in the result came from the
model. Each run asserts:

- the result has the photograph's dimensions;
- **the lightness is the photograph's**: the mean of `|lightness(result) − lightness(source)|` is under 1 L unit, which
  catches a model output enlarged in place of the compose;
- **colour arrived**: the mean chroma (`sqrt(a² + b²)` through `lab::rgb_to_lab`) is above 2, where the gray input is
  zero, which catches an all-zero or unread output;
- at each precision, **no two variants' results are identical**, which catches two variants handed one graph.

It prints cold and warm times per run for the record. It is not a measurement, and the profiles are not re-derived from
it. On CoreML, Mumbai's first run can include the roughly five-minute compile the reference recorded. The check's doc
says so, so that a long first run is not mistaken for a hang. The one run made here (M2 Max, 2026-09-23) took 8 s;
why it did not compile for five minutes was not investigated.

### Session ownership, providers and crates

**One session per run, owned by the session cache.** `Colorize` holds a `SingleGraph`, which is an artifact id, a
profile and a name, and owns nothing native. `inference::process` acquires the session handle for the run, passes it
as `sessions[0]`, and releases it to the cache's lifetime rules afterwards, exactly as for light adjustment. The input
and output tensors are allocated per run: at most 3.7 MB each, for Jaipur's 560 square. The compose's scoped threads
(`inference::rows`) borrow the sampler, the output and the axis tables, and have all joined before `colorized`
returns. Nothing outlives the call.

**The memory budget needs nothing new.** Session residency is accounted generically, and no per-model declaration
exists to add. These are the largest graphs after Osaka's, at 0.91 GB for FP32 Delhi and Mumbai, and the existing
eviction is what bounds them.

**Providers.** Each variant's `EpProfile` is D1's. `CpuAndGpu` is a CoreML option, and on the CPU, CUDA and TensorRT
providers the profile is the default. Automatic CPU fallback is unchanged.

**`rust-sak`:** no feature used, and none missing. **No third-party crate added.**

**Division across crates:**

- `opai`: all of the logic.
- `perf`: a test and fixtures only (D5).
- `gui` and `cli`: untouched (see Non-Goals). No logic is placed in a front end.

### Platforms validated

- **The fake-backend suite and every unit test** run on every CI platform (macOS, Windows, Linux) with no runtime,
  model file, GPU or network.
- **The live check** runs by hand on macOS arm64 (M2 Max), on the CPU provider and on CoreML. Its results go into the
  task's completion note.
- **Windows and Linux CPU** are not run live. They are the same code path as the macOS CPU run, and CI covers their
  arithmetic.
- **CUDA and TensorRT** are untested: no NVIDIA machine is in reach. The profiles declare nothing for either.

## Slices

Colorization, in order. **This is slice 3, the last.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-colorization-1-lab` | `models/colorization/lab.rs`: the sRGB transfer, OpenCV-convention CIELab both ways, the `u16`-keyed linearization table with `lightness` and `gray`, the 8-bit threshold encode and the 16-bit direct encode, and the reference's colour tests plus the 16-bit ones. No behaviour change | – | **done** |
| 2 | `add-colorization-2-compose` | `inference/bilinear.rs` (promoted from São Paulo) and `inference/rows.rs` (promoted from Osaka's colour fix); the alpha-flattening stretch; the Ab contract's input and planes; Jaipur's Rgb contract's input and chroma; the fused compose at the run's depth with its separate-steps oracle; the resampler and compose-cost measurements. No behaviour change | 1 | **done** |
| 3 | `add-colorization-3-pipeline` | `Graph::{Ab, Rgb}` and `ColorizationVariant::graph()`; `process.rs` and `Colorization::pipeline`; `delhi/`, `mumbai/` and `jaipur/`'s profiles with ported prose, and `ColorizationVariant::profile()`; the `Operation::pipeline` refusal arm removed; **`UnsupportedReason::FamilyNotImplemented` deleted**, with the refused-family tests, prose, public-API pin and `perf` fixtures; the fake-backend suite and the live check. Specs: `colorization` (new), `execution-providers` (modified), `image-enhancement` (modified) | 1, 2 | **current** (est. 12) |

Every decision slice 2's "Decisions slice 3 inherits" lists is carried out here as recorded: no tiling, stretched and
not fitted, the two contracts as data, the run's depth kept and alpha not carried, all three profiles ported with
Delhi's provenance stated, `FamilyNotImplemented` deleted with the four `inference/process.rs` tests, the
`image-enhancement` delta, autopilot unchanged apart from its doc, the progress schedule chosen (D3), and every
`expect(dead_code)` removed.

## Risks / Trade-offs

- **[A contract is swapped, or read at the wrong channel count, and a plausible photograph comes back]** → D4 asserts
  each variant's exact shapes and that its recorded input is its own contract's bit for bit. A swap fails both.
- **[The compose is handed the wrong square]** → The far-from-square test puts different colours in the two halves of
  the square. Composed at the other contract's side, the halves would misalign. The live check's lightness bound
  catches the grosser case.
- **[Delhi's profile is a borrowed setting and slows its graph]** → Stated in the spec and in Delhi's file, not hidden.
  CoreML at FP16 is the only configuration it touches, and re-measuring is the stated remedy.
- **[Mumbai's first CoreML run takes about five minutes, and a user reads it as a hang]** → This is the reference's
  record (not reproduced here: this build's one first run took 8 s), and it is recorded in the spec and in Mumbai's file. A progress change for a cold compile would reach
  every family and is not this slice's.
- **[A default `perf` sweep now downloads 4 GB more]** → This follows from the benchmark's standing rule of running
  whatever can run. A caller who does not want colorization names the models.
- **[The refusal-before-install rule loses its end-to-end tests]** → Accepted with the user on 2026-09-23. The rule
  stays in the spec, and `plan` still runs over the whole chain before any install. Nothing tests that ordering end
  to end any more, and a test-only refusal seam was declined. If a later version publishes a model before its
  pipeline, the tests come back with the refusal that makes them writable.
- **[Removing a public enum variant breaks a downstream `match`]** → The enum is `#[non_exhaustive]`, so every
  downstream match already has a catch-all, and no arm that compiles today names anything else that was removed. The
  only consumers are this workspace's crates, all updated here.

## Open Questions

- **The `image-enhancement` "result is not an image" scenario describes a runtime refusal that the carrier split
  turned into a type error.** This slice keeps its wording and removes only its reference to the deleted deferral. A
  follow-up change can restate it as a guarantee of the carrier types. That would not change this slice's code or
  tasks.
