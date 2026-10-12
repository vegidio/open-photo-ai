## Why

Seven of the eight families produce an image, and `Opai::process` is built for exactly that: an ordered chain, a
per-operation image cache, an output bit depth, an identity composed from the operations applied. Detection produces
none of those things. It produces a set of faces, and the library already says so — `models/face.rs` defines `Face`,
`Faces` and the `DetectionOutput` alias, and its own comment records that the alias exists *"instead of leaving a
caller to discover it when inference is wired up"*.

This is the change that wires it up. It adds the second inference entry point — one operation, a typed non-image
result — and makes New York the first model to run through it. Nothing downstream can start without it: face recovery
takes the faces a detection run found, and Autopilot's analysis is a detection run under another name, so the two
families that follow both wait on this one.

## What Changes

**A second inference entry point, `Opai::execute`.**

- `Opai::execute<O: DataOperation>(&Picture, &O, Option<ExecuteOptions>) -> Result<Executed<O::Output>, InferenceError>`.
  One operation rather than a chain, and a result type the operation itself names.
- A new public `DataOperation` trait carrying `type Output` and the operation's own identity for reporting.
  `Detection` implements it with `Output = Faces`. The binding is checked at compile time: an upscale cannot reach
  `execute`, and a detection run cannot be asked for anything but faces.
- A new public `ExecuteOptions { provider, on_progress, cancel }`. Deliberately not `ProcessOptions`: `depth` and
  `cache` are meaningless for a run that produces no pixels, and a field a caller can set that does nothing is a
  guarantee they can get wrong.
- A new public `Executed<T> { value, providers }`, the sibling of `Enhanced { picture, providers }`. A detection run
  downgraded to the CPU returns the same faces several times slower, so the report is on the result for the reason
  `ProviderReport`'s own documentation gives.
- The execute path reads and writes **no image cache**, matching the reference.

**The pipeline seam splits so both drivers can share what they share.**

- `Pipeline<B>` becomes `Model<B>` (`required`, `sessions`) plus two run contracts: `ImagePipeline<B>: Model<B>`
  returning
  an image and taking a depth, and `DataPipeline<B>: Model<B>` with `type Output` and no depth.
- The acquire-sessions / report-the-install / hop-to-a-blocking-thread block is lifted out of `chain::run` into a
  helper over `&dyn Model<B>` that both drivers call. Not copied — `config.yaml` is explicit that copying is never the
  answer to a second caller.

**A third session call, for a graph with more than one output.**

- `Backend::run_named_outputs` and `session::run_named_outputs`, addressing outputs **by name**. `run_tile` and
  `run_graph` deliberately spell no tensor names because a one-in-one-out graph needs none; a three-output graph
  inverts that argument, and a re-export that reordered `loc`, `conf` and `landmarks` would otherwise decode
  confidences as box offsets and return plausible garbage. Touches the production backend and four test doubles.

**New York runs.**

- `models/detection/newyork/`: letterbox the image to the detector's fixed 640x640 (aspect preserved, zero-padded,
  BGR/CHW, the RetinaFace per-channel means subtracted), one graph run, then the RetinaFace decode — 16,800 priors,
  a 0.5 confidence threshold, per-survivor box and landmark decoding, non-maximum suppression at 0.4 IoU, and a
  rescale back into the source image's own coordinate space.
- New York's measured provider tuning, currently `EpProfile::default()`, becomes the one the reference measured:
  NHWC on CUDA and sequential execution at **both** precisions, worth -17.5% at FP32 and -35.1% at FP16 on the graph.

**A refusal stops being a deferral.**

- `Operation::pipeline` refuses `Operation::Detection` with `UnsupportedReason::FamilyNotImplemented` today — *"this
  family's pipeline is not written yet."* After this change that is false and permanently so. It gains its own arm
  saying the result is not an image and naming `execute` as where it belongs.

### Deliberate divergence from the reference

- **The Go `Execute[T any]` type assertion has no counterpart.** Go decides at runtime whether the model produces `T`
  and fails with *"operation type not supported"*; that is a Go limitation, not a design, and in Rust the mismatch
  does not compile. The error that refusal produced is not ported, because nothing can reach it.
- **`Executed<T>` carries a provider report; Go's `Execute` returns a bare `T`.** Additive — the same faces, plus the
  answer to "did this run get the GPU it asked for?" that the image path already gives.
