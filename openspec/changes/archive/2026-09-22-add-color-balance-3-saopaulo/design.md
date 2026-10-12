## Context

See proposal.md — *Why*. What follows is only the state that shapes the decisions.

Two slices have landed, and between them they have already placed everything this one does not have to write:

```text
  inference/present.rs          plan(w, h, canvas) -> Plan            the rounding fit and the extension
                                presented(src, plan, canvas,          Lanczos3 to the planned size, reflection-padded
                                          norm, &mut dest)            into planar CHW
  inference/mix.rs              blended(src, corrected, bias)         the photograph moved by a control
  color_balance/samples.rs      Samples::new(tensor, offset, canvas,  the cropped CHW->HWC reader, at a PLANE OFFSET
                                             crop, norm)
  color_balance/mapping.rs      fit(operation, source, destinations)  the 11-term kernel and the MULTI-DESTINATION
                                Mapping::evaluate(rgb)                11x11 ridge solve, one elimination
```

The two family-tier files carry a plane offset and a destinations slice that have had exactly one caller passing zero
and one since slice 2, and slice 2's design records why they were placed there rather than under `rio/`: **this
slice is the second caller they were written for, and it owes no promotion.** That claim is what this slice tests
first.

What remains missing is all of it São Paulo's own. `ColorBalanceVariant::canvas()` answers a literal `656` for this
variant — slice 2's note says "until it has a file to hold it and the prose that goes with it" — and `profile()`
answers `EpProfile::default()`, which in this project spells *nobody has measured this graph* and is, for this model,
wrong in a way worth fixing: it **was** measured, and the defaults are the answer.

The reference's mixed-illuminant half divides as follows:

```text
  colorbalance/mixed.go       REACHED BY          THIS SLICE
  --------------------------  ------------------  --------------------------------------------
  planCanvas                  rio + saopaulo      slice 1 (inference/present.rs)
  chwToHWC                    rio + saopaulo      slice 2 (color_balance/samples.rs)
  kernelP, fitPolynomialMappings, mapPixel        slice 2 (color_balance/mapping.rs)
  utils.BlendWithIntensity    rio + saopaulo      slice 1 (inference/mix.rs)
  MixedSpec (variant.go)      saopaulo            model tier (saopaulo/mod.rs)
  sampleAxis                  saopaulo            model tier (saopaulo/weights.rs)
  blendWeighted               saopaulo            model tier (saopaulo/process.rs)
  ProcessMixed                saopaulo            model tier (saopaulo/process.rs)
```

## Goals / Non-Goals

**Goals:**

- Make `cb_saopaulo_fp32` and `cb_saopaulo_fp16` produce a photograph, through a contract that is São Paulo's and a
  vocabulary that is already the family's.
- Close the family: no colour balance operation refused, and the seam's refusal arm gone.
- Reproduce the reference's arithmetic wherever a difference would be a different photograph, and say here where it
  deliberately is not.

**Non-Goals:**

- **No change to `samples.rs`, `present.rs` or `mix.rs`, and no change to what `mapping.rs` answers.** If any of them
  needs editing to serve this model, the slice that placed it got the boundary wrong and that is the finding, not a
  patch. The one exception is D5's: `Mapping::evaluate` is split into `kernel` + `evaluate_kernel` so the fused pass
  can build the features once, with `evaluate`'s answer unchanged bit for bit.
- **No `gui` change.** The add-enhancement menu's `RUNNABLE_ENHANCEMENTS` gate still names face recovery and upscale
  and is pinned to that pair by `lib/enhancements.test.ts`. Widening it — to colour balance *and* to light
  adjustment, which it has not offered since Paris and Lyon landed — is a front-end change with its own screen
  behaviour and its own series.
- **No re-derivation of the graph's own properties.** The multi-scale ensemble, the dropped decoder columns, the
  spliced `Clip` that takes the FP16 graph to one CoreML partition: all of them are properties of the published
  `.onnx`, which is the same file the reference runs. Nothing here re-exports anything.
- **No threading.** See D6.

## Decisions

### D1 — One model directory, three files, and nothing promoted

