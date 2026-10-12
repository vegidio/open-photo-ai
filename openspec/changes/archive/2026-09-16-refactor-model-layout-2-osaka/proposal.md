## Why

Slice 1 made `chain.rs` model-blind, so nothing outside a model's own code names a model any more. What
it deliberately did not do is move a file. Osaka is still split across two directories that do not
reference each other: its 2,350-line pipeline lives under `inference/`, while the row that declares it,
its measured profile, its precisions and its graph vocabulary live under `models/upscale/`. Neither
directory answers "what is Osaka" on its own, and `inference/` — the tier that is supposed to hold only
what no model owns — currently holds one model's entire execution code plus a `BROKEN_OPTIMIZERS`
constant about one model's transformer.

This is slice 2 of the three-slice series slice 1 opened. It collapses Osaka into one directory. Because
slice 1 removed every dispatch site that named a model, this slice edits no dispatch: it is a file move
plus the import and doc-link repair that follows it.

## What Changes

- **`inference/osaka/` disappears.** `mod.rs`, `latent.rs`, `region.rs`, `noise.rs`, `colorfix.rs` and
  `live.rs` move verbatim to `models/upscale/osaka/`.
- **`canvas.rs` is promoted to `inference/canvas.rs`.** It is the second of the two blending strategies
  the doctrine names as cross-family, and it is the one file under `osaka/` that no model owns.
- **The multi-graph vocabulary moves to `models/upscale/osaka/graph.rs`**: `GraphRole`, `Graph`,
  `DiffusionVariant`, `UnknownRole` out of `variant.rs`, and `GraphSet` and `ResolvedGraph` out of
  `resolve.rs`. The diffusion branch of `Upscale::resolve()` and the diffusion branch of
  `Upscale::graph()` move with them, so a family-tier file no longer builds a model-tier value.
- **`OsakaPrecision` moves to `models/upscale/osaka/precision.rs`**, leaving `models/precision.rs`
  holding the two cross-family types (`Precision`, `FloatPrecision`).
- **The `OSAKA` row, its `profile()` arm, `BROKEN_OPTIMIZERS` and `TRT_BUILDER_OPTIMIZATION_LEVEL` move
  into `models/upscale/osaka/mod.rs`**, together with every word of the prose recording how they were
  measured — the transformer's `+1.3%` NHWC figure, the miscompilation transcript, and the account of the
  separator mistake that constant already carries.
- **The `models/` and `inference/` module docs are rewritten to the doctrine.** `models/mod.rs` opens
  with "Nothing here runs anything", which stops being true the moment a pipeline lands under it;
  `inference/mod.rs` describes the diffusion driver as living in `[osaka]` beneath it.
- **`openspec/config.yaml`'s known-promotion-candidates list gains `models/upscale/osaka/graph.rs`,
  `latent.rs` and `region.rs`.** Slice 1 listed `colorfix.rs` and `noise.rs` there for a stated reason —
  a diffusion-generic file parked under one model reads as model-specific and gets reimplemented rather
  than promoted. Three more files acquire exactly that property in this slice.
- **No behaviour change, no public API change, and no test deleted.** `GraphRole`, `GraphSet`,
  `UnknownRole` and `OsakaPrecision` are exported from the crate root and covered by
  `tests/public_api.rs`; every one keeps its spelling and its path, because the re-export chain through
  `models/mod.rs` is repointed rather than removed.

**Estimated task count: 13.** Within the 15-task budget, so this stays one change rather than splitting
further. Slice 3 — the three convolutional models and the collapse of the six exhaustive matches over
`UpscaleVariant` into one `model()` — is proposed when it is reached.

## Capabilities

### New Capabilities

None. This slice introduces no observable behaviour.

### Modified Capabilities

None. The three specs describing this code — `image-enhancement`, `models` and `inference-tiling` —
cover refusals, resolution, pass sequences, depths, identities and progress, and each continues to hold
verbatim. The graph vocabulary keeps its public paths, `UnknownRole`'s message is unchanged, and the
`EpProfile` Osaka resolves to is the same value composed from the same constants in a new file.

`.openspec.yaml` therefore sets `skip_specs: true`, as slice 1's did. Specs in this project are written
against observable behaviour; behaviour does not change, so no spec should. Writing a requirement about
directory structure to satisfy validation would contradict this project's own `specs` rule.

## Impact

**Crates touched: `opai` only.** `gui`, `cli` and `perf` reach inference through `Opai::process`,
`Operation` and the catalogue, and none of them changes shape. Nothing moves toward a front end.

**`rust-sak`: nothing applies.** This slice writes no infrastructure — no hashing, HTTP, retries,
downloads, extraction, temp files, config directories, image codecs, caching, telemetry or hardware
probes. It moves existing files between modules of one crate. `rust-sak`'s existing use inside the
touched files is carried through untouched; no feature is added or dropped.

**Other crates:** none introduced. `rand_chacha` and `rand_core` travel with `noise.rs` and stay exactly
as they are declared today.

**Files:**

| File | Change |
| --- | --- |
| `crates/opai/src/inference/osaka/` | removed |
| `crates/opai/src/inference/canvas.rs` | new — promoted from `osaka/canvas.rs` |
| `crates/opai/src/models/upscale/osaka/mod.rs` | the pipeline, plus the `OSAKA` row, its profile arm and both session constants |
| `crates/opai/src/models/upscale/osaka/{latent,region,noise,colorfix,live}.rs` | moved verbatim |
| `crates/opai/src/models/upscale/osaka/graph.rs` | new — the multi-graph vocabulary and the diffusion resolution |
| `crates/opai/src/models/upscale/osaka/precision.rs` | new — `OsakaPrecision` |
| `crates/opai/src/models/upscale/variant.rs` | loses the graph vocabulary, the `OSAKA` row and the Osaka profile arm |
| `crates/opai/src/models/upscale/resolve.rs` | loses `GraphSet`, `ResolvedGraph` and both diffusion branches |
| `crates/opai/src/models/upscale/mod.rs` | declares `osaka`; the contract seam's `Graphs` arm resolves locally |
| `crates/opai/src/models/precision.rs` | loses `OsakaPrecision` |
| `crates/opai/src/models/mod.rs` | re-exports repointed; module doc rewritten |
| `crates/opai/src/inference/mod.rs` | declares `canvas`, drops `osaka`; module doc rewritten |
| `crates/opai/src/inference/{blend,tensor}.rs` | doc links repointed |
| `crates/opai/src/models/catalogue.rs` | `OSAKA` and `OsakaPrecision` imported from their new homes |
| `openspec/config.yaml` | three files added to the known-candidates list |

**Deliberate divergence from the Go implementation: none in behaviour.** The reference has no equivalent
structure — it keeps every pipeline in one `models` package keyed by operation-id string — but structural
divergence is expected in this project and needs no justification. Nothing a user can observe differs
before and after this slice.

**Risk:** the measured execution-provider profiles and the prose behind them are the highest-value text
being moved. Preserving Osaka's verbatim, and keeping `osaka_declares_every_measured_setting_at_both_precisions`
asserting the same values, is called out as its own task rather than left to the move.
