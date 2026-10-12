## Slices

This refactor is three changes. Slice 1 established the seam and moved no files; slices 2 and 3 are then
file moves with no dispatch edits, because after slice 1 nothing outside a model's own code names a model.

| # | Change | Delivers | Depends on | State |
| --- | --- | --- | --- | --- |
| 1 | `refactor-model-layout-1-seam` | The `Pipeline<B>` trait, both impls in place, `chain.rs` made model-blind, the `Operation` → `Pipeline` seams in `models/`, and the module-layout doctrine written into `config.yaml`. No files move. | — | **done** |
| 2 | `refactor-model-layout-2-osaka` | `inference/osaka/` disappears. `canvas.rs` is promoted to `inference/`; `mod.rs`, `latent.rs`, `region.rs`, `noise.rs`, `colorfix.rs` and `live.rs` move to `models/upscale/osaka/`. The multi-graph vocabulary (`GraphRole`, `Graph`, `DiffusionVariant`, `GraphSet`, `ResolvedGraph`, `UnknownRole`) moves to `models/upscale/osaka/graph.rs`, `OsakaPrecision` to `models/upscale/osaka/precision.rs`, and the `OSAKA` row, its profile arm and `BROKEN_OPTIMIZERS` into `osaka/mod.rs`. The `models/` and `inference/` module docs are rewritten to the new doctrine. | 1 | **current** |
| 3 | `refactor-model-layout-3-conv` | `inference/upscale.rs` becomes `models/upscale/passes.rs`; `ConvVariant`, `ScaleBucket` and `FOUR_ONLY` become `models/upscale/conv.rs`; the `TOKYO`, `KYOTO` and `SAITAMA` rows with their profiles, measurement prose and tests move into `models/upscale/{tokyo,kyoto,saitama}/`. The six exhaustive matches over `UpscaleVariant` collapse into one `model()`. `catalogue.rs` is repointed. One directory per model, done. | 2 | pending |

Slice 3 is what retires the delegating arms this slice leaves behind in `variant.rs`. They are the
deliberate cost of moving one model out of a four-model family in isolation, and the section **D5** below
says why paying it here is better than deferring the whole move to slice 3.

## Context

See `proposal.md` — Why. What matters for the approach:

- **Slice 1 already removed every dispatch site that named a model.** `chain.rs` holds
  `Arc<dyn Pipeline<B>>` and names nothing; `Operation::pipeline` knows only families;
  `Upscale::pipeline` knows only contracts and reaches `Diffusion::new` through one arm. So this slice
  edits no dispatch. Its whole content is `mod` declarations, `use` paths, one constructor seam and the
  prose.
- **The osaka files' imports are already absolute.** Every `use` inside `inference/osaka/` except the
  `super::` ones between its own submodules is spelled `crate::…`. A file moved between two directories
  therefore needs its `super::`-relative links and its module doc repaired, not its body rewritten.
- **`GraphSet`'s fields are private to `resolve.rs`.** `Upscale::resolve()` builds one with a struct
  literal. Moving the type without moving the construction leaves a family-tier file unable to compile
  against it, so the diffusion *branch* of `resolve()` moves with the type rather than a constructor being
  widened to let a distant caller keep building it.
- **Four of the moving names are public API.** `GraphRole`, `GraphSet`, `UnknownRole` and `OsakaPrecision`
  are re-exported at the crate root and asserted by `crates/opai/tests/public_api.rs`, which imports them
  from `opai::` by name and calls `graphs.graph(GraphRole::Transformer)` and
  `UpscaleVariant::Osaka(OsakaPrecision::Int8)`. Nothing about their paths may move.
- **The fake-backend suite is the reason this refactor is cheap to verify.** Osaka's region packing,
  scheduler step, noise field, per-graph shapes and refusals are all arithmetic against a counting fake,
  on every platform, with no ONNX Runtime and no weights. The `#[ignore]`d live suite is the only part
  that needs the real 7.2 GB. **Neither may lose a test to this slice.**
- **`osaka/mod.rs` and `variant.rs` carry more reasoning than code.** The miscompilation transcript on
  `BROKEN_OPTIMIZERS`, the account of the separator mistake that was read as a runtime limitation, and the
  transformer's `+1.3%` NHWC measurement against `-8.0%`/`-9.4%` on the VAE halves are the artefacts of
  three previous changes. They are the most expensive thing in the diff and the easiest to lose in a move.

## Goals / Non-Goals

**Goals:**

- `models/upscale/osaka/` is the whole of Osaka: its row, its precisions, its profile, its graph
  vocabulary, its pipeline, its helpers and its tests. Deleting Osaka becomes deleting that directory plus
  the arms that name it.
