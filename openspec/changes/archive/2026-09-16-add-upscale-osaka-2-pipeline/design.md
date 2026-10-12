## Context

See proposal.md — Why. What matters here is what slice 1 left standing.

Everything under `inference/osaka/` is written, tested against the real FP16 weights, and unreachable.
`latent`, `noise` and `region` each sit under an `expect(dead_code)` naming this slice as their caller;
`chain::plan_graphs` is written and carries the same marker; `chain::plan_one` refuses
`Resolution::Graphs` with `DIFFUSION_UNSUPPORTED`, and the run loop repeats that refusal rather than
panicking so that lifting one and forgetting the other is a failed run instead of a crash.
`Planned::Graphs` is built and exercised only by tests.

What already exists and is reused unchanged:

- `inference::grid` — the partitioning rule, written in slice 0 explicitly so that the diffusion
  upscaler could use it without `run_tiled`. `Layout::overlaps_x`/`overlaps_y` give the width each tile
  *actually* shares, which is not the configured overlap at the last column and row.
- `inference::pad::source_offset` — the reflection rule as a coordinate map rather than a materialised
  copy.
- `inference::blend::ramp_weight` — the raised cosine, written to be usable without `blend_tile`
  because "the diffusion upscaler feathers its tiles into a weighted float accumulator with this same
  curve".
- `inference::tensor` — `Sampler`, `image_to_chw`, `chw_to_image`, and `Normalisation::Signed`, whose
  `expect(dead_code)` slice 1 already claimed.
- `osaka::region::restore_region`, which takes a `Sampler`, a `Tile` and three held session handles.

The three functions this slice writes — the accumulator, the à trous transform and the colour fix —
are pure arithmetic over float buffers. That is why they are in the second slice: see slice 1's D1.

## Goals / Non-Goals

**Goals**

- Osaka runs through the public API, at every scale in the supported range, on an image of any size.
- Nothing is materialised that a coordinate map can serve, and nothing is quantised to a channel depth
  and back on its way into the picture.
- The convolutional path is untouched. `run_tiled`, `run_passes` and `blend_tile` keep their current
  behaviour and their current tests.

**Non-Goals**

- A memory budget, an admission check or a memory-pressure warning. See D8 — deferred by slice 1's D9
  and still deferred, now with a second figure attached.
- Bit-parity with the Go application's output. Unchanged from slice 1's D6: the noise stream differs.
- Any change to the public API, the catalogue, the cache tag or either front end. Osaka is already
  offered; what changes is that choosing it stops failing.
- Reworking `blend_tile` into the accumulator. They are two contracts — see D3.

## Slices

Two slices. **This change is slice 2, and it is the last.**

| # | Name | Delivers | Depends on | State |
|---|------|----------|-----------|-------|
| 1 | `add-upscale-osaka-1-region` | The provider settings, the shaped inference seam, the `Planned` split, and one 960x960 region restored end to end against the real model. Osaka stays refused through the public API. | Nothing beyond what was merged | **done** (archived `2026-09-16`) |
| 2 | `add-upscale-osaka-2-pipeline` | The Lanczos resample to target size, the tile grid over the whole image, the weighted float-CHW canvas, the wavelet colour fix, depth and progress — and lifting the refusal. Osaka works. | Slice 1 | **current** |

**One change sits beside this series rather than in it.** `fix-osaka-disabled-optimizers` withdraws
slice 1's narrowing of `EpProfile` to a single disabled optimizer, which was drawn from a sweep whose
separator was hardcoded to a comma where the reference joins with a semicolon. It touches `EpProfile`,
`variant.rs` and `build.rs`, none of which this slice goes near, so the two are independent and can
archive in either order. It is worth taking first only so that this slice's end-to-end run on the real
weights exercises the provider configuration the application will actually ship.

## Decisions

### D1. Osaka drives its own pipeline; `run_tiled` is not widened to fit it

Restated from slice 1 because this is the slice that could have gone the other way. Four of
`run_tiled`'s assumptions are hardwired and all four are wrong here: it drives a **single** session
where a region is three; it feeds `[1,3,H,W]` in `[0,1]` where this needs `[-1,1]` and two of its three
graphs are neither three-channel nor at the image's resolution; it takes an integer output scale where
this model preserves resolution; and it pads to the tile size where this must align to a multiple of
16. Adding four conditionals to the driver every other model takes, to serve one model, would put the
diffusion contract's edge cases in front of every convolutional run.

