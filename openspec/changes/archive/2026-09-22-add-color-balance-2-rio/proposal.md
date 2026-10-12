## Why

Colour balance is one of the four families [`Operation::pipeline`](crates/opai/src/models/operation.rs) still refuses
by name, and slice 1 has already put the half of its contract that is not its own where it can be reached: the
fixed-canvas plan and presentation in `inference/present.rs`, the per-run control blend in `inference/mix.rs`. What is
left between them is the part that is this family's alone, and Rio is the model that needs all of it — the crop that
drops the reflection before anything is fitted, the 11-term polynomial colour mapping solved against the graph's own
output, and the evaluation of that mapping over the photograph's own pixels.

This is **slice 2 of three**. It makes one of the eight families' twenty operations runnable that was not: after it,
`cb_rio_fp32` and `cb_rio_fp16` produce a photograph and São Paulo is refused alone, which is exactly the state face
recovery sat in for the one slice between Athens and Santorini.

## What Changes

**The family's shared vocabulary — two new family-tier files, both written for São Paulo as well as Rio.**

- **`models/color_balance/samples.rs`** — the cropped CHW→HWC reader. The graph's square holds the photograph in its
  top-left corner and a mirror of the photograph's own edge in the rest; this unpacks the top-left `scaled_width` x
  `scaled_height` region of a planar CHW tensor into flat RGB triples in unit space, decoding through the
  [`Normalisation`](crates/opai/src/inference/tensor.rs) the family declares rather than assuming one.

  **The crop is the point of it, not an optimisation.** The padded columns are a mirror of the photograph's own edge,
  and this is a *global* fit, so leaving them in re-weights the border content against the rest of the picture and
  moves every pixel of the result. The reference measured that over 72 photographs — three sources, six aspect ratios,
  four illuminant casts, scored against what the dynamic-shape graph rendered — and cropping first takes the median
  from **47.9 dB to 52.7 dB** and the worst case from 38.8 dB to 41.0 dB. It costs nothing: the extension is only ever
  on the right and the bottom, so the wanted region is already contiguous from the origin.

- **`models/color_balance/mapping.rs`** — the 11-feature kernel `[r, g, b, rg, rb, gb, r², g², b², rgb, 1]`, the
  multi-destination solver for `W = (XᵀX + λI)⁻¹XᵀY`, and the evaluation of one fitted mapping at one pixel. The solve
  accumulates in `f64`, adds a `1e-8` ridge to the diagonal, and runs Gauss-Jordan elimination with partial pivoting,
  exactly as the reference does.

  **Written in its multi-destination form now, with Rio passing one destination** — the decision slice 1 recorded and
  this slice inherits. `XᵀX` depends only on the source, so the `O(N·121)` accumulation is shared across destinations
  and is very nearly the whole cost; the reference measures 32 ms for two fitted together against 53 ms fitted
  separately, and records that the single-destination case is bit-for-bit unchanged by the generalisation. The
  alternative is rewriting the ridge, the pivoting and the elimination in slice 3 — and that loop is the one place in
  this family where a mistake renders a plausible photograph and no error.

**Rio, the model — `models/color_balance/rio/`.**

- `mod.rs`: the **656** square its graph was exported at, the profile measured for it, and the prose behind both.
- `process.rs`: Rio's contract, and Rio's alone. Present onto the square, one graph run, read the un-padded source and
  output samples back out of the two tensors, fit one mapping, evaluate it over the photograph's own pixels at the
  depth the caller asked for, and move the photograph towards the result by the run's [`Bias`] through the
  `inference::mix::blended` slice 1 promoted.

**The seams.**

- `ColorBalanceVariant` gains `canvas()` and a real `profile()`, both matches reaching the model's own file — the
  shape [`LightAdjustmentVariant`](crates/opai/src/models/light_adjustment/variant.rs) already has. Both variants
  report 656; only Rio's profile is a measurement.
- `ColorBalance::pipeline` — the **family's contract seam**, one arm per contract. Rio's reaches `rio::process`;
  São Paulo's refuses, so this seam returns a `Result` where light adjustment's and face recovery's cannot fail. Slice
  3 replaces that arm with the mixed contract and the `Result` stays, because a seam that cannot refuse is the
  exception rather than the rule here.
- `Operation::pipeline` loses `Self::ColorBalance(_)` from its refusal arm and hands to the family.

**A new `InferenceError` variant.** The ridge makes `XᵀX` positive-definite, so after partial pivoting the pivot should
never be zero — and *should* is not a guarantee in `f64`. Dividing by it fills the weights with NaN, which the apply
renders as a destroyed image with nothing anywhere to say why. The reference fails loudly and so does this: a variant
naming the operation and the column, raised rather than rendered. `InferenceError` is `#[non_exhaustive]`, so this is
an addition rather than a break, but it is a public API change and
`crates/opai/tests/public_api.rs` records it.

### Deliberate divergence from the reference implementation

**One, and it is already shipped.** The per-run control is a [`Bias`] on `-1.0..=1.0` where the reference's `intensity`
is `0..1`; the negative half extrapolates past the photograph and has no counterpart there. It was published by the
catalogue before this slice and is stated in this slice's spec because this is where it first does something.

**Everything else matches the reference's behaviour**, including the figures that decide it: the 656 square, the crop
before the fit, the `f64` accumulation, the `1e-8` ridge, the partial pivoting and the loud failure on a singular
system. Structural divergence is expected and is not called out — the sample reader and the solver sit at family tier
here where Go has them as package-level functions beside Rio's own pipeline, and Rio's contract sits in its own
directory where Go has `process.go` and `mixed.go` side by side.

