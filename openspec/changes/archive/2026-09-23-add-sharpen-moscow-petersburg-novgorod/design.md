## Context

See proposal.md — Why. The identity layer for this family shipped with `add-models-2-enhancements` and
`refactor-models-family-layout`. What is missing is the pipeline, and the pipeline already exists in the wrong tier.

Four facts about the surrounding code shape everything below.

**The contract is written, tested, and tied to denoise only by its inputs.** `models/denoise/process.rs` holds the
guard check (`diverged`), the progress share (`TILES`), a test-only guard counter (`GUARDED`), and the pipeline
(`Denoiser`). The pipeline runs `run_tiled` at scale 1 and then `mix::blended`. What ties it to denoise is its inputs:
it takes `DenoiseParams`, whose only field is a `Strength`, and it reads the denoise range constant `RANGE`. Nothing in
its body is about denoising.

**The archive already decided that this change promotes it.** The archived denoise design lists "Sharing the contract
with sharpen" as a non-goal *because* promotion belongs to the change that introduces the second caller.
`process.rs`'s own header says the same. `config.yaml` forbids the alternatives: a `use` from `models/sharpen/` into
`models/denoise/` is "a promotion that has not happened yet", and a copy is ruled out by name.

**The canvas promotion is the precedent for how to move it.** `add-color-balance-1-canvas` moved `present` and `blended`
out of light adjustment into `inference/`. It made the moved function take the normalisation from its caller rather than
hard-coding one family's range (its D2), and it moved a test fixture to `inference/test_support.rs` once three callers
needed it (its D5). Both apply here directly.

**The reference measured all three sharpen graphs.** Unlike denoise, where Stockholm had never been measured, nothing
here is unmeasured. Moscow's and Novgorod's profiles and Petersburg's measured default are all in `sharpen/*/*.go` with
their prose.

## Goals / Non-Goals

**Goals:**

- One contract in `inference/filter.rs` that all six variants of both families run through unchanged.
- Denoise renders byte for byte what it renders today, proven by its existing assertions with none of them edited.
- The whole pipeline testable against a fake backend on every CI platform: no ONNX Runtime, no GPU, no model file, no
  network.
- `run_tiled` left exactly as it is.

**Non-Goals:**

- **Re-measuring any sharpen profile.** Decided when this change was scoped; see D5.
- **Changing anything denoise does.** No delta to the `denoise` spec. If a denoise assertion has to change, the move was
  not a move and the task stops there.
- **Reusing the strength-independent intermediate across runs.** The model's output does not depend on the strength, so
  a slider could in principle re-blend without re-running the grid. The archived denoise design declines this for the
  reasons `add-light-adjustment-paris-lyon` D9 gave, and they apply unchanged: that seam belongs to the run cache and
  would serve four families, not to one pipeline file.
- **Any front-end work.** Neither `gui` nor `cli` gains a line.

## Slices

Not sliced. Fourteen tasks, `opai` only. The promotion takes five and fits inside the budget alongside the family that
needs it, which is where `config.yaml` puts a promotion when it fits. The canvas promotion was cut into its own slice
only because folding it in would have made 21 tasks.

## Decisions

### D1. `inference/filter.rs` holds the contract; each family keeps its range, guard and profiles

```
  BEFORE                                          AFTER
  models/denoise/process.rs  (family)             inference/filter.rs  (every family)
    RANGE = Unit                                    diverged(output, threshold)
    diverged(), TILES, GUARDED                      TILES, GUARDED (test-only)
    Denoiser + OnOneGraph + ImagePipeline           Filter + OnOneGraph + ImagePipeline
    pipeline(name, artifact, profile,               pipeline(name, artifact, profile,
             DenoiseParams, guard)                           norm, strength, guard)

  models/denoise/mod.rs                            models/denoise/mod.rs
    Denoise::pipeline --> process::pipeline          RANGE = Unit    <-- moved here, pinned by its test
                                                     Denoise::pipeline --> filter::pipeline(.., RANGE, ..)

                                                   models/sharpen/mod.rs
                                                     RANGE = Unit    <-- sharpen's own statement
                                                     Sharpen::pipeline --> filter::pipeline(.., RANGE, ..)
```

