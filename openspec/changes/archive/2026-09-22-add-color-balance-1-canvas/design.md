## Context

See proposal.md — *Why*. What follows is only the state that shapes the moves.

`models/light_adjustment/process.rs` is 1,240 lines holding one family's whole contract. Five of the items in it
describe no part of light adjustment:

```text
  models/light_adjustment/process.rs          TIER TODAY      TIER AFTER THIS SLICE
  ------------------------------------------  --------------  ---------------------
  Plan, plan(), fit_rounded(), scaled()        family          cross-family
  presented()                                  family          cross-family
  RANGE                                        family          family      (stays)
  corrected(), gain(), EPS                     family          family      (stays)
  blended()                                    family          cross-family
  STEPS                                        family          family      (stays)
  Adjust + Model/ImagePipeline impls           family          family      (stays)
```

Colour balance's own reading of the reference confirms the split is real rather than convenient. Go's
`colorbalance/process.go` opens with `planCanvas` — a separate copy of `lightadjustment`'s, calling the same
`utils.FitLongSide` — then the same `imaging.Resize`, the same `utils.ReflectionPad`, the same `utils.ImageToCHW`,
and closes in `Model.Run` with the same `utils.BlendWithIntensity`. What it does *between* those is a polynomial
colour mapping, which is nothing light adjustment does. The reference duplicates the front and shares the back
through `internal/utils`; this slice shares both.

**The 15-task budget is what makes this a slice rather than a step.** Slice 2 is 13 tasks with the promotion
already done. Folding the promotion into it makes 21, and `config.yaml` names the alternative explicitly: the move
becomes its own slice ordered ahead, never a follow-up.

## Goals / Non-Goals

**Goals:**

- Put the shared half of the whole-image contract where slice 2 can build Rio against it from its first line.
- Keep light adjustment rendering byte-for-byte what it renders today, proven by its own unchanged suite.
- Leave each moved item under a name its *next* caller will also recognise, so no second promotion is owed.

**Non-Goals:**

- **No colour balance code.** No `rio/`, no `saopaulo/`, no solver, no sample reader, no pipeline, no change to
  `ColorBalance::pipeline`'s absence or to `Operation::pipeline`'s refusal arm. Colour balance is still refused
  after this slice, by the same arm and with the same message.
- **No re-derivation of the arithmetic.** The moved bodies are the bodies that exist, including their comments and
  their measured figures. Where a line changes, it is named in D2.
- **No touching `inference/blend.rs` or `inference/canvas.rs`.** See proposal.md for why `blended` is not filed
  with either.
- **No rewrite of `config.yaml`'s module-layout doctrine.** This slice obeys it; it does not amend it.

## Decisions

### D1 — `inference/`, not `models/`, and two files rather than one

`config.yaml`: *"EVERY FAMILY -> `models/` for vocabulary, `inference/` for execution primitives"*, and
`inference/` holds *"only what no model owns: the tile grid, padding, tensor conversion, the two blending
strategies, the session seam and the chain driver."* The promoted code is execution — `presented` calls
`inference::tensor::padded_to_chw` and reaches `inference::pad`'s mirror rule through it — so `inference/` is where
the rule points, and no doctrine change is needed to put it there.

*Alternative considered: `models/canvas.rs`.* Rejected. `inference/canvas.rs` is already the tile accumulator, so
the name would have to be defended twice over, and placing execution code under `models/` would require amending a
rule that has held across the archive to accommodate one file.

**Two files, because the two halves have different futures.** The split is not tidiness; it is a prediction about
callers, which is the only thing `config.yaml` says a tier may be decided by:

```text
  inference/present.rs          inference/mix.rs
  --------------------          ----------------
  Plan, plan()                  blended()
  fit_rounded(), scaled()
  presented(.., norm, ..)

  callers now:                  callers now:
    light_adjustment              light_adjustment
    color_balance (slice 2)       color_balance (slice 2)

  callers next:                 callers next:
    colorization                  denoise, sharpen  <-- TILED. No canvas at all.
    (fixed-canvas families)       colorization
```

Denoise and sharpen carry a `Strength` and end by blending a finished result towards the photograph exactly as
these two do, and they run on a **tile grid**. A `blended` filed beside the canvas geometry would be promoted a
second time by the first family that wants it without a square — which is the outcome the promotion rule exists to
prevent.

