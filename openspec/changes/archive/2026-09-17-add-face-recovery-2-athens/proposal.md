## Why

Athens can be named, parameterised, catalogued and cached, and it cannot run. Slice 1 closed everything around the
model — `Fidelity`, the per-variant constructor, the fidelity segment in the run cache tag, the catalogue's per-variant
parameters — and deliberately built no pipeline. This is the slice that builds it, and it is the point at which the
faces `Opai::execute` already returns become an enhanced picture rather than a set of coordinates with nothing to
consume them.

Two things make it the most structurally novel model in the catalogue so far, and they are why it is a slice of its
own. **Athens' graph takes two inputs** — the aligned face and the fidelity weight — where every model this crate has
ever run takes one. And **its unit of work is not a region of the image**: it is a 512-pixel square that exists only
after a similarity transform maps five landmarks onto a canonical template, so none of the tile grid, the padding rule
or the two blending strategies in `inference/` applies to it.

## What Changes

**A fourth session call, for a graph fed a value beside its image.**

- `session::run_weighted` and `Backend::run_weighted`: one image tensor of a declared shape, one `f32` bound as the
  graph's second input, one output read back at a declared shape. `run_tile`, `run_graph` and `run_named_outputs`
  are untouched, and the fourth follows the same rule the other three do — three contracts, three functions, and now
  four. The inputs stay **positional**, unlike `run_named_outputs`' outputs: the two differ in rank and extent
  (`[1,3,512,512]` against `[1]`), so an export that swapped them fails at the runtime rather than producing a
  plausible wrong answer, which is the whole of the argument that made the outputs need names.

**Athens runs.**

- `models/face_recovery/athens/`: the ArcFace landmark template and the least-squares similarity transform fitted to
  it, the affine warp that reads the source image straight into the graph's `[1,3,512,512]` tensor, the fidelity
  weight bound as the second input, the feathered elliptical blend mask, and the composite back into the frame
  through the forward transform over the face's bounding box plus a half-extent margin.
- Per face: two progress steps and three cancellation checks, over the whole `0.0..=1.0` of the operation.
- `FaceRecovery::pipeline`, the family's contract seam, and the `Operation::pipeline` arm that reaches it. Face
  recovery leaves the six-family refusal arm.

**Athens' measured provider tuning**, currently `EpProfile::default()` for both variants — the material slice 1's D10
deferred to this slice on the grounds that a session first has to exist. CoreML restricted to the CPU and GPU at
**both** precisions (worth 1.57x on an M2 Max at FP16; CodeFormer's 72 GroupNorms and several hundred transposes are
what the Neural Engine thrashes on), sequential execution (-20.6% at FP32 and -13.2% on the graph under CUDA), and
the CUDA NHWC layout at **FP16 only** (-7.7% at FP16, +5.9% at FP32 on an RTX 5090). Santorini keeps the defaults;
its measurement is slice 3's.

**The benchmark harness stops timing a no-op.** `perf` builds every face-recovery row over `Faces::empty()` today,
which is a refusal while Athens cannot run and would become a sub-millisecond "success" the moment it can. It gains
the reference harness's own answer: one detection pass before the sweep, at the sweep's precision, whose faces feed
every face-recovery row and whose cost is charged to none of them. An image with no face fails those rows with that
reason rather than measuring nothing.

### Deliberate divergence from the reference

- **No 20% progress head for a detection this operation does not run.** `RestoreFaces` reserves `progressAfterDetect
  = 0.2` because in the reference face recovery acquires the detector itself. Here detection is a separate operation
  the caller has already run, so the whole range belongs to the faces.
- **The result is RGB at the run's requested depth, including over an empty selection.** The reference returns the
  input image untouched when there is no face, and 8-bit NRGBA otherwise. Every pipeline in this crate returns RGB at
  the resolved `ChannelDepth`, with alpha composited against black on the way in, and face recovery is not made the
  exception: a single-operation chain would otherwise hand back 8-bit pixels to a caller that asked for 16. It costs
  one full-image conversion for a run that composites nothing.
- **The warp reads the source at full precision straight into the float tensor.** The reference warps into an 8-bit
  NRGBA image and converts that to CHW, so a 16-bit source is quantised twice before the model sees it. Sampling
  bilinearly out of `Sampler` into the tensor in one pass removes the intermediate image and the quantisation; the
  arithmetic is otherwise the reference's, reflection padding included.
- **A read more than a full axis outside the picture is clamped rather than reflected a second time.**
  `reflectCoord` applies its two corrections in sequence, so a coordinate that is still outside after the first one
  is folded again — and for a coordinate past the far edge by more than an axis it returns a **negative index**,
  which is an out-of-range read rather than a reflection. This port mirrors once and clamps the result into the
  picture, so the degenerate case is a border pixel instead of an index no buffer has. Inside one axis — every
  coordinate a fitted alignment produces — the two are identical, which is what the reflection test pins.
