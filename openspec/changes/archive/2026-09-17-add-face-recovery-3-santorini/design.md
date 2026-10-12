## Context

See proposal.md — Why, and `specs/face-recovery/spec.md` and `specs/execution-providers/spec.md` for the
requirements. What follows is the state of the code this design has to work with, and the one thing the reference
implementation does not transfer unchanged.

**Everything this model needs is written, and all of it is in another model's directory.** Slice 2 built
`transform.rs`, `warp.rs`, `mask.rs` and `composite.rs` under `models/face_recovery/athens/`, wrote each of them
model-agnostically — none names a variant, each takes the square as a parameter — and recorded them in
`models/mod.rs`'s list of files parked at model tier, against the sentence *"Santorini promotes Athens' four"*. The
pipeline that sequences them is in `athens/mod.rs` alongside Athens' measured profile, and is the fifth thing to
move even though the list names four.

**What is genuinely different about Santorini.** One thing: `Inputs: []string{"input"}` against Athens'
`{"input", "weight"}`. Same 512 square, same `[-1, 1]` standardisation on both halves of the conversion, same
alignment, same mask, same composite, same per-face loop — the reference shares all of it through one
`facerecovery` package and branches in exactly one place, `if fidelity >= 0` inside `runInference`.

**What the seam already offers.** `Backend::run_graph` — one tensor of a declared shape in, one of a declared shape
out — has existed since Osaka's region loop and is precisely the unweighted call. `Backend::run_weighted` is slice
2's. **No session call is added by this change**, which is the useful consequence of slice 2 having added the fourth
one against the existing `accept` checker rather than generalising `run_graph`.

