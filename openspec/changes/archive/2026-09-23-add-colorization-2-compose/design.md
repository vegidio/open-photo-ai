## Context

See proposal.md for why. Four facts about the surrounding code shape everything below.

**Slice 1's Lab module is the whole per-pixel vocabulary.** `lightness([u16; 3])` and `gray([u16; 3])` read a `u16`
table. `rgb_to_lab` and `lab_to_linear_rgb` are the conversions, and `srgb_u8` and `srgb_u16` are the encodes, each
pinned bit for bit against the general expression it replaces. Nothing in this slice adds per-pixel colour arithmetic.
It only arranges those calls.

**Every pixel arrives through `Sampler::rgb`.** It returns `[u16; 3]` for any source layout, with 8-bit channels
widened to `v * 257` and straight alpha premultiplied against black. It borrows an `Rgb8`, `Rgb16` or `Rgba16` buffer
and converts anything else to an owned `Rgba16` once (`inference/tensor.rs`). The reference's `RgbPixBuffer` fast path,
its `NRGBA` translucent branch and its generic `At()` fallback, which its compose tests cover one by one, collapse here
into those two arms.

**The align-centres bilinear read already exists, at the wrong tier.** `models/color_balance/saopaulo/weights.rs`
holds `Tap`, `Axis` and `sample`. `Axis::new(src, dst)` is the reference's `sampleAxis` expression for expression, in
`f64`. `sample` is the reference's compose grouping (along x within each row, then between the rows), written without
`mul_add`. Its header and the archived São Paulo design D1 both say colorization promotes it.

**Row-banded parallelism already exists, at the wrong tier.** `models/upscale/osaka/colorfix.rs::for_each_row` splits
an `f32` plane's rows statically across `available_parallelism` on `std::thread::scope`, with a 64-row minimum per
worker. Its tests pin the result as bit-identical to the serial form. The reference's compose splits its rows the same
way.

## Goals / Non-Goals

**Goals:**

- Every function slice 3's `process.rs` needs between "a photograph" and "a graph tensor", and between "a graph output"
  and "a composed photograph at the run's depth", written and proven, with no signature left for slice 3 to invent.
- The fused compose proven **bit for bit** against a separate-steps oracle at both depths and over every source layout
  `Sampler` distinguishes.
- Both promotions proven to be moves: São Paulo's and Osaka's suites pass with no assertion edited.

**Non-Goals:**

- **The `Graph` enum, `ColorizationVariant::graph()`, `process.rs`, the graph run, progress and cancellation.** These
  are slice 3's. Deciding how the pipeline dispatches over the two contracts is slice 1's recorded decision (a two-arm
  `Graph::{Ab, Rgb}`) and slice 3 carries it out.
- **Any model directory beyond what the Rgb contract needs.** `jaipur/` is created because its adapters are model
  tier (D5). Its profile and prose are slice 3's, and so are `delhi/` and `mumbai/`.
- **Revisiting `inference::present`'s alpha handling.** Light adjustment and colour balance stretch straight alpha
  unweighted. Colorization does not (D3). Whether they should is not this change's question.
- **Acting on the cost measurement.** D8 records it. The escape hatch slice 1 named is its own change if it is needed.

## Decisions

### D1. `inference/bilinear.rs` receives São Paulo's `weights.rs` whole

```
  BEFORE                                         AFTER
  models/color_balance/saopaulo/weights.rs       inference/bilinear.rs      (every family)
    pub(super) Tap, Axis, sample                   pub(crate) Tap, Axis, sample
    tests                                          tests, moved unedited
  saopaulo/process.rs                            saopaulo/process.rs
    use super::weights::{self, Axis}               use crate::inference::bilinear::{self, Axis}
                                                 models/colorization/compose.rs (D6)
                                                   use crate::inference::bilinear::{self, Axis}
```

**`inference/`, not `models/`.** It is an execution primitive that no one model owns: the inverse of a pixel-centre
resize, read through tables built once per axis. `config.yaml` puts those in `inference/`, beside `present`, whose
resize it inverts for São Paulo.

**Named `bilinear`**, not `weights`. Only São Paulo's planes were weights, and colorization's are chroma. The name does
not collide with `blend` (overlapping tiles), `mix` (the control blend), `canvas` (the tile accumulator) or `present`
(the fixed-square fit).