**`inference/`, not `models/`.** The contract is execution code: it drives `run_tiled`, reads raw tile tensors, and
calls `mix::blended`. `config.yaml` puts execution primitives that no single model owns in `inference/`, and the canvas
promotion made the same call for the same reason.

**Named `filter`** because denoise and sharpen are both image filters: same resolution in, same resolution out, over a
tile grid. The name does not clash with `blend` (combining overlapping tiles), `mix` (the control blend), `canvas` (the
tile accumulator) or `present` (the fixed-square fit). The struct becomes `Filter`, replacing `Denoiser`.

**The range is the caller's.** This is canvas D2 again. Both families read `[0, 1]` because the reference's two
`process.go` files each pass `standardize: false`, and each family states that for itself. A shared file that hard-coded
one would be right for two families by coincidence. Each family keeps its own `RANGE` constant and its own test pinning
it as a literal. `filter.rs` gains a test that it honours whichever range it is handed.

**The strength arrives as a `Strength`, not a `*Params`.** `DenoiseParams` and `SharpenParams` are each family's public
vocabulary, and the shared file must name neither. A bare `f32` was rejected because it would let an unbounded value
reach the blend, while `Strength` is bounded at construction and is already cross-family vocabulary in `models/`.
`Filter` resolves it with `as_f32()` once per run, as `Denoiser` does today. Each family seam still builds from what its
`params()` publishes, which keeps the archived rule that a pipeline cannot be built from something the library did not
say a run needs.

**`models/denoise/process.rs` is deleted, not reduced.** Once the pipeline leaves, all that remains is `RANGE` and its
one test. A file holding one constant is a file nobody would create. The constant goes to `denoise/mod.rs` beside the
seam that uses it, and sharpen's goes in the same place in `sharpen/mod.rs`, so the two families look alike.

**Visibility.** Everything moved is `pub(crate)` or narrower. `diverged` stays unexported beyond the crate, nothing is
re-exported from `lib.rs`, and `tests/public_api.rs` is untouched: the mechanical check that the move is invisible from
outside.

**Rejected: leaving the contract in `models/denoise/` and importing it from sharpen.** `config.yaml` names this exact
move a bug.

**Rejected: a `models/filter.rs` at the family-agnostic `models/` level.** `models/` holds vocabulary, and this is
execution.

### D2. The tests move by what they assert, and the fake backend becomes shared

`models/denoise/process.rs`'s tests split like this:

| Test group | Destination |
| --- | --- |
| the range literal test | `denoise/mod.rs`, beside `RANGE`; sharpen gets its own in `sharpen/mod.rs` |
| the four `diverged` tests | `filter.rs` |
| the fake-backend suite (size, strength 0/1/past 1, guard trip at 8 and 16 bits, threshold, pass-through, progress, cancellation, zero area, depth, one stage) | `filter.rs` |
| `Fake`, `FakeSession`, `session()` | `inference/test_support.rs` |
| new: the range is honoured (D1) | `filter.rs` |

**The suite builds its pipeline directly** with `filter::pipeline`, `Normalisation::Unit`, a strength, and either
`Some(3.0)` or `None` as the guard. It no longer goes through `Denoise::new(..).pipeline()`. A shared module's tests
should not reach up into one family to construct what they test.

**The proof that it was only a move** is the canvas slice's, restated: **no assertion changes.** The construction line
of each test changes, and nothing after it. A test that needed a different expected value means behaviour moved, and
the task stops.

**The fake backend moves to `inference/test_support.rs`** because it gains three callers. The shared suite uses it, and
each family keeps two seam tests that need it (below). Leaving it in `filter.rs`'s test module would mean either copies
or a `use` reaching into another module's test scope. That is the promotion rule applied to a fixture, as canvas D5 did
for `photograph`.

**What stays with each family is what only the family knows**, run through its own seam against the shared fake:

