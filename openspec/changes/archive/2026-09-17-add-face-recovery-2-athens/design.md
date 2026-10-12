## Context

See proposal.md — Why, and `specs/face-recovery/spec.md` for the requirements. What follows is the state of the code
this design has to work with, and the four places the reference implementation's material does not transfer
unchanged.

**The seams are all in place and none of them fits.** `inference/` holds the tile grid, the padding rule, the two
blending strategies and the three session calls, and a face-recovery run uses **none** of them: its unit of work is a
512-pixel square produced by an affine transform, not a tile of a grid; its edge handling is reflection inside a warp,
not a mirrored pad of a short region; and its combination step is a feathered alpha composite in the aligned square's
own space, not a ramped overlap between neighbouring tiles. The one seam it does need — feeding a graph — stops one
input short.

**What slice 1 left ready.** `FaceRecovery` carries its variant, its faces and an `Option<Fidelity>`; `params()`
answers `FaceRecoveryParams { faces, fidelity }` and is the value a pipeline is constructed from (slice 1, D6);
`cache_tag` already folds both the selection and the fidelity in, so nothing about caching is this slice's work.
`Face` carries five landmarks in the order `models/face.rs` documents — left eye, right eye, nose, left mouth corner,
right mouth corner — which is the order the ArcFace template is written in, and quantised to two decimals on
acceptance.

**What the reference carries, read against rather than assumed.** `models/facerecovery/inference.go` (the per-face
loop, the two-input run, the progress schedule), `face.go` (`alignFace`, `createCircularMask`, `blendFaceInto`,
`calculateSimilarityTransform`), `transform.go` (`warpAffine`, `invertAffine`, `bilinearInterpolate`,
`reflectCoord`), `athens/athens.go` (the row and the whole of the measured provider prose),
`detection/types.go` (`ArcfaceTemplate` at `ArcfaceTemplateSize = 512`), and `cmd/perf/{models,bench,main}.go`
(`detectFacesOnce`, `faceEntry`, `detectFaces`).

**Four things do not transfer unchanged**, and the decisions below are mostly about them: the reference reserves a
fifth of its progress bar for a detection this operation does not run; it works in 8-bit NRGBA throughout where this
crate works in 16-bit samples and a caller-chosen output depth; it rebuilds nothing per run because a package-level
`sync.Once` holds the mask; and its `Fidelity: -1` sentinel is already an `Option` here.

## Goals / Non-Goals

**Goals:**

- One model that runs, end to end, with every piece of its geometry testable on its own arithmetic and the whole of
  it testable with no ONNX Runtime, no GPU and no model file.
- A session seam that grows by exactly one call, in the one file allowed to name `ort`, with the three existing calls
  and every test double untouched in behaviour.
- Nothing promoted, moved or generalised ahead of its second caller — and nothing written in a way that makes the
  promotion slice 3 has to do harder than a file move.
- A benchmark harness that measures Athens rather than measuring the absence of faces.

**Non-Goals:**

- **Santorini.** Its graph, its pipeline, its measured profile and the promotion of this slice's geometry to family
  tier are slice 3, in that order. Nothing here is written to *exclude* a second caller — the warp, the mask, the
  composite and the transform all take their sizes and their inputs as parameters — but nothing is promoted on
  speculation either.
- **Tiling a face.** The aligned square is the model's whole input at one fixed size; there is no grid, no overlap
  and no seam between regions inside it.
- **Autopilot, crop, the export queue, per-image face selection UI, and any `gui` or `cli` work.** Neither front end
  reaches inference yet.
- **Re-measuring the provider profile.** The figures are the reference's, transcribed from `athens.go` with its
  experimental conditions intact. What this slice validates is that the settings are *declared*, not that the
  percentages reproduce on other hardware.
- **Widening the run cache, or touching the cache tag.** Slice 1 finished that.

## Slices

The face recovery series: making Athens and Santorini expressible, then running them.