**What the reference carries, read against rather than assumed.** `models/facerecovery/santorini/santorini.go` (the
row and the whole of the measured provider prose), `santorini_test.go` (which of those settings are asserted and
why), `facerecovery/variant.go` (`Variant.Fidelity` as the per-model `1.0` or `-1`) and `facerecovery/inference.go`
(`runInference`'s single branch).

## Goals / Non-Goals

**Goals:**

- The promotion happens **before** the second caller is written, and is a file move rather than a rewrite: the four
  geometry files change only their module path and their imports.
- One restoration contract for the family, with the two variants differing at exactly one call site — the shape the
  reference has and the shape slice 2's D8 predicted.
- Athens is provably unchanged by having been widened: the weighted path, the `Fidelity::MAX` default for an
  operation carrying none, and every geometric property slice 2 pinned all still hold, against a fake backend with
  no ONNX Runtime.
- Santorini's measured tuning declared with the prose that says why two of its settings are the ones a later reader
  will try to "fix".

**Non-Goals:**

- **A fifth session call, or any change to the four that exist.** Santorini takes the one that has been there since
  Osaka.
- **Promoting anything to `inference/`.** The four files gain a second caller inside one family, which is a
  family-tier move. A cross-family promotion needs a second *family*, and there is none.
- **Re-measuring either profile.** The figures are the reference's, transcribed with their experimental conditions.
  What this change validates is that the settings are *declared*.
- **Any `perf` work.** Slice 2's D11 built the detection pass over the family rather than over Athens; Santorini
  rows already resolve, already receive the faces and already fail when there are none.
- **Autopilot, crop, the export queue, per-image face selection UI, and any `gui` or `cli` work.** Neither front end
  reaches inference yet.
- **A new `UnsupportedReason` arm, or removing one.** The five deferred families still answer
  `FamilyNotImplemented`; face recovery simply stops being one of the things that reaches it.

## Slices

The face recovery series: making Athens and Santorini expressible, then running them.

| # | Slice | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-face-recovery-1-params` | `Fidelity`; the twenty per-variant constructors; the `Analysis` carrier; full-value operation equality with quantized faces; Athens' fidelity in the run cache tag; the catalogue's per-variant parameters and non-numeric kinds; `params()` consumed at the pipeline-construction seam | `add-models-3-faces`, `add-detection-newyork` (both done) | **done** (archived `2026-09-17-add-face-recovery-1-params`) |
| 2 | `add-face-recovery-2-athens` | Athens' pipeline: the ArcFace landmark template and similarity transform, the 512px aligned tile, the feathered elliptical blend mask, the composite back into the frame, the fidelity weight bound as a second input tensor, and the benchmark harness's one detection pass | 1 | **done** (archived `2026-09-17-add-face-recovery-2-athens`) |
| 3 | `add-face-recovery-3-santorini` | Slice 2's geometry and pipeline promoted to family tier; Santorini's pipeline over a graph that takes the image alone; its measured provider profile; the family's seam reduced to one infallible arm | 2 | **this change** |

Slice 3 closes the series. After it, `models/mod.rs`'s list of files parked at model tier holds Osaka's five and
nothing else, and every model in the catalogue that is not one of the five deferred families can run.

## Decisions

### D1. The promotion is a move, and it happens first

`models/face_recovery/athens/{transform,warp,mask,composite}.rs` become `models/face_recovery/{…}.rs`, moved with
their tests, edited only where the module path shows: the `mod` declarations, the `use super::…` lines that now
reach one level less far, and the one sentence in each file's header that says which model it belongs to. **Nothing
in their bodies changes**, and that is checkable — a diff of a `git mv` that touches a function body is a rewrite
wearing a move's clothes, and the task says to verify it.

They do not go to `inference/`. The doctrine's tiers are decided by callers: two models of one family is a
family-tier file, and `inference/mod.rs` holds what *no* model and *no* family owns. An affine warp being
general-purpose in the abstract is not the test; having a caller outside this family is, and it has none.

`models/mod.rs`'s list loses its four face-recovery entries in the same task, because that list is the mechanism by
which the next change knows what to move, and a list that still names a promoted file is worse than no list.

### D2. The pipeline is promoted too, and Athens keeps only what is Athens'

`athens/mod.rs` today is 889 lines: a tile size, a normalisation, a progress constant, the measured profile and its
prose, the `Model`/`ImagePipeline` impls, the per-face loop, the depth dispatch, the full-frame carry, and a fake
backend with the pipeline's tests. All of it but **the profile and its prose** is the family's.

It becomes `models/face_recovery/restore.rs`, holding the one restoration contract, and `athens/mod.rs` is left as a
model row: `profile(precision)` and the comment that explains every figure in it. `santorini/mod.rs` is the same
file for the other model. That is the shape `detection/newyork/` and `upscale/tokyo/` already have, and the shape
the doctrine names — *its row, its measured EP profile and the prose behind it*.

*Alternative considered:* leaving the pipeline in `athens/` and having Santorini reach into it. That is the `use`
across a model boundary the doctrine calls a bug rather than a shortcut, and it would leave the family's only
pipeline named after one of its two models.

*Alternative considered:* promoting the four geometry files now and the pipeline later, when it "needs" to be
shared. It needs to be shared in this change — Santorini is the caller — and deferring it is the copy the doctrine
exists to prevent.

### D3. The square and the normalisation are family-tier constants, not per-variant values

The reference carries `TileSize` and `Inputs` as fields on its `Variant` struct, because a Go registration shim is a
data row and everything that varies has to be expressible in it. Here both variants are 512 and both standardise to
`[-1, 1]`, so there is one value each, and a per-variant field would be a shape describing a variance nothing has.

What makes this safe rather than a shortcut is that **the geometry already takes the square as a parameter**:
`transform::alignment(&landmarks, tile)`, `warp::align(…, tile, range)`, `mask::plane(edge)` and
`composite::blend(…, tile, range)` are all functions of it, and the spec requires the template to be scaled to
whatever square a model accepts. A third variant at another size therefore turns a constant into a per-variant value
at the point it exists, and changes nothing else.

*Alternative considered:* a per-variant square now, for parity of shape with the reference. Rejected on the
project's own rule against publishing structure whose subject does not exist — and a per-variant field that both
variants fill with the same literal is a field nobody can tell is load-bearing.

### D4. The weight is resolved from the variant row, not from the operation's `Option`

`FaceRecoveryVariant` answers what weight a run binds, given what the operation carries:

- `Athens(_)` → `Some(carried.unwrap_or(Fidelity::MAX))`
- `Santorini(_)` → `None`

and `FaceRecovery::pipeline` hands the resolved `Option<f32>` to `restore::pipeline`. This is
`facerecovery.Variant.Fidelity` — `1.0` or `-1` — put where the reference puts it, without the sentinel.

**It cannot be read off `FaceRecoveryParams.fidelity` alone, and that is the trap this decision exists to avoid.**
That field is `None` for two different reasons: Santorini has nowhere to bind a weight, and a *deserialized* Athens
operation can reach a pipeline with nothing in it, because the field is `#[serde(default)]` (slice 2, D7). Choosing
the call from the `Option` would send such an Athens operation down the single-input path, where the runtime refuses
it for a missing required input — a stored selection that used to run, failing after an unrelated change, for a
reason the error does not name. The variant knows which graph it is; the operation only knows what it was given.

*Alternative considered:* the seam matching the variant itself, `Athens => Some(…)`, `Santorini => None`, inline in
`FaceRecovery::pipeline`. Identical behaviour, and it puts a per-model property in the family's seam rather than on
the model's row, which is where every other per-model property in this crate lives — the profile, the artifact, the
codename.

*Alternative considered:* keeping `Fidelity::MAX` inside the pipeline as slice 2 has it, and passing a
"takes a weight" boolean beside the option. Two parameters that can disagree, where one cannot.

### D5. One match, two existing calls, no fifth

`restore.rs` holds the weight as `Option<f32>` and matches it once, at the point the tensor is run:

```text
Some(weight) => B::run_weighted(session, &aligned, shape, weight, &mut restored, shape)
None         => B::run_graph(session, &aligned, shape, &mut restored, shape)
```

Both are already on the trait, both take the same declared shapes, and both fail the same two ways through the same
`accept` checker. This is `runInference`'s `if fidelity >= 0`, at the same place in the loop.

The match is **inside** the per-face loop rather than resolved into a closure or a function pointer above it: it is
a branch on a value that does not change during a run, which every branch predictor on every target handles for
free, and the alternative costs an indirect call per face plus a lifetime puzzle to name the closure's type over
`B`. Readability decides it, and the reference agrees.

### D6. The family's seam stops returning a `Result`

`FaceRecovery::pipeline<B>` returns `Shared<B>` rather than `Result<Shared<B>, InferenceError>`, and
`Operation::pipeline`'s face-recovery arm wraps it in `Ok`. The family has no variant it can refuse and no
resolution that can fail, so a `Result` would be a failure case no caller can reach and no test can construct.

This is deliberately **not** symmetric with `Upscale::pipeline`, which keeps its `Result` because it has a real
guard: a scale resolving to zero passes. Uniformity between the two seams would be bought by making one of them
lie, and the enum arm that adapts the two shapes is one line.

### D7. Santorini's measured provider profile

`santorini/mod.rs` answers `EpProfile { coreml_specialization: FastPrediction, execution_mode: Sequential,
..EpProfile::default() }` at both precisions, and `FaceRecoveryVariant::profile`'s Santorini arm reaches it.
Transcribed from `santorini.go`. See `specs/execution-providers/spec.md` for what each is worth.

**This is the first model in the catalogue to declare a specialization strategy**, and the first place the
`coreml_specialization` field has had a non-default value since it was added. Its value is a trade rather than a
win — 1.04x at FP16 in steady state against 20-30ms of extra FP32 session build — and it is only defensible because
sessions stay resident: a model rebuilt per image would pay the cold start every time and lose.

**Two settings are deliberately absent, and the prose has to say so beside the ones that are present**, because
both are what a reader tidying the family's two rows into agreement would add:

- **The CoreML compute-unit restriction Athens carries.** Copying it costs about 9%. Santorini's FP16 graph runs
  FP16 through its upsampling path so the Neural Engine takes it whole; Athens is a CodeFormer and thrashes. The
  difference is the *export* — before these weights were re-exported their resize nodes were held at FP32 with 84
  cast nodes around 42 upsamples, and Athens' rule looked right for them too. That sentence is the one worth
  carrying, because it says what to re-measure if the weights move again.
- **The CUDA NHWC layout.** Not a slower setting here but a **failing** one: 23 of this graph's 95 convolutions
  take a weight computed by a reshape rather than an initializer, which is exactly the modulated-convolution hazard
  `providers/profile.rs` already documents for that flag, and the run fails with a channel/group mismatch.

Both are pinned by tests, for the reason `santorini_test.go` gives: getting either wrong is silent on the one that
costs 9% and loud only at run time on the other, and neither is visible in a review of a two-line struct literal.

### D8. One fake backend, serving both calls

The fake in `restore.rs`'s tests implements `run_weighted` **and** `run_graph`, records which one it was asked for
and with what, and answers with a known tensor. Everything Santorini needs asserting is then assertable with no ONNX
Runtime, no GPU and no model file, on every CI platform — the property slice 2 established and this change extends
rather than repeats.

The four remaining doubles elsewhere in the crate are untouched: no trait method is added, so nothing that
`unimplemented!()`s a call it does not make has to change.

### D9. Sessions, lifetimes, platforms and providers

**Unchanged from slice 2, and that is a decision rather than an omission.** Santorini's `Model::sessions` answers
one `(artifact, profile)` pair, `pipeline::acquire` opens it before the run, and the pipeline receives
`&[SessionHandle<B::Session>]` it cannot retain, drop or reorder — so "a session a run holds cannot be reclaimed"
stays a property of the chain driver's reference counts rather than of anything written here. The per-face loop
takes the session lock **per graph run**, so a second image in an export queue interleaves at face granularity, and
`run_graph` keeps that property exactly as `run_weighted` does: both take the lock inside the call.

Memory per face is the tensor in and the tensor out — 3 MB at 512 square — held for one face at a time. The mask is
process-scoped immutable data with no `Drop`, shared by both variants now rather than built per variant, which is
one fewer copy than the reference has (its mask is a field on each `Variant`).

**Platforms and providers.** The pipeline and all of its geometry are exercised on every CI platform through the
fake backend, with no runtime and no model file. The provider profile is **transcribed, not measured**: its figures
were taken by the reference on an M2 Max (CoreML) and an RTX 5090 (CUDA). **No execution provider has been exercised
against this graph in this project**, and TensorRT has no figure for it in either implementation.

### D10. Crates, dependencies and `rust-sak`

**`opai` alone. No new crate of any kind, first-party or third-party, and none was weighed.** This change moves
files, adds a two-field struct literal and matches on an `Option` at one call site; there is no new arithmetic, no
I/O and no infrastructure of any sort for `rust-sak` to have covered. Its `image`, `crypto`, `fetch`, `fs`, `memo`
and `sysinfo` modules have no bearing on any of it, and nothing here is general-purpose to another project.

`perf` is untouched for the reason D11 of slice 2 gives: its detection pass was written over the family. `gui` and
`cli` are untouched because neither reaches inference yet, and nothing here is front-end logic — a front end's share
of face recovery is the face overlay and the fidelity control, which is a later change consuming the catalogue slice
1 published.

## Risks / Trade-offs

- **The promotion is a rewrite wearing a move's clothes** → The four geometry files move with `git mv` and the
  review is the diff: anything outside a `mod` line, a `use` line or a header sentence is a body edit, and the
  verification step says to check exactly that. Their tests move with them and must pass unedited.
- **Widening Athens' pipeline changes Athens** → The two failure directions are opposite and both are pinned: an
  Athens operation carrying no fidelity must still reach `run_weighted` with `Fidelity::MAX` (not `run_graph`), and
  a Santorini operation must reach `run_graph` with no weight of any value (not `run_weighted` with a default).
  Either mistake is a runtime error on a real graph and a silent pass without a test, which is why the fake records
  *which call* it received rather than only what it was given.
- **A regression in the geometry would now affect two models instead of one** → True, and it is the point: a
  regression in one shared file is a regression one set of tests catches, where the alternative is two copies and
  the disagreement between them. The tests move with the files, so coverage does not dip during the move.
- **Santorini runs for the first time and has never been compared against the reference's output** → The same gap
  slice 2 stated and it is inherent: no test can assert a restoration is correct without the weights and a reference
  image. What is asserted is every step around the graph. A visual check against the Go application on one
  photograph is a task, not a test.
- **Santorini's profile is declared on hardware nobody here has measured** → Stated in the prose and in the
  requirement. The risk is asymmetric and small: the specialization strategy changes no pixel, and the execution
  mode is bit-identical by measurement on three providers.
- **Someone "fixes" the two settings Santorini deliberately does not carry** → The two tests and the prose exist
  for precisely this, and the prose names the export rather than the architecture as the reason, so a future
  re-export is the trigger to re-measure rather than a reason to copy Athens.
- **The seam's signature change ripples** → It has one caller, `Operation::pipeline`, and the compiler finds it.

## Migration Plan

Nothing persisted is invalidated. Santorini's cache tag was finished in slice 1, carries no fidelity segment by
construction, and no Santorini run has ever executed — so there is no stored result to miss and nothing on a user's
disk to sweep. Athens' tag is untouched, and both operations' serialized forms are unchanged.

The one behavioural change visible to an existing caller is that `Opai::process` serves an operation it used to
refuse. Additive, and the refusal it replaces was `Unsupported`, which no caller can have depended on.

## Open Questions

- **Whether Santorini's transcribed profile reproduces on this project's hardware.** Not answerable here and not
  blocking: the settings are declared and the figures are labelled as the reference's. A measurement pass across the
  whole catalogue on this project's own builds is its own change, and it is the one that would change these numbers
  rather than this one.
- **What a front end offers as the default fidelity, and how the control is labelled** — and, now that both models
  run, whether it is hidden or disabled when the user has Santorini selected. Carried forward from slices 1 and 2
  unchanged; answerable when `gui` first draws one, and nothing in the library depends on it.