**What changes in the file:** visibility to `pub(crate)`. `sample`'s `canvas` parameter becomes `side`, and its doc
changes from "one weight map" to "one square plane". The module header drops "Model tier: one caller" and gains the two
callers. The header's reasoning against `image`'s `Triangle` and against `face_recovery::composite::Taps` stays, because
it is still true for both callers. **No test body changes** beyond its `use` lines, because a move that needed a
different expected value was not a move.

**Rejected: `use super::super::color_balance::saopaulo::weights` from colorization.** `config.yaml` names an import
across a model boundary as a bug. **Rejected: a copy in colorization.** `config.yaml` names that too, and the reference
did it (its `sampleAxis` is a copy of colorization's helper), which is the warning.

### D2. `inference/rows.rs` receives `for_each_row`, generalised over the element

```rust
pub(crate) fn for_each_row<T, F>(dst: &mut [T], row_len: usize, height: usize, row: F)
where
    T: Send,
    F: Fn(usize, &mut [T]) + Sync;
```

`colorfix` calls it with `row_len = width` over `f32`. The compose calls it with `row_len = 3 * width` over the output
buffer's `u8` or `u16` samples. The 64-row minimum and the process-wide `available_parallelism` read move with it
unchanged, and so does its prose: why the split is static, why a pool is not used, and why only CPU passes that are not
ONNX are parallelised.

**This is a promotion, not a coincidence of shape.** The reference's compose splits its rows across cores. Writing
that split again inside `compose.rs` would be the second copy `config.yaml` forbids. The function is small, but the
rule does not scale with size.

**Only the function moves.** `colorfix.rs` stays at Osaka's model tier and on `config.yaml`'s list of known candidates,
because the wavelet colour fix itself still has one caller. Its two convolutions, and their documented reasons for not
being folded together, are untouched. Its bit-identical-to-serial tests pass unedited.

**Rejected: `rayon`.** It is a new crate for work that `colorfix` already decided a static split serves, and the ONNX
runtime threads its own graphs, so a second pool would compete with it.

### D3. The stretch flattens a source with alpha before it resamples

`stretch::stretched(source, side, filter) -> DynamicImage`, family tier, called by both contracts' input builders:

```
  Rgb8, Rgb16                    borrowed as they are       --> resize_exact(side, side, filter)
  other 8-bit layouts            flattened to Rgb8   ---+
  (L8, La8, Rgba8)               through Sampler::rgb   +--> resize_exact(side, side, filter)
  everything else                flattened to Rgb16  ---+
```

**Why flatten.** The reference's `imaging.Resize` weights every tap by its alpha: `C' = Σ w·a·c / Σ w·a` and
`A' = Σ w·a` (`imaging@v1.6.2/resize.go`, read, not assumed). Its input builders then read the result through
`Sample16`, which premultiplies by `A'`. So what reaches the graph is `C'·A' = Σ w·(a·c)`, and that is exactly an
unweighted resample of the photograph already composited against black. Flattening through `Sampler::rgb` first
therefore reproduces the reference's sums, and only the rounding differs. It also means the stretch reads the same
premultiplied pixels the compose reads for L, so the graph and the compose see one photograph.

**Why at the source's own depth class.** An 8-bit source stays 8-bit, which is what `imaging.Resize` produces, and an
opaque 8-bit pixel comes back exactly (`v * 257 / 257`). A 16-bit source stays 16-bit, which is slice 1's "the run's
depth is kept", applied to the graph's input.

**Borrowed where nothing needs flattening**, as `detection::newyork::input` borrows an `Rgb8` source. At 24
megapixels, a copy made only to be resampled and dropped is 72 MB.

**Rejected: `DynamicImage::resize_exact` on the raw source, as `inference::present` does.** That resamples straight
alpha unweighted, so the colour stored under fully transparent pixels bleeds into the gray rendering at every
transparency edge. The reference does not do that. **Rejected: flattening every source to `Rgb16`.** An opaque `Rgba8`
PNG and an `Rgb8` JPEG of the same picture would then stretch at different precisions, for no reason a user could
see.

### D4. The resampler is `image`'s, and a measurement checked it

Both pairs share a kernel: `imaging.Linear` and `image`'s `Triangle` are `1 - |x|` on support 1, and `imaging.Lanczos`
and `Lanczos3` are `sinc(x)·sinc(x/3)` on support 3, with support scaled by the ratio when downsampling in both
libraries. The known difference is that `imaging` writes its horizontal pass to an 8-bit `NRGBA` before the vertical
pass, where `image` keeps floats.

**The measurement** (task 2.4) stretches `fixtures/test.dat`, which is opaque, and a synthetic translucent image with
both libraries, at 560 (bilinear) and at 512 (Lanczos). It compares the premultiplied samples each input builder would
read, in 8-bit levels, and records the maximum and the mean for each.

**The acceptance bound was 2 levels maximum, and the measurement exceeded it.** The bound assumed that rounding the
intermediate was the whole difference: half a level, amplified by about 1.3 in the vertical Lanczos pass, plus one
for the final rounding. A difference beyond 2 was to mean that alignment, support scaling or edge handling differ.
The figures, in 8-bit levels of the premultiplied samples (task 2.4 has the full table):

| source | bilinear at 560 | Lanczos at 512 |
| --- | --- | --- |
| `test.dat` (opaque) | max 1.00, mean 0.20 | max 5.0, mean 0.19 (37 of 786,432 samples beyond 1.5) |
| translucent 700×500 | max 2.28, mean 0.31 | max 49, mean 0.54 |

**The cause is `imaging`'s intermediate, not the kernels.** A control stretch used `imaging`'s own weight tables,
flattened the source against black, and kept floats between the passes. `image` agrees with it to within its final
rounding: 0.50 and 0.50 on `test.dat`, and 0.97 and 1.13 on the translucent source. So alignment, support scaling and
edges agree. The bound missed two things the 8-bit intermediate does beyond rounding:

- **It clips.** Lanczos overshoot at a high-contrast edge is clamped to [0, 255] before the vertical pass. That gives
  the opaque photograph's 37 outliers.
- **It divides by the weighted alpha sum** `Σ w·a` and stores straight colour. Beside a transparent region, the
  negative Lanczos lobes drive that sum towards zero, and the clamped quotient carries into the second pass. That gives
  the translucent source its differences of up to 49 levels. The bilinear kernel has no negative lobes, so its 2.28
  is only the separate rounding of a low-alpha pixel's colour and alpha.

**Decided with the user on 2026-09-23: keep `image`'s resampler.** Both behaviours are artefacts of a lossy
intermediate, and reproducing them would mean a second resampler that exists only to be less precise. On an opaque
photograph they reach a handful of pixels at sharp edges, by at most 5 levels of a gray input. This is recorded among the proposal's deliberate
divergences.

**The chosen filter is a named constant in each contract's file** (`ab::FILTER`, `jaipur::rgb::FILTER`), with the
measured figures in its comment.

**Rejected: porting `imaging`'s resampler.** That would put a second resampler in the crate, kept to reproduce a
rounding step that loses precision, a clamp and an unstable division. The measurement shows the kernels agree, which
was the only case in which it would have been worth it.

### D5. Each contract's adapters live at the tier its callers give it

```
  models/colorization/
      stretch.rs       family  stretched(): flatten + resize_exact (D3); both contracts
      ab.rs            family  Delhi and Mumbai: SIDE = 512, CHANNELS = 2, FILTER = Lanczos3,
                               input() = stretched, then gray() on three planes;
                               planes() = the two a/b slices of the output
      compose.rs       family  both contracts (D6)
      jaipur/
          mod.rs       model   declares rgb; profile and prose are slice 3's
          rgb.rs       model   Jaipur alone: SIDE = 560, CHANNELS = 3, FILTER = Triangle (D4),
                               input() = stretched, then ITU-601 luma on three planes;
                               chroma() = clamp to [0, 1], rgb_to_lab, keep a/b
```

**The Rgb contract is model tier because Jaipur is its only model.** Upscale set the precedent: its pass contract is
family tier because three models share it, and Osaka's diffusion contract lives in `osaka/` because one model owns it.
A second RGB-output model would promote `rgb.rs` to family tier.

**Each input builder does its own stretch**, so the filter sits beside the square it was measured for and a builder
cannot be handed an image stretched the other contract's way. `input(source) -> Vec<f32>` returns the CHW tensor at
`3 × SIDE × SIDE`.

**The chroma readers differ in ownership, and nothing reconciles them.** `ab::planes` borrows two slices of the output
tensor. `rgb::chroma` has to compute new planes, so it returns owned ones. The compose takes `&[f32]`, so each of slice
3's two contract arms passes what it holds, and no `Cow` or common return type is needed.

**The ITU-601 luma is computed as the reference computes it**: `(0.299·r + 0.587·g + 0.114·b) / 65535` in `f32`, over
the 16-bit samples. That is `Sampler::rgb`'s scale, so at 8 bits it is `Sample16`'s numbers, value for value.

**The prose travels with each constant.** For 512: DDColor was trained on the gray-Lab rendering. For 560: DeOldify's
`render_factor` 35 times its render base 16, shared by its stable and artistic generators, and why Jaipur's
re-export did not move it. For the Rgb contract: the reference transfers chroma in Lab where DeOldify used YUV, which
is perceptually equivalent and reuses the tested conversion.

### D6. The fused compose

```rust
pub(super) fn composed<T: Encoded>(
    source: &Sampler<'_>,
    (a, b): (&[f32], &[f32]),
    side: u32,
    (width, height): (u32, u32),
) -> ImageBuffer<Rgb<T>, Vec<T>>;
```

Per photograph pixel, in the reference's order:

```
  L      = lab::lightness(source.rgb(x, y))
  a, b   = bilinear::sample(plane, side, rows.tap(y), columns.tap(x))      for each plane
  linear = lab::lab_to_linear_rgb(L, a, b)
  out    = T::encode(linear[c])                                            srgb_u8 | srgb_u16
```

- **`Encoded`** is a trait in `compose.rs` over `Channel + Send`, `pub(super)` because it bounds `composed` and no
  more visible than that, implemented for `u8` (`srgb_u8`) and `u16`
  (`srgb_u16`). Slice 1 D3 left this door open ("slice 2 can add one over these two functions if its compose reads
  better with it"), and it does: it lets the depth be dispatched once, by slice 3's pipeline, as light adjustment's
  `adjusted::<T>` is. A function-pointer parameter would put an indirect call in the innermost loop.
- **Both axis tables are built once** (`Axis::new(side, width)` and `Axis::new(side, height)`), before any row runs.
- **Rows are split through `rows::for_each_row`** over the output's raw samples with `row_len = 3 * width`. Every row
  reads the shared `Sampler`, planes and tables immutably and writes only its own slice, so the split changes nothing
  in the result.
- **No alpha in the output**, as `tensor::chw_to_image` does not carry it: the composite against black has already
  happened in `Sampler::rgb`. The reference writes alpha 255, which is the same pixels in a wider buffer.
- **The only allocation is the result**, plus the two axis tables. The reference measured fusion as the difference
  between 192 MB and 48 MB at 12 megapixels. `Sampler::new`'s owned conversion for layouts it cannot borrow is the
  caller's, and is unchanged here.
- **Panics on a zero-length axis**, through `Axis::new`. Slice 3's pipeline refuses a zero-area photograph before
  anything is allocated, as every whole-image pipeline does. The doc says so.

### D7. The oracle, and what the tests pin

**The oracle is the reference's `process_reference_test.go`, ported into `compose.rs`'s test module:**

- `l_plane`: `rgb_to_lab` of every pixel as unit floats (`sample as f32 / 65535.0`).
- `resize_plane`: the reference's per-pixel bilinear resize, with its **own** coordinate arithmetic and its identity
  short-circuit. It does not use `Axis`, so the equality test also proves the promoted `Axis` against the reference's
  form.
- `compose_lab`: `lab_to_rgb`, then `T::from_unit`.

**The equality test is bit for bit, never within a tolerance.** It checks every pixel and channel, at both depths:

| Axis of variation | Cases |
| --- | --- |
| shape (`side` → `width × height`) | the reference's five: 16→64×48, 16→16×16, 16→7×5, 32→129×31, 8→1×1; plus 16→40×300, tall enough that `for_each_row` splits it on any machine with two or more cores |
| source layout | `Rgb8`, opaque `Rgba8`, translucent `Rgba8`, `Rgb16`, translucent `Rgba16`, `L8`. These cover `Sampler`'s borrowed and owned arms and both premultiply paths |
| output depth | `u8`, `u16` |

The chroma is the reference's `synthChroma`, which spans ±120 so the test runs far from the neutral axis.

Why equality can hold, and why it would be a finding if it did not: `lightness` is `rgb_to_lab`'s L bit for bit, and
`srgb_u8` is `u8::from_unit ∘ linear_to_srgb` bit for bit (both pinned in slice 1). `srgb_u16` is that composition by
definition. `sample` and `resize_plane` group the same products in the same order. At the identity axis `frac` is
zero, and `a·1 + b·0 == a` for any finite `b`.

**Also ported:**

- `resize_plane`'s own tests: a constant plane stays constant within `1e-5` at any size, and the 2×2 → 4×4 values. The
  reference's "copies rather than aliases" half is dropped, because a `Vec` return cannot alias its input.
- **Zero chroma preserves gray**, at both depths: each channel within one level of the source's gray, and within one
  level of each other. The tolerance is the reference's reason: the neutral axis holds only to about `1e-5`.
- Each contract's input and chroma tests (D5): the input is neutral and in `[0, 1]`, a pixel is spot-checked against
  the formula, gray RGB gives chroma within 0.01 of zero, and out-of-range RGB clamps rather than producing NaN.
- The stretch's tests (D3): the result is `side × side`, a borrowed `Rgb8` equals `resize_exact` directly, a
  translucent source equals the stretch of its own flattening, a 16-bit source stays 16-bit, and an 8-bit one stays
  8-bit.

### D8. The cost of the compose, measured and not acted on

Task 3.3 times `composed` on the reference's 4000×3000 benchmark photograph (its `synth` pattern) and 512-square
chroma, in a release build, at `u8` and at `u16`. It records both against the reference's 231 ms on an M2 Max. The
timing harness is temporary: the numbers go into the task's completion note and the harness does not stay in the
tree.

**What the numbers can decide is recorded, not done.** If the 16-bit compose costs enough to matter against a graph
run of 100–900 ms, slice 1's escape hatch applies: a threshold search over a coarser 16-bit table, added in `lab.rs`
with no caller changing. That would be its own change.

### Session ownership, providers and crates

**No session, memory budget or execution provider is touched.** Everything here is CPU arithmetic over buffers the
caller owns. The compose's worker threads are scoped, borrow immutably, and have all joined by the time it returns, so
slice 3 can call it from a pipeline's blocking thread without anything outliving the call. The only process-wide state
is slice 1's two `LazyLock` tables and the `available_parallelism` read that moves with `for_each_row`, all immutable
once built.

**`rust-sak`:** no feature used, and nothing missing from it. It has no resampler, and the rest is arithmetic. **No
third-party crate added.** `image`'s `resize_exact` is already a dependency.

**Division across crates:** all of it in `opai`. `gui`, `cli` and `perf` are untouched.

### Platforms validated

**Everything but the two measurements runs in the unit suite on every CI platform** (macOS, Windows, Linux), with no
ONNX Runtime, model file, GPU or network. The row split is exercised only where the runner has two or more cores, and
the serial path is exercised everywhere.

**The two measurements run by hand on macOS arm64 (M2 Max)**, as slice 1's reference figures were. No execution
provider is involved in either. Nothing here is validated on CUDA or TensorRT, because nothing here reaches them.

## Slices

Colorization, in order. **This is slice 2.** Slice 3 is proposed when it is reached.

The series was re-cut on 2026-09-23. Slice 1's table named slice 2 `add-colorization-2-pipeline`, estimated at 14
tasks. Proposing it found two promotions the tier rule requires ahead of the compose (D1, D2), which took that change
past 15 tasks. The arithmetic and the promotions became this slice, and the wiring became slice 3.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-colorization-1-lab` | `models/colorization/lab.rs`: the sRGB transfer, OpenCV-convention CIELab both ways, the `u16`-keyed linearization table with `lightness` and `gray`, the 8-bit threshold encode and the 16-bit direct encode, and the reference's colour tests plus the 16-bit ones. No behaviour change | – | **done** |
| 2 | `add-colorization-2-compose` | `inference/bilinear.rs` (promoted from São Paulo) and `inference/rows.rs` (promoted from Osaka's colour fix); the alpha-flattening stretch; the Ab contract's input and planes; Jaipur's Rgb contract's input and chroma; the fused compose at the run's depth with its separate-steps oracle; the resampler and compose-cost measurements. No behaviour change | 1 | **current** (est. 12) |
| 3 | `add-colorization-3-pipeline` | `Graph::{Ab, Rgb}` and `ColorizationVariant::graph()`; `process.rs` (zero-area refusal, stretch, one graph run, chroma, compose, progress, cancellation) and `Colorization::pipeline`; `delhi/`, `mumbai/` and `jaipur/`'s profiles with ported prose, and `ColorizationVariant::profile()`; the `Operation::pipeline` refusal arm removed; **`UnsupportedReason::FamilyNotImplemented` deleted**, with the refused-family tests, prose and public-API pin; the fake-backend suite and the live check. Specs: `colorization` (new), `execution-providers` (modified), `image-enhancement` (modified) | 1, 2 | pending (est. 12) |

### Decisions slice 3 inherits

Slice 1's list, carried forward so slice 3 takes it from a document, with what this slice and the re-cut added.

- **No tiling, stretched not fitted, two graph contracts as data** (slice 1). The two arms read their square, channel
  count, input builder and chroma reader from `ab.rs` and `jaipur/rgb.rs` (D5). Neither arm reaches `process.rs`'s
  shared steps with anything contract-specific except those.
- **The run's depth is kept, and alpha is not carried through** (slice 1). The pipeline dispatches the depth once,
  over `composed::<u8>` and `composed::<u16>` (D6).
- **All three profiles are ported, not re-measured** (slice 1): CoreML `CpuAndGpu` at FP16 and the default at FP32.
  Mumbai's and Jaipur's prose is in `mumbai.go` and `jaipur.go`. **Delhi's `delhi.go` carries no prose at all**, so
  Delhi's file says that its setting is the reference's, declared beside Mumbai's (the same DDColor architecture), and
  that the reference records no measurement for Delhi's own graph.
- **`UnsupportedReason::FamilyNotImplemented` is deleted** (slice 1). `tests/public_api.rs`'s deferral pin goes. The
  `perf/src/sweep.rs` refusal test moves to `IncompleteGraphSet`, and `perf/src/main.rs`'s example of a declined model,
  which uses Delhi, moves off colorization.
- **The refused-family tests are deleted, not repointed** (slice 1), **including the four `inference/process.rs` tests
  that use a refused operation to test what a refusal does**. Those are
  `a_chain_whose_second_operation_cannot_be_served_is_refused_whole`,
  `a_chain_whose_second_operation_is_unsupported_installs_nothing_for_its_first`,
  `an_unservable_chain_is_refused_even_when_its_first_operation_is_already_stored` and
  `a_refused_chain_is_recorded_with_what_was_refused_and_why`, together with their `unservable()` fixture. Decided with
  the user on 2026-09-23. No operation is refusable once colorization runs, `IncompleteGraphSet` cannot be reached from
  a test, and a test-only refusal seam in `plan` was declined.
- **`image-enhancement` takes a delta.** Its "refused rather than served wrongly" requirement promises a "this cannot
  be run yet" answer and a scenario for a family with no pipeline. Nothing produces either once the variant is
  deleted. `autopilot`'s standing rule and `performance-benchmark`'s declined-model requirements are written over
  whatever cannot run, so they need no delta.
- **Autopilot is unchanged** (slice 1). Only the doc comment naming `FamilyNotImplemented` changes.
- **The progress schedule is slice 3's to choose.** The reference reports only 0 and 1. Light adjustment's reasoning
  for counted even steps over a fixed-cost graph plus photograph-sized work applies here too.
- **Every `expect(dead_code)` this series added comes off in slice 3**: `lab`'s, and this slice's on `stretch`, `ab`,
  `compose` and `jaipur`.

## Risks / Trade-offs

- **[A promotion silently changes what São Paulo or Osaka renders]** → Both suites move or stay with no assertion
  edited (D1, D2), and any assertion that needs an edit stops the task. `colorfix`'s tests already pin the row split as
  bit-identical to the serial form. São Paulo's live check is not re-run, because the moved code is identical and its
  unit suite pins the tables numerically.
- **[The fused compose drifts from the separate steps, and slice 3 renders a slightly wrong photograph with no
  error]** → D7's equality is bit for bit over six shapes, six source layouts and two depths. The oracle's resize has
  its own coordinate arithmetic, so it does not share a bug with `Axis`.
- **[`image`'s resampler differs from `imaging`'s by more than rounding]** → Measured, and it does: by up to 5 levels
  on an opaque photograph and 49 beside transparency (D4). A control run with `imaging`'s own weights showed the cause
  is `imaging`'s clipped, alpha-divided 8-bit intermediate, not the kernels. Kept deliberately, as a recorded
  divergence.
- **[Flattening changes what a translucent photograph's gray rendering looks like, compared with the rest of the
  crate]** → Deliberate (D3). It matches the reference's alpha-weighted sums. `present`'s different choice for the
  other whole-image families is left as it is and recorded as a non-goal.
- **[The row split is untested on a single-core runner]** → The serial path is what such a runner exercises, and the
  split is `for_each_row`'s, whose own tests pin it. The 40×300 case exercises it wherever two cores exist.
- **[Slice 2 delivers no user-visible value, and the series grew from two slices to three]** → Accepted, as slice 1
  accepted it. The project stays working throughout, and slice 3 starts from arithmetic that is already proven.
