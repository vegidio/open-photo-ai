## Context

See proposal.md — Why. The identity layer for this family shipped with `add-models-2-enhancements` and
`refactor-models-family-layout`; what is missing is the pipeline.

Three facts about the surrounding code shape everything below.

**Both halves of the contract already exist, and each was written with this family in mind.** `run_tiled` is the
driver Tokyo, Kyoto and Saitama run through, at the same 256/16 geometry and `Normalisation::Unit` the reference's
denoise uses (`ImageToCHWInto(..., standardize: false)`). `inference::mix::blended` is `BlendWithIntensity`'s formula,
and its header already names denoise and sharpen as its next callers. It has no clamp on the factor, so a strength of
3 extrapolates as the reference's does.

**The guard was deferred here explicitly.** `inference/mod.rs` lists the reference's per-tile divergence guard under
"What is deliberately not here": every upscale run has it off, so in the driver it would be untested code guarding a
model that cannot be loaded, and it "belongs with the denoise pipeline".

**The reference's denoise package is thin, and its two Restormer model files are almost all prose.** Their measured
CoreML tables and the account of the re-export behind them are the hard-to-reconstruct part of the port. Carrying them
across is a deliverable, as it was for Paris and Lyon.

## Goals / Non-Goals

**Goals:**

- One contract in `models/denoise/process.rs` that all three variants run through unchanged.
- The guard testable on its own, and the whole pipeline testable against a fake backend on every CI platform — no ONNX
  Runtime, no GPU, no model file, no network.
- `run_tiled` left exactly as it is.

**Non-Goals:**

- **Sharing the contract with sharpen.** The reference's `sharpen/process.go` is identical to denoise's line for line,
  and Moscow and Novgorod are the same Restormer backbone as Gothenburg and Malmö. The tier rule says promotion happens
  in the change that introduces the second caller, before that caller is written. That is sharpen's change, and this one
  keeps the contract inside the denoise family.
- **Re-measuring Gothenburg or Malmö.** Decided when this change was scoped; see D6. Stockholm, which nobody has
  measured, is measured here; see D8.
- **Reusing the strength-independent intermediate across runs.** The model's output does not depend on the strength and
  only the blend does, but the strength is in the identity and the run-cache tag, so every slider position re-runs the
  grid. The reasons `add-light-adjustment-paris-lyon` D9 gave for not building this apply unchanged. The seam is the run
  cache's, worth four families, and does not belong in one family's pipeline file.
- **Any front-end work.** Neither `gui` nor `cli` gains a line.

## Slices

Not sliced. Thirteen tasks, `opai` only, three variants sharing one contract exactly. The precedents are
`add-upscale-tokyo-saitama` and `add-light-adjustment-paris-lyon`.

## Decisions

### D1. The contract is a family-tier file; the guard and the profile are model-tier

```
  models/denoise/
      mod.rs            family  identity (exists) + pipeline<B>() seam
      variant.rs        family  rows and enum (exist) + guard() + profile() matching into the model dirs
      process.rs        family  NEW: the pipeline: OnOneGraph + ImagePipeline, the guarded tile step
      stockholm/mod.rs  model   NEW: GUARD = 3.0, profile() as measured (D8), the NAFNet prose, tests
      gothenburg/mod.rs model   NEW: profile(), the measured table and re-export prose, its tests
      malmo/mod.rs      model   NEW: profile(), its own table and prose, its tests
```

This follows the tier rule directly. `process.rs` has three callers in one family, so it is family tier. A threshold
and a measured profile each have one caller, so they are model tier. Deleting Malmö means deleting its directory plus
two match arms.

**The threshold does not go on `SimpleVariant`.** That row type is shared across five families, and a threshold there
would be a field every other row has to supply and nobody reads. `variant.rs` gains `guard() -> Option<f32>`, an
exhaustive match returning `stockholm::GUARD` for Stockholm and `None` for the other two, the same way `profile()`
reaches the model directories.

**Rejected:** a zero threshold meaning "off", as the reference's `DivergenceThreshold float32` does. `Option` says the
same thing without a sentinel a later reader has to know about.

### D2. The guard lives in the per-tile step, and `run_tiled` does not change

At scale 1 the tensor a tile is presented as and the tensor the model returns for it have the same shape. So "keep
this tile's input pixels" is, exactly:

```
run_tile(input, output):
    B::run_tile(session, input, output)?
    if let Some(threshold) = guard
        and output.iter().any(|v| v.abs() > threshold):
            output.copy_from_slice(input)
```

The driver then decodes and blends that output like any other, so a kept tile meets its neighbours through the same
ramp as every other tile. The comparison is strict, as the reference's is: exactly 3.0 is not a blow-up.

This is the reference's behaviour at a better depth. The reference returns the padded `image.Image` tile, which is
8-bit `RGBA`, so on a 16-bit run a guarded tile is quantised inside a 16-bit result. Here `to_unit` and `from_unit` are
exact inverses at both depths, so the copied tensor decodes to the input's own pixels at whatever depth the run
resolved to. The reference also has an `imaging.Resize` branch for a guarded tile at scale other than 1. That branch is
incidental: denoise never reaches it, and nothing else enables the guard. It is not ported.