- `inference/` holds nothing any single model owns. After this slice its contents are the tile grid,
  padding, tensor conversion, **both** blending strategies, the session seam, the `Pipeline` trait and the
  chain driver — and `upscale.rs`, which slice 3 takes.
- Every public path, every error message, every measured constant and every line of measurement prose
  survives the move byte-for-byte.
- The test count does not decrease, in the fake-backend suite or in the live one.

**Non-Goals:**

- Touching Tokyo, Kyoto or Saitama, `inference/upscale.rs`, `ConvVariant`, `ScaleBucket` or `FOUR_ONLY`.
  All of it is slice 3.
- Introducing `UpscaleModel` or `UpscaleVariant::model()`. Slice 1's D2 describes seam 1 and slice 3
  builds it; collapsing the six exhaustive matches needs all four models moved first, and doing half of it
  here would leave three models reachable through a trait object and one through a match.
- Any change to `Backend`, to `Pipeline`, or to the way sessions are acquired, held or released.
- Any change to a measured EP profile, a session option, a constant or the prose recording how it was
  arrived at. A value that changes in this slice is a bug in the move.
- Promoting `colorfix.rs`, `noise.rs`, `latent.rs`, `region.rs` or `graph.rs` out of `osaka/`. Each still
  has exactly one caller; the doctrine parks a file at the tier its callers put it at today and promotes it
  in the change that adds the second caller.

## Decisions

### D1 — `canvas.rs` is promoted to `inference/`, and it is the only file that is

The doctrine names "the two blending strategies" among what `inference/` holds, and `blend.rs`'s own
module header already points at `canvas` as the other one. It is cross-family on its substance, not just
by a rule: it is the order-independent accumulator any model that draws its own per-region content needs,
and it depends on nothing but `blend::ramp_weight` and `grid::Tile`. Leaving it under `osaka/` would make
the next diffusion family's choice "import across a model boundary or copy it", which the doctrine names
as the two things that are never the answer.

Nothing else under `osaka/` qualifies today:

| File | Tier, and why |
| --- | --- |
| `canvas.rs` | **cross-family** — the second blending strategy, named as such by the doctrine |
| `latent.rs` | model — SeedVR2's VAE stride, latent channel count, patchify and its one-step scheduler |
| `region.rs` | model — the three-graph region loop, written against `RegionGraphs` |
| `noise.rs` | model — deterministic seeded noise, written model-agnostically, one caller |
| `colorfix.rs` | model — wavelet colour fix, written model-agnostically, one caller |
| `graph.rs` | model — role-addressed multi-graph resolution, one caller |

`canvas::Shared` and `pipeline::Shared` then both live directly under `inference/`. They do not collide —
each is reached by its module path and neither is re-exported from `inference/mod.rs` — and renaming
either would be a rename this slice has no behavioural reason to make.

**The file's body is model-free; its test module is not.** Two of `canvas.rs`'s tests are written at Osaka's
real geometry and import `latent::{REGION_EDGE, REGION_OVERLAP}` — a reach from `inference/` into a model's
directory that the promotion creates rather than removes. They move to `osaka/mod.rs`, where the geometry
they assert is declared, and what they share with the tests that stay behind is lifted into a
`pub(crate) mod fixtures` on `canvas.rs` — `flat` and `regions`, the placement helpers — with
`axis_weights` widened to `pub(crate)` for the ramp test that leaves. Shared rather than copied, for the
reason the doctrine gives for everything else: a pipeline's tests place regions the way `Canvas` expects
them placed, and two spellings of that would be free to drift into a disagreement no test could see. No
assertion is dropped; the split is between what checks the accumulator's arithmetic and what checks one
model's geometry.

Rejected: promoting `latent.rs` and `region.rs` too, on the grounds that a second SeedVR2-shaped model
would want them. It would be promoting on a prediction rather than on a caller, which is what the
promotion rule exists to prevent, and it would move the pipeline's substance back out of the directory
this slice exists to fill. They are listed in `config.yaml` as candidates instead — see D4.

### D2 — The diffusion resolution moves with the vocabulary, as a function on `graph.rs`

`GraphSet` and `ResolvedGraph` move out of `resolve.rs`, and the branch that builds them moves with them:

```
models/upscale/osaka/graph.rs
    pub(crate) fn resolve(variant: UpscaleVariant) -> Option<GraphSet>
    pub(crate) fn artifact_for(variant: UpscaleVariant, role: GraphRole) -> Result<ArtifactId, UnknownRole>

models/upscale/resolve.rs
    Upscale::resolve()  →  if let Some(set) = osaka::graph::resolve(…) { return Resolution::Graphs(set) }
    Upscale::graph(role) →  osaka::graph::artifact_for(…)
```

