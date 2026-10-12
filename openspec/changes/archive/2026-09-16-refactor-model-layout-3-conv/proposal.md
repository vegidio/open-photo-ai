## Why

Slice 2 collapsed Osaka into one directory and left the other three upscale models where they have always
been: `TOKYO`, `KYOTO` and `SAITAMA` are three rows in `models/upscale/variant.rs`, their measured
execution-provider profiles are three arms of a match in the same file, and the pipeline that runs all
three is `inference/upscale.rs` — the last file in the tier that is supposed to hold only what no model
owns. Six exhaustive matches over `UpscaleVariant` still exist because three of the four models had not
moved, and two of them (`conv_row`, `diffusion_row`) exist only to tell one contract from the other at a
call site outside both.

This is slice 3, the last of the series slice 1 opened. It finishes what the doctrine in `config.yaml`
describes: one directory per model, `inference/` holding nothing any model owns, and a family reaching its
models through exactly two seams.

## What Changes

- **`inference/upscale.rs` becomes `models/upscale/passes.rs`.** `PassSequence`, `run_passes`,
  `correct_overshoot` and `RunPass` move verbatim. It is family tier, not model tier — three models share
  it — which is the example the doctrine in `config.yaml` already uses by name. `inference/` is then left
  holding the tile grid, padding, tensor conversion, both blending strategies, the session seam, the
  `Pipeline` trait and the chain driver, and nothing else.
- **`ScaleBucket`, `ConvVariant` and `FOUR_ONLY` become `models/upscale/conv.rs`**, together with the
  bucket walk that turns a requested scale into a pass sequence — the convolutional contract's resolution,
  which lives with the vocabulary it reads rather than in the family's resolver.
- **The `TOKYO`, `KYOTO` and `SAITAMA` rows move into `models/upscale/{tokyo,kyoto,saitama}/mod.rs`**, each
  with the execution-provider profile measured for it, every word of the prose recording how it was
  measured, and the tests asserting it. That prose is the most expensive text in this slice: Tokyo's
  ANE-compiler account and its `-23%` / `+3.9%` and `-24.2%` / `+3.3%` tables, Kyoto's FP32-islands lesson
  and its 25%-at-4x figure, Saitama's `-17.6%` re-export comparison, and the argument for why the FP32 arms
  are load-bearing rather than a formality.
- **The six exhaustive matches over `UpscaleVariant` collapse into one.** `profile`, `label`, `codename`,
  `conv_row`, `diffusion_row` and `is_diffusion` become one `model()` returning
  `&'static dyn UpscaleModel` — seam 1 from slice 1's D2, with one arm per model. `precision()` stays a
  match, because it is over the precision domain each arm carries rather than over the models.
  `conv_row()` and `diffusion_row()` disappear entirely.
- **`catalogue.rs` stops naming models.** It reaches the upscale family's models through one ordered
  family-tier list instead of importing four rows, so deleting a model stays "the directory plus two arms"
  rather than reaching a cross-family file. The two widened precision lists it keeps for that
  (`FLOAT_PRECISIONS`, `OSAKA_PRECISIONS`) move to sit with the domains they widen, so which variant admits
  which precisions is answered by the model rather than decided beside it.
- **The doc links that point at the moved names are repointed** — `models/simple.rs`,
  `models/upscale/osaka/graph.rs`, `models/upscale/osaka/latent.rs`, `inference/mod.rs`,
  `models/mod.rs` and `providers/options.rs`, whose test comment cites `UpscaleVariant::profile`'s
  Osaka section for why two TensorRT modes are left pinned.
- **No behaviour change, no public API change, and no test deleted.** Every measured constant, every error
  message and every published path is the same before and after. `Pass`, `Resolution`, `UpscaleVariant`,
  `Upscale::resolve` and `Upscale::graph` keep their spellings and their exported paths.

**Estimated task count: 14.** Within the 15-task budget, so this stays one change. It is the last slice:
nothing follows it.

## Capabilities

### New Capabilities

None. This slice introduces no observable behaviour.

### Modified Capabilities

None. The three specs over this code — `image-enhancement`, `models` and `inference-tiling` — describe
refusals, resolution, pass sequences, overshoot correction, depths, identities and progress, and every one
continues to hold verbatim. A 1.5x Kyoto request still resolves to one 2x pass and one correcting resample;
an 8x Tokyo request still requires one artifact; Osaka still refuses at FP32. What moves is which file
those answers are written in.