- **The blend mask is blurred by a separable Gaussian written here**, not by `disintegration/imaging`'s. The mask is
  an `f32` alpha plane rather than an NRGBA image, so there is nothing to round-trip through 8 bits; the two kernels
  differ by fractions of a percent at the feathered rim, which is the same class of difference as New York's
  resampler.
- **Santorini's refusal keeps saying the family's pipeline is not written yet**, for the one slice between this
  change and the next. It is inaccurate at the family's granularity and it adds nothing to delete: a new public
  `UnsupportedReason` arm describing a variant gap would be surface whose subject stops existing when slice 3 lands.

### Infrastructure: what `rust-sak` already covers

**Nothing here, and nothing here is missing from it.** Its `image` module is encode and decode across eight formats;
it does not resample, warp, blur or composite, and should not — none of that is codec work. `crypto`, `fetch`, `fs`,
`memo` and `sysinfo` have no bearing on a similarity transform. Everything this change writes is either ONNX Runtime
plumbing, which belongs in the one file that names `ort`, or CodeFormer's own geometry, which is general-purpose to
nothing and lives with the model. **No new third-party crate**, and none was weighed: the warp, the mask and the
blend are a few hundred lines of arithmetic over buffers this crate already owns, and `imageproc` — the obvious
candidate — would be a dependency for one function whose edge-padding and interpolation rules would then have to be
checked against the reference's anyway.

### Task estimate

**12.** The session call and its six test doubles, the transform, the warp, the mask, the composite, the pipeline,
the contract seam, the provider profile, the pipeline's tests, two in `perf`, and the toolchain pass.

## Capabilities

### New Capabilities

- `face-recovery`: what a face-recovery run does to an image — the faces it takes from the caller rather than
  detecting, the canonical alignment each one is restored at, how closely the result is held to the original face,
  how it is feathered back into the frame, what an empty selection produces, and what the run reports and stops on.

### Modified Capabilities

- `inference-tiling`: the session seam gains a fourth call, for a graph fed a **second input that is a value rather
  than an image** — the sibling of the existing requirements for a declared input shape and for several named
  outputs.
- `image-enhancement`: the deferral half of the refusal is stated per operation rather than per family. A family can
  now run one of its variants and not the other, which the current requirement's wording does not admit even though
  the fact a caller acts on — *this will run in a later version* — is unchanged.
- `execution-providers`: Athens' measured settings join New York's and Osaka's as a pinned requirement, including the
  one already named as the contrast in New York's — that the NHWC layout is declared at FP16 **only** here.
- `performance-benchmark`: a model whose input is another model's output is given it once, before the sweep, and is
  never timed producing it.

## Impact

**Crates: `opai` and `perf`.** No `gui`, no `cli`: neither front end reaches inference yet, and every line here is
library logic both would need.

- `crates/opai/src/inference/session.rs` — `run_weighted`, over the existing `accept` checker.
- `crates/opai/src/inference/process.rs` — the `Backend` method, plus its two fakes.
- `crates/opai/src/inference/tensor.rs` — `Normalisation`'s range mapping reachable for a **fractional** channel
  value and for a float coming back, so the warp and the composite share the one definition of `[-1, 1]` rather than
  each carrying a copy that can disagree with it. Behaviour is unchanged: `encode` is the same function over the
  same arithmetic, split so that `encode_unit` can be reached without a `u16` to round to.
- `crates/opai/src/lib.rs` — the production implementation.
- `crates/opai/src/inference/test_support.rs`, `inference/execute.rs`, `models/upscale/osaka/{mod,region}.rs`,
  `models/detection/newyork/mod.rs` — the four remaining test doubles.
- `crates/opai/src/models/face_recovery/athens/` — new: the template and transform, the warp, the mask, the
  composite, and the pipeline that sequences them.
- `crates/opai/src/models/face_recovery/mod.rs` — the family's contract seam, built from `FaceRecoveryParams`.
- `crates/opai/src/models/face_recovery/variant.rs` — Athens' profile replaces `EpProfile::default()`.
- `crates/opai/src/models/operation.rs` — face recovery leaves the six-family refusal arm.
- `crates/perf/src/{sweep,select,report}.rs` — the detection pass and the faces it supplies.

**Public API: additive, and small.** `Opai::process` gains an operation it can serve; no signature changes, no type
is removed, and `UnsupportedReason` gains no arm. Everything the pipeline is built from is `pub(crate)`.

**The riskiest part is the geometry.** A similarity transform fitted to the wrong landmark order, an inverse applied
in the forward direction, or a mask sampled in the wrong space all produce a plausible image with a face in the wrong
place rather than an error — so each of the four pieces is tested on its own arithmetic before the pipeline composes
them, and the composition is tested against a fake backend with no runtime at all.
