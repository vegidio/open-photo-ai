## Why

Adding a model to this project means editing files in three directories that have nothing to do with
each other, and `inference/chain.rs` — the driver every family will share — names two specific
pipelines and one specific model by module path. Osaka's 2,350-line pipeline sits under `inference/`
while its declaration sits under `models/`, so neither directory answers "what is Osaka" on its own.
Seven more families are still to be written against this seam, and each one added under the present
shape adds another arm to a driver that is supposed to be family-blind.

This is slice 1 of a three-slice series that gives every model one directory and reduces the driver's
knowledge of models to nothing. It moves no files: it establishes the seam the other two slices move
files across, and writes down the architecture they establish so the seven remaining families are
held to it.

## What Changes

- **A `Pipeline<B: Backend>` trait** in `inference/pipeline.rs`: what a planned operation needs on
  disk, which sessions it opens and under which profile each, how many model runs it is, and how it
  runs. Object-safe over the generic backend, so the existing fake-backend test suite is unaffected.
- **Both existing pipelines implement it, where they already live.** The convolutional pass sequence
  (`inference/upscale.rs`) and the diffusion pipeline (`inference/osaka/`) gain impls; no file moves
  in this slice.
- **`chain.rs` stops naming models.** `Planned` is reduced from a two-arm enum over the contracts to
  the operation plus its pipeline; `Work` and `plan_graphs` are deleted outright, along with
  `use super::osaka::{self, region::RegionGraphs}` and `use super::upscale::run_passes`. The run loop
  acquires sessions from the trait and calls one method. Roughly 150 lines removed.
- **The `Operation` → `Pipeline` mapping moves into `models/`**, expressed as two seams: one match
  per model (the variant enum) and one match per contract (`Resolution`). Neither reaches `chain.rs`.
- **The module-layout doctrine in `openspec/config.yaml` is rewritten** from the current one-paragraph
  bullet into the three-tier rule (one model / one family / every family) plus the promotion rule that
  governs what happens when a file gains or loses callers. Written here rather than in slice 3 so that
  slices 2 and 3 execute against a stated rule instead of establishing one retroactively.
- **No behaviour change and no public API change.** Every exported name keeps its spelling; no module
  path visible outside the crate moves in this slice.

## Capabilities

### New Capabilities

None. This slice introduces no observable behaviour.

### Modified Capabilities

None. The three specs that describe what this code does — `image-enhancement`, `models` and
`inference-tiling` — describe refusals, pass sequences, depths, identities and progress, and every one
of them continues to hold unchanged, verbatim. The refusal of the seven unwritten families moves from
a match in `chain.rs` to a match in `models/`, and remains the same refusal with the same message.

`.openspec.yaml` therefore sets `skip_specs: true`. This is a pure refactor: specs describe behaviour,
behaviour does not change, so no spec should change either. Inventing a requirement about directory
structure to satisfy validation would also contradict this project's own `specs` rule, which requires
requirements to be written against observable application behaviour rather than internal structure —
and all twelve existing specs honour that.

## Impact

**Crates touched: `opai` only.** Neither front end sees any of this: `gui` and `cli` consume
`Opai::process`, `Operation` and the catalogue, and none of those change shape. `perf` likewise
reaches the crate only through its public API.

**`rust-sak`:** nothing applies, and nothing was expected to. This slice adds no infrastructure — no
hashing, HTTP, retries, downloads, extraction, temp files, config directories, image codecs, caching,
telemetry or hardware probes. It is a module boundary and a trait over code that already exists.
`rust-sak`'s existing use in the touched files (`crypto::xxh3_string` in `identity_after`, `memo` behind
`RunCache`) is untouched.

**Files:**

| File | Change |
| --- | --- |
| `crates/opai/src/inference/pipeline.rs` | new — the trait |
| `crates/opai/src/inference/chain.rs` | `Planned` reduced to operation + pipeline; `Work`, `plan_graphs` deleted; dispatch via the trait |
| `crates/opai/src/inference/upscale.rs` | gains an impl; `run_passes` otherwise unchanged |
| `crates/opai/src/inference/osaka/mod.rs` | gains an impl; `restore_image` otherwise unchanged |
| `crates/opai/src/models/operation.rs` | gains the family → pipeline seam |
| `crates/opai/src/models/upscale/mod.rs` | gains the contract → pipeline seam |
| `openspec/config.yaml` | module-layout doctrine rewritten |

**Deliberate divergence from the Go implementation:** none in behaviour. The reference has no
equivalent seam at all — its driver switches on an operation-id string — but structural divergence is
expected in this project and needs no justification. Nothing a user can observe differs before and
after this slice.

**Risk:** the measured execution-provider profiles and their supporting prose (Tokyo's CoreML figures,
Kyoto's export-regression story, Osaka's transcribed reference numbers) are not touched in this slice.
They move in slices 2 and 3, where preserving them verbatim is called out as its own task.