- **The guarded variant trips.** Stockholm, or Petersburg, given a backend exploding on one tile, keeps the input there.
  This test fails if the family seam stops handing `guard()` to the pipeline.
- **The unguarded variants pass through.** Gothenburg and Malmö, or Moscow and Novgorod, given the same backend, keep
  the exploding tile's output.

The pipeline suite proves the mechanism once. These two prove that each family wires it to the right models.

**`GUARDED` moves with the guard.** It is test-only, and both live checks read it to report how many tiles were kept. A
counter shared across both families means the two live checks must not run together. They already run by hand,
`#[ignore]`d, one module at a time, and each resets the counter before its own runs.

### D3. The sharpen family follows denoise's shape exactly

```
  models/sharpen/
      mod.rs             family  identity (exists) + RANGE + pipeline<B>() seam
      variant.rs         family  rows and enum (exist) + guard() + profile() matching into the model dirs
      live.rs            family  NEW: #[ignore]d live check, #[cfg(test)]
      moscow/mod.rs      model   NEW: profile(), the Restormer prose, the export caveat, four tests
      petersburg/mod.rs  model   NEW: GUARD = 3.0, profile() = default (measured), the NAFNet sweep prose, tests
      novgorod/mod.rs    model   NEW: profile(), the precision-decides-speed prose, four tests
```

- **`guard()` and `profile()`** are exhaustive matches reaching into the model directories, as `DenoiseVariant`'s are.
  The threshold is not a field on `SimpleVariant`, for the reason the archived denoise D1 gives.
- **`Sharpen::pipeline` returns `Shared<B>` with no `Result`.** All three variants run through one contract, so there is
  no state left to refuse. `Operation::pipeline` wraps it in `Ok`, as it does denoise's.
- **Codename `petersburg`, label `Petersburg`, constructor `Sharpen::stpetersburg`.** All three already ship and are
  pinned: the label in `catalogue.rs`, the constructor in `tests/public_api.rs`. None of them changes.

### D4. The families that stand in for "refused" become colorization alone

Several tests and doc comments use sharpen as their example of a family with no pipeline. With sharpen served, each moves
to colorization, the last family refused:

- `models/operation.rs`: `served()`, the deferred-set test (`{"colorization"}`), and the plan-refusal example.
- `inference/process.rs`: `unservable()` and the served list in `an_operation_with_no_pipeline_is_refused_by_name`.
- `autopilot.rs`: the doc comment's "add a `Sharpen` arm" example becomes `Colorization`.

When colorization lands, each of these fails loudly rather than silently testing nothing. That is how they are written
today, so it needs no new mechanism.

### D5. The three profiles are ported, as the reference recorded them

Each is a `match precision` in the model's own file, as `gothenburg::profile` is. No wrapper is added.

- **Moscow and Novgorod:** `CpuAndGpu` at FP16, the default at FP32. Each gets Gothenburg's four tests: FP16 carries
  `CpuAndGpu`; FP32 carries the default; neither precision declares a sequential mode; nothing else is declared.
- **Petersburg:** the default at both precisions, pinned by tests in the style of Stockholm's measured default. The
  prose records the reference's sweep table. The point is that the default is a measured answer, and a later editor
  should not "fill in" Moscow's setting because the three models share a family.

**Why porting is enough.** Every setting is CoreML-only and every declared one is FP16-only, so a wrong one costs speed
on a Mac and never produces a wrong image. The precision-split rule holds: nothing is declared at a precision it was not
shown to help.

**The one caveat is Moscow's export.** The reference's prose records two magnitudes for the same switch (−45% per tile
on one export, −22% on a single-partition build) without saying which export is published. The sign is the same either
way, so the setting stands. The prose says so and repeats Gothenburg's precondition: count the published graph's CoreML
partitions before re-measuring.

### D6. Session ownership and lifetime

This is unchanged from every other single-graph family, and is stated because the design rules require it. `Filter`
embeds a `SingleGraph`, so `Model::required` is a one-element slice and `Model::sessions` is one
`(&ArtifactId, &EpProfile)` pair. No profile is ever in scope without the artifact it was measured for.