`resolve` takes no scale — a graph set is the same three graphs whatever the request asked for, and the
scale is the convolutional contract's question — and it answers `Option<GraphSet>` rather than being
guarded by an `is_diffusion()` call at the seam. The `Option` is the contract question asked in the one
place that can answer it: the family's resolver learns that some of its variants answer in graphs and the
rest in passes, and nothing more about which are which. A boolean on the variant would have put that
knowledge back in `resolve.rs`, which is the coupling this decision exists to remove.

The alternative — keep the branch in `resolve.rs` and widen `GraphSet`'s fields or give it a
`pub(crate) fn new` — would leave a family-tier file composing a model's artifact names from a model's row
through a model's pinning rules. That is the coupling this series is removing, expressed as a constructor
instead of as a match.

`Resolution` itself stays in `resolve.rs`, as the family's contract vocabulary, with
`Resolution::Graphs(GraphSet)` naming a model-tier type. That is not a boundary violation: it is **seam 2**
from slice 1's D2, the one match from the resolved contract to a pipeline, and it already names `Diffusion`
one line further on in `models/upscale/mod.rs`. A family naming its own models is the seam; a *model*
naming another model is the thing the doctrine forbids, and no file in `osaka/` references Tokyo, Kyoto or
Saitama before or after this slice.

### D3 — `variant.rs` keeps its exhaustive matches and delegates the Osaka arms

Six matches over `UpscaleVariant` stay where they are — `precision`, `profile`, `label`, `codename`,
`conv_row`, `diffusion_row` — and their `Self::Osaka(_)` arms become one-line delegations into
`models/upscale/osaka/`:

```rust
Self::Osaka(precision) => osaka::profile(precision),
Self::Osaka(_) => Some(osaka::OSAKA),
```

Exhaustiveness is what slice 1's D2 argues hardest for and what `variant.rs`'s own header rests on: a
variant added without an answer must not compile. Replacing these matches with a registry or a partial
lookup to avoid six one-line arms would trade that for the thing the doctrine's rejected alternative
describes — a model that compiles clean and is silently absent. Slice 3 collapses all six into one
`model()` returning `&'static dyn UpscaleModel`, at which point the six arms become one; it cannot be done
here, because three of the four models have not moved yet.

`variant.rs` therefore imports `DiffusionVariant` and `OSAKA` from `osaka/`. Family tier reaching into one
of its own models is exactly what seam 1 is for, and it is temporary: the import disappears in slice 3.

### D4 — Three more files join the known-promotion-candidates list in `config.yaml`

Slice 1 wrote `colorfix.rs` and `noise.rs` into that list, with a stated reason: a diffusion-generic file
parked under one model reads as a judgement that it is model-specific, and the next diffusion model
reimplements it rather than promoting it. `graph.rs`, `latent.rs` and `region.rs` acquire exactly that
property the moment this slice lands. `graph.rs` is the strongest of the three — role-addressed multi-graph
resolution is a *contract*-level idea, and every field of `Graph`, `DiffusionVariant` and `GraphSet` would
serve a second diffusion model in any family unchanged.

Rejected: leaving the list as slice 1 wrote it, on the grounds that the doctrine is slice 1's to own. The
list's purpose is to mark files whose location under-describes them, and this slice is the one that creates
three more of them. Deferring the entry to "whenever the second diffusion model arrives" is precisely the
ordering the promotion rule forbids.

### D5 — Osaka moves before the three convolutional models, not after

The series could have moved the three convolutional models first and left the largest one for last. Osaka
goes first because it is the only one of the four whose execution code is under `inference/` at all: until
it moves, `inference/` contains one model's 2,350-line pipeline and a constant about one model's
transformer, and the doctrine is contradicted by the tier it is written about. Slice 3's move is then
`inference/upscale.rs` → `models/upscale/passes.rs`, after which `inference/` holds nothing model-specific
at any point in the series' history rather than only at its end.

The cost is D3's delegating arms, which live for one slice. The alternative ordering pays a larger one:
the `inference/`/`models/` split stays wrong for two slices instead of zero.

## Division across crates

`opai` only. `gui`, `cli` and `perf` reach this code through `Opai::process`, `Operation`, `catalogue()`
and the re-exported model vocabulary, and every one of those keeps its shape and its path. No logic moves
toward a front end, and nothing a front end would need is introduced.

## Dependencies

**`rust-sak`:** no feature added or dropped, and nothing in this slice is the kind of thing `rust-sak`
covers. It writes no infrastructure at all — no hashing, HTTP, retries, downloads, extraction, temp files,
config directories, image codecs, caching, telemetry or hardware probes — because it writes no new code:
it moves files between modules of one crate. `rust-sak`'s existing use in the touched files (`crypto` and
`memo` reached via `cache.rs`, `image` via the loader) is not on any path this slice edits.

