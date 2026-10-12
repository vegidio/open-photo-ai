## Why

Paris and Lyon can be named, parameterised, catalogued, downloaded, opened and cached, and they cannot run. Everything
around this family ships today — `LightAdjustment`, `LightAdjustmentVariant`, `Bias`, both constructors, the catalogue
rows, the artifact names, the run-cache tag, `models/build.rs`'s codename lookup and `perf`'s selection arm. Paris at
FP16 is even the graph `sessions/live.rs` opens, because at 122 KB it is the smallest this project publishes. The only
thing between a user and a light adjustment is one line of `Operation::pipeline`'s refusal arm.

It is the right family to run next for two reasons beyond its turn in the order. It is the **first non-tiled,
whole-image contract in the catalogue** — a fixed square canvas rather than a grid of regions — which is a shape three
more families will need and none has yet. And it is the cheapest possible first instance of it: one graph, one run, one
input, no second tensor, no landmarks, no affine transform, and both of its variants run through it identically.

## What Changes

**The light adjustment contract, which is a fixed canvas and a gain map rather than a tile grid.**

- `models/light_adjustment/process.rs`: the whole image fitted so its longest side lands on the variant's canvas,
  reflection-padded out to the square, run once, cropped back out of the padding, and applied to the **full-resolution
  original as a per-channel gain** rather than being upsampled and returned. The reference's `Process` and
  `buildResult`, in that order.
- `LightAdjustment::pipeline`, the family's contract seam, and the `Operation::pipeline` arm that reaches it. Light
  adjustment leaves the five-family refusal arm; denoise, sharpen, colour balance and colorization stay in it, each for
  its own unchanged reason.
- The per-run `Bias` is applied **after** the gain map, as a linear blend of the original towards the adjusted image
  that extrapolates past the endpoint for a negative value. It never reaches the graph, which is why one session serves
  every position of a slider.

**Both variants, in one change.** Paris and Lyon differ in nothing but their measured profile and the prose recording
it: same 1024 square, same single-input graph, same normalisation, same gain map. The precedent is
`add-upscale-tokyo-saitama` rather than the `add-face-recovery-*` series — Athens and Santorini were split because they
differ at the session call itself, and these two do not differ anywhere.

**A cropped decode of a model's planar output**, as a third sibling in `inference/tensor.rs`. `chw_to_image` requires
the destination to be the tensor's whole extent; this family needs the top-left `sw x sh` out of a padded square
**before** the upsample, or the aspect ratio of everything downstream is wrong. `image_to_chw` and `padded_to_chw`
already sit over one private body for exactly this reason, and the decode side gains the same treatment.

**Paris' and Lyon's measured provider tuning**, currently `EpProfile::default()` for both. Paris: CoreML restricted to
the CPU and GPU with sequential execution, at **FP16 only**. Lyon: CoreML restricted to the CPU and GPU, at **FP16
only**. Both are seeded from the reference's measurements and re-confirmed here before they land — Lyon's especially,
whose figures rest on a claim that is checkable rather than inherited: that its FP16 graph is a **single CoreML
partition**. Before its window-partition and qkv rewrites it was 207 partitions and CoreML was slower than the CPU, so a
graph that is not one partition today would make every number in its profile a measurement of partition handoff.

### Deliberate divergence from the reference

- **The canvas geometry is not detection's.** `models/detection/newyork/input.rs` already has a `fit` that scales the
  longest side onto a square, and this family must not use it. New York's **truncates**, because the reference's
  detection preprocessing truncates and the rescale that undoes it has to be exact. `utils.FitLongSide`, which is what
  light adjustment goes through, **rounds**. Two functions that look identical and are not; sharing them would move
  every canvas by a pixel on some aspect ratios, which is a slightly different photograph and no error at all.
- **The intermediates carry the depth the run resolved to.** The reference quantises the downscaled input and the
  model's low-resolution output to 8-bit `image.RGBA` unconditionally, so a 16-bit source computes its gain map from
  8-bit values and then writes that answer into a 16-bit result. Here the pipeline body is monomorphised over
  `T: Channel` at the resolved `ChannelDepth`, dispatched once at the top exactly as `face_recovery/restore.rs` does.
  At `ChannelDepth::Eight` this **is** the reference; at `Sixteen` it is what the caller asked for. `inference/mod.rs`
  already states this rule for the crate, and this is the family where the reference would have broken it.
- **The gain is computed in unit space rather than on 16-bit channel values.** `applyGain` divides two values that
  `RGBA()` returned on a `0..65535` scale and guards the denominator with `eps * 65535.0`; the scale cancels in the
  ratio, so the same arithmetic written over `Channel::to_unit`'s `[0, 1]` needs a bare `1e-3` and is identical at both
  depths. The reference has to write the constant in a scale because Go's accessor picks the scale for it.
- **No alpha is carried.** `buildResult` writes `A: 255` into every pixel. Every pipeline in this crate returns RGB with
  alpha composited against black on the way in, and a fourth channel forced opaque is a quarter of the largest
  allocation the application makes, spent claiming the model returned an opacity it was never given.
