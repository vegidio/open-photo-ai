## Context

See proposal.md — Why. The identity layer for this family shipped with `add-models-2-enhancements` and
`refactor-models-family-layout`; what is absent is the pipeline.

Three facts about the surrounding code shape everything below.

**`inference/` already has every primitive this needs but one.** `tensor::padded_to_chw` is the whole-image,
reflection-padded CHW conversion this family's presentation step is, written for a caller that did not exist yet.
`Backend::run_graph` takes a declared input shape and a declared output shape, which is exactly a fixed-square graph.
`InferenceError::Untileable` is the zero-area refusal. The one gap is on the way back out: `chw_to_image` insists the
destination be the tensor's whole extent, and this family needs a sub-region of it.

**`face_recovery/restore.rs` is the structural precedent, not `run_tiled`.** It is the crate's one existing
`ImagePipeline` whose unit of work is not a region of a grid, and the shape it established — a family-tier contract
file, per-model directories holding only a measured profile and its prose, a one-element `required()`, a single
`(artifact, profile)` pair from `sessions()`, and a body monomorphised over `T: Channel` dispatched once at the top —
is the shape this family wants unchanged.

**The reference's `lightadjustment` package is two files of contract and two of registration**, and its two model
files are 90% prose explaining why the canvas is fixed and why it is 1024. That prose is the part of the port that is
hard to reconstruct and easy to lose, and carrying it across is a deliverable rather than a courtesy.

## Goals / Non-Goals

**Goals:**

- One contract in `models/light_adjustment/process.rs` that both variants run through unchanged.
- Presentation, correction and blending each testable on their own arithmetic, before the pipeline composes them.
- The whole pipeline exercisable against a fake backend on every CI platform — no ONNX Runtime, no GPU, no model file,
  no network — which is what `restore.rs` and `run_tiled` both already achieve.
- Both variants' measured provider settings, with the measurements re-confirmed rather than inherited on trust.

**Non-Goals:**

- **Any generalisation towards colour balance.** Its reference contract is a fixed canvas too, and then an 11-term
  polynomial fit and a weighted blend of several renderings — it shares the *presentation* step and nothing after it.
  Promoting the canvas plan on the strength of a family that has not been written is the speculative structure the
  tier rules exist to prevent. The promotion happens in the change that introduces the second caller, before that
  caller is written.
- **Reusing the bias-independent intermediate across runs.** See D9.
- **Any front-end work.** Neither `gui` nor `cli` draws a light adjustment control, and nothing here needs one.

## Slices

Not sliced. Fourteen tasks in one change, `opai` only. This is a single family's pipeline, and its two variants share
one contract exactly — the precedent is `add-upscale-tokyo-saitama` rather than the `add-face-recovery-1/2/3` series,
whose split existed because Athens and Santorini differ at the session call itself. The series this descends from —
`add-models-1/2/3`, then the `refactor-models-*` trio — is archived and finished.

## Decisions

### D1. The contract is a family-tier file; the canvas and the profile are model-tier

```
  models/light_adjustment/
      mod.rs        family    the operation's identity (exists) + pipeline<B>() seam
      variant.rs    family    the rows and the enum (exists) + canvas() + profile() matches
      process.rs    family    NEW: plan, present, run, correct, blend
      paris/mod.rs  model     NEW: CANVAS, profile(), the IAT prose, their tests
      lyon/mod.rs   model     NEW: CANVAS, profile(), the CIT-EC prose, their tests
```

Straight from the tier rule: `process.rs` has two callers in the same family, so it is family tier; a canvas size and a
measured profile have exactly one caller each, so they are model tier. Deleting Lyon is deleting its directory plus two
match arms.

**The canvas does not go on `SimpleVariant`.** That row type is shared by sixteen variants across five families, and
four of those families have no canvas. A field there would be a value the other four must supply and nobody reads. The
reference puts `Canvas` on its own per-family `Variant` struct for the same reason, and duplicates the struct between
`lightadjustment` and `colorbalance` rather than sharing one.

**Rejected:** a `CanvasVariant` row type mirroring `ConvVariant`. `ConvVariant` earns its existence because upscale
variants differ in what selects their weights; these two differ in nothing a row could express, so the row would carry
one field and a function pointer per model and buy nothing over a match.

### D2. The canvas plan rounds, and deliberately does not use `newyork::input::fit`