What can be shared **is** shared, and slice 0 already arranged for it: the grid, the reflection rule
and the ramp curve are each already factored out with this caller named in their comments. The
duplication that remains is a loop over tiles, which is the shape of the thing rather than its
substance.

### D2. Nothing is materialised as a padded image

The reference builds a reflection-padded copy of the resampled image and tiles that. This project
already expresses reflection as a coordinate map — `pad::source_offset`, whose doc comment records why —
and both readers of the padded extent can go through it:

- **Each region's input.** The grid runs over the *padded* dimensions, and each tile is clipped to the
  resampled image's own extent before it is handed to `restore_region`; `image_to_chw` then mirrors the
  region out to 960 from the source itself. The mirror is about the same last pixel either way, so this
  is arithmetically identical to cropping from a padded copy rather than merely close to it.
- **The colour fix's reference.** This one genuinely needs the whole padded plane as planar float, so
  `tensor` gains a non-square, whole-image sibling to `image_to_chw` that reads a padded extent out of
  a `Sampler` through the same `source_offset`.

**Why a sibling rather than generalising `image_to_chw`.** The same reasoning as slice 1's D2 for
`run_tile`/`run_graph`. `image_to_chw` takes one square `shape` and a `Tile`, and its contract is "one
region at the shape the model accepts"; this one takes a width and a height and reads a whole image.
Collapsing them would give the tile path two more parameters it has no use for and one more way for a
caller to disagree with the grid. `chw_to_image` needs no sibling — it already takes its dimensions
from the destination buffer, so it is non-square today.

**Alternative considered — materialising the padded image, as the reference does.** Rejected: at 4x
over a 12-megapixel photograph that is a second 192-megapixel image held alongside everything in D8,
to express a rule this crate already has as a function.

### D3. The accumulator is a second way of combining tiles, not a replacement for the first

`blend::blend_tile` stays exactly as it is and keeps every test it has. The accumulator is new code in
`osaka::canvas`.

The reason is not that the two are similar enough to merge and not quite: it is that `blend_tile`
blends each tile against **whatever is already in the buffer**, which can average two tiles and no
more. Where four tiles meet, its result depends on the order they were written. For the convolutional
models that corner is invisible, because two tiles covering one pixel very nearly agree; for a model
drawing its own noise per region they genuinely disagree, and that corner is exactly where a seam
shows. The accumulator weights every contribution into a sum and a weight plane and divides once, so a
pixel's value is order-independent by construction.

They also differ in what they hold: `blend_tile` writes into an `ImageBuffer` at the caller's channel
depth, so each tile is quantised on its way in. The accumulator holds planar float, and the depth is
applied once to the finished image. Quantising a stochastic tile before averaging it is throwing away
precision exactly where the averaging needs it.

What the two **must** share is the curve, and they do: `ramp_weight` is called by both, so a tuning
change cannot be applied to only one of them.

### D4. The feather is the width the tiles actually share

This is the one deliberate divergence from the reference's behaviour in this slice, and it is small.

The reference ramps every interior edge over its configured 128 and relies on the final division to fix
the last column and row, where the move-a-tile-back rule makes the real overlap much larger. That
works — normalising a sum of weights is correct whatever the weights were — but it makes the division
load-bearing for correctness rather than merely for tidiness.

This project already answered the same question the other way for `blend_tile`, and
`inference-tiling` carries it as a requirement: *"The width combined SHALL be the overlap those two
tiles actually have"*. `Layout::overlaps_x`/`overlaps_y` exist for precisely this, and using them makes
each pair of abutting ramps a partition of unity **before** the division. The two ways of combining
tiles then answer the same question the same way, which is worth more than matching a figure.

Edges lying on the image border are not ramped at all, on either path: nothing overlaps them, so
ramping there drives the accumulated weight towards zero along the outside of the picture and amplifies
whatever survived the division. The reference is explicit about this and it is carried over unchanged.

### D5. The colour fix runs on the assembled padded canvas, before the crop

Two orderings were available: correct the padded canvas and then crop to the target size, or crop first
and correct the target-size image. The à trous transform replicates edge pixels outside the plane, so
the two differ within roughly one kernel support — about 60 pixels — of the right and bottom edges,
where the first sees the mirrored extension and the second sees a replicated crop boundary.

The reference corrects the padded canvas, and that is followed here. It is not a coin flip: the
extension is a mirror of real picture content, and a replicated edge is a constant, so the mirrored
version is the better-conditioned input to a low-pass filter. The extension is discarded immediately
afterwards either way.