**Other crates:** none introduced. `rand_chacha` and `rand_core`, which `noise.rs` uses for its
deterministic seeded field, travel with the file and keep their existing declarations. No third-party crate
is added, and none is considered, since nothing new is being built.

## Ownership and lifetime

Unchanged, and that is the point. `Diffusion` is a plain-data implementation of `Pipeline<B>` holding a
graph set, a display name, a required-artifact list, a scale and the positions its roles were acquired at.
It borrows `&[SessionHandle<B::Session>]` for the duration of `run` and cannot retain, drop or reorder one;
the chain acquires every handle for an operation before any of it runs and holds all three until the whole
chain returns. Moving the type between two modules of one crate changes no lifetime, no `Arc`, no borrow and
no reference count. The verification for this is that the existing chain tests — the ones asserting a
session a run holds cannot be reclaimed by the idle sweep, an explicit release or a provider switch — pass
untouched, and the task list requires them not to be edited.

The one ownership-adjacent thing the move *could* break is the pairing of a graph with the profile it was
measured under: `ResolvedGraph` carries `role`, `artifact` and `profile` in one value precisely so that a
caller cannot look up settings beside an artifact and look them up wrongly. It moves as one struct with its
doc comment, and the test asserting the transformer's `cuda_prefer_nhwc` is `false` while both VAE halves'
is `true` — the project's one per-graph override, and the case that would break silently — moves with it.

## Platforms and execution providers

Validated by the existing fake-backend suite, which is arithmetic and runs on macOS, Windows and Linux with
no ONNX Runtime, no GPU, no model file and no network — so every property it covers is checked everywhere.
Beyond it, the `#[ignore]`d live suite exercises the real graphs on macOS/Apple Silicon, CoreML and CPU,
which is the hardware available here.

**CUDA and TensorRT are untested for this slice**, as they were for the changes that wrote the code being
moved. That is acceptable for the same reason: no provider-selection code, no session-building code and no
EP profile is touched. Osaka's `cuda_prefer_nhwc` overrides and its `trt_builder_optimization_level` are
moved as literals and asserted by a test that names their values, so a CUDA or TensorRT machine would find
them exactly as it would today.

## Risks / Trade-offs

- **Measurement prose is dropped or summarised in the move.** The `BROKEN_OPTIMIZERS` transcript, the
  separator-mistake account and the `+1.3%`/`-8.0%`/`-9.4%` NHWC figures are three changes' worth of work
  and none of it is load-bearing to the compiler. → Moved with the item it documents, in the same task, and
  verified by diffing the comment text rather than by reading the result — the task list says so
  explicitly.
- **A test is lost because a `#[cfg(test)] mod tests` block is merged into another file's.** Osaka's
  assertions currently live in four separate test modules across `variant.rs`, `resolve.rs`,
  `precision.rs` and `inference/osaka/*`. → The test count is compared before and after with
  `cargo test -p opai -- --list`, and no assertion may be dropped rather than repointed. Same discipline as
  slice 1's task 5.2, which is where this check comes from.
- **A public path moves without anyone noticing, because `tests/public_api.rs` imports from `opai::` and
  would still compile if a re-export were silently repointed through a different module.** → It would still
  compile, which is the point: the re-export chain is allowed to change, the paths are not. Verified by
  diffing `cargo public-api` (or `cargo doc`'s index) against `HEAD`, not by the suite alone.
- **`cargo doc` degrades quietly.** `inference/mod.rs`, `blend.rs` and `tensor.rs` carry intra-doc links
  into `super::osaka::…`, and `osaka/live.rs`'s header prints the command
  `cargo test -p opai osaka::live -- --ignored`, whose path changes to `models::upscale::osaka::live`.
  Broken intra-doc links are warnings here, not errors, so nothing fails. → A task builds the docs and
  requires zero new rustdoc warnings, and the printed command is re-run rather than re-read.
- **`variant.rs` and `resolve.rs` are left mid-refactor for the duration of slice 3.** Both will import
  from `osaka/` and both still hold the convolutional rows. → Stated in D3 and D5 as a one-slice cost, with
  slice 3 named as what retires it, and the slices table above is repeated in slice 3's design so the plan
  survives this change's archival.
- **`git` records the moves as delete-plus-add rather than as renames, because several files are edited in
  the same commit as they move.** Review then reads as 4,000 new lines. → The task list keeps the mechanical
  move and the edits that follow it as separate tasks, so `git log --follow` and a rename-detecting diff
  still work on the files that move verbatim.

## Open Questions

None. What moves where was settled by slice 1's table, and the three questions this slice had to answer on
its own — which file is cross-family (D1), who builds a `GraphSet` once its type has moved (D2), and what
happens to the six exhaustive matches while three of four models are still in place (D3) — are each decided
above with the alternative that was rejected.