`models/detection/newyork/input.rs:138` already scales a photograph's longer side onto a square. It **truncates**,
because the reference's detection preprocessing truncates and the coordinate rescale that undoes it has to undo exactly
what it did. `utils.FitLongSide`, which light adjustment goes through, **rounds**.

They differ on most aspect ratios by a pixel, and a pixel here is a slightly different scale factor, a slightly
different set of pixels shown to the model, and a slightly different photograph out. No error, no diagnostic.

So this change writes its own, in `process.rs`, and the tier rule is not being broken: a `use` from
`light_adjustment/` into `detection/newyork/` would be an import across a model boundary, which is the thing that rule
names as a bug rather than a shortcut. There is no promotion to make, because the two are not the same function.

This is recorded here because it is exactly the kind of near-duplication a later reader will try to fold, and the
comment beside the copy has to be enough to stop them.

### D3. The decode gains a cropped sibling in `inference/tensor.rs`

The model's output is `3 * 1024 * 1024` floats; what is wanted is the top-left `sw x sh` of it, **before** the upsample
— cropping afterwards would mean upsampling the reflected extension and then discarding a region whose aspect no longer
matches anything.

`tensor.rs` already has this exact shape on the input side: `image_to_chw` and `padded_to_chw` are two public contracts
over one private `extend_to_chw`, split because a tile path with a width and a height it has no use for is one more way
for a caller to disagree with the grid. The decode side gains the same treatment — `chw_to_image` unchanged, plus a
sibling that reads a sub-extent at the tensor's stride, both over one body.

**Rejected:** decoding the full square into a 1024-square buffer and cropping with `imageops::crop_imm`. It is three
lines instead of twenty and costs one extra 1024-square buffer (3 MB at `u8`, 6 at `u16`), which is nothing. It is
rejected because it decodes and normalises the reflected extension — a quarter of the square on a 4:3 photograph — and
then throws it away, and because the crop would then be a `DynamicImage` operation sitting between two tensor
operations, which is a seam in the wrong place.

**Rejected:** giving `chw_to_image` an optional extent. Same argument `padded_to_chw` already won.

### D4. The whole body is monomorphised over `T: Channel` at the resolved depth

`ImagePipeline::run` dispatches once at the top —

```rust
match depth {
    ChannelDepth::Eight   => self.adjusted::<u8,  B>(..).map(u8::into_dynamic),
    ChannelDepth::Sixteen => self.adjusted::<u16, B>(..).map(u16::into_dynamic),
}
```

— exactly as `restore.rs` does, so the two full-resolution loops are compiled per channel type rather than branching
per pixel over 24 million of them.

The reference cannot do this: it forces every intermediate to `image.RGBA`, so a 16-bit source computes its gain map
from 8-bit values and writes that answer into whatever the caller asked for. `inference/mod.rs` already states the
crate's rule — the depth is the caller's choice and the conversions are written once against `Channel` — and this is
the family where following the reference would have broken it. At `ChannelDepth::Eight` the two are the same
computation.

**The gain is computed in `[0, 1]` unit space**, through `Channel::to_unit` / `from_unit`, which is what makes the
body depth-independent. `applyGain` guards its denominator with `eps * 65535.0` only because Go's `RGBA()` picked a
16-bit scale for it; the scale cancels in the ratio, so in unit space the guard is a bare `1e-3` and is the same
number at both depths.

### D5. The correction follows the reference's two-upsample form

```
   inUp  = Lanczos3(resized,  W, H)     the low-passed photograph the model was shown
   outUp = Lanczos3(outLR,    W, H)     the model's answer
   result[i] = clamp(source[i] * outUp[i] / (inUp[i] + 1e-3))
```

Both sources are upsampled and the ratio is taken per pixel at full resolution. `inUp` cannot be replaced by the
photograph itself: the ratio would then be exact per pixel and the result would be `outUp`, which is the low-resolution
model output enlarged — the one thing the whole gain-map arrangement exists to avoid.