- **Progress is not `0 / 0.9 / 1`.** The reference reports those three constants around a call whose cost profile they
  do not describe: the graph is a **fixed** 1024-square cost whatever the photograph is, while everything after it —
  two full-resolution resamples and two full-resolution loops — is proportional to the photograph. On a 0.4-megapixel
  source the graph dominates and on a 24-megapixel one it is noise, so no constant split is right at both ends. See
  design.md.

### Infrastructure: what `rust-sak` already covers

**Nothing is needed from it, and nothing here is missing from it.** Its `image` module is encode and decode across
eight formats plus, since `add-crop-1-rust-sak`, an arbitrary-angle `rotate`. It has no resampler and should not: the
Lanczos resample this family needs is `image`'s own `resize_exact`, which is already this crate's established idiom in
`models/upscale/passes.rs` and `models/upscale/osaka/mod.rs`. `crypto`, `fetch`, `fs`, `memo` and `sysinfo` have no
bearing on a canvas plan or a gain map. **No new third-party crate**, and none was weighed — the tensor conversion, the
reflection padding and the session seam all exist in `inference/`, and what is left is arithmetic over buffers this
crate already owns.

### Task estimate

**14.** The cropped decode and its tests, the canvas plan, the pipeline's two trait impls, the run body, the gain map
and blend, Paris' directory, Lyon's directory, the two variant seams, the family seam, the refusal arm and its inverted
tests, the fake-backend suite, the three documentation sites, the live check, the confirming sweep, and the toolchain
pass.

## Capabilities

### New Capabilities

- `light-adjustment`: what a light adjustment run does to an image — the fixed square its graph accepts and how a
  photograph of any size and aspect is fitted to it, why the result is applied as a gain rather than returned, what the
  per-run bias does and in which direction, what a run reports and stops on, and what it refuses.

### Modified Capabilities

- `execution-providers`: Paris' and Lyon's measured settings join Kyoto's, Tokyo's, Saitama's, Athens', Santorini's and
  New York's as pinned requirements. Both are precision-specific declarations, which the capability already admits;
  what is new is that **two graphs of completely different architectures reach the same answer on the same hardware**,
  measured separately, and neither is evidence for the other.

`image-enhancement` deliberately gets **no delta**, for the reason `add-upscale-tokyo-saitama` gave when it declined
one. Its refusal requirement is written generically — "where the system has no pipeline for an operation" — and names
no family; its depth, progress, cancellation and catalogue requirements all speak of "an operation" throughout. A
family gaining a pipeline makes the implementation match a requirement that does not move. What *is* new — that an
operation may run its model at a resolution other than the photograph's and return the photograph's own pixels
corrected — is observable only in this family today, so it is stated in `light-adjustment` where it can be tested
rather than promoted to a tier no second family has reached.

## Impact

**Crates: `opai` only.** No `gui`, no `cli`, no new crate. Neither front end draws a light adjustment control yet, and
every line here is library logic both would need. **`crates/perf`'s code is used but not modified** — it keeps no list
of what the library runs and finds out by attempting, so an unnamed sweep simply measures four more rows.

- `crates/opai/src/inference/tensor.rs` — the cropped decode, over the existing shape checker.
- `crates/opai/src/models/light_adjustment/process.rs` — new: the canvas plan, the pipeline, the gain map, the blend.
- `crates/opai/src/models/light_adjustment/paris/`, `.../lyon/` — new: each variant's canvas, its measured profile and
  the prose behind it, and the tests asserting both.
- `crates/opai/src/models/light_adjustment/variant.rs` — `canvas()`, and `profile()` reaching the two model directories
  instead of returning the default.
- `crates/opai/src/models/light_adjustment/mod.rs` — the family's contract seam, built from `LightAdjustmentParams`.
- `crates/opai/src/models/operation.rs` — light adjustment leaves the refusal arm; the cross-family tests that name the
  five and that assert only the measured variants declare a profile both move.
- `crates/opai/src/lib.rs` — `Opai::process`'s "What is refused" names two families running today and must name three.
- `crates/opai/src/error.rs` — `Unsupported`'s doc comment enumerates the refused families.
- `crates/perf/README.md` — its unnamed-sweep note claims only upscale has a pipeline, which face recovery already made
  false and this makes false twice. Corrected to state the rule rather than the list.

**Public API: additive, and small.** `Opai::process` gains four operations it can serve. No signature changes, no type
removed, `UnsupportedReason` gains no arm, and everything the pipeline is built from is `pub(crate)`.

**The riskiest part is that every failure mode here is a worse photograph rather than an error.** A wrong normalisation
range, a truncating fit where the reference rounds, a gain applied before the crop instead of after, or the padding left
in the upsample all produce a plausible image and no diagnostic. So the canvas plan, the cropped decode and the gain map
are each tested on their own arithmetic before the pipeline composes them, the composition is tested against a fake
backend with no runtime at all, and a live check puts a real photograph through both graphs end to end.
