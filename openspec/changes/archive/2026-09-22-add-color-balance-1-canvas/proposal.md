## Why

Colour balance's two models need, from their first line, three things that exist today at **light adjustment's
family tier**: the plan that fits a photograph's longer side onto a fixed square and extends the rest, the Lanczos
resample and reflection pad that write it into a graph's planar CHW square, and the blend that moves the photograph
towards a finished result by a per-run control. Rio and São Paulo present at a 656 square exactly as Paris and Lyon
present at 1024, and both end by blending towards the photograph by a `Bias`.

`config.yaml` says what happens next, and says it twice. A family-tier file a second family needs is promoted to
cross-family tier; copying is named as the thing that "ends in two versions disagreeing"; and the promotion happens
**before** the new caller is written — *"Where the move will not fit that change's 15-task budget it becomes its own
slice ordered AHEAD of the one that needs it - never a follow-up afterwards, which is how a 'temporary' copy gets
made permanent."*

This is slice 1 of a three-slice series. It adds no model, no pipeline and no observable behaviour: it moves the
shared half of the light adjustment contract to where the next two slices can build against it.

## What Changes

**`inference/present.rs`** — new, cross-family tier. The fixed-canvas front half of a whole-image run:

- `Plan` and `plan(width, height, canvas)`: the size the photograph is resampled to and the extension that fills
  the square, moved verbatim with the prose that records why a fixed-shape graph enlarges a small photograph as
  readily as it shrinks a large one.
- `fit_rounded` and `scaled`: the **rounding** fit, and the documentation of why it is deliberately not
  `detection/newyork/input.rs`'s truncating one. That argument is unchanged by the move and gains force from it —
  the two now sit at the same tier, so the only thing keeping them apart is the comment saying they are different
  functions.
- `presented`: resample to the planned size, reflection-pad into the square, write planar CHW, hand back the
  resampled photograph.

**`inference/mix.rs`** — new, cross-family tier. `blended(source, corrected, bias)`: the photograph moved towards —
or, at a negative value, away from — a finished result, per channel in unit space and bounded.

**The one signature change, and it is not cosmetic.** `presented` currently reads `RANGE` as a module constant.
`RANGE` does **not** move: it is `Normalisation::Unit` because that is what the reference's conversion does for
*this family's* graphs, and face recovery's restorers are trained on `[-1, 1]`. A cross-family `presented` that
hard-coded one range would be a silently wrong photograph for the first family that does not share it, so the
function takes `norm: Normalisation` as a parameter and each family keeps stating its own. Light adjustment's test
that the graph is fed the unit range rather than the signed one stays behind, where the claim belongs; the moved
function gains a test that it honours whatever range it is handed.

**What stays in `models/light_adjustment/process.rs`**, and why each one is that family's alone:

| Stays | Why |
| --- | --- |
| `corrected`, `gain`, `EPS` | The gain map. Colour balance fits a polynomial colour mapping instead — a different answer to a different question, sharing no arithmetic. |
| `RANGE` | Above: a family's contract with its own graphs. |
| `STEPS` | Both families happen to be four steps. A shared constant would make that a rule rather than a coincidence. |
| `Adjust` and its two trait impls | The family's pipeline. |

**No behaviour change and no public API change.** Everything moved is `pub(super)` becoming `pub(crate)`; nothing
moved is exported, nothing re-exported changes spelling, and light adjustment renders byte-for-byte what it renders
today. The fake-backend suite and the live check both stay where they are and both must still pass unchanged.

**Documentation sites.** `inference/mod.rs` opens by enumerating what the module holds — *"the tile grid, the
padding rule, the tensor conversions, both blending strategies, the session seam..."* — and gains two entries.
`models/mod.rs` and `light_adjustment/process.rs` each describe the moved functions as light adjustment's, and stop.

### Where `blended` does not go, which is the placement decision in this slice

