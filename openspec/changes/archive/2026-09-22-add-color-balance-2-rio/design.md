## Context

See proposal.md — *Why*. What follows is only the state that shapes the decisions.

Slice 1 landed, and what it left is exactly the front and the back of this run:

```text
  inference/present.rs   plan(w, h, canvas) -> Plan        the rounding fit and the extension
                         presented(src, plan, canvas,      Lanczos3 to the planned size, reflection-padded
                                   norm, &mut dest)        into planar CHW; hands back the resampled photo
  inference/mix.rs       blended(src, corrected, bias)     the photograph moved towards a result by a control
```

`ColorBalance` already carries its identity, its [`Bias`], its artifact, its cache tag and its serialized form — all
of it shipped and tested. What it does **not** have is a `pipeline`, a `canvas` or a profile that is a measurement:
`ColorBalanceVariant::profile` answers `EpProfile::default()` for both variants today, which is how this project
spells *nobody has measured this graph*.

The reference's colour balance package divides as follows, and the division is what this slice has to reproduce at
the right tiers rather than at the same file boundaries:

```text
  colorbalance/process.go     REACHED BY          THIS SLICE
  --------------------------  ------------------  --------------------------------------------
  planCanvas                  rio + saopaulo      already promoted (inference/present.rs)
  chwToHWC                    rio + saopaulo      family tier, new
  kernelP                     rio + saopaulo      family tier, new
  fitPolynomialMappings       rio + saopaulo      family tier, new (multi-destination)
  fitPolynomialMapping        rio                 not ported — a one-element call at the seam
  mapPixel                    rio + saopaulo      family tier, new
  applyMapping                rio                 model tier (rio/)
  Process                     rio                 model tier (rio/)
  utils.BlendWithIntensity    rio + saopaulo      already promoted (inference/mix.rs)
```

The 15-task budget is what makes this a slice and not the whole family: Rio's contract plus the shared vocabulary is
14 tasks, and São Paulo's nine-channel output layout, weight-plane upsample and fused blend is another 11.

## Goals / Non-Goals

**Goals:**

- Make `cb_rio_fp32` and `cb_rio_fp16` produce a photograph, through a contract that is Rio's and a vocabulary that is
  the family's.
- Leave every shared piece under a name and at a tier São Paulo will build against from its first line, so slice 3
  owes no promotion.
- Reproduce the reference's arithmetic where a difference would be a different photograph, and say in this document
  where it deliberately is not reproduced.

**Non-Goals:**

- **No São Paulo code.** No `saopaulo/`, no channel layout, no weight plane, no bilinear upsample, no fused blend. It
  is refused after this slice, by a new arm in a new seam rather than by the old one.
- **No `gui` change.** The add-enhancement menu's own `RUNNABLE_ENHANCEMENTS` gate is a front-end decision with its
  own screen behaviour; widening it is a `gui` slice.
- **No re-measurement of Rio's profile from scratch.** The figures are the reference's, re-confirmed by a sweep, which
  is what Paris and Lyon did.
- **No change to `inference/present.rs` or `inference/mix.rs`.** This slice is their second caller. If either needs
  editing to serve it, slice 1 got the boundary wrong and that is the finding, not a patch.

## Decisions

### D1 — Three tiers for three pieces, and the family-tier two are placed ahead of their second caller

```text
  models/color_balance/
    samples.rs     FAMILY   the cropped CHW->HWC reader          rio (now) + saopaulo (slice 3)
    mapping.rs     FAMILY   kernel, multi-destination fit, eval  rio (now) + saopaulo (slice 3)
    rio/
      mod.rs       MODEL    CANVAS = 656, profile, the prose     rio
      process.rs   MODEL    Rio's contract, end to end           rio
    live.rs        FAMILY   the live check (#[cfg(test)])
```