**One thing is *not* a divergence although it reads like one.** The reference's `applyMapping` carries a fast path over
a raw pixel buffer and a generic fallback through `image.At`, with a comment arguing the two are output-identical.
There is one path here, because [`Sampler`](crates/opai/src/inference/tensor.rs) already is that fast path for every
variant the loader produces — and it was written because the reference's 16-bit handling was wrong for years in the
other direction.

### The one decision slice 1 deferred to this slice's design

`mul_add` versus separate roundings in the full-resolution loop. Go permits an implementation to contract `x + y*z`
and the reference writes explicit conversions in `mixed.go` to forbid it; Rust never contracts implicitly, so the
reference's behaviour is the Rust default and the polynomial accumulation is written plainly. `blended` keeps the
`mul_add` it already ships with. See design.md D6.

### Task estimate

**14**, against the 15-task budget. Slice 1 estimated 13; the extra one is the new `InferenceError` variant and its
public-API snapshot, which that estimate folded into the solver. Slice 3 is unchanged at 11.

### Infrastructure: what `rust-sak` already covers

**Nothing applies, and nothing was expected to.** This slice adds no hashing, HTTP, retries, downloads, extraction,
temp files, config directories, image codecs, caching, telemetry or hardware probes. Its arithmetic is an 11x11 dense
solve and a per-pixel polynomial, and `rust-sak`'s surface — `crypto`, `fetch`, `fs`, `image`, `image-raw`, `memo`,
`o11y`, `sysinfo` — contains no linear algebra and should not: a fixed 11x11 Gauss-Jordan written against this
family's sample layout is not infrastructure another project could want. **No new third-party crate either**, and
specifically no `nalgebra` or `ndarray`: the system is a fixed 11x11 with a hand-written pivot order that the
reference's own comment makes load-bearing, and a general solver would be a dependency taken to hide 40 lines while
making the divergence from the reference unreadable. The live check reaches the real weights through the installer
this crate already has.

## Capabilities

### New Capabilities

- `color-balance`: what a colour balance run does to a photograph — the fixed square it is presented at, why the
  extension is dropped before anything is fitted, why the result is the photograph mapped rather than the model's
  output enlarged, what the bias means in each direction, what a run reports and stops on, and what it refuses.
  Written for the family; the requirement that says which variants run names Rio alone and says São Paulo is refused,
  which slice 3 replaces.

### Modified Capabilities

- `execution-providers`: one requirement added — **Rio's measured settings**. It is the first model in the catalogue
  to ask CoreML for the Neural Engine *and* be right to, which is the load-bearing contrast with Paris and Lyon, and
  the second to declare CoreML's fast-prediction specialization.

## Impact

**Crates touched: `opai` only.** The GUI's add-enhancement menu gates on its own `RUNNABLE_ENHANCEMENTS` list, which
names face recovery and upscale today and is pinned to that pair by `lib/enhancements.test.ts`; widening it to
colour balance — and to light adjustment, which it does not offer either although the library has served both of its
variants since the slice before this one — is a `gui` change with its own screen behaviour and is not this slice's. `perf`
already builds a colour balance operation from a codename, a precision and a bias — `crates/perf/src/select.rs` has
the arm — so the confirming sweep needs no code at all, which is the property that arm was written for. `cli` has no
line to change.

| File | Change |
| --- | --- |
| `crates/opai/src/models/color_balance/samples.rs` | new — the cropped CHW→HWC sample reader |
| `crates/opai/src/models/color_balance/mapping.rs` | new — the kernel, the multi-destination solver, the per-pixel evaluation |
| `crates/opai/src/models/color_balance/rio/mod.rs` | new — `CANVAS`, `profile`, and the prose behind both |
| `crates/opai/src/models/color_balance/rio/process.rs` | new — Rio's contract and its fake-backend suite |
| `crates/opai/src/models/color_balance/live.rs` | new, `#[cfg(test)]` — the live check against the real Rio weights |
| `crates/opai/src/models/color_balance/variant.rs` | `canvas()` added; `profile()` stops answering `EpProfile::default()` for Rio |
| `crates/opai/src/models/color_balance/mod.rs` | `pipeline()` — the family's contract seam; the module declarations |
| `crates/opai/src/models/operation.rs` | colour balance leaves the refusal arm; `served` in its tests gains it |
| `crates/opai/src/inference/process.rs` | **tests only** — the driver's own `served` predicate, back to per-operation |
| `crates/opai/src/error.rs` | one `InferenceError` variant for a singular fit |
| `crates/opai/tests/public_api.rs` | the new error variant |
| `crates/opai/src/inference/present.rs`, `mix.rs` | **untouched** — this slice is their second caller, which is what slice 1 was for |

**Public API.** One added `InferenceError` variant on a `#[non_exhaustive]` enum, and two operations that used to be
refused now run. Nothing is removed or re-spelled.

**Risk is concentrated in one place and it is the usual one for this family: every failure mode is a plausible
photograph and no error.** A truncating fit, the extension left in the samples, the source and destination swapped in
the fit, a channel read from the wrong plane, the polynomial evaluated in the wrong feature order — none of them is
reportable. The mitigations are three: the sample reader and the solver are tested against hand-computable systems
where a wrong answer is a wrong *number*; the fake-backend suite drives Rio's whole contract with no runtime, so every
CI platform runs it; and the live check against the real `cb_rio_fp16` and `cb_rio_fp32` weights is run by hand,
because it is the only thing that proves a real graph renders a plausible photograph through this path.