| # | Slice | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-face-recovery-1-params` | `Fidelity`; the twenty per-variant constructors; the `Analysis` carrier; full-value operation equality with quantized faces; Athens' fidelity in the run cache tag; the catalogue's per-variant parameters and non-numeric kinds; `params()` consumed at the pipeline-construction seam | `add-models-3-faces`, `add-detection-newyork` (both done) | **done** (archived `2026-09-17-add-face-recovery-1-params`) |
| 2 | `add-face-recovery-2-athens` | Athens' pipeline: the ArcFace landmark template and similarity transform, the 512px aligned tile, the feathered elliptical blend mask, the composite back into the frame, and the fidelity weight bound as a second input tensor | 1 | **this change** |
| 3 | `add-face-recovery-3-santorini` | Santorini's pipeline: the same alignment, tile, mask and blend through the family-tier code slice 2 promotes, over a graph that takes the image alone | 2 | pending |

Slice 3 is where the geometry below moves from `models/face_recovery/athens/` to family tier, because that is the
change that introduces its second caller, and the project's rule is that the promotion happens **in** that change and
**before** the caller is written. D9 names the four files that move and what has to be true of them for the move to
be a move rather than a rewrite.

## Decisions

### D1. A fourth session call, taking the second input as a value

`session::run_weighted(handle, input, input_shape, weight, output, output_shape)` and the matching
`Backend::run_weighted`, with `weight: f32`. It reuses the existing `accept` checker for the output, so the two
failures a declared-shape run already has — a length disagreement and no tensor at all — are unchanged and stay
exercisable with no runtime.

**A fourth function rather than a generalisation of `run_graph`.** That file's own header sets the rule: three
contracts, three functions, because a shape recovered from a buffer is a guarantee the compiler keeps and a shape
passed as a parameter is a guarantee a caller can get wrong. Widening `run_graph` to `&[&[f32]]` inputs would give
every existing caller a slice to build for one tensor and a way to pass two by mistake, and would make the weight a
one-element buffer whose meaning is positional in a list rather than named in the signature. Taking it as an `f32`
says what it is.

**The inputs stay positional, and this is the opposite call from `run_named_outputs`.** That function spells names
because three output tensors of comparable shape are interchangeable to the runtime and a reordered export would be
read wrongly with nothing able to notice. Here the two inputs are `[1,3,512,512]` and `[1]`: an export that swapped
them fails at the runtime with a shape error, loudly and on the first run. Naming them would buy nothing and would
make the call fail on a graph that named its inputs something other than what this crate spelled — which is the
argument `run_tile` and `run_graph` already make.

*Cost, stated plainly:* the `Backend` trait gains a method, so all six test doubles gain one. Five of them can
`unimplemented!()` it — they double models that take one input — and Athens' own fake implements it. That is the same
shape the multi-output call took in `add-detection-newyork` and it is why the doubles are listed as one task rather
than six.

### D2. The alignment samples the photograph straight into the graph's tensor

`transform.rs` computes the similarity transform from the face's five landmarks onto the ArcFace template scaled to
the tile size, and `warp.rs` walks the 512x512 destination, maps each pixel through the **inverse** transform, and
writes the bilinear sample into the planar CHW `f32` tensor under `Normalisation::Signed`. There is no intermediate
image.

The reference warps into an 8-bit NRGBA image and then converts that to CHW, which quantises twice: a 16-bit source
is narrowed to 8 bits by the warp and then widened back to a float. Fusing the two loops removes an allocation per
face and a quantisation step, and it is the same arithmetic otherwise — `Sampler` is already this crate's answer to
"read a pixel of any `DynamicImage` variant as 16-bit RGB with alpha composited against black", and bilinear
interpolation over four of its reads is what `bilinearInterpolate` does over four of Go's.

**Reflection at the edges, not clamping.** `warpAffine` passes `reflect = true`; `blendFaceInto` passes `false`. Both
are ported as they are: a face at the border is aligned against a mirrored continuation of the picture rather than a
smeared edge pixel, while the composite's reads are inside the aligned square by construction and clamp only against
rounding.

**The transform is fitted and applied in `f32`,** as the reference's is. Float addition is not associative, and
`warpAffine`'s own comment records that regrouping the per-row terms out of the inner loop can shift a result by one
ulp; the port keeps the expression shape rather than hoisting, so that a pixel-level comparison against the reference
stays meaningful. `f64` would be defensible on its own merits and would make every such comparison approximate.