`.openspec.yaml` therefore sets `skip_specs: true`, as slices 1 and 2 did. Specs in this project are
written against observable behaviour; behaviour does not change, so no spec should. Writing a requirement
about directory structure to satisfy validation would contradict this project's own `specs` rule.

## Impact

**Crates touched: `opai` only.** `gui` and `cli` do not name the upscale family at all; `perf` reaches it
through `catalogue()` and `Operation::from_catalogue_with`, both of which keep their shape and their
output. No logic moves toward a front end and none is introduced there.

**`rust-sak`: nothing applies.** This slice writes no infrastructure — no hashing, HTTP, retries, downloads,
extraction, temp files, config directories, image codecs, caching, telemetry or hardware probes — because
it writes no new code beyond one trait and its four implementations. It moves existing files and existing
constants between modules of one crate. `rust-sak`'s existing use in the touched crate (`crypto` and `memo`
through `cache.rs`, `image` through the loader) is on no path this slice edits, and no feature is added or
dropped.

**Other crates: none introduced.** `image` and `image::imageops::FilterType`, which `run_passes` and
`correct_overshoot` use for the correcting resample, travel with the file and keep their existing
declarations.

**Files:**

| File | Change |
| --- | --- |
| `crates/opai/src/inference/upscale.rs` | removed — moved to `models/upscale/passes.rs` |
| `crates/opai/src/models/upscale/passes.rs` | new — `PassSequence`, `run_passes`, `correct_overshoot`, `RunPass` and their tests, verbatim |
| `crates/opai/src/models/upscale/conv.rs` | new — `ScaleBucket`, `ConvVariant`, `FOUR_ONLY`, the bucket walk, and `impl UpscaleModel for ConvVariant` |
| `crates/opai/src/models/upscale/tokyo/mod.rs` | new — the `TOKYO` row, its profile and prose, its tests |
| `crates/opai/src/models/upscale/kyoto/mod.rs` | new — the `KYOTO` row, its profile and prose, its tests |
| `crates/opai/src/models/upscale/saitama/mod.rs` | new — the `SAITAMA` row, its profile and prose, its tests |
| `crates/opai/src/models/upscale/variant.rs` | keeps the enum, `precision()`, `from_codename()`; gains the `UpscaleModel` trait and `model()`; loses six matches and three rows |
| `crates/opai/src/models/upscale/resolve.rs` | `resolve()` and `graph()` go through `model()`; the convolutional branch leaves |
| `crates/opai/src/models/upscale/mod.rs` | declares the four new modules; the contract seam reaches `passes::PassSequence` |
| `crates/opai/src/models/upscale/osaka/mod.rs` | the `OSAKA` row gains its profile; `profile()` widens to `Precision` |
| `crates/opai/src/models/upscale/osaka/graph.rs` | `impl UpscaleModel for DiffusionVariant`; `resolve` and `artifact_for` take a precision instead of a variant |
| `crates/opai/src/models/upscale/osaka/precision.rs` | gains `OsakaPrecision::PRECISIONS`, the widened list |
| `crates/opai/src/models/precision.rs` | gains `FloatPrecision::PRECISIONS`, the widened list the seven simple families and the three convolutional rows share |
| `crates/opai/src/models/catalogue.rs` | the upscale entry is built from the family's model list rather than from four rows; both precision `LazyLock`s go away |
| `crates/opai/src/models/simple.rs` | one doc link repointed |
| `crates/opai/src/providers/options.rs` | one test comment's citation of `UpscaleVariant::profile`'s Osaka section repointed at `models::upscale::osaka::profile` |
| `crates/opai/src/inference/mod.rs` | drops `upscale`; the module doc's remaining references repointed |
| `crates/opai/src/models/mod.rs` | module doc: the three tiers stated against a layout that is now complete |

**Deliberate divergence from the Go implementation: none in behaviour.** The reference keeps every
pipeline in one `models` package keyed by an operation-id string and has no equivalent structure to diverge
from; structural divergence is expected in this project and needs no justification. One thing worth naming
because it *looks* like an adoption of the reference's shape: `ConvVariant` gains a
`fn(Precision) -> EpProfile` field, which is the signature the reference's `func(Precision) EPProfile`
has. It is not the same decision, and design.md D3 says why — the reference needs it because a Go variant
cannot carry its own precision, and it pairs it with an `Fp16Only` wrapper that is not adopted here.

**Risk:** the measured profiles and the prose behind them are three changes' worth of work and none of it
is load-bearing to the compiler, so a summarising move would pass every test. Preserving Tokyo's, Kyoto's
and Saitama's verbatim is each its own task, verified by diffing the comment text against `HEAD` rather
than by reading the result.