**Why Rio's contract is at model tier.** `config.yaml`'s test is who else could use it. Rio's pipeline answers *fit
one mapping and evaluate it*; São Paulo's answers *fit one mapping per rendering and blend them by a predicted weight
map*. They differ at the graph's output shape and at everything after it. That is the Osaka precedent — `passes.rs` is
family tier because three upscalers share it, and the diffusion contract sits in `osaka/` because one model has it —
rather than light adjustment's, where two models genuinely share one contract. It also keeps the deletion rule intact:
deleting Rio is deleting `rio/` plus two enum arms.

*Alternative considered: `models/color_balance/process.rs`, mirroring the reference's file name and sitting beside a
later `mixed.rs`.* Rejected. The name would say *the family's contract* where there are two, and the first reader
looking for São Paulo's would find Rio's under the general name.

**Why the shared two are at family tier now, with one caller.** This is the decision in this slice that most deserves
writing down, because the doctrine can be read both ways. `config.yaml` parks Osaka's five files at model tier with
exactly one caller and a second expected — so "one caller today" is not by itself an argument for family tier. What
makes these two different is that their **second caller is named, is in the next slice, and is already visible in the
signature**: slice 1 settled that the solver is written in its multi-destination form here, and a multi-destination
solver sitting in `rio/` would be a model's directory holding a parameter that exists only for another model. That is
a cross-model dependency expressed in a signature instead of in a `use`, which is the thing the rule is about.

The concrete failure it avoids: with `mapping.rs` under `rio/`, deleting Rio deletes São Paulo's solver. Osaka's five
files have no such property — deleting Osaka costs nothing anybody else has.

*Alternative considered: park both under `rio/` and promote them in slice 3, which is what the rule's letter says.*
Rejected on the above, and the cost is named so the trade is visible: this slice ships two family-tier files with one
caller each for the length of one slice. If slice 3 were cancelled, the same rule that placed them there demotes them,
and `config.yaml` names demotion as explicitly as promotion.

*Alternative considered: the sample reader into `inference/tensor.rs`, beside `padded_chw_to_image`.* Rejected, and it
is the near-miss worth recording. That module's contracts all produce **images** — `image_to_chw`,
`padded_to_chw`, `chw_to_image`, `padded_chw_to_image` — and its whole argument is about how a pixel converts. What is
wanted here is neither an image nor a pixel: it is a sample sequence for a least-squares fit, which no other family
asks a tensor for. A third family wanting one promotes it there, and that will be an argument about samples rather
than about images.

### D2 — The samples are a borrowing view, not a materialised vector

The reference builds `[][3]float32` for the source and one per destination. Here `Samples<'a>` borrows the tensor and
answers `len()` and one triple at an index, decoding through the [`Normalisation`] it was constructed with:

```rust
// A view over the top-left crop of one planar CHW plane triple.
pub(super) struct Samples<'a> { data: &'a [f32], canvas: u32, crop: (u32, u32), norm: Normalisation }
```

At Rio's 656 square a materialised source is 430,336 triples — about 5 MB — and slice 3 wants three views over one
output tensor, which would be 15 MB of copies of data that is already in memory and already laid out for a strided
read. Nothing outlives the tensors: the input scratch and the output buffer are both owned by the run, and the fit
borrows them for its single pass.

`norm` is a constructor parameter rather than assumed, for the reason `presented` takes one after slice 1: colour
balance declares `Normalisation::Unit`, and a reader that hard-coded it would be a silently wrong photograph for the
first family with a signed range. For `Unit` the decode is the identity, so this is free and it is still the right
signature.

*Alternative considered: an `Iterator<Item = [f32; 3]>`.* Rejected — the fit walks a source and several destinations
in lockstep and wants them by index, and an indexed view monomorphises to the same loop with no zip.

### D3 — The fit accumulates in `f64` and the ridge stays `1e-8`