*Alternative considered:* warping into a `DynamicImage` and reusing `padded_to_chw`. It reuses more code and produces
a value that is easy to dump to disk while debugging, at the price of the double quantisation above and a 512x512
allocation per face. Rejected on the quantisation; the debugging value is available from the tensor with three lines
in a test.

### D3. The mask is an `f32` alpha plane, built once for the process

`mask.rs` builds the feathered ellipse as `Vec<f32>` of `TILE * TILE` alpha values — full opacity inside 0.7 of the
half-extent, falling linearly to zero at 1.0, then blurred by a separable Gaussian at σ = 15 over a radius of 3σ —
and holds it in a `LazyLock`, which is the direct analogue of the reference's package-level `sync.Once`. It is 1 MB,
immutable, and outlives any run; building it costs about 50 ms of separable convolution and rebuilding it per face
would put that inside every measurement this model is ever benchmarked on.

**Alpha as float, not as an image.** The reference builds an NRGBA mask because Go's blur takes an image; blurring a
float plane is both simpler and more accurate, and the composite reads a weight rather than a colour, so there is
nothing an image gives it. It also removes the round trip through 8-bit alpha that would otherwise quantise the
feathered rim to 256 levels — which is precisely the region the mask exists to make smooth.

**A separable Gaussian written here, not `image::imageops::blur`.** That function takes an `ImageBuffer` of a pixel
type, so using it means materialising the mask as an image again purely to blur it. The kernel is a dozen lines over
a float plane, it is exercised directly by a test, and it makes the feathering independent of a dependency's choice
of approximation. This is a deliberate, sub-percent divergence from `disintegration/imaging`'s kernel at the rim, in
the same class as New York's Lanczos difference.

*Alternative considered:* a closed-form smoothstep falloff with no blur at all, which would need no convolution and
no cached plane. Rejected because the blur is not decoration: it is what turns the ellipse's linear ramp into the
falloff the reference's output was tuned against, and replacing it with an analytic curve changes every composited
pixel by an amount nobody has measured.

### D4. The composite reads and writes one buffer, in the aligned square's space

`composite.rs` takes the destination buffer, the restored tensor, the mask, the forward transform and the face's
bounding box. It walks the box expanded by `0.5 * max(width, height)` and clipped to the image, maps each destination
pixel **forward** into the aligned square, skips it if it lands outside, samples the mask there, skips it if the
weight is below 0.001, samples the restored tensor there, and writes the weighted mix of the restoration and what is
already in the buffer.

**Forward-mapping the destination rather than warping the tile back.** It is the reference's own optimisation and its
reasoning holds here: the mask lives in the aligned square's coordinates, which is exactly where the forward
transform lands, so neither the mask nor the restoration has to be pre-warped across the whole frame.

**One buffer, read and written in the same pass.** Each destination pixel is read as background and written in the
same iteration, so a second buffer would buy nothing, and reading the destination is what makes two overlapping faces
composite in the order the operation carries them — the reference's behaviour, and the reason a run over one
selection is deterministic.

**The restored tensor is sampled as floats.** It comes back as planar CHW in `[-1, 1]`; decoding it to an image and
then sampling that image would quantise the restoration to the output depth before the blend instead of after.
Sampling the tensor bilinearly and quantising once, at the write, is the same choice `canvas.rs` already makes for
the diffusion upscaler and for the same reason.

### D5. The result is RGB at the resolved depth, uniformly

The pipeline dispatches on `ChannelDepth` into a generic body — the `run_tiled` idiom, `u8` or `u16` — reads the
source through `Sampler` into an `ImageBuffer<Rgb<T>>`, composites into it, and returns `T::into_dynamic`. **This
happens whether or not there is a face**, which is the empty-selection behaviour the spec pins.

The reference returns its input untouched when the selection is empty and 8-bit NRGBA otherwise. Carrying that over
would make face recovery the one operation whose output form depends on the photograph's contents: a caller asking
for sixteen bits would get eight from a chain of one Athens over an image with no face in it, and would get whatever
variant the decoder happened to produce — greyscale, alpha, 32-bit float — rather than the RGB every other operation
returns. It costs one full-image conversion for a run that composites nothing, which is the price of the operation
being uniform with the other seven.