*Alternative considered: `blended` into `inference/blend.rs`.* Rejected, and it is the near-miss worth recording.
That module is about combining **overlapping model outputs** into one image; its two strategies share
`ramp_weight`, and the module doc's whole argument is that a tuning change must not reach one of them and not the
other. `blended` has no ramp, no overlap and no seam. Filing them together on the strength of the word would make
`blend.rs` mean *things with "blend" in the name* — the same mistake `fit_rounded` refuses when it declines to be
`detection/newyork/input.rs`'s `fit`.

*Alternative considered: one file holding all three.* Rejected for the caller argument above; it saves one task and
buys a promotion later.

### D2 — `presented` takes the normalisation; `RANGE` does not move

This is the only line of moved code that changes, and it changes because the move would otherwise be unsound.

```rust
// today, in models/light_adjustment/process.rs
pub(super) const RANGE: Normalisation = Normalisation::Unit;
pub(super) fn presented(source, planned, canvas, dest) -> Result<DynamicImage, TensorShape> {
    tensor::padded_to_chw(dest, &Sampler::new(&resized), .., .., RANGE)?;   // <-- module constant
}

// after
pub(crate) fn presented(source, planned, canvas, norm, dest) -> Result<DynamicImage, TensorShape> {
    tensor::padded_to_chw(dest, &Sampler::new(&resized), .., .., norm)?;    // <-- the caller's
}
```

`RANGE` is `Normalisation::Unit` because that is what the reference's conversion does for **light adjustment's**
graphs — its own doc comment says so, and contrasts it with the `[-1, 1]` face recovery's restorers are trained on.
A cross-family `presented` that hard-coded one range would be correct for two families by luck and a silently
wrong photograph for the third. Colour balance happens to want `Unit` too, and will state that for itself in slice
2 rather than inherit it.

The test that follows the constant follows the claim: `the_graph_is_fed_the_unit_range_rather_than_the_signed_one_
face_recovery_uses` stays in `light_adjustment/process.rs`, because it asserts light adjustment's choice.
`present.rs` gains its own test that the function honours whichever range it is handed, which is the property a
shared function actually owes its callers.

*Alternative considered: promote `RANGE` alongside.* Rejected on the above. *Alternative considered: a defaulted
range.* There is no defaulting in this crate's style and no right default here — the two values differ by the
photograph they produce, with no error either way.

### D3 — `presented` keeps handing back the resampled photograph

Light adjustment needs it: it is the gain map's denominator, and `corrected`'s doc records that resampling it a
second time would be the same arithmetic on a photograph-sized buffer twice, with two copies free to drift. Colour
balance will not want it — it reads its low-resolution source samples straight out of the tensor `presented` just
filled, exactly as `colorbalance/process.go` reads `inputData` — and will drop it.

A returned value one caller discards is cheaper than the alternatives: a second entry point that skips the return
is two traversals to keep in step, and returning `()` would push the resample back into light adjustment and
re-create the drift `corrected` argues against. The resample is performed either way; only the binding differs.

### D4 — visibility widens, exports do not

Everything moved is `pub(super)` today and becomes `pub(crate)`. `fit_rounded` and `scaled` stay private to
`present.rs`: they are `plan`'s internals, and a caller choosing between a rounding fit and a truncating one by
hand is the mistake their shared documentation exists to prevent.

Nothing moved is `pub`, nothing is re-exported from `lib.rs`, and `crates/opai/tests/public_api.rs` is untouched —
which is the mechanical check that this slice is invisible from outside the crate.

### D5 — how the tests divide

`light_adjustment/process.rs` has one flat `mod tests` of 41 functions. It splits by what each asserts, not by
where the helper it uses lives:

| Test group | Destination |
| --- | --- |
| 7 plan tests, incl. the truncating-fit contrast and the zero-area panic | `present.rs` |
| 4 presentation tests + the `red` helper | `present.rs` |
| the `photograph` helper | `inference/test_support.rs` — see below |
| the unit-range test | **stays** — it is light adjustment's claim (D2) |
| 5 blend tests + the `flat` helper | `mix.rs` |
| 5 gain-map tests + `corrected_by` helper | stays |
| the whole fake-backend pipeline suite (9 tests + `Fake`, `FakeSession`, `session`, `adjusting`) | stays |