Samples are `f32`, `XᵀX` and `XᵀY` accumulate in `f64`, the elimination runs in `f64`, and the resulting weights are
cast back to `f32`. This is the reference's arithmetic, and it is not a matter of taste: `XᵀX` sums up to 430,336
products of features that include `r²g²`-scale terms, and accumulating that in `f32` loses the low-order bits the
ridge is supposed to be the only perturbation of.

The `1e-8` ridge, the partial pivoting by largest absolute value in the column, and the elimination order are
transcribed. Changing any of them changes which photograph comes out with no error anywhere, and there is no
measurement in this project that would catch a small one.

**One destination block of three columns per destination, laid out end to end**, so a single elimination solves all of
them. Rio passes one; the code path is the reference's and, per its own comment, bit-for-bit unchanged by the
generalisation.

### D4 — The family seam returns a `Result`, and São Paulo's refusal is the ordinary one

`ColorBalance::pipeline` is the family's contract seam — one arm per contract — and it returns
`Result<Shared<B>, InferenceError>`, unlike `LightAdjustment::pipeline` and `FaceRecovery::pipeline`, which cannot
fail. Rio's arm builds `rio::process::pipeline`; São Paulo's returns `InferenceError::Unsupported` with
`UnsupportedReason::FamilyNotImplemented`.

**The same reason as a family with no pipeline at all, deliberately.** `operation.rs` already argues this: a third
answer telling *a family nobody has written* apart from *a variant nobody has written* exists only while a family is
partly written, and the fact a caller acts on — this cannot be run yet — is the same either way. This is the state
face recovery sat in between Athens and Santorini, and `Operation::pipeline`'s own test keeps its per-operation shape
precisely because a family lands one variant first.

The `Result` **stays after slice 3**, when both arms serve. A seam that cannot refuse is the exception here, not the
rule, and re-widening a signature is a worse trade than carrying an `Ok` for one arm.

### D5 — A singular fit is a new `InferenceError` variant, raised rather than rendered

```rust
#[error("{operation} could not fit a colour mapping: the system is singular at column {column}")]
ColourMapping { operation: String, column: usize },
```

The ridge makes `XᵀX` positive-definite, so after partial pivoting the pivot should never be zero — and *should* is
not a guarantee in `f64`. A zero pivot divides into the weights and fills them with NaN, and the apply renders that as
a destroyed photograph with nothing anywhere to say why.

*Alternative considered: fall back to the identity mapping.* Rejected, and it is the tempting one. It returns a
plausible photograph — the original, unchanged — from a run the user asked to change it, which is indistinguishable
from a bias of zero and from a model that had nothing to correct.

*Alternative considered: reuse `InferenceError::Run`.* Rejected: that variant carries a tile index and an erased
source from the runtime, and a fit that failed had no runtime involvement at all.

**What the variant does *not* cover, found while writing the fake-backend suite and recorded here rather than left
in the code alone.** The paragraph above says "the ridge makes `XᵀX` positive-definite", and the sentence carries
more than it looks: `XᵀX` is built from the **source** samples alone. Nothing the graph returns reaches it. A graph
answering with NaN therefore lands entirely in `XᵀY`, the elimination finds its pivots exactly as it would have, and
the weights come out NaN with no zero pivot anywhere — so this refusal is not on that path. What the apply then
renders is black, because `f32::NAN as u8` saturates to zero.

That is the same destroyed-picture-with-no-diagnostic outcome this variant exists to prevent, reached by a different
route, and the decision is to leave it: a graph emitting NaN is a broken or wrongly-installed model rather than a
state this pipeline can tell apart from a strong correction, and the alternative — sweeping a few hundred thousand
output samples for finiteness before every fit, or the fitted weights afterwards — buys a better error message for a
condition the artifact hash already guards, at a cost on every run. `rio/process.rs` states it as a test with the
reasoning beside it, so the refusal's absence there reads as a decision rather than an oversight.