**The chain driver acquires the handle before the pipeline runs, and the pipeline borrows it for the duration of the
call.** The per-tile closure borrows `&sessions[0]` for the length of the `run_tiled` call and no longer. Nothing in
`filter.rs` opens, retains or drops a session. So "a session a run holds cannot be reclaimed" remains a property of the
driver's reference counts, not a rule this file has to follow, exactly as it was when the same code lived in denoise.

One session serves every strength, because the strength is not part of the artifact name. Sharpen gets the same test
denoise has for this.

`Filter` is plain data: an artifact id, a profile, a range, an optional threshold, a strength and a display name. That
is why it satisfies `Send + Sync` for free. Peak scratch is `run_tiled`'s: two tile tensors, one decoded tile and the
result, with the blend working in place on the result.

### D7. What the guard's verification can and cannot claim

The mechanism is proven once, in `filter.rs`'s suite. The wiring is proven per family, by the two seam tests (D2).
Neither can prove that Petersburg actually diverges, or that 3.0 is where to catch it. The threshold is the reference's,
carried over on trust exactly as Stockholm's was, and `petersburg/mod.rs`'s prose and the `sharpen` spec both say so.
The sharpen live check reports how many tiles the guard kept on the fixture. Stockholm's kept none in any of its twelve
runs.

## Crates and dependencies

**None added, and none weighed.**

**`rust-sak`:** nothing is needed from it. The tile grid, the tensor conversion, the blend and the session seam live
in `inference/`, and the guard is a scan over an `f32` slice. None of `crypto`, `fetch`, `fs`, `image`, `image-raw`,
`memo`, `o11y` or `sysinfo` is relevant. **The gap is nil**, so there is nothing to decide between extending `rust-sak`
and writing locally.

**Division across crates:** all logic is in `opai`. `gui`, `cli` and `perf` are untouched. The GUI's sharpen control is
built from the catalogue and reaches a run through `Opai::process`, and `perf` already selects sharpen variants by
codename.

## Platforms and providers validated

**Validated by the fake-backend suite on every CI platform** (macOS, Windows and Linux) with no ONNX Runtime, no GPU, no
model file and no network. It covers the tile run at scale 1, the guard, the blend, the progress schedule, the depth
dispatch, cancellation, the refusal, and both families' wiring of the guard.

**The live checks run by hand on macOS arm64 (M2 Max), on the CPU provider and on CoreML, at both precisions.** Sharpen's
is new and covers all three variants. Denoise's is re-run unchanged to show that the move changed nothing a real graph
renders.

**No sharpen profile is validated on this project's build** (D5). The figures are the reference's: Petersburg's and
Novgorod's are from an M2 Max, and Moscow's end-to-end figures are from an Apple Silicon Mac the reference does not
name. **CUDA and TensorRT are untested**, and nothing here declares a setting for either.

## Risks / Trade-offs

- **The move silently changes what denoise renders.** Every failure mode here produces a plausible photograph and no
  error: a range swapped, a strength resolved differently, a guard no longer handed through. → Denoise's assertions move
  unedited (D2). The family seam tests catch a lost guard. Denoise's live check is re-run against the real weights.
- **Petersburg's guard is unverified against Petersburg.** → D7: the mechanism is proven, the threshold is carried on
  trust, the prose and the spec say so, and the live check reports whether it fired.
- **The ported profiles could be wrong on this build.** → They are CoreML-only and cost only speed. Moscow's export
  ambiguity is recorded with the precondition for re-measuring (D5).
- **A shared `GUARDED` counter could make one family's live check report the other's tiles.** → The live checks are
  `#[ignore]`d and run one module at a time. Each resets the counter before its runs, so the count it reports is its own.
- **The task budget is tight: 14 of 15.** → If the test move in D2 turns out larger than one task, the promotion (group 1
  of tasks.md) is cut into its own slice ahead of sharpen, as `config.yaml` prescribes. It is not compressed.