The fake-backend suite is the load-bearing one: it drives `Adjust::run` end to end with no runtime at all, so it
exercises every moved function through its new path. It must pass **unmodified apart from imports** — if it needs
an assertion changed, the move was not a move.

**`photograph` goes to `inference/test_support.rs` rather than into `present.rs`'s `mod tests`**, because the split
above gives it three callers rather than one. The presentation tests move to `present.rs`, the blend tests move to
`mix.rs` and the gain-map tests stay in `process.rs`, and all three groups assert on the same fixture — a picture
with no flat region, so a pixel read from the wrong place is a different number rather than a plausible one. Leaving
it in `present.rs` would mean either two more copies of it or a `use` reaching out of one module's test scope into
another's, and `test_support.rs`'s own module doc already makes the argument against the copies: four copies of the
picture is four things that can quietly stop being the same, at which point two tests disagree about what "the
picture" was and neither is wrong. The module is `#[cfg(test)]`, so nothing about this reaches a shipped binary.

This is the same tier rule the slice is about, applied to a fixture instead of to a function: a second caller moves
the thing rather than copying it. The helper's documentation there states what distinguishes it from `gradient`, the
fixture the tiling tests want and the one a presentation or blend test must not have.

### D6 — validation, and what this slice cannot prove on CI

Sessions, memory budgets and execution providers are untouched: no session is opened, no profile is read, no
provider is named, and no ownership or lifetime changes — the moved functions take buffers and images and return
buffers and images. `Adjust` continues to own its artifact, its profile and its canvas exactly as it does today.

Platforms: the moved code is arithmetic over `image` buffers with no conditional compilation, so it is exercised on
every CI platform by the existing suite. The one thing CI cannot prove is that a **real graph still renders a
plausible photograph through the new module path**, because every failure mode here produces an image and no
diagnostic. `models/light_adjustment/live.rs` is that proof and is run by hand on macOS/CoreML as part of the
toolchain task. Windows, Linux, CUDA and TensorRT are untested here and unaffected — no provider-specific code is
touched.

### Crates and `rust-sak`

**No new crate of any kind**, first-party or third-party. `rust-sak` contributes nothing to this slice and has no
module that could: its surface is `crypto`, `fetch`, `fs`, `image`, `image-raw`, `memo`, `o11y` and `sysinfo`, and
none of them is a resampler, a canvas plan or a per-pixel blend. The Lanczos resample the moved code performs is
`image`'s `resize_exact`, already this crate's established idiom in `models/upscale/passes.rs`; `rust-sak` does not
wrap it, and should not — a resampler is not infrastructure in the sense that module set covers. **No gap, so
nothing to fill and no `rust-sak` change proposed.**

### Division across crates

**`opai` only.** Nothing here is front-end logic and nothing could be: `gui` and `cli` reach this crate through
`Opai::process`, `Operation` and the catalogue, none of which move or change shape, and `perf` likewise. No `gui`
or `cli` task appears in tasks.md because neither crate has a line to change.

## Slices