A zero-area image is refused before anything is allocated, reusing `InferenceError::Untileable` rather than adding an
arm. Its name is tiling-flavoured and this pipeline does not tile, but it is the crate's existing "this image has no
area" refusal, its message names the dimensions, and a second public error arm saying the same thing in other words
is surface nobody benefits from.

### D6. Progress is the faces' to divide, and none of it is reserved

Two steps per face over `0.0..=1.0`: one when its restoration returns, one when its composite lands. Cancellation is
checked before each face, between its two steps, and after the composite — the reference's schedule, and the only one
a per-face loop offers.

**No 0.2 head for detection.** `progressAfterDetect` exists because `facerecovery` acquires the detector itself in
the reference; here detection is a separate operation with its own report, already finished before this one starts.
Keeping the constant would leave the bar sitting at 20% before the first face and would make an empty selection report
0.2 and stop.

An empty selection reports `1.0` and returns. `stages()` answers the number of faces, which is what the driver's
per-step `debug` record calls a model run — one graph run per face, and none for an empty selection.

### D7. An operation carrying no fidelity restores at maximum

`FaceRecoveryParams.fidelity` is an `Option`, and `FaceRecovery::athens` always fills it — but the field is
`#[serde(default)]`, so a deserialized Athens operation can reach the pipeline with `None`. The pipeline binds
`fidelity.unwrap_or(Fidelity::MAX)`.

`Fidelity::MAX` is the value the reference hard-codes for every Athens run, so an operation that names none runs
exactly as the reference does, and a selection persisted before the control existed keeps working. The alternatives
were refusing it — a new public refusal for a state a caller cannot see the cause of — and binding zero, which is the
*most* aggressive restoration and the worst possible default for a value that went missing. Its cache tag carries no
`-w` segment, so it keys separately from an explicit `1.0`; that is a duplicate entry, not a wrong one, and adding a
segment for a value the operation does not carry would be the cache lying about the operation.

### D8. Santorini keeps the refusal it has

`FaceRecovery::pipeline` is the family's contract seam: an Athens arm reaching `athens::pipeline`, and a Santorini arm
returning `InferenceError::Unsupported` with the existing reason — the same one the six unwritten families return.
`Operation::pipeline` stops refusing the family outright and hands over to the seam.

That reason's text says the *family's* pipeline is not written, which is inaccurate for exactly one slice. The
alternative is a new public `UnsupportedReason` arm distinguishing a variant gap from a family gap, and this project's
rule is not to publish API whose subject is a state it is working out of: the arm would exist to be deleted in slice
3, and something would depend on it in the meantime. The spec delta resolves the inaccuracy the other way, by stating
the deferral per operation rather than per family, which is the fact a caller acts on and is true in both cases.

**What the seam looks like in slice 3.** Both variants align, tile, mask and composite identically; the only
difference is whether a weight is bound. That is one contract with an `Option<Fidelity>`, not two, so the expected
end state is a single arm over both variants and a match on the option at the point the tensor is built — which is
where the reference's `if fidelity >= 0` branch already sits. This slice writes the Athens arm so that becoming the
shared arm is a widening rather than a rewrite.

### D9. Everything is model tier, and slice 3 moves four files

`models/face_recovery/athens/` gets `mod.rs` (the pipeline, the tile size, the graph's normalisation, and the
measured profile with its prose), `transform.rs`, `warp.rs`, `mask.rs` and `composite.rs`. Nothing goes in
`inference/`, and nothing goes directly under `models/face_recovery/`.

The project's rule is that a file's tier is decided by its callers **today**, and today every one of these has
exactly one. Putting the warp in `inference/` on the grounds that an affine warp is general-purpose would be a
promotion ahead of its second caller, and `inference/mod.rs` states what that module holds: what no model and no
family owns.

**They are written model-agnostically from the first line**, so that slice 3's promotion is a file move plus an
import change: `transform.rs`, `warp.rs` and `composite.rs` take the tile size as a parameter and name no variant;
`mask.rs` is a function of the tile size with the `LazyLock` at 512 beside it. They join Osaka's five files as known
promotion candidates parked at model tier, and `models/mod.rs`'s list of those is extended rather than a second list
started.