- **Sub-pixel coordinate drift is expected.** The reference letterboxes with `disintegration/imaging`'s Lanczos; this
  uses the `image` crate's `Lanczos3`. Different kernels, so boxes will differ in the third decimal — which is
  precisely the axis the reference's own FP16/FP32 comparison says distinguishes two detector runs. A mismatch there
  is the resampler, not a defect.

### Infrastructure: what `rust-sak` already covers

**Nothing this change needs, and nothing it needs is missing from it.** `rust-sak`'s `image` module is encode and
decode across eight formats; it does not resample, and should not — resampling is not codec work. The letterbox resize
is `image::imageops::resize` from the `image` crate this crate already depends on. Everything else here — the anchor
grid, the variance decode, non-maximum suppression, the coordinate rescale — is model arithmetic specific to
RetinaFace, general-purpose to nothing, and belongs at model tier under `models/detection/newyork/`. **No new
third-party crate is introduced.**

### Task estimate

**15**, which is the hard limit exactly and leaves no slack. Counted before anything below was written: the trait
split, the extracted helper, the multi-output session call and its four doubles, the execute driver and its public
surface, the refusal correction, preprocessing, anchors, decoding, filtering, the pipeline seam, the provider profile,
two test tasks, and the toolchain pass. If any one of them grows in design, the change splits rather than compresses.

## Capabilities

### New Capabilities

- `data-inference`: running a single operation whose result is not an image — how a caller names the result type, what
  a run reports, what it stops on, what it hands back beside the result, and why nothing it produces is cached.
- `face-detection`: what a detection run does to an image — the fixed-size letterbox it feeds the detector, the
  confidence threshold and overlap suppression it applies, and the coordinate space the faces come back in.

### Modified Capabilities

- `image-enhancement`: the refusal for an operation with no image pipeline splits in two. A family whose pipeline is
  not yet written and a family whose result is not an image are different facts, and after this change detection is
  permanently the second one rather than temporarily the first.
- `inference-tiling`: the session seam gains a third call, for a graph that returns several output tensors addressed
  by name — the sibling of the existing requirement that a model may be fed a tensor whose shape is declared rather
  than inferred.
- `execution-providers`: New York's measured settings join Osaka's as a pinned requirement, including the one that is
  most likely to be broken by tidying — that the NHWC flag is on at **both** precisions here, the opposite of the
  answer next door in Athens.

## Impact

**Crates: `opai`, plus one line in `perf`.** Nothing in `gui` or `cli` calls `process` today and nothing will call
`execute` in this change; both front ends reach inference in their own later changes, and everything here is library
logic both would need.

`perf` gains `#![recursion_limit = "256"]` and nothing else. Lifting the acquisition out of `chain::run` into an
async helper nests `Opai::process`'s future one level deeper, which overflows the default limit when rustc computes
the layout of `perftest`'s own `sweep` future — a compile error in that crate about a compile-time recursion, not
about its code, and it costs nothing at runtime. It is the compiler's own prescribed fix.

Affected code:

- `crates/opai/src/lib.rs` — `Opai::execute`, and the `Backend` impl gains the multi-output method.
- `crates/opai/src/inference/pipeline.rs` — the trait split.
- `crates/opai/src/inference/chain.rs` — loses the extracted acquisition block; its behaviour is unchanged and its
  existing tests are expected to pass unedited.
- `crates/opai/src/inference/execute.rs` — new, the single-operation driver.
- `crates/opai/src/inference/session.rs` — `run_named_outputs`.
- `crates/opai/src/models/operation.rs` — `DataOperation`, and the corrected refusal arm.
- `crates/opai/src/models/detection/` — `newyork/` and the `DataOperation` impl.
- `crates/opai/src/error.rs` — one `UnsupportedReason` arm.
- `crates/opai/tests/public_api.rs` — the new public surface is pinned the way `process` already is.
- `crates/perf/src/main.rs` — the one-line recursion limit above.

**Public API: additive.** `Opai::process`, `ProcessOptions` and `Enhanced` are untouched. `UnsupportedReason` is
already `#[non_exhaustive]`, so the new arm is not a break.

**The riskiest edit is the extraction from `chain::run`.** That loop holds the guarantee that a session a run holds
cannot be reclaimed, and the cache rewind that walks backwards over a prefix that would not decode. It is a pure
lift with no behaviour change, and the design states how it is verified as one.