```text
  models/color_balance/
    samples.rs     FAMILY   (slice 2, untouched)   rio + saopaulo
    mapping.rs     FAMILY   (slice 2, evaluate split in two — D5)   rio + saopaulo
    rio/           MODEL    (slice 2, untouched)
    saopaulo/
      mod.rs       MODEL    CANVAS, the channel layout, profile, the prose
      weights.rs   MODEL    the align-centres axis tables and the weight-plane read
      process.rs   MODEL    São Paulo's contract, end to end
    live.rs        FAMILY   gains São Paulo's checks
```

**Why `weights.rs` stays at model tier with the doctrine's promotion rule in view.** It has one caller and no second
one is named in this family: Rio has no weight map, and the rule parks a file at model tier until a second caller
appears. The near-miss worth recording is `models/face_recovery/composite.rs::Taps`, which is also a bilinear read of
a square plane — and it is a different function, not the same one written twice. `Taps` resolves the geometry **per
pixel** from an arbitrary `(x, y)` because a face lands at a sub-pixel offset under an affine warp; this mapping is a
pure axis-aligned scale, so both axes are separable and are resolved **once per axis for the whole photograph**,
which is the entire point of the shape. Promoting either into the other would put per-pixel `floor`s back into a loop
that exists to have hoisted them.

**The second caller is named and is not in this family.** The reference's own comment on `sampleAxis` says it is a
copy of the colorization package's helper and that a third caller should promote it. Colorization — Delhi, Mumbai,
Jaipur — is unwritten here, and when it lands it is a **second family**, which under this project's rule promotes
this file to cross-family tier (`inference/`) in the change that introduces that caller and before it is written.
Stated here so the promotion is taken from a document rather than rediscovered.

*Alternative considered: `weights.rs` inside `process.rs`.* Rejected only on testability — the axis tables are the
one piece of this model whose correctness is a table of numbers a test can write down, and a separate file is what
lets that test sit beside them rather than inside a suite about photographs.

### D2 — The channel layout is named in the model's own file, and pinned without a session

```text
  input   [1, 3, 656, 656]   the reflection-padded square
  output  [1, 9, 656, 656]   3 weight planes (daylight, shade, tungsten), then shade's RGB, then tungsten's
```

`SETTINGS = 3` is the only number; `RENDERINGS = SETTINGS - 1`, `CHANNELS = SETTINGS + 3 * RENDERINGS` and
`rendering_offset(i) = SETTINGS + 3 * i` are derived from it, as the reference's `MixedSpec` derives them. Daylight
is not synthesized: it is the photograph, so a graph returning it would hand back what it was given.

**These are the contract between the conversion script and this code**, not an implementation detail, and getting one
wrong means reading a weight plane where a rendering is and rendering nonsense from correctly loaded weights. Named
rather than inlined so a test pins the layout with no session open — the same reason upscale's `GraphSpec` makes its
naming rule a method.

**No seam widens for the nine channels.** `Backend::run_graph` already takes an input shape and an output shape
separately, and `GraphShape` already carries a channel count — it is the packed transformer input and the
sixteen-channel latent that put it there. This model declares `GraphShape::new(9, canvas, canvas)` for its output and
nothing else changes.

*Alternative considered: a `MixedSpec`-shaped value at family tier, as the reference has.* Rejected. The reference
needs one because its `Variant` is a data struct shared by both models and the layout has to travel in it; here the
layout belongs to one export, in that export's directory, and a family-tier type parameterised by a constant that
only one model supplies is a shared name for an unshared thing.

### D3 — The weight map is upsampled by hand-rolled align-centres bilinear, resolved once per axis

This is the decision slice 2 explicitly deferred to here. Two candidates, and they are **not the same filter**:

- **Hand-rolled**, as the reference: for destination index `i`, `s = (i + 0.5) * src_len / dst_len - 0.5`, clamped at
  zero, `lo = min(floor(s), src_len - 1)`, `hi = min(lo + 1, src_len - 1)`, `frac = s - lo`. Four taps, weights
  summing to one, clamped at both edges.
- **`image`'s `Triangle` filter** over an `ImageBuffer<Luma<f32>>` per plane. A support-based resampler with its own
  normalisation and its own edge handling, which is a *different* answer at every pixel and a materialised
  full-resolution plane per setting — three of them, ~290 MB at 24 megapixels — for a value the fused pass consumes
  immediately.