Colour balance, in order. This is slice 1; the two that follow are proposed when they are reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-color-balance-1-canvas` | `inference/present.rs` and `inference/mix.rs`: the fixed-canvas plan and presentation, and the per-run control blend, promoted out of light adjustment's family tier with their tests and prose. `presented` gains a `norm` parameter. No behaviour change | – | **current** |
| 2 | `add-color-balance-2-rio` | The cropped CHW→HWC sample reader; the multi-destination 11×11 polynomial solver; Rio's full-resolution apply; Rio's pipeline; `rio/` with its 656 canvas and measured FP16 profile; the variant seams; `ColorBalance::pipeline` serving Rio and **refusing São Paulo**; the `Operation::pipeline` refusal arm; the fake-backend suite, live check and confirming sweep. Specs: `color-balance` (new), `execution-providers` (modified) | 1 | pending (est. 13) |
| 3 | `add-color-balance-3-saopaulo` | The 9-channel output layout (3 settings + 3×2 renderings); the align-centres bilinear weight-plane upsample; the fused weighted blend; São Paulo's pipeline; `saopaulo/` with its measured *default* profile; the family seam losing its refusal; the fake-backend suite, live check and confirming sweep. Specs: `color-balance` (modified), `execution-providers` (modified) | 2 | pending (est. 11) |

### Decisions settled in exploration that slices 2 and 3 inherit

Recorded here so the next two slices take them from a document rather than from a conversation.

- **Two models, two contracts, two slices.** Rio returns a corrected image and fits one global polynomial; São
  Paulo returns a per-pixel weight map over three white-balance renderings plus two of the renderings themselves,
  and blends them. They differ at the graph's output shape and at everything after it, which is the
  `add-face-recovery-2-athens` / `-3-santorini` precedent rather than `add-upscale-tokyo-saitama`'s.
- **The polynomial solver is written in its multi-destination form in slice 2**, with Rio passing a one-element
  slice. `X^T X` depends only on the source, so the accumulation is shared across destinations — the reference
  measures 32 ms for two fitted together against 53 ms fitted separately, and records that the single-destination
  case is bit-for-bit unchanged by the generalisation. The alternative is rewriting the ridge, the pivoting and the
  elimination in slice 3, and that loop is the one place in this family where a mistake renders a plausible
  photograph and no error. The cost — one parameter with one caller for a slice — is stated in slice 2's proposal.
- **Both variants run at a 656 square.** Rio's is tradeable and São Paulo's is not: Rio's graph output is only ever
  sampled into the 11-term fit, while half of São Paulo's output is the weight map, which is upsampled and
  multiplies the image, so its resolution is a real cap on how finely the blend can follow an illuminant boundary.
- **The per-run parameter is `Bias` on `-1.0..=1.0`, not the reference's `intensity` on `0..1`.** Already shipped
  and already published by the catalogue; the negative half extrapolates past the photograph, as light
  adjustment's does, and has no counterpart in the reference. Slice 2's spec states what it means.

### Deferred to the slice that can measure them

Neither would change this slice's approach, its files or its task list.

- **The bilinear weight-plane upsample (slice 3):** hand-rolled align-centres sampling as the reference does, or
  `image`'s Triangle filter over an `ImageBuffer<Luma<f32>>`. These are not the same filter, and the reference's
  comment is explicit that the alignment convention must be the inverse of the resize that built the canvas.
  Decided by measurement in slice 3, not by reading.
- **`mul_add` versus separate roundings in the full-resolution loops (slice 2):** Go must write explicit
  conversions to *forbid* fused multiply-add, because a contracted product falls on the other side of an exact
  integer once truncated to 8 bits. Rust does not contract implicitly, so the reference's behaviour is the Rust
  default — but `blended` uses `mul_add` today. A family-wide call for slice 2's design, on code slice 1 only
  relocates.

## Risks / Trade-offs

**[A move is rewritten by accident, and the arithmetic drifts]** → The moved bodies and their comments are
transplanted, not retyped. The mechanical check is D5's rule: the fake-backend pipeline suite and the gain-map
tests stay in `light_adjustment/process.rs` and must pass with **no change but imports**. An assertion that needs
editing means something was rewritten, and the task stops there.

**[Every failure mode here is a plausible photograph and no error]** → The relocated tests are the first net — the
plan tests pin the rounding fit against a truncating one on a ratio where they disagree, the presentation tests
pin the interior and the mirrored margin separately, the blend tests pin all three positions of the bias. The
second net is the live check against the real Paris and Lyon weights, run by hand, which is the only thing that
proves a real graph still renders through the new path.

**[`presented`'s new parameter is passed the wrong range at a call site]** → There is exactly one call site in this
slice, and it passes the constant that used to be read inside the function. Slice 2 adds the second and states its
own range in the same change. The risk is real but it is a one-line risk with a test beside it on both sides.

**[Slice 1 delivers no user-visible value]** → Accepted, and it is the trade `config.yaml` already chose: *"never a
follow-up afterwards, which is how a 'temporary' copy gets made permanent."* The project stays in a working,
testable state throughout — light adjustment renders identically, colour balance is refused before and after by the
same arm — and slice 2 starts against the shared location rather than against a copy it would owe a promotion for.

**[`inference/mix.rs` has two callers at the end of slice 2 and a thin-looking reason to exist]** → Accepted on the
caller prediction in D1. If denoise and sharpen were to land without wanting it, the rule that put it there is the
same rule that would move it back down — `config.yaml` names demotion as explicitly as promotion.