Three things about the correction are properties rather than details, and the `image-enhancement` delta
carries them: it runs **once on the assembled image** and never per region; its reference is the
**resampled** image rather than the original input; and it replaces low frequencies while keeping the
restored image's own detail.

**Alternative considered — correcting each region as it comes out of the decoder**, which would let the
base plane be dropped after the region loop and save 2.3 GB at 4x. Rejected outright: it is the failure
mode the reference names. Each region drifts independently, so correcting a region against its own crop
bakes that region's drift in as a step at its boundary instead of removing it.

### D6. Lifting the refusal is two edits, and the third is `clippy`'s

- `plan_one`'s `Resolution::Graphs` arm becomes `plan_graphs(operation)`, which slice 1 wrote for this.
- The run loop's `let Planned::Passes { .. } = operation else { return Err(unsupported(..)) }` becomes a
  match with a diffusion arm calling the new driver, with the three handles travelling into the same
  blocking closure the pass sequence's do.
- `DIFFUSION_UNSUPPORTED` and every `expect(dead_code)` naming this slice are then unused, and **an
  `expect` that is no longer needed is a `clippy` failure**. That is the check that all of them were
  claimed rather than a list to work through by hand.

The acquisition loop above needs no change at all: `Planned::sessions` already answers both contracts,
pairing each artifact with the profile measured for it, which is what slice 1's D3 built it for.

### D7. The driver reports and fails exactly as the convolutional one does

The Osaka driver returns `TilingError<RegionError<E>>` rather than an error type of its own:
`Untileable` for an image with no area, `Cancelled` where the caller stopped it, `Tile { index, source }`
naming the region that failed. `InferenceError::from_tiling` then needs no new arm, and a failed
Osaka run reads in a log exactly like a failed Kyoto one.

Cancellation is checked once per region and progress is reported once per region, as
`run_tiled` does per tile, with the `0.0` at the start and the exact `1.0` at the end emitted by the
driver. A region is tens of seconds of work, so per-region is the finest granularity available and also
the right one — the bar moves at a rate a user can read.

`ChannelDepth` is dispatched once at the top, as `run_tiled` dispatches it, so the body is written once
and monomorphised for `u8` and `u16` rather than branching per pixel.

### D8. The memory is recorded with a number, and still not acted on

Slice 1's D9 deferred the memory budget and gave it a figure: 29.0 GB resident for one 960x960 region
on the CPU provider. This slice adds the whole-image term, which is float and scales with the
**output**:

| Requested | Output over a 12 MP photo | One planar float buffer | Peak float held |
|---|---|---|---|
| 1x | 12 MP | 0.14 GB | ~0.4 GB |
| 2x | 48 MP | 0.58 GB | ~1.7 GB |
| 4x | 192 MP | 2.30 GB | ~6.9 GB |

Three buffers are live at the peak: the resampled base plane, the accumulator's sum, and then either
the accumulator's weight plane or the colour fix's scratch. The ordering keeps it to three rather than
four — the weight plane is dropped when the accumulator resolves, and the accumulator resolves **in
place**, so the sum is the result. The colour fix likewise works in place on that buffer and allocates
one plane per role rather than two per channel.

That is as far as this slice goes. **No budget, no admission check, no warning**, for the reason slice 1
gave: this project has no memory budget to read, and a warning built on a guess about available memory
is worse than none. Nor is the scale range narrowed to keep the figure down — a cap describing what the
project has not yet built is exactly the kind of temporary surface this project does not add, and the
range is a property of the model rather than of this pipeline.

What this slice owes the eventual budget change is an honest number, and the table above is it.

### D9. The à trous transform keeps its two convolutions separate

The horizontal and vertical passes are written separately rather than as one axis-generic function
taking a stride. Each hoists its edge clamping out of the inner loop in the way its own axis allows,
and those ways differ: along a row the clamp depends on the pixel, so the row splits into a clamped
margin and a branch-free interior; down a column the five row offsets are identical for every pixel in
the row, so they are computed once per row. A shared implementation would either keep the clamp per tap
— the cost being removed — or iterate columns outermost and stride across memory instead of along it.

This is the reference's own reasoning and it is carried over because it still holds: the colour fix runs
over the whole assembled image and is the one CPU-bound pass in this pipeline that is not ONNX.

Five levels is the depth, reaching a support of roughly 60 pixels — well above the texture the model
synthesizes and well below the scale of the drift being corrected.

## Risks / Trade-offs