`inference/blend.rs` already exists and is not its home. That file is specifically about **combining overlapping
model outputs into one image** — `blend_tile`, `ramp_weight`, and `canvas.rs` as "the second way of combining
tiles", with the shared ramp the invariant neither strategy may drift from. `blended` shares no curve, no seam and
no overlap with any of it; filing them together would make that module mean *things with "blend" in the name*.

Nor does it belong beside the canvas geometry. Its next callers are **denoise and sharpen**, which carry a
`Strength` and blend a finished result towards the photograph in exactly this way — and which are *tiled*, so they
have no canvas at all. A `blended` parked under `present.rs` would need promoting a second time by the first family
that wants it without a square.

### Deliberate divergence from the reference implementation

**None in behaviour, and none is possible:** no pixel this slice touches is computed differently afterwards. The Go
project has no counterpart to this boundary at all — `planCanvas` is duplicated between `lightadjustment/process.go`
and `colorbalance/process.go`, and `BlendWithIntensity` sits in `internal/utils` — and structural divergence is
expected here and needs no justification. The duplication this slice prevents is the one the reference lives with.

### Task estimate

**8**, against the 15-task budget: the geometry, the presentation with its new parameter, the blend, the two test
relocations, light adjustment's call sites and module doc, the two module-prose sites, and the toolchain pass. The
remaining two slices are estimated at 13 and 11; see design.md for the series.

### Infrastructure: what `rust-sak` already covers

**Nothing applies, and nothing was expected to.** This slice adds no infrastructure — no hashing, HTTP, retries,
downloads, extraction, temp files, config directories, image codecs, caching, telemetry or hardware probes. It moves
existing arithmetic between two modules of one crate. The Lanczos resample the moved code performs is `image`'s own
`resize_exact`, already this crate's idiom, and `rust-sak` has no resampler and should not gain one. **No new
third-party crate**, and none was weighed.

## Capabilities

### New Capabilities

None. This slice introduces no observable behaviour.

### Modified Capabilities

None. `light-adjustment` is the only spec describing this code, and every requirement in it — the canvas fit, the
reflection extension, the gain map, the bias and its endpoints, the progress schedule, the refusals — continues to
hold verbatim, computed by the same arithmetic reached through a different module path.

`.openspec.yaml` therefore sets `skip_specs: true`. Specs describe behaviour, behaviour does not change, so no spec
should change either — and inventing a requirement about directory structure would contradict this project's own
`specs` rule, which requires requirements to be written against observable application behaviour rather than
internal structure.

## Impact

**Crates touched: `opai` only.** Neither front end sees any of this; `gui`, `cli` and `perf` reach the crate through
its public API, and nothing public moves or changes shape.

| File | Change |
| --- | --- |
| `crates/opai/src/inference/present.rs` | new — `Plan`, `plan`, `fit_rounded`, `scaled`, `presented` |
| `crates/opai/src/inference/mix.rs` | new — `blended` |
| `crates/opai/src/inference/mod.rs` | two module declarations; the opening enumeration of what the module holds |
| `crates/opai/src/models/light_adjustment/process.rs` | the five functions and their tests removed; imports and the `RANGE` call site; the module doc that presents them as this family's |
| `crates/opai/src/models/mod.rs` | the tier prose naming what light adjustment owns |
| `crates/opai/src/inference/test_support.rs` | the `photograph` fixture, which three modules now assert on — see design.md D5 |
| `crates/opai/src/inference/blend.rs`, `canvas.rs` | **untouched**, and deliberately so — see above |

**Public API: unchanged.** No type added, removed or re-spelled; no signature visible outside the crate moves.

**Risk is confined to one thing, and it is checkable.** Every failure mode of the code being moved is a plausible
photograph and no error — a truncating fit where the reference rounds, a normalisation range silently swapped, a
pad left in. The mitigation is that the arithmetic is not being rewritten, only relocated, and light adjustment's
existing suite covers all of it: the plan tests, the presentation tests, the blend tests, the fake-backend pipeline
suite and the live check against the real Paris and Lyon weights must all pass unchanged. The live check is run by
hand as part of the toolchain task, because it is the only one that proves a real graph still renders a plausible
photograph through the new module path.