The profile lives with the model — `athens/mod.rs` — because the doctrine says a model's measured EP profile and the
prose behind it are the model's own; `FaceRecoveryVariant::profile` becomes a match reaching it. Detection's profile
sits in its family's `variant.rs` only because that family has one model.

### D10. Athens' measured provider profile

Transcribed from `athens/athens.go`, whose comment carries both the numbers and the conditions they were taken under.
`CoreMlComputeUnits::CpuAndGpu` at **both** precisions, `ExecutionMode::Sequential`, and `cuda_prefer_nhwc` at
**FP16 only**. See `specs/execution-providers/spec.md` for what each is worth and why.

Two figures in this project already quote it secondhand and **agree with the source rather than contradicting it**,
which was checked rather than assumed: `providers/profile.rs` and New York's spec requirement say NHWC is worth -4%
at FP16 and +8% at FP32, which are the reference's **end-to-end** figures from `internal/utils/ep_profile.go`, while
`athens.go`'s -7.7% and +5.9% are the same flag measured on the **graph alone** on an RTX 5090. Both pairs go into the
requirement, each labelled, so that the next reader does not have to repeat the check.

**Athens is the one model whose execution mode is set against its own CoreML measurement** — sequential leans 0.1-0.3%
the other way there, where Tokyo, Santorini and New York each gain 3.5-6.5%. The mode is a session setting rather than
a per-provider one, so there is one answer to give; the prose records the exception so that a later reader measuring
CoreML alone does not "fix" it.

### D11. The benchmark harness detects once, before the sweep

`perf` resolves its selection before the image is loaded and before `Opai` exists, so the faces cannot be supplied at
selection time. The sweep instead makes one detection pass after the picture is loaded and before the first model is
measured — `Detection::newyork` at the sweep's own precision through `Opai::execute` — and rebuilds each selected
face-recovery row over the faces it found, through `FaceRecovery::athens(variant precision, faces, fidelity)` with the
values the row already carries. That is the reference harness's `detectFacesOnce`/`faceEntry` shape, and the rebuild
uses only the public API slice 1 published.

- **At the sweep's precision, not pinned.** A run that restores at FP16 detects at FP16, which is what the
  application does; pinning would benchmark a tier combination it never produces.
- **Once, not per model.** Every model boundary drains the session cache, so a per-model detection would pay a full
  detection session build inside each face-recovery row — untimed but real wall clock, and on TensorRT larger than
  everything else in the sweep.
- **Its failure, or a photograph with no face in it, fails those rows** with that reason rather than measuring an
  operation that composites nothing. The error is kept rather than discarded, so the row says why.
- **Only when a face-recovery row is selected**, so a sweep of upscalers neither runs it nor can be failed by it.
- **Listing and selecting stay offline**, which is an existing requirement of that capability: the detection pass
  happens after selection resolves, not inside it.

### D12. Crates, dependencies and `rust-sak`

**`opai` and `perf`; no new crate of any kind.** `rust-sak` has nothing that applies here and nothing missing: its
`image` module is encode and decode, which is not resampling, warping, blurring or compositing, and extending it with
an affine warp would be putting one model's geometry into a general-purpose library on the strength of the word
"affine". `crypto`, `fetch`, `fs`, `memo` and `sysinfo` have no bearing on any of this. `imageproc` was weighed as the
one plausible third-party answer and rejected: it would be a dependency for one function whose padding rule,
interpolation and edge behaviour would each have to be checked against the reference's anyway, which is most of the
work of writing it.

`gui` and `cli` are untouched. Neither reaches inference yet, and nothing here is front-end logic: a front end's share
of face recovery is drawing the face overlay and the fidelity control, which is a later change and consumes the
catalogue slice 1 already published.

### D13. Sessions, lifetimes, platforms and providers