**[A 4x Osaka run over a large photograph exhausts host memory and takes the process down]** →
Unmitigated, stated, and quantified in D8. It is the same exposure every model here has and Osaka makes
it likely rather than theoretical. The mitigation is the memory budget, which is its own change; what
this slice does is make sure that change has real numbers to start from rather than an estimate.

**[The accumulator's divergence from the reference (D4) changes output in the last column and row]** →
Intended, and it is the direction the `inference-tiling` requirement already points. The check is that
no step appears across a seam far wider than the configured overlap, which is the same property
`blend_tile`'s tests already assert for the convolutional path and which the reference's arrangement
also satisfies. Both are correct; they are correct at different points.

**[The colour fix or the accumulator is subtly wrong in a way a unit test misses]** → This is the risk
slice 1's D1 judged to be small, and the judgement stands: both are deterministic arithmetic over float
buffers with properties that can be stated exactly — a uniform tiling sums to weight 1 everywhere, a
constant plane is its own low frequency, correcting an image against itself is the identity, and
correcting against a uniformly shifted copy recovers the shift. What a unit test cannot check is
whether the result *looks* right, which is what the live test on the real weights is for.

**[An image whose target size is below one region wastes most of a region's work]** → Accepted and
inherent. The transformer accepts exactly 960x960, so a 200x200 target is padded to 960x960 and runs a
full region for it. The reference has the same property. Nothing is refused for it: a small image
restores correctly, it just costs what a region costs.

**[The live end-to-end test needs the 6.8 GB of FP16 weights and tens of seconds per region]** →
`#[ignore]`d with its reason stated, as slice 1's region tests are. CI has neither the weights nor the
time, and the arithmetic this slice adds is covered by tests that need no model at all.

## Migration Plan

None, and nothing to roll back to. No persisted format, no public signature and no serialized form
changes: `Upscale`'s representation, its cache tag and the catalogue are all untouched. The one
user-visible change is that an Osaka operation that failed now succeeds, which no stored value depends
on — a cache entry for an Osaka run cannot exist, because no Osaka run has ever completed.

## How the work divides

**`opai`** — all of it. The pipeline, the accumulator, the colour fix, the tensor sibling and the
refusal are library concerns, and both front ends would need every one of them.

**`gui` and `cli`** — nothing. Osaka is already published in the catalogue and already selectable in
both, so there is no component, no i18n key and no Tauri command to add. `perf` is likewise unchanged;
it selects operations from the catalogue and Osaka was always selectable there, so this slice is what
makes benchmarking it possible without touching it.

## Crates and rust-sak

**No crate is added by this slice**, and `rust-sak`'s enabled feature set is unchanged — `image`,
`memo`, `fetch`, `fs`, `crypto` and `sysinfo` as before.

**`rust-sak` has nothing for either piece of infrastructure here.** Its `image` module is decode,
encode and probe across eight formats behind one API; it has no resampling, no filtering and no
convolution, and no other module is close. Neither gap is worth filling there: the resample is served
by the `image` crate already in the tree, through the same `FilterType::Lanczos3` that
`upscale::correct_overshoot` uses today, so adding a resize to `rust-sak` would duplicate a dependency
this project already has; and the à trous wavelet colour fix is SeedVR2's own correction for a defect of
single-step diffusion rather than a general-purpose image operation, so it is genuinely specific to this
application and belongs in `opai`.

`rand_chacha` and `rand_core`, added by slice 1 for the per-region noise, are used unchanged.

## Session ownership and lifetime

Unchanged from slice 1's D10, and this is the slice where it is exercised through the public API for
the first time.

The chain acquires all three of an Osaka operation's sessions **before any of them runs**, holds each
`SessionHandle` for the whole operation, and drops them when it returns. Holding the handle *is* the
guarantee: the session cache's "in use" test is the handle's own reference count, so a model a run
holds cannot be reclaimed by the idle sweep, by an explicit release or by a provider switch. The three
handles travel into the blocking closure and back out of it exactly as the pass sessions do, so there
is no window in which one could be reclaimed — not between two regions, and not between the encode and
the decode of one region.

The tile loop changes nothing about this. It holds the same three handles across **every** region of
the image rather than across one, which makes the operation longer but not different in kind, and is
also why the ~7 GB the three graphs weigh does not come down partway through a run.

The three may be built on **different providers**, because the CPU fallback is per session. A machine
where the transformer fails to build on CoreML but the VAE halves succeed runs every region across two
providers. That is correct and already reported through `ProviderReport`, and it is worth knowing
because it is a plausible explanation for a run slower than either provider alone.

## What the end-to-end run showed

Task 6.1, on macOS arm64 over the CPU provider in a `release` build: the fixture resampled to 700x520 and upscaled 2x
through `Opai::process` — four 960x960 regions, 317 s.

```text
  dimensions        700x520 -> 1400x1040
  correlation to the resampled base      0.992214
  detail energy     result 0.026307      base 0.013353      1.97x
  largest step      across a seam 0.280118      elsewhere 0.708867
  channel means     result 0.5632 0.4513 0.3916
                    base   0.5633 0.4514 0.3916
```

The result carries **twice** the high-frequency detail of the Lanczos base while still correlating with it at 0.992,
so it restored rather than resampled. The largest step across a region boundary is well *below* the largest step the
picture has elsewhere, so the accumulator leaves no seam. The per-channel means track the reference's to four decimal
places, which is what the colour fix exists for.

**One of task 6.1's four properties was restated, because the first run showed it could not discriminate.** The task
worded the restoration check as the result correlating with a Lanczos resample of the input more strongly than that
resample correlates with the input itself. Correlating a 1400x1040 resample against a 700x520 input means bringing the
input to the result's size first, so the baseline becomes two interpolations of one photograph: measured, the result
against its base was **0.992214** and the base against an enlargement of the input **0.997828**. No restoration clears
that bar, whichever way the phrasing is read.

What separates the two is detail, not correlation — a Lanczos upsample synthesizes none, since every value it produces
is a weighted combination of pixels the input already had. So the check became the mean absolute Laplacian of the
result against the base's, as a strict inequality with no threshold to tune. It is a stronger claim than the original,
not a weaker one: a pipeline that quietly returned its own base passes the correlation and fails this.

## Measured against the Go application

Task 7.1, run by hand on macOS arm64 (M2 Max) over CoreML: the Rust `crates/perf` and the Go `cmd/perf`, both at
FP16, both `--runs 5 --scale 4` with the default single warm-up, over the same 640x640 embedded sample — which is
byte-identical between the two projects (`fixtures/test.dat` and `cmd/perf/test.dat` share a SHA-256). The Rust sweep
ran first on a cold machine, the Go sweep immediately after on a hot one.

| | Rust `perftest` | Go `cmd/perf` |
|---|---|---|
| output | 2560x2560 | 2560x2560 |
| cold | 38.667 s | 58.225 s |
| **min** | **35.701 s** | **35.380 s** |
| median | 44.171 s | 38.977 s |
| mean | 66.320 s | 52.722 s |
| max | 121.382 s | 103.250 s |
| stddev | 39.092 s | 28.882 s |

**The Rust sweep reported five timed inferences rather than a refusal**, which is what this check exists for: the
refusal is lifted through the public API and not merely in a test. Both report the same output dimensions for the same
input and scale.

**The steady-state figures agree to within 1%** — 35.70 s against 35.38 s, over nine 960x960 regions per run. That is
~4.0 s per region against the 4.38 s slice 1 measured for a single region on the same provider, so the image-scale work
this slice adds — the Lanczos resample, the grid, the accumulator and the wavelet colour fix — costs nothing
measurable beside the model.

**The spread was investigated rather than recorded.** Both sweeps ramp monotonically to roughly three times their own
minimum, with resident memory flat at ~8.5 GB throughout: the Rust runs went 38.7, 35.7, 36.4, 44.2, 93.9, 121.4 s.
The ramp is present in *both* implementations and in the same shape, so it is the machine throttling under sustained
CoreML load rather than anything about either pipeline — which is why the minima are the figures to read. The medians
and means are not comparable between the two here, because the Rust sweep started cold and the Go sweep started on a
machine that had already been at full load for eighteen minutes.

**Pixel equality was not asserted and must not be.** The noise fields differ by design — see the series note and slice
1's D6 — so the two implementations produce different images from the same input at the same settings. What is
compared is the geometry, the timing and the fact that a run happens at all.

## Validated against

**Platforms:** written for macOS arm64, Windows and Linux; nothing in it is platform-specific. The live
verification runs on macOS arm64, which is the machine this change has.

**Execution providers:** the CPU provider and CoreML are exercisable here and are what the end-to-end
run is checked on. **CUDA and TensorRT are untested by this change**, as they were by slice 1 — the
per-graph NHWC override is a CUDA-only setting whose plumbing is tested and whose effect is not.

Nothing in this slice is provider-dependent: the resample, the grid, the accumulator and the colour fix
are all CPU arithmetic that runs identically whatever the three sessions were built on.