**Rejected: a `guard: Option<f32>` parameter on `run_tiled`,** as the reference's `WithDivergenceGuard` option is. Every
upscale caller would pass `None` forever. The parameter would only make sense at scale 1, while the driver takes any
scale, so it would need either a refusal or a resample nobody uses. It would also bring back into `inference/` the code
that module's header says belongs here. `run_tiled` already takes the per-tile step as a parameter precisely so a
caller can do this.

The guard is written as a small function over the two slices, `fn diverged(output: &[f32], threshold: f32) -> bool`,
so it can be unit-tested with no pipeline around it.

### D3. Progress: the tiles take most of the share, the blend reports the end

`run_tiled` reports its own `0.0..=1.0` and lands on exactly 1 at the last tile, and its caller emits the initial
`0.0`. The blend after it is a full-resolution loop, which on a 24-megapixel photograph is not free. So the pipeline:

1. reports `0.0`;
2. hands `run_tiled` a progress closure that maps its fraction onto `0.0..=TILES`, a named constant below 1;
3. checks for cancellation, blends, and reports exactly `1.0`.

`TILES` is a fixed share, not a figure fitted to measurements. `add-light-adjustment-paris-lyon` D6 gives the reason: a
split derived from one image size on one provider is a constant nobody can re-derive from the code. The spec requires
only that the end is reported after the blend, and that progress never goes backwards.

Cancellation is checked by the driver between tiles and once more by the pipeline before the blend. A cancelled run
returns no image at all.

### D4. The body is `run_tiled` then `blended`, with no dispatch of its own

`run_tiled` already dispatches on `ChannelDepth` and returns a `DynamicImage` at that depth. `blended` takes an
`ImageBuffer<Rgb<T>, _>`, so the pipeline dispatches once, the way `Adjust::run` does, into a body generic over
`T: Channel` that recovers the typed buffer from the driver's result and blends it against a `Sampler` over the input.
The driver's result is not a borrowed view, so `blended` consumes it in place and no third full-resolution buffer is
allocated.

**Rejected: skipping the blend at strength 1**, even though 1 is the default. At 1, `blended` computes `o + 1·(p − o)`
in `f32`, which lands within an ulp of `p`, and `from_unit`'s rounding takes it back to `p`'s own channel value. So
the loop already returns the model's output exactly, and a fast path would be purely a speed question. The answer is
not worth a second path: one full-resolution loop is small next to the hundreds of model runs the tile grid costs on
the same photograph. The spec's "model's output unchanged" at 1 is tested as exact equality.

### D5. Session ownership and lifetime

This is unchanged from every other single-graph family, and is stated because the design rules require it, not because
anything here is new. The pipeline embeds a `SingleGraph`, so `Model::required` is a one-element slice and
`Model::sessions` is one `(&ArtifactId, &EpProfile)` pair. No profile is ever in scope without the artifact it was
measured for.

The chain driver **acquires the handle before the pipeline runs, and the pipeline borrows it for the duration of the
call**. The per-tile closure borrows `&sessions[0]` for the length of the `run_tiled` call and no longer. Nothing here
opens, retains or drops a session. So "a session a run holds cannot be reclaimed" remains a property of the driver's
reference counts, not a rule this file has to follow.

One session serves every strength, because the strength is not part of the artifact name. An existing test in
`denoise/mod.rs` pins this.

The pipeline itself is plain data: one artifact id, one profile, one optional threshold, one strength and a display
name. That is why it satisfies `Send + Sync` for free. Peak scratch is `run_tiled`'s: two tile tensors, one decoded tile
and the result, with the blend working in place on the result.

### D6. Gothenburg's and Malmö's profiles are `match precision` in each model file, ported as the reference recorded them

The reference expresses them as `utils.Fp16Only(EPProfile{CoreMLComputeUnits: CPUAndGPU})`. Here the precision travels
inside `DenoiseVariant`, so each is a two-arm match in the model's own file, as `saitama::profile`, `paris::profile` and
`lyon::profile` already are. No wrapper is added.

**They are not re-measured.** This was decided when the change was scoped, and differs from Paris and Lyon, which were
re-confirmed on an M2 Max before landing. What makes the difference acceptable:

- Both profiles are CoreML-only and FP16-only. A wrong one gives a slower Mac at one precision, never a wrong image.
- Neither declares anything at FP32. This is the precision-split rule, and it keeps a measurement from being applied
  too widely.
- Each model's prose records where the figures came from, and repeats the single-partition precondition that anyone
  re-measuring has to check first.

The four reference tests on Gothenburg's profile are ported as unit tests in `gothenburg/mod.rs`: FP16 carries
`CPUAndGPU`, FP32 carries the default, no sequential mode at either precision, and nothing else declared. Malmö gets
the same four. The reference only pinned Gothenburg, but the two profiles are the same claim, and an unpinned copy is
the one a sweep of another model would overwrite.

### D7. What the guard's verification can and cannot claim

