## Context

See proposal.md — Why. What matters for the approach is the shape of what is already here.

`Opai::process` is the only inference entry point, and every layer under it is built for pixels:

```
Opai::process(&Picture, &[Operation], Option<ProcessOptions>) -> Enhanced
  |
  +-- chain::process::<B>()     log bracket, depth resolution, RunCache, identity_after
        |
        +-- plan::<B>(ops)      Operation::pipeline::<B>()  -> Arc<dyn Pipeline<B>>
        +-- run(...)            per operation: cache read, acquire sessions, spawn_blocking
                                { Pipeline::run -> DynamicImage }, cache write; every
                                SessionHandle held until the whole chain returns
```

`Pipeline<B>::run` returns a `DynamicImage` and takes a `ChannelDepth`. `Backend` has two run methods — `run_tile`,
whose shape is recovered from the buffer length, and `run_graph`, whose shapes are declared — and both take
`outputs.values().next()`, the single output tensor. There are five `Backend` implementations: `Opai` and four test
doubles (`inference/test_support.rs`, `chain.rs`'s fake, `osaka/mod.rs`'s and `osaka/region.rs`'s).

The detection vocabulary is already complete and public: `Point`, `Rect`, `Confidence`, `Face`, `Faces` and the
`DetectionOutput` alias in `models/face.rs`, the `DetectionVariant` enum and the New York row in
`models/detection/`. `Detection::artifact()` already resolves `dt_newyork_fp32` and `dt_newyork_fp16`. What does not
exist is anything that runs one.

This is **not one slice of a series.** It is a single change that both opens the path and puts the first model on it,
deliberately: a change delivering only the path would ship a public trait nothing implements and an entry point with
no reachable model, which is the half-wired state the slicing rule exists to prevent.

Constraints that shape everything below: the 15-task limit, the module-layout doctrine in `config.yaml` (a model's
execution code lives with the model; a second caller promotes a file rather than copying it), and the requirement
that the arithmetic stay exercisable on a CI runner with no ONNX Runtime, no GPU, no model file and no network.

## Goals / Non-Goals

**Goals:**

- One inference entry point for operations whose result is not an image, with the result type decided by the
  operation rather than by the caller, checked at compile time.
- New York running end to end and producing faces that match the reference within resampler tolerance.
- The chain driver's behaviour **bit-identical** after the extraction it undergoes. Its existing tests should pass
  unedited; if any needs an edit, the extraction was not a pure lift.
- Everything but the model run exercisable with no runtime: preprocessing, the anchor grid, the decode, the
  suppression, the rescale, the progress weighting, the cancellation, the session-holding.

**Non-Goals:**

- **No front-end work.** Neither `gui` nor `cli` calls `process` today and neither will call `execute` here. A Tauri
  command, a CLI subcommand and the per-image face selection UI are their own changes.
- **No face recovery.** It consumes `Faces` and is the obvious next change; nothing here anticipates it beyond
  producing the value it will take.
- **No Autopilot.** It is a detection run under another name plus a policy on top; the policy is not designed here.
- **No caching of a non-image result**, and no second store for one.
- **No re-measurement of the provider profile.** New York's settings are transcribed from the reference's
  measurements, not reproduced.
- **No alpha, no bit depth, no tiling on this path.** The detector reads a fixed 8-bit-derived square and returns
  coordinates.

## Decisions

### D1. The pipeline trait splits into a base and two run contracts

```rust
pub(crate) trait Model<B: Backend>: Send + Sync {
    fn required(&self) -> &[ArtifactId];
    fn sessions(&self) -> Vec<(&ArtifactId, &EpProfile)>;
}

pub(crate) trait ImagePipeline<B: Backend>: Model<B> {
    fn stages(&self) -> usize;
    fn run(&self, input: &DynamicImage, sessions: &[SessionHandle<B::Session>], depth: ChannelDepth,
           progress: Option<&dyn Fn(f64)>, cancelled: &dyn Fn() -> bool)
        -> Result<DynamicImage, InferenceError>;
}

pub(crate) trait DataPipeline<B: Backend>: Model<B> {
    type Output;
    fn run(&self, input: &DynamicImage, sessions: &[SessionHandle<B::Session>],
           progress: Option<&dyn Fn(f64)>, cancelled: &dyn Fn() -> bool)
        -> Result<Self::Output, InferenceError>;
}

pub(crate) type Shared<B>        = Arc<dyn ImagePipeline<B>>;
pub(crate) type SharedData<B, T> = Arc<dyn DataPipeline<B, Output = T>>;
```

**`Model<B>` holds what both drivers ask of a pipeline and nothing else.** What must be on disk, and which sessions to
open under which settings — the two questions the acquisition loop asks. It is object-safe, so the shared helper in D2
takes `&dyn Model<B>` with no generic parameter at all.

**`stages()` moves down onto `ImagePipeline<B>`, not the base.** It is read by exactly one thing: `chain::run`'s per-step
`debug` record, where it distinguishes a two-pass Kyoto run from a three-graph Osaka one. Detection is always one, and
the execute driver writes its own record. A method on the base that one of the two contracts answers with a constant
is a method that only looks shared.

**`DataPipeline::run` does not take a depth.** That is the whole reason this is two traits rather than one with an
associated `Output`. Bits per channel is a property of pixels a caller receives; a run that hands back coordinates has
no answer to give and would have to document ignoring the parameter. The single-trait alternative was considered and
is genuinely lighter — one name instead of three — but it buys that by putting a parameter into a signature where it
means nothing, which is the shape `config.yaml`'s conventions and `session.rs`'s own "two contracts, two functions"
comment both argue against.

The alternative of one trait generic over the output was also rejected for a second, smaller reason: `chain.rs` would
have to spell `dyn ImagePipeline<B, Output = DynamicImage>` in every position where it spells `Shared<B>` today, and the
helper in D2 would become generic over an output it never touches.

### D2. The acquisition block is lifted, the blocking hop is not

The block both drivers need is the one that turns a pipeline into open sessions:

```rust
// inference/pipeline.rs, beside the traits it is written over
pub(crate) async fn acquire<B: Backend>(
    backend: &B,
    model: &dyn Model<B>,
    reporter: &Arc<OperationProgress>,
    provider: ExecutionProvider,
) -> Result<Vec<SessionHandle<B::Session>>, InferenceError>
```

It reports the install over the head of the operation's range, divided between the artifacts `required()` names, then
acquires each `(artifact, profile)` pair in order. That is the part where the two drivers were going to be identical
line for line, and it is the part that carries the invariant worth writing once: no profile is ever in scope that is
not already attached to the artifact it was measured for.

**The `spawn_blocking` hop is written out in both drivers rather than shared, and that is deliberate.** Its two
closures have nothing in common but their shape: the chain's captures a `RunCache` and a key and writes the encoded
result back inside the same blocking call; the execute driver's captures neither and has no encode. Factoring a
six-line plumbing shape whose bodies are disjoint would produce a generic function taking a closure that does all the
work — a parameterisation with one meaningful argument. `config.yaml`'s rule is against copying *logic*; there is
none here to copy.

It lives in `inference/pipeline.rs` rather than in a new module because it is the traits' own companion and nothing
else calls it. A file's tier is decided by its callers, and both of them are drivers reaching it through `Model<B>`.

### D3. The operation names its own result type, through a sealed trait

```rust
// models/operation.rs
pub(crate) mod sealed {
    pub trait DataModel {
        type Output;
        fn as_operation(&self) -> Operation;
        fn pipeline<B: Backend>(&self) -> Result<SharedData<B, Self::Output>, InferenceError>;
    }
}

/// Implemented inside this crate only.
pub trait DataOperation: sealed::DataModel {}
impl<T: sealed::DataModel> DataOperation for T {}
```

`Detection` implements `sealed::DataModel` with `type Output = Faces`, `as_operation` returning
`Operation::Detection(*self)` for the progress label and the records, and `pipeline` returning the New York pipeline.

**Sealed, and that is load-bearing rather than defensive.** `SharedData<B, T>` and `Backend` are `pub(crate)`, so a
public trait carrying that method would put a crate-private type in a public interface. The public-trait-in-a-private-
module pattern lets `Opai::execute<O: DataOperation>` name the bound publicly while the method it dispatches through
stays inside the crate, and it means `O::Output` resolves through the supertrait with no second declaration to keep in
step.

**`pipeline<B>` is a generic method rather than an object-safe one**, unlike `Operation::pipeline`. Nothing here is
held as a trait object: `execute` is monomorphised per operation type, which is what makes the binding compile-time in
the first place.

This is the seam the layout doctrine calls seam 2, for the detection family: one arm, one contract.

### D4. `Opai::execute`, and the driver behind it

```rust
pub struct ExecuteOptions {
    pub provider: ExecutionProvider,
    pub on_progress: Option<OnInference>,
    pub cancel: CancellationToken,
}

pub struct Executed<T> {
    pub value: T,
    pub providers: ProviderReport,
}

pub async fn execute<O: DataOperation>(
    &self,
    source: &Picture,
    operation: &O,
    options: Option<ExecuteOptions>,
) -> Result<Executed<O::Output>, InferenceError>
```

`ExecuteOptions` is a separate type from `ProcessOptions`, not a subset of it: `depth` and `cache` are the two fields
that would be meaningless, and `ProcessOptions`' own documentation argues that a bundle with a `Default` plus
`..Default::default()` is what makes a field added later source-compatible. Both types get that independently. Neither
is `#[non_exhaustive]`, for the reason written into `ProcessOptions`.

`Executed<T>` mirrors `Enhanced { picture, providers }`. It exists because a run downgraded to the CPU returns the
same faces several times slower and is otherwise invisible — the argument `ProviderReport`'s own documentation makes
for putting the report on the result rather than on the progress stream, which applies here unchanged. The reference's
`Execute[T any]` returns a bare `T`; this is additive over it.

The driver lives in `inference/execute.rs`, a sibling of `chain.rs` rather than a function inside it. It:

1. Brackets the run with the same `info` pair `chain::process` writes — the operation's cache tag and the source's
   identity in, the elapsed time and the outcome out — so one log reads the same for both paths.
2. Builds `ChainProgress::new(on_progress, 1)`. A one-operation chain is exactly what this is, so the install/run
   split, the monotonic clamp and the "a model already on disk does not give up the head of its range" property all
   come for free and are already tested.
3. Calls `pipeline::acquire`, hands the run to a blocking thread, holds the handles until it returns.
4. Folds `ProviderReport::new(provider, held.iter().map(SessionHandle::provider))` — the same fold, over one handle.

**No `RunCache` is constructed and no key is derived**, so there is nothing to disable and no per-run flag. That is
the reference's behaviour too.

**Ownership and lifetimes.** Identical to the chain's, which is the point of reusing the acquisition: the driver owns
every `SessionHandle` for the whole call and drops them on return. The session cache's "in use" test is the handle's
own reference count, so a model an execute run holds cannot be reclaimed by the idle sweep, by `release_sessions`, or
by a provider switch — and the handles travel into the blocking closure and back out of it, so there is no window in
which one could be found reclaimed. Detection takes exactly one handle; nothing about the guarantee depends on that.

### D5. A third session call, addressed by name

```rust
pub(crate) struct NamedOutput<'a> {
    pub(crate) name: &'a str,
    pub(crate) buffer: &'a mut [f32],
}

fn run_named_outputs(
    handle: &SessionHandle<Self::Session>,
    input: &[f32],
    input_shape: GraphShape,
    outputs: &mut [NamedOutput<'_>],
) -> Result<(), Self::Error>;
```

`run_tile` and `run_graph` deliberately spell no tensor names, and their comments say why: `Session::run` with
positional inputs uses the session's own declared names, so a conventionally exported one-in-one-out graph needs none
and a graph declaring something else still runs. **That argument does not survive three outputs.** RetinaFace returns
`loc`, `conf` and `landmarks`, and taking them positionally makes the result depend on declaration order — a
re-export that reordered them would decode confidences as box offsets and return detections that are wrong rather than
absent, with nothing at any layer able to notice. The reference reached the same conclusion from the same evidence and
put `Outputs: []string{"loc","conf","landmarks"}` on the variant specifically because *"Detection graphs return three
of them rather than the single `output` the rest of the codebase's models use."*

The outputs are checked by **length**, not by shape. `GraphShape` is `[1, c, h, w]` and these are rank-3 — `[1, 16800,
4]`, `[1, 16800, 2]`, `[1, 16800, 10]` — so declaring a `GraphShape` for them would be a lie dressed as a check. The
name carries the identity and the length carries the arity; between them a mismatched export is caught.

The failure modes get a pure checker beside `session::accept`, for the reason that one has: constructing an
`ort::Error` loads the runtime's API table, so putting the checks in a function that returns this crate's own error
keeps "a named output is missing" and "a named output is the wrong length" checkable on a runner with no runtime.
Nothing is written on any failure, including for the outputs that were present.

**Cost: five `Backend` implementations.** One production and four doubles. The doubles only need enough to satisfy the
trait; the detection tests drive a purpose-built fake that returns synthetic anchor tensors.

### D6. New York lives entirely under `models/detection/newyork/`

Per the layout doctrine: one model, one directory, and its execution code with it rather than in `inference/`.

```
models/detection/newyork/
  mod.rs        the NEW_YORK row, the measured profile and its prose, the DataPipeline impl, TARGET_SIZE
  input.rs      fit-to-640, zero-pad, BGR/CHW, minus the per-channel means
  anchors.rs    the 16,800 priors, generated once
  decode.rs     one anchor's box and landmarks from the variance-encoded outputs
  filter.rs     threshold, suppression, rescale into the source's coordinate space
```

Nothing under it is promoted to family or cross-family tier, because detection has exactly one model and no second
caller exists. `input.rs` in particular looks like a candidate for `inference/tensor.rs` and is not one: that module's
conversions mirror edge pixels outwards and normalise over `[0,1]` or `[-1,1]`, while this one zero-pads and subtracts
per-channel means in BGR order. They share the letters CHW and nothing else. `inference/mod.rs` already states that
nothing in `inference/` resizes, and this resizes.

The constants, transcribed:

| | |
| --- | --- |
| Input | `[1, 3, 640, 640]`, BGR planar, minus (104, 117, 123) per channel |
| Padding | the negative means, i.e. what a zero pixel becomes; anchored top-left, not centred |
| Anchors | strides 8/16/32 over 80x80, 40x40, 20x20; min sizes {16,32}, {64,128}, {256,512}; **16,800** total |
| Outputs | `loc` 67,200 floats, `conf` 33,600, `landmarks` 168,000 |
| Decode | variance 0.1 on centres and landmarks, 0.2 on the exponentiated extents |
| Threshold | `conf[i*2 + 1] > 0.5`, strictly greater |
| Suppression | IoU `> 0.4` suppresses; areas and intersection both carry the Pascal-VOC `+1` |
| Rescale | boxes are scaled by 640, then by `orig / resized` on each axis using the **integer** resize dimensions |

Two things about that table are easy to get wrong and are worth the tasks naming them. The decode is deferred to the
survivors — the reference's own optimisation, and a real one: exponentiating 16,800 anchors to discard 16,798 of them
is the dominant cost of the decode. And the rescale uses the integer dimensions the image was actually resized to,
not the ideal float ratio, so that it undoes exactly what the preprocessing did rather than approximately.

**Face order is descending confidence**, which is what the suppression's own ordering produces. `Faces` documents that
the order is the order it was given and that a front end refers to faces by position, so the order is a contract. The
reference sorts with Go's unstable `sort.Slice`; this uses a stable sort, so two candidates with identical confidence
come back in anchor order rather than arbitrarily. That is a deliberate tightening of an unspecified behaviour, not a
divergence from an intended one.

**Progress** reports at the reference's phase boundaries — 0.2 once the tensor is built, 0.6 once the graph returns,
1.0 once the decode finishes. They are boundaries rather than measurements; only the middle one takes real time.
Cancellation is checked at the same three points, which is the only schedule a single whole-image run offers.

### D7. Detection's refusal by the chain becomes permanent

`Operation::pipeline` refuses `Operation::Detection` today with `UnsupportedReason::FamilyNotImplemented` — *"this
family's pipeline is not written yet"*. After this change that statement is false and will stay false. Detection moves
out of the seven-family refusal arm into one of its own, with a new `UnsupportedReason` variant saying the result is
not an image and naming `execute` as where the operation belongs.

`UnsupportedReason` is already `#[non_exhaustive]`, so the variant is additive for an external matcher.

The value of separating them is that a front end can act on one and not the other: an unwritten pipeline is a "not
yet" it can grey out and re-offer next version, and a non-image operation is a category error it should never have
made. Collapsing them would make both read as "wait for a later release", and one of them never arrives.

### D8. Crates and dependencies

**`opai` only.** `gui` and `cli` are untouched; see Non-Goals.

**`rust-sak`: nothing new, and nothing missing.** The features already enabled (`crypto`, `fetch`, `fs`, `image`,
`image-raw`, `memo`, `sysinfo`) cover what this change touches, which is the model transfer and the loaded image.
`rust-sak`'s `image` module is codecs — encode and decode across eight formats — and does not resample; that is
correct, and resampling does not belong in it either. Nothing here is a candidate for extending `rust-sak`: the
anchor grid, the variance decode, non-maximum suppression and the coordinate rescale are RetinaFace's arithmetic and
are general-purpose to nothing.

**No new third-party crate.** The letterbox resize is `image::imageops::resize` with `FilterType::Lanczos3`, from the
`image` crate this crate already names because `rust-sak`'s decoders return its `DynamicImage`. `imageops` is not
behind a feature.

### D9. Platforms and providers this design is validated against

Stated plainly, because the parts differ:

- **The arithmetic — preprocessing, anchors, decode, suppression, rescale, progress weighting, cancellation, session
  holding, the named-output checks — runs on every CI platform with no ONNX Runtime, no GPU, no model file and no
  network.** That is the property the `Backend` seam exists for and this change keeps it.
- **The real model run is exercised on macOS, on the CPU and CoreML providers**, which is the development machine.
- **CUDA and TensorRT are untested here.** The provider settings in D6 are transcribed from the reference's
  measurements on an RTX 5090 and are **not** reproduced. The requirement pinning them checks that the declaration
  says what the reference measured; it does not check that the measurement still holds.
- **Windows and Linux are untested beyond CI's arithmetic**, as for every model change so far.

## Risks / Trade-offs

**Lifting the acquisition out of `chain::run` touches the riskiest loop in the crate** → That loop holds the guarantee
that a session a run holds cannot be reclaimed, and the cache rewind that walks backwards over a prefix that would not
decode. The mitigation is stated as a verification rather than as care: `chain.rs`'s existing tests must pass
**unedited**. They already check the rewind, the session-holding, the three progress weightings, the refusal ordering
and what a cancelled run returns. If any of them needs a change, the lift was not a lift and the task is wrong.

**Different Lanczos kernels mean the faces will not match the reference exactly** → They will differ in sub-pixel
placement, which is the same axis the reference's own FP16/FP32 comparison found. Ported tests assert with a
tolerance, and the tolerance is stated in the test rather than tuned until it passes. This is written into the
proposal as an expected divergence so that a later reader does not treat a third-decimal mismatch as a defect. If the
faces differ in *count* or in *order*, that is a port bug and not the resampler — suspect the port first.

**The task budget is at the limit with no slack** → Fifteen tasks, and `config.yaml` makes that a hard limit rather
than a target. If any task grows during implementation, the correct response is to stop and re-slice, not to fold two
steps into one. The natural cut, if it comes to that, is D1–D5 (the path) from D6–D7 (the model) — with the
understanding that the first half alone ships an entry point with no reachable model, which is why it is not the plan.

**`NamedOutput` widens a trait with five implementations** → Four are test doubles that only need to satisfy the
signature. The cost is real but one-time, and the alternative — overloading `run_graph` with an output count — would
put the shape recovery that `session.rs` defends at length behind a parameter a caller can get wrong.

**An `Executed<T>` most callers will destructure immediately** → A front end that only wants the faces writes
`let Executed { value: faces, .. } = ..`. That is the same tax `Enhanced` already charges and the same reason: the
provider report is invisible if it is not returned, and the caller who needs it most — a benchmark — is the one least
able to ask for it any other way.

**A sealed public trait is awkward in rustdoc** → `DataOperation` will render with a supertrait from a private module
and no visible methods. Mitigated by documenting on `DataOperation` itself what implementing types provide and that
the crate is the only implementor; the alternative, exposing `SharedData` and `Backend` publicly to avoid the seal,
would make the whole inference seam public API.

## Open Questions

- **Whether `sealed::DataModel` and `DataOperation` collapse into one trait.** If `#[doc(hidden)]` methods returning
  `pub(crate)` types turn out not to trip the `private_interfaces` lint under this edition, one sealed trait is
  simpler than a trait plus a blanket impl. Either spelling satisfies every requirement and neither changes the task
  breakdown; it is decided at the keyboard.
- **Whether the anchor grid is a `LazyLock` or built per run.** The reference memoises it with a `sync.Once`. 16,800
  priors is 268 KB and a few hundred microseconds against a ~12 ms run, so both are defensible; the measurement
  decides, and it changes nothing above `anchors.rs`.