**So the refusal is a guard on an arithmetic invariant, not a reachable state, and both of its tests say so.** They
drive `eliminate` directly with a matrix the accumulation cannot produce — every element zero, and a rank-2 one for
the column — because the only honest way to exercise the case the ridge was supposed to make impossible is to remove
the ridge. `mapping.rs`'s own `a_degenerate_source_still_fits_rather_than_dividing_by_nothing` is the other half of
that claim: a photograph of one flat colour, which is the worst input a real run can carry, still fits. The variant
is carried anyway because *should* is not a guarantee in `f64`, and `public_api.rs` pins the message text — the
first time anyone reads it will be the first time it happens.

`InferenceError` is `#[non_exhaustive]`, so the addition is not a break; `crates/opai/tests/public_api.rs` records it,
and that file changing is the mechanical sign that this slice is visible from outside the crate — which slice 1's was
not.

### D6 — `mul_add`: the family-wide call slice 1 deferred

**The polynomial accumulation is written plainly — `acc += k[i] * w[i]` — with no `mul_add`.** Rust does not contract
`x + y*z` implicitly, so plain arithmetic rounds each product and gives a fixed, defined result; `mul_add` would round
once and give a different one. The reference's `mixed.go` writes explicit conversions specifically to *forbid* the
contraction Go permits, and records why: a contracted product falls on the other side of an exact integer once
truncated to 8 bits, and changes the byte. Writing it plainly here reaches the behaviour the reference works to get,
by doing nothing.

**`blended` keeps the `mul_add` it already ships with**, and the inconsistency is deliberate rather than overlooked.
It is a single term, not an accumulation, so there is one rounding either way and `mul_add`'s is the more accurate
one; both endpoints the spec pins — a bias of 0 returning the photograph byte for byte, a bias of 1 landing the whole
correction — are exact in both forms. Changing it would be editing a slice-1 file this slice promised not to touch,
to no measurable end.

The rule, stated once for the families that follow: **an accumulation of many products is written plainly; a single
fused term may use `mul_add`.**

### D7 — Five progress steps, and five cancellation checks

```text
  1  presented    fixed cost: the square, whatever the photograph is
  2  run          fixed cost: one graph run
  3  fit          fixed cost: bounded by the canvas, not the photograph
  4  apply        proportional to the photograph
  5  blend        proportional to the photograph
```

Counted rather than accumulated, so the last lands exactly on the end of the operation's share — light adjustment's
`STEPS` and face recovery's `restore`, for the same reason. Five rather than light adjustment's four because there is
a real fifth stage, and the boundaries double as the cancellation points: a run cancelled between them returns no
image at all.

The reference's `0 / 0.9 / 1` is not carried, for the reason light adjustment already recorded: three constants
describe no run when three of the five stages are fixed and two scale with the photograph.

*Alternative considered: fuse the apply and the blend into one full-resolution pass, as São Paulo's `blendWeighted`
fuses its own.* Rejected **for this slice**, on two grounds. It would mean not calling `blended`, which is the
function slice 1 promoted for this caller, and the fusion's payoff in the reference is avoiding three materialised
full-resolution renderings — a São Paulo problem that Rio, with one mapping, does not have. Rio's apply already writes
the buffer `blended` then consumes in place, so the cost is one extra pass over an owned buffer and no extra
allocation.

### D8 — Depth is dispatched once at the top

`run` matches `ChannelDepth` once and calls a `corrected::<T, B>` monomorphised over `Channel`, exactly as
`Adjust::adjusted` and `restore` do. The apply walks the photograph's own resolution — twenty-four million iterations
on a 24-megapixel source — and a per-pixel branch on the depth there is the one place this pattern is load-bearing
rather than stylistic. The fit is `f32`/`f64` throughout and is not monomorphised.

### D9 — Sessions, ownership and lifetimes