**Hand-rolled wins on correctness before it wins on cost.** The weight map is the un-padded region of a canvas built
by a pixel-centre-aligned resample (slice 1's `presented`, through `image`'s `resize_exact`), so centre alignment is
the geometrically consistent *inverse* of the resize that produced it. Align-corners would re-map corner to corner
and shift by up to half a pixel against the photograph the weights are meant to line up with. The reference is
explicit that the convention inside the graph is the opposite and has to be — there it must match how the checkpoint
was trained — but nothing downstream of the graph is trained, so the resize that made the canvas is what governs.

**The tables are computed once per axis**, before the pixel loop: `lo`, `hi` and `frac` for every destination column
and every destination row. Neither depends on pixel data, and both are re-read once per row and once per column
respectively.

**Bilinear interpolation is a convex combination of four samples, so it commutes with the sum over the planes**: a
weight vector that is a partition of unity at each of the four taps is still one after interpolation. That is why the
blend needs no renormalising, and it is stated here because the alternative — renormalising defensively — would hide
a layout error rather than fix one.

*Alternative considered: sampling the weight map nearest-neighbour.* Rejected; it is what produces a visible blocky
boundary between illuminants at the canvas's own 656 grid, which is the thing the upsample exists to avoid.

### D4 — One solve for both renderings, which is what slice 2 built

`Samples::new(&output, rendering_offset(i), canvas, crop, RANGE)` for each rendering, one `Samples` over the input
scratch for the source, and **one** `mapping::fit` taking both destinations. `XᵀX` depends only on the source, so the
`O(N·121)` accumulation — very nearly the whole cost — is paid once: the reference measures 32 ms for two fitted
together against 53 ms fitted apart at this canvas.

The plane offset and the destinations slice are used exactly as slice 2 wrote them. **If either needs a change here,
slice 2's central placement decision was wrong**, and that is a finding for the apply phase to report rather than
something to patch quietly.

`RANGE` is declared in `saopaulo/mod.rs` as `Normalisation::Unit`, not inherited from `rio/process.rs` where the same
constant lives. It is a property of an **export**, and the second export says so where it is written. (Rio's is
`pub(super)` within `rio`, so this is not reachable anyway — which is the visibility doing the right thing.)

### D5 — One fused full-resolution pass, and the clamp is inside it

```rust
// per pixel, once:
let k = kernel(r, g, b);                    // 11 features, built once and reused across both mappings
let mut out = w[0] * source;                // daylight is the photograph itself
for (i, mapping) in mappings.enumerate() {
    out += w[i + 1] * clamp01(mapping.evaluate_from(k));
}
```

**Fused rather than three materialised renderings.** At 12 megapixels each rendering would be a ~144 MB float buffer
and there is nothing to do with them afterwards but multiply and add. The reference measures the fusion at 1008 ms
and 672 MB down to 56 ms and 48 MB, where the 48 MB is the output image — the working set becomes the result rather
than three intermediates plus it.

**The features are built once per pixel and reused across both mappings.** That is most of the arithmetic — eleven
terms including three multiplies — so evaluating the kernel per rendering would very nearly double the loop for
nothing. `Mapping::evaluate` takes an `rgb` triple and builds the kernel itself, which is right for Rio's one
mapping; this pass needs the *other* half of that function, so the kernel is built here and the weights applied to
it. Whether that is a second method on `Mapping` or the existing one refactored into two is an apply-phase call, with
one constraint: **`evaluate`'s observable answer does not change**, and `mapping.rs`'s existing tests are what say so.

**`clamp01` per rendering, before the weight multiplies it, not on the blend afterwards.** The upstream pipeline
materialises the renderings and clips them, and an 11-term polynomial extrapolates well outside `[0, 1]` on saturated
pixels — so clamping only at the end lets a blown highlight pull the blend somewhere no rendering goes, by however
much its own weight allows. This is **not** the same bound as `Channel::from_unit`'s, which comes later and bounds
the *sum*; both are needed and they are not redundant.

**The daylight term is not clamped**, because the photograph's own channels are in range by construction.

**The accumulation is written plainly — no `mul_add`** — which is slice 2's D6 rule inherited: an accumulation of
many products is written plainly, a single fused term may use `mul_add`. The reference's `mixed.go` writes explicit
`float32(...)` conversions around every product specifically to *forbid* Go's permitted contraction, and records why:
one ulp falls on the other side of an exact integer once truncated to 8 bits and changes the byte. Rust does not
contract implicitly, so writing it plainly reaches the reference's behaviour by doing nothing. The bilinear sums keep
the reference's grouping too — along x within each row, then between the rows.