**One session, acquired by the driver, borrowed by the run.** Athens' `Model::sessions` answers one `(artifact,
profile)` pair — the single-graph shape of every family but upscale — and `pipeline::acquire` opens it before any of
the operation runs. The pipeline receives `&[SessionHandle<B::Session>]` and cannot retain, drop or reorder them, so
"a session a run holds cannot be reclaimed" stays a property of the chain driver's own reference counts. The per-face
loop takes the session lock **per graph run**, not across the whole operation, which is what `run_tile`'s comment
defends and what lets a second image in an export queue interleave at face granularity.

**Nothing else here owns a native resource.** The mask is immutable process-scoped data with no `Drop`; `Faces` is an
`Arc` slice; the transform is 6 floats. Memory per face is the tensor in, the tensor out and nothing else — 3 MB at
512x512 — held for one face at a time rather than for the selection.

**Platforms and providers.** The geometry is platform-independent arithmetic and is exercised on every CI platform
with no runtime, no GPU and no model file, through a fake backend, exactly as New York's decode is. The provider
profile is **transcribed, not measured**: the settings were measured by the reference on an M2 Max (CoreML) and an
RTX 5090 (CUDA), and this change validates that they are declared rather than that they reproduce. **No execution
provider was exercised against this model in this project**, and TensorRT in particular has no figure for this graph
in either implementation beyond the execution mode being a tie.

## Risks / Trade-offs

- **A transform fitted to the wrong landmark order, or an inverse applied forwards, produces a plausible image with
  the face in the wrong place rather than an error** → Each piece is tested on its own arithmetic before the pipeline
  composes them: the template's five points are pinned as literals, the fit is checked against a known rotation and
  scale, the inverse is checked by round-tripping points, and the warp is checked by aligning a synthetic face whose
  landmarks are already at the template's positions — which must come back as the identity crop.
- **The blend samples the mask in the wrong space and feathers the wrong region** → The composite is tested with a
  restoration of one flat colour and a mask of known shape, so the written region and its falloff are assertable
  without a model; and with an identity transform, where the written window must be exactly the bounding box plus its
  margin, clipped.
- **The Gaussian written here differs from the reference's at the rim** → Accepted and stated; it is sub-percent on
  the alpha of a feathered edge, in the same class as New York's resampler difference, and the mask is a float plane
  where the reference's is quantised to 8-bit alpha, so ours is the more accurate of the two.
- **Six test doubles gain a method they will never call** → Five `unimplemented!()` with a reason, which is what they
  already do for calls their model does not make. The alternative — a default method on the trait — would let a real
  backend forget to implement it.
- **Athens is measurable in `perf` for the first time, and its numbers will not match the reference's** → Expected:
  different warp precision, a different blur kernel, and a composite that quantises once rather than twice. The
  percentages in the profile are the reference's own and are labelled as transcribed; anyone quoting them from this
  project's hardware must re-measure, which the prose says.
- **`perf` grows a second inference call before its sweep, so a sweep can now fail before it measures anything** →
  Only for sweeps that selected a face-recovery row, and that failure is the honest outcome: the alternative is the
  sub-millisecond success this slice exists to prevent. Every other row is unaffected.
- **A model that runs but is never compared against the reference's output** → The largest gap in this slice and it
  is inherent: no test can assert a restoration is correct without the weights and a reference image. What is
  asserted is every step around the graph, on synthetic data, plus the `-w` segment reaching the run. A visual check
  against the Go application on one photograph is a task, not a test.

## Migration Plan

Nothing persisted is invalidated. Athens' cache tag was finished in slice 1 and no Athens run has ever executed, so
there is no stored result to miss and nothing on a user's disk to sweep. The serialized form of an operation is
untouched.

The one behavioural change visible to an existing caller is that `Opai::process` now serves an operation it used to
refuse — additive, and the refusal it replaces was `Unsupported`, which no caller can have been depending on.

## Open Questions

- **Whether Santorini shares Athens' contract arm or gets its own.** D8 records the expected answer — one arm with an
  `Option<Fidelity>` — but it is slice 3's to settle against the real graph, since it is the slice that first sees
  whether the two agree on anything beyond their input geometry. Nothing in this slice depends on the answer.
- **What a front end offers as the default fidelity, and how the control is labelled.** Carried forward from slice 1
  unchanged: `1.0` is the reference's value and the parity default, and whether a control presents it as "fidelity"
  at `1.0` or inverts it is answerable when `gui` first draws one.