Nothing new. `Rio` holds `[ArtifactId; 1]` and one `EpProfile` and answers `Model::required` and `Model::sessions`
with them — the single-graph shape of every family but upscale. The session is borrowed as a `SessionHandle` for the
duration of `ImagePipeline::run` and is never stored, so the registry's lease outlives the run by construction and the
use-after-free the Go original is exposed to is unwritable here. No memory budget changes: one graph at a fixed
656 square is a smaller footprint than any model already in the catalogue.

The two tensors — input scratch and output — are allocated by the run at `3 * 656 * 656` floats each (about 5 MB
together), owned by it, and borrowed by the `Samples` views for the fit. They are dropped before the full-resolution
apply allocates, so the peak is the photograph's buffers rather than the sum.

### D10 — What proves this, and on which platforms

**On every CI platform, against a fake backend:** the plan and the presentation (slice 1's, unchanged), the cropped
sample reader against a hand-laid tensor, the solver against systems whose answer is computable by hand — an exact
linear map recovered to round-off, a scaled destination, a shuffled feature order failing — the apply's bounding, the
five-step progress schedule, the cancellation at each boundary, the zero-area refusal, the depth dispatch at 16 bits,
and São Paulo's refusal. None of it opens a session.

**By hand, on macOS/CoreML, against the real weights:** `models::color_balance::live`, `#[ignore]`d like every other
live check in this crate, stating the three properties that fail three different ways of getting this contract wrong —
the result carries the photograph's own dimensions, a bias of 0 returns the photograph unchanged, and a positive and a
negative bias move the colour in opposite directions.

**Validated against:** macOS on CoreML and the CPU provider, at both precisions. **Untested here:** Windows, Linux,
CUDA and TensorRT. Nothing in this slice is provider-specific — the profile names one CoreML option and the rest is
arithmetic over `image` buffers with no conditional compilation — so the CPU-provider path is what the other platforms
run and it is covered by the fake-backend suite on every runner.

### Crates and `rust-sak`

**No new crate of any kind**, first-party or third-party. `rust-sak`'s surface is `crypto`, `fetch`, `fs`, `image`,
`image-raw`, `memo`, `o11y` and `sysinfo`; none of them is a linear solver, and a fixed 11x11 Gauss-Jordan written
against this family's sample layout should not become one — it is not general-purpose, and its pivot order is
load-bearing in a way a library's would not be. **No gap, so nothing to fill and no `rust-sak` change proposed.**

No `nalgebra`, no `ndarray`, no `faer`. Each would hide forty lines behind a dependency while making the divergence
from the reference — which is the thing that has to be checkable — unreadable. The resample is `image`'s
`resize_exact`, reached through slice 1's `presented`, and is already this crate's idiom.

### Division across crates

**`opai` only.** `gui` reaches the library through `Opai::process` and the catalogue and has no line to change; its
own gate on which families the menu offers is a front-end decision with its own screen behaviour. `cli` likewise.
`perf` already names a colour balance operation from a codename, a precision and a bias — `crates/perf/src/select.rs`
has the arm — so the confirming sweep is a command, not a change. No `gui`, `cli` or `perf` task appears in tasks.md.

## Slices

Colour balance, in order. This is slice 2.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-color-balance-1-canvas` | `inference/present.rs` and `inference/mix.rs`: the fixed-canvas plan and presentation, and the per-run control blend, promoted out of light adjustment's family tier with their tests and prose. `presented` gained a `norm` parameter. No behaviour change | – | **done** (archived 2026-09-22) |
| 2 | `add-color-balance-2-rio` | The cropped CHW→HWC sample reader; the multi-destination 11×11 polynomial solver; Rio's full-resolution apply; Rio's pipeline; `rio/` with its 656 canvas and measured FP16 profile; the variant seams; `ColorBalance::pipeline` serving Rio and **refusing São Paulo**; the `Operation::pipeline` refusal arm; the fake-backend suite, live check and confirming sweep. Specs: `color-balance` (new), `execution-providers` (modified) | 1 | **current** (est. 14) |
| 3 | `add-color-balance-3-saopaulo` | The 9-channel output layout (3 settings + 3×2 renderings); the align-centres bilinear weight-plane upsample; the fused weighted blend; São Paulo's pipeline; `saopaulo/` with its measured *default* profile; the family seam losing its refusal; the fake-backend suite, live check and confirming sweep. Specs: `color-balance` (modified), `execution-providers` (modified) | 2 | pending (est. 11) |

### What slice 3 inherits from this one

Recorded here so it is taken from a document rather than from a conversation.

- **`Samples` and `mapping` are where they will be needed**, at family tier, and the solver already takes several
  destinations. Slice 3 passes two and owes no promotion.
- **`Samples` takes an offset into the tensor**, which is what lets São Paulo build one view per rendering over a
  nine-plane output. Rio passes zero. The one parameter with one caller is stated here rather than discovered there.
- **Both variants report a 656 canvas**, and the reason differs: Rio's is tradeable because its graph's output is only
  ever sampled into the fit, and São Paulo's is not, because half of its output is a weight map that is upsampled and
  multiplies the image.
- **The accumulation rule in D6** is the family's, not Rio's. São Paulo's fused blend accumulates and is written
  plainly.
- **The bilinear weight-plane upsample stays deferred** to slice 3, where it can be measured: hand-rolled
  align-centres sampling as the reference does, or `image`'s Triangle filter over an `ImageBuffer<Luma<f32>>`. These
  are not the same filter, and the reference is explicit that the alignment convention must be the inverse of the
  resize that built the canvas.

## Risks / Trade-offs

**[Every failure mode in this family is a plausible photograph and no error]** → Three nets, in order of what they
catch. The sample reader and the solver are tested against systems whose answers are *numbers* — an exact linear
colour map is recovered to round-off, and a feature order permuted by one fails — so the two places a silent error
would hide are the two places a test can be exact. The fake-backend suite drives Rio's whole contract on every CI
platform. The live check against the real weights is the only thing that proves a real graph renders a plausible
photograph, and it is run by hand.

**[The extension is left in the samples]** → The single highest-value assertion in the slice, because it is worth
4.8 dB of median and shows up nowhere else: a test presents a non-square photograph, reads the samples back, and
asserts the count is `scaled_width * scaled_height` rather than `canvas²` **and** that the last sample is the
photograph's own corner rather than a mirrored one. A count-only assertion passes on a reader that crops the right
number of pixels from the wrong place.

**[The source and the destination are swapped in the fit]** → It produces a photograph, and on a mild cast it produces
a nearly plausible one. Pinned by a test that fits a known non-symmetric map and asserts the recovered weights, and by
the live check's direction property.

**[Rio's profile is transcribed rather than re-derived, and the -3.0% it declares is the smallest figure in the
catalogue]** → Accepted, and mitigated the way Paris and Lyon's were: the confirming sweep is a task, run through
`perftest` against both precisions, and the requirement states the *mechanism* — a fixed-shape resident graph, which
is the documented precondition for the specialization — rather than resting on the number. If the sweep contradicts
it, the profile is dropped to the default and the requirement goes with it; a setting this project cannot reproduce
is not one it declares.

**[The two family-tier files have one caller each until slice 3]** → Accepted on D1, with the demotion rule named as
the remedy if slice 3 does not land. The alternative costs slice 3 two promotion tasks before it can write a line, and
leaves a multi-destination signature sitting in one model's directory in the meantime.

**[`InferenceError` gains a variant, so this slice is visible from outside the crate]** → It is `#[non_exhaustive]`,
so no downstream match breaks, and `public_api.rs` records the change deliberately rather than absorbing it. The
alternative on the path it guards — a zero pivot divided into the weights, and the NaN-filled mapping rendered — is
what the variant exists to prevent. It guards **that** path and not every route to a NaN mapping; D5 says which, and
why the other one is left to the model that caused it.
