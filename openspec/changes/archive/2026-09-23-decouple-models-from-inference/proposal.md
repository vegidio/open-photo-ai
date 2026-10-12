## Why

`crate::inference` is two things under one name: the orchestrator that plans and runs a chain of operations, and the
cross-family home of the imaging primitives and the pipeline contract every model is written against. So every model
imports the orchestrator — 39 files under `models/`, about 126 `use` lines — and the orchestrator imports `models` back
through `Operation::pipeline`, which makes the two one knot rather than two layers. It is cheapest to untie now,
between feature slices, and more expensive with every model that lands against the current paths.

## What Changes

- **New `imaging` workspace crate** holding the leaf primitives no model and no orchestration owns: `tensor`, `grid`,
  `pad`, `blend`, `canvas`, `bilinear`, `rows`, `present`, `mix`, plus `run_tiled`, `RunTile`, `TileGeometry`,
  `TilingError` and `ChannelDepth`. It depends on `image` and `thiserror` only, and knows nothing of `opai`, so the
  compiler enforces that none of it reaches back into models, sessions or orchestration.
- **The pipeline contract gets a module of its own inside `opai`** — the `Model`, `ImagePipeline` and `DataPipeline`
  traits, `Shared`/`SharedData`, the `Backend` trait (today in `inference::process`) and the session seam
  (`inference::session`). Models depend on this contract; they no longer depend on `inference`.
- **`inference` becomes orchestration only**: planning and running a chain (`process`), running a data operation
  (`execute`), progress composition, output-depth resolution and the live checks.
- **Request-to-implementation dispatch stays in `models`**: once the contract has its own module, `Operation::pipeline`,
  `DataModel::pipeline` and `as_subject` name only `models` and that contract, so `inference` depends on `models` and
  nothing points back. `Subject` moves from `inference::progress` into `models`, and `PassWeights` into the upscale
  family, because `models` is what uses them.
- **`inference::filter` moves to `models`** at the cross-family tier: it is model code denoise and sharpen share, and
  already speaks `Strength` and `ArtifactId`.
- **Test fixtures split along the same line**: picture fixtures move to `imaging` behind a `test-support` feature;
  the fake backends stay with the contract.
- **The tier rule in `models/mod.rs` and the `inference` module header are rewritten** to name the new homes.
- **Performance is measured before and after**, and the move is not done until the release build is no slower —
  several per-pixel primitives are non-generic and would stop inlining once they sit in another crate.

No **BREAKING** change: `opai`'s public API is untouched. Nothing moved is public today, and the public types that
live beside it (`OutputDepth`, `Subject`, `InferenceProgress`) stay in `opai`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a pure restructuring: no spec states a module path, and no behaviour, output pixel, progress report or
refusal changes. The change sets `skip_specs: true` for that reason.

## Impact

- **Crates**: `opai` (restructured) and the new `imaging` (added to the workspace members). `gui`, `cli` and `perf`
  are untouched — they only see `opai`'s public API, which does not change. No logic moves into a front end.
- **`rust-sak`**: nothing in it covers this. Its `image` module is codecs, probing and rotation; tiling, tensor
  conversion and blending are this project's own, which is why they are a local crate rather than a `rust-sak`
  extension.
- **Dependencies**: `imaging` depends on `image`, pinned exactly as `opai` pins it so the two resolve to one
  `DynamicImage`, and on `thiserror`. No new third-party crate.
- **Code**: every file under `models/` that imports `inference`, most of `inference/`, and the prose and intra-doc
  links that name `inference::` paths — a wide diff with almost no logic in it.
- **Not moved**: `opai::image` (file IO) stays where it is. No model calls it, and moving it would drag `rust-sak`'s
  codec and RAW features, with their licensing, into a crate that is otherwise a leaf.
- **Parity**: no divergence from the Go implementation's behaviour.