**Rejected: computing the ratio at 1024 square and upsampling the gain plane once.** It halves the scratch (see D11)
and saves one resample. It is not the same arithmetic — Lanczos is linear, so `Lanczos(out)/Lanczos(in)` is not
`Lanczos(out/in)` — and the two diverge exactly at edges, where `in` is small and the ratio is steep, which is where
ringing in an upsampled ratio plane would show. Every canvas-size measurement in the reference's two model files
(41.9 dB at 1024 against 31.2 at 768, and Lyon's 46.5 dB against 512's 23.1) was taken against the two-upsample form,
so adopting the cheaper one would invalidate the numbers that justify the canvas. It is recorded here as measured-and-
rejected so the next reader does not re-derive it: if it is ever wanted, it is a change that must re-take those
measurements.

### D6. Progress is counted steps, and the asymmetry is written down rather than tuned

The reference reports `0`, `0.9`, `1` around its `Process` call. Those constants describe no run: the model's share is
a **fixed** 1024-square cost whatever the photograph is, and everything after it — two full-resolution resamples and
two full-resolution loops — is proportional to the photograph. On a 0.4-megapixel source the model dominates; on a
24-megapixel one it is noise. No constant split is right at both ends.

This change reports four evenly-counted steps — presented, run, corrected, blended — counted rather than accumulated,
so the last lands on exactly the end of the operation's share rather than on whatever a run of additions drifted to.
That is `restore.rs`'s own schedule for its per-face steps and for the same reason.

**Rejected:** a weighting fitted to the measured split. It would be a constant correct for one image size and one
provider, derived from numbers nobody could re-derive from the code, and the spec's requirement is only that the report
advance during the full-resolution work rather than that it be proportional to it.

Cancellation is checked at each of the four boundaries, which is the only schedule a four-step run offers, and a
cancelled run returns no image at all.

### D7. Session ownership and lifetime

Unchanged from every other single-graph family, and stated because the rule requires it rather than because anything
here is new. `Model::required` returns a one-element slice; `Model::sessions` returns one `(&ArtifactId, &EpProfile)`
pair, so no profile is ever in scope that is not already attached to the artifact it was measured for and
`pipeline::acquire` needs no branch.

The handle is **acquired by the chain driver, before the pipeline runs, and borrowed for the duration of the call**.
The pipeline cannot retain it, drop it or reorder it, so "a session a run holds cannot be reclaimed" stays a property
of the driver's own reference counts rather than a rule this file follows. Nothing here opens a session, and nothing
here holds one past its call.

One session serves every bias, because the bias is not in the artifact name — which is what makes dragging a control
free of installs and opens, and is pinned by an existing test in `mod.rs`.

### D8. `Fp16Only` becomes a `match precision`, not a wrapper

The reference needs `utils.Fp16Only` because a Go variant cannot carry its own precision, so a helper is the only way
to stop a measurement taken at one precision being applied at the other. Here the precision travels inside
`LightAdjustmentVariant`, and the same statement is a two-arm match in the model's own file — which is what
`saitama::profile` already does and what `conv.rs` explicitly says no wrapper should be added for.

### D9. The bias-independent intermediate is recorded and not built

The graph's output and the gain map do not depend on the bias at all; only the final blend does. But the bias is in the
operation's identity and in its run-cache tag, so every position of a control is a cache miss and re-runs the model and
both full-resolution resamples.

This is deliberate, and it stays out of this change for three reasons rather than one:

1. **The run cache keys on one derivation.** `cache.rs` states outright that the read path and the write path cannot
   disagree about a key they do not each compute. A bias-stripped key is a second derivation of the same kind of thing.
2. **The saving is smaller than it looks.** The kept intermediate would be a full-resolution PNG; on a 24-megapixel
   photograph its decode is of the same order as the two resamples it saves, and the inference actually avoided is
   about 10 ms on the GPU.
3. **It is not this family's problem.** Denoise and sharpen carry a `Strength` and colour balance carries a `Bias`, and
   in all four families the model's output is independent of the parameter while only the last step is not. The seam is
   `inference/process.rs`' and the run cache's, worth four families, and building it inside one family's pipeline file
   would put a general mechanism in the most specific place available.

Written down here rather than left to be rediscovered when a slider exists.

## Ownership and lifetime

Covered in D7. Nothing in this change allocates, holds or frees a native resource: the pipeline is plain data — one
artifact id, one profile, one canvas size, one bias and a display name — which is what lets it satisfy `Send + Sync`
for free while the chain moves it onto a blocking thread.

Peak scratch is D11's, and all of it is Rust-owned `Vec`s and `ImageBuffer`s dropped at the end of the call.

## Crates and dependencies

**None added, and none weighed.**