### D6 — The pass runs on one thread, and the promotion path is named rather than taken

The reference splits `blendWeighted` across `runtime.NumCPU()` row bands. **That is not carried**, and it changes no
pixel: the rows are strictly row-independent and each pixel's arithmetic is unchanged, so this is a wall-clock
property alone — roughly 0.5–1 s on a 24-megapixel photograph.

The reason is the doctrine rather than the difficulty. The one parallel CPU pass in this crate is
`models/upscale/osaka/colorfix.rs::for_each_row`, at **model tier**, written against `&mut [f32]` planes. Using it
here means promoting it to cross-family tier and generalising it over a row-chunked buffer of any element type,
ahead of this slice's own work — and writing a second `std::thread::scope` band loop inside `saopaulo/` instead is
the copy the rule exists to forbid. Neither is worth spending this slice on: every other full-resolution pass in this
crate is serial, including Rio's apply, face recovery's composite and light adjustment's.

**Recorded as an open parity gap** in proposal.md, with the promotion named, so the change that wants the speed knows
what it is buying and what it has to move first.

### D7 — Five progress steps, five cancellation checks

```text
  1  presented    fixed cost: the square, whatever the photograph is
  2  run          fixed cost: one graph run
  3  fit          fixed cost: bounded by the canvas — two destinations, one elimination
  4  blend        proportional to the photograph: the fused weighted pass
  5  bias         proportional to the photograph: `inference::mix::blended`
```

Five, as Rio's are, and the same count for a different reason: Rio's apply and blend are two passes, while here the
apply *is* the weighted blend and the fifth step is the bias. Counted rather than accumulated so the last lands
exactly on the end of the operation's share. The boundaries double as the cancellation points, and a cancelled run
returns **no image at all** — not the photograph, and not the partly blended buffer.

### D8 — Depth is dispatched once at the top

`run` matches `ChannelDepth` once and calls a `blended::<T, B>` monomorphised over `Channel`, exactly as Rio's
`corrected`, `Adjust::adjusted` and `restore` do. The fused pass walks the photograph's own resolution — twenty-four
million iterations on a 24-megapixel source, each evaluating two polynomials — and it is the clearest case in the
crate for hoisting the branch. The fit is `f32`/`f64` throughout and is not monomorphised.

This is also where the reference is diverged from deliberately: `blendWeighted` writes into an `image.NewRGBA`, so a
16-bit photograph comes back at 256 levels per channel whatever the caller asked for. The family's spec already
requires otherwise, Rio already honours it, and this model does the same.

### D9 — The family seam loses its refusal and keeps its `Result`

`ColorBalance::pipeline`'s São Paulo arm becomes `Ok(saopaulo::process::pipeline(...))`. The signature does not
narrow, which is the decision slice 2 recorded when it widened it: *a seam that cannot refuse is the exception here
rather than the rule, and re-widening a signature later is a worse trade than carrying an `Ok` for one arm.* Three of
the remaining families will need this seam to refuse again as they land one variant at a time.

`Operation::pipeline` needs no change — colour balance left its refusal arm in slice 2 — but its test's `served`
predicate does, and that predicate is the crate-wide statement of which operations run. São Paulo moves from the
refused list to the served one, and the test that walks **every** operation the library can name is what proves
nothing else moved with it.

### D10 — The profile is a measured default, reached through the model's own file

`ColorBalanceVariant::profile()`'s São Paulo arm stops answering `EpProfile::default()` inline and calls
`saopaulo::profile(precision)`, which returns the defaults — with the measurement and its conditions in the doc
comment above it. `canvas()`'s arm likewise stops carrying a literal and reaches `saopaulo::CANVAS`.

**The value is identical and the change is not cosmetic.** A model nobody measured and a model measured to want the
defaults are the same configuration and different facts, and the second one only exists somewhere a reader can find
it if the arm reaches the model's file. The prose that goes there is the load-bearing part: nothing beats the CoreML
defaults at this canvas; fast prediction's 3.3% at FP16 sits inside that row's own 6-7% spread; execution mode is a
wash even though this application runs the parallel mode; and restricting the graph to the CPU and Neural Engine
costs +62% at FP16 and +732% at FP32 — the same conclusion Rio reaches from the opposite direction, which is the
contrast that makes carrying a profile sideways within one family a measurement nobody took.