The guard's fake-backend tests prove the mechanism:
- A backend that returns 4.0 on one tile and a copy of the input on the rest produces a result whose guarded region is
  the input's pixels.
- The other regions are the model's output.
- The run completes.
- A value of exactly 3.0 does not trip the guard.
- Gothenburg's and Malmö's pipelines, given the same exploding backend, pass its output through.

The tests cannot prove that Stockholm actually blows up, or that 3.0 is where to catch it. No photograph that triggers
Stockholm was available when this change was written. So the threshold is the reference's, carried across on trust, and
`stockholm/mod.rs`'s prose and the `denoise` spec both say this. The live check runs Stockholm over `fixtures/test.dat`
and reports **how many tiles were guarded**, through a test-only count. A photograph that does trigger it can later be
added as a fixture without changing the contract.

### D8. Stockholm is measured here, and the default has to win to stay

Stockholm has never been measured, in the reference or here. D6's argument for porting instead of measuring (the
figures exist and were taken carefully) does not apply to it, and "unmeasured" is a worse state to record than either
answer. So this change measures it, and its profile is whatever the measurement supports. That includes the default,
if nothing beats it, in which case the default becomes a **measured** outcome pinned by tests, not an unmeasured one
waiting to be overwritten.

Method, as Paris and Lyon were measured:

The graph is measured as published. Counting its CoreML partitions or re-exporting it is out of scope: the question
is only whether the defaults are the best option for this graph, or whether a profile beats them.

1. **One candidate per build.** `perf` has no flags for provider settings, so each candidate is a temporary
   `stockholm::profile()` and a rebuild, measured with `perf -- stockholm` at FP32 and FP16 on the provider it concerns.
   The candidates are the default; CoreML at `CPUAndGPU` and at `CPUAndNeuralEngine`; sequential execution on CoreML
   and on the CPU provider; and the fast-prediction hint.
2. **Adopt per precision.** A setting is declared only at a precision where it measured outside run-to-run spread,
   which is the precision-split rule every other profile here follows. A setting that is free at one precision is not
   carried to the other.

The candidate list deliberately does **not** start from Gothenburg's answer. `CPUAndGPU` is on it because it is a
standard candidate for any CoreML graph, not because a Restormer won with it.

Until the measurement lands, `stockholm::profile()` returns the default at both precisions, which is the one state that
needs no evidence. The `execution-providers` requirement is written as the rule the outcome must follow and is rewritten
with the figures by the task that takes them.

## Crates and dependencies

**None added, and none weighed.**

**`rust-sak`:** nothing is needed from it. The tile grid, the tensor conversion, the blend and the session seam all
live in `inference/`. The guard is a scan over an `f32` slice. None of `rust-sak`'s modules (`crypto`, `fetch`, `fs`,
`image`, `memo`, `sysinfo`) is relevant to any of this. **The gap is nil, so there is nothing to decide between
extending `rust-sak` and writing locally.**

**Division across crates:** all logic is in `opai`. `perf` changes only a test example (see Risks). `gui` and `cli` are
untouched.

## Platforms and providers validated

**Validated by the fake-backend suite on every CI platform** — macOS, Windows and Linux — with no ONNX Runtime, no GPU,
no model file and no network. It covers the tile run at scale 1, the guard, the blend, the progress schedule, the depth
dispatch, cancellation and the refusal.

**The live check runs by hand on macOS arm64 (M2 Max), on the CPU provider and on CoreML**, at both precisions, for all
three variants. It is `#[ignore]`d like every other live check in this crate.

**Gothenburg's and Malmö's profiles are not validated on this project's build** (D6). Their figures are the
reference's, taken on an M2 Max, macOS 26.6, ONNX Runtime 1.26. **Stockholm's is measured on this project's build**, on
the M2 Max, against the CPU provider and CoreML (D8). **CUDA and TensorRT are untested**, and nothing here declares a
setting for either.

## Risks / Trade-offs

- **The guard is unverified against the model it exists for.** → D7: the mechanism is proven, the threshold is carried
  over on trust, the prose and the spec say so, and the live check reports whether it fired.
- **The ported profiles could be wrong on this build.** → They are FP16-only, CoreML-only and cost only speed, and each
  model file records the precondition a re-measurement has to check.
- **Every other failure mode is a worse photograph rather than an error.** A wrong normalisation, a guard that
  compares against the decoded pixels instead of the raw output, or a blend applied before the tile run all produce a
  plausible image and no diagnostic. → The guard's arithmetic is tested alone. The composition is tested against a fake
  backend, including a guard-trip test that could only pass if the check reads the raw output. `Normalisation::Unit` is
  pinned as a literal against the reference's `standardize: false`.
- **`perf`'s tests use Stockholm as their example of a declined model.** The outcomes are fabricated, so they would
  keep passing, but they would then describe something that is no longer true. → They move to Colorization, which
  stays refused after this change and is not about to be implemented.
- **`Opai::process`'s "What is refused" is already stale**: it lists three running families, but colour balance runs
  too. → It is corrected to five running families, not just edited to add denoise.