**`rust-sak`:** nothing is needed from it. Its `image` module is encode and decode across eight formats plus the
arbitrary-angle `rotate` that `add-crop-1-rust-sak` added; it has no resampler, and should not — the Lanczos resample
here is `image`'s own `DynamicImage::resize_exact` with `FilterType::Lanczos3`, which is already this crate's
established idiom in `models/upscale/passes.rs:242` and `models/upscale/osaka/mod.rs:308`. `crypto`, `fetch`, `fs`,
`memo` and `sysinfo` have no bearing on a canvas plan or a gain map. **The gap is nil, so there is nothing to decide
between extending `rust-sak` and writing locally.**

**Third-party:** none. The presentation step is `tensor::padded_to_chw`, the run is `Backend::run_graph`, the
resamples are `image`'s, and the correction and the blend are arithmetic over buffers this crate already owns.
`imageproc` — the obvious candidate for the last two — would be a dependency for two loops whose exact arithmetic has
to match the reference's anyway.

## Platforms and providers validated

**Validated on macOS arm64 (Apple Silicon, M2 Max), against the CPU provider and CoreML.** That is where both
variants' profiles are re-confirmed before they land, and it is the hardware every figure in the
`execution-providers` delta was taken on.

**CUDA and TensorRT are untested here**, as they were for `add-upscale-tokyo-saitama` and `add-face-recovery-2-athens`.
Neither profile in this change declares a CUDA or TensorRT setting, so nothing adopted here rests on a figure this
change's hardware cannot reproduce — which is the difference from Tokyo's `Sequential`.

**Windows and Linux** are covered by the fake-backend suite, which exercises the whole pipeline — the plan, the
presentation, the crop, the correction, the blend, the progress schedule, the depth dispatch, the cancellation and the
refusal — with no ONNX Runtime, no GPU, no model file and no network. What no CI platform proves is that a real
session produces a plausible photograph; that is the live check, `#[ignore]`d and run by hand, as every other live
check in this crate is.

**A re-measurement precondition, repeated from the spec because it is the thing most likely to be skipped:** Lyon's
figures are only meaningful if its FP16 graph still compiles as a single CoreML partition. Before its
window-partition and qkv rewrites it compiled as 207 and CoreML was slower than the CPU provider, which would make
every compute-unit figure a measurement of partition handoff.

## Risks / Trade-offs

- **Every failure mode in this change is a worse photograph rather than an error.** A wrong normalisation range, a
  truncating fit where the reference rounds, a correction applied before the crop, the extension left in the upsample —
  all four produce a plausible image and no diagnostic. → The canvas plan, the cropped decode, the correction and the
  blend are each tested on their own arithmetic before the pipeline composes them; the composition is tested against a
  fake backend; and a live check puts a real photograph through both graphs. The normalisation in particular is pinned
  as a literal: `Normalisation::Unit`, from `standardize: false` on both sides of the reference's conversion, against
  the `Signed` that face recovery uses.
- **A fixed canvas means a thumbnail costs what a photograph does.** A 400x300 image runs at the full square. → This is
  the cost of the only export that reaches the GPU at all, it is recorded in the spec rather than hidden, and the
  reference pays it too.
- **Images below the canvas change more than images above it.** They used to run at their own size in the reference's
  pre-canvas export and now get resampled up like everything else. → Inherited, not introduced; recorded in the model
  prose so a future re-export knows what moved.
- **Peak scratch is significant on a large photograph.** Two full-resolution three-channel buffers: ~144 MB at `u8`
  and ~288 MB at `u16` on 24 megapixels, plus 12 MB of tensors and the result. → The same order as the reference, an
  order below Osaka's measured 1.72 GB at 8x, and D5 records the cheaper form and why it is not taken.
- **The profiles are re-confirmations of someone else's measurements, on one machine.** → Both are CoreML-only and
  FP16-only, so the blast radius of a wrong one is a slower Mac at one precision rather than a wrong image; both
  declare nothing at FP32, which is the precision-split rule that keeps a measurement from being over-applied; and
  Lyon's carries an explicit structural precondition to re-check before trusting any of its numbers.

## Open Questions

None that can be deferred. The four that were open — whether to slice, the correction's form, the intermediates'
depth, and whether to build the bias-independent cache — were resolved before these artifacts were written and are
recorded as D5, D4, D9 and the Slices section above.