Anyone re-measuring must confirm first that both graphs are still one CoreML partition, **on the runtime this
application pins** — on a newer runtime the stray input `Cast` the FP16 export needed a spliced `Clip` for is
absorbed, so a check run there silently passes a graph the shipping runtime splits.

### D11 — Sessions, ownership, lifetimes and the peak

Nothing new in kind. The pipeline type, `Mixed` (as Rio's is `Balance`, named for its contract), holds `[ArtifactId; 1]` and one `EpProfile` and answers `Model::required` and
`Model::sessions` with them — the single-graph shape of every family but upscale. The session is borrowed as a
`SessionHandle` for the duration of `ImagePipeline::run` and never stored, so the registry's lease outlives the run
by construction.

The one number that differs from Rio's is the output tensor: **nine** planes of 656² floats is **15.5 MB** against
Rio's 5.2 MB, plus the 5.2 MB input scratch. The scratch and the source view are dropped after the fit; the
**weight planes are not** — the first three planes of the output are read throughout the fused pass, so the output
tensor lives until the result is built. Peak is therefore the photograph's own buffers plus ~15.5 MB, still smaller
than any model in the catalogue that tiles. No memory budget changes.

### D12 — What proves this, and on which platforms

**On every CI platform, against a fake backend** answering nine planes it was told to answer:

- the channel layout — `CHANNELS`, `rendering_offset`, and a test that a rendering read at the wrong offset produces
  a different photograph, which is what makes the offsets worth pinning;
- the axis tables against hand-computed numbers, including both edge clamps and the exact-integer case where a
  destination pixel lands on a source centre;
- the weight read: a map that is all-daylight returns the photograph; a map that is all-shade returns shade's global
  mapping; a map split down the middle produces a boundary at the photograph's own middle rather than displaced by
  half a source pixel;
- the clamp: a fake rendering whose fit extrapolates past 1 does not pull the blend past what the renderings span;
- one solve for both renderings — the two mappings recovered are the ones each rendering gives alone;
- the fused pass's bounding, the five-step schedule, cancellation at each of the five boundaries, the zero-area
  refusal, the depth dispatch at 16 bits, and **both** variants reaching a pipeline through the family seam.

**By hand, on macOS/CoreML, against the real weights**, joining `models::color_balance::live`: the result carries the
photograph's own dimensions; a bias of 0 returns the photograph unchanged; a positive and a negative bias move mean
chromaticity in opposite directions; and — the check that is São Paulo's rather than the family's — the result's
**luminance structure tracks the photograph's**, which a weight plane read as a rendering, or a rendering read as a
weight, destroys while still producing an image.

**Validated against:** macOS on CoreML and the CPU provider, at both precisions. **Untested here:** Windows, Linux,
CUDA and TensorRT. Nothing in this slice is provider-specific — the profile names no option at all and everything
after the graph is arithmetic over `image` buffers with no conditional compilation — so the CPU-provider path is what
the other platforms run and the fake-backend suite covers it on every runner.

### Crates and `rust-sak`

**No new crate, first-party or third-party.** `rust-sak`'s surface is `crypto`, `fetch`, `fs`, `image`, `image-raw`,
`memo`, `o11y` and `sysinfo`; none of them is a resampler whose alignment convention has to be the exact inverse of
another resample's, and it should not become one — that is a property of this pipeline rather than something another
project could want. **No gap, so nothing to fill and no `rust-sak` change proposed.** `image`'s `Triangle` filter is
rejected on correctness in D3, not on dependency grounds; `image` remains the resampler that *builds* the canvas,
through slice 1's `presented`.

### Division across crates

**`opai` only.** `gui` reaches the library through `Opai::process` and the catalogue and has no line to change; its
own gate on which families the menu offers is a front-end decision with its own screen behaviour. `cli` likewise.
`perf` already names a colour balance operation from a codename, a precision and a bias — `crates/perf/src/select.rs`
has the arm — so the confirming sweep is a command rather than a change. No `gui`, `cli` or `perf` task appears in
tasks.md.

## Slices

Colour balance, in order. This is slice 3, and it closes the series.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-color-balance-1-canvas` | `inference/present.rs` and `inference/mix.rs`: the fixed-canvas plan and presentation, and the per-run control blend, promoted out of light adjustment's family tier with their tests and prose. `presented` gained a `norm` parameter. No behaviour change | – | **done** (archived 2026-09-22) |
| 2 | `add-color-balance-2-rio` | The cropped CHW→HWC sample reader; the multi-destination 11×11 polynomial solver; Rio's full-resolution apply; Rio's pipeline; `rio/` with its 656 canvas and measured FP16 profile; the variant seams; `ColorBalance::pipeline` serving Rio and refusing São Paulo; the `Operation::pipeline` refusal arm; `InferenceError::ColourMapping`; the fake-backend suite, live check and confirming sweep. Specs: `color-balance` (new), `execution-providers` (modified) | 1 | **done** (archived 2026-09-22) |
| 3 | `add-color-balance-3-saopaulo` | The 9-channel output layout (3 settings + 3×2 renderings); the align-centres bilinear weight-plane upsample; the fused weighted blend; São Paulo's pipeline; `saopaulo/` with its measured *default* profile; the family seam losing its refusal; the live check and confirming sweep. Specs: `color-balance` (modified), `execution-providers` (modified) | 2 | **current** (est. 12) |

### What this slice leaves for others, and to whom

- **The GUI does not offer colour balance**, and has not offered light adjustment since Paris and Lyon landed.
  `RUNNABLE_ENHANCEMENTS` and `lib/enhancements.test.ts` are where that lives. It is a `gui` change and its own
  series, not a follow-up to this one.
- **The fused pass is single-threaded** (D6). The change that wants the reference's wall-clock promotes
  `osaka/colorfix.rs::for_each_row` to `inference/` first.
- **`weights.rs` is at model tier** (D1). Colorization is the second caller that promotes it, in the change that
  introduces it and before that caller is written.
- **Upstream's bilateral post-processing of the weight map is not implemented**, exactly as the reference does not
  implement it: it cannot be expressed in ONNX and a sparse-CG solve at full resolution is out of proportion to what
  it buys on a map that is already smooth. It costs a little edge snapping at illuminant boundaries. A guided filter
  would recover most of it if it ever proves worth having.

## Risks / Trade-offs

**[A channel read at the wrong offset is a photograph and no error]** → The new risk this slice adds, and the one the
previous two did not have: half this graph's output is *shown* rather than sampled into a fit. A weight plane read as
a rendering, a rendering read as a weight, or the two renderings swapped against their weight planes each produces an
image. Three nets: the layout is derived from one constant and pinned by a test with no session open; the
fake-backend suite drives a map that selects each rendering in turn, where a swap is a *different colour* rather than
a slightly different one; and the live check's luminance-structure property fails on any of them against real
weights.

**[The upsample is aligned at corners instead of centres]** → A half-pixel shift of the whole weight map against the
photograph, invisible on a smooth map and visible as a displaced boundary on a sharp one. Pinned by the axis-table
tests against hand-computed numbers and by the split-map boundary test, which is the one assertion in the slice that
fails on exactly this mistake and nothing else.

**[The clamp moves from each rendering to the blend]** → A blown highlight pulled somewhere no rendering goes, on
saturated pixels only, which is where nobody looks first. Pinned by a fake rendering whose fitted mapping
extrapolates past 1 (D5).

**[`Mapping::evaluate` is refactored to expose the kernel and changes an answer]** → It is Rio's apply, so the
regression would be in a shipped model rather than in this one. `mapping.rs`'s existing tests — the exactly-linear
map recovered to round-off, the permuted feature order failing — run unchanged, and the constraint is stated in D5
rather than left to be inferred.

**[São Paulo's profile is the defaults, so nothing about it is exercised by configuration]** → Accepted, and it is
why the requirement states the *measurement* rather than the absence. The confirming sweep is a task: if it finds
something that beats the defaults outside its own run-to-run spread, the profile gains it and the requirement
changes with it.

**[The fused pass is serial and a 24-megapixel photograph pays ~0.5–1 s for it]** → Accepted on D6, deliberately,
with the promotion path named. It changes no pixel, so the remedy is available to any later change without touching
this contract.
