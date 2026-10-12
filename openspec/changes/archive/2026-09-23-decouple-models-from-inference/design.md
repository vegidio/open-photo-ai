## Context

See proposal.md — Why. The current shape:

```
   inference::process --plan--> Operation::pipeline()   (models/operation.rs)
          ^                              |
          |                              v
   pipeline traits  <--implements-- models/<family>/process.rs
   tensor/grid/... <----uses--------------+
```

What `models/` imports from `inference/` falls into four groups, and each gets a different home:

| Group | Items | Home after this change |
|---|---|---|
| Imaging primitives | `tensor`, `grid`, `pad`, `blend`, `canvas`, `bilinear`, `rows`, `present`, `mix`, `run_tiled`, `RunTile`, `TileGeometry`, `TilingError`, `ChannelDepth` | `imaging` crate |
| Pipeline contract | `pipeline` (`Model`, `ImagePipeline`, `DataPipeline`, `Shared`, `SharedData`, `OnOneGraph`, `SingleGraph`, `checkpoint`, `reporter`), `process::Backend`, `session` (`GraphShape`, `NamedOutput`, `run_tile`, `run_graph`, ...) | `opai::pipeline` module |
| Cross-family model code | `filter` (the denoise/sharpen tiled filter and its divergence guard) | `opai::models::filter` |
| Run vocabulary | `progress::Subject` (the `Operation` \| `Analysis` union the public catalogue's `build` returns) | `opai::models`, beside the two carriers it unites (D8) |
| One family's arithmetic | `progress::PassWeights` (the multi-pass upscale's split of its range) | `models/upscale/passes.rs` (D8) |
| Dispatch | `Operation::pipeline`, `DataModel::pipeline`, `as_subject` | stays in `models`, naming only `crate::pipeline` (D3) |

The primitives are genuine leaves today: each of them imports only `image`, `std` and other primitives — nothing from
`models`, `sessions`, `providers` or the orchestration files. The contract is not a leaf: `pipeline.rs` uses
`sessions`, `providers`, `error` and `models::ArtifactId`, and `session.rs` uses `ort`. That is what decides the split
between a crate and a module below.

## Goals / Non-Goals

**Goals:**

- One direction of dependency: `inference` → `models` → {`opai::pipeline`, `imaging`}. Nothing under `models/`
  names `crate::inference`.
- The primitives' independence enforced by the compiler, not by a comment.
- No change in any output pixel, progress report, refusal or public item, and no change in speed.

**Non-Goals:**

- Moving `opai::image` (file IO). No model calls it, it depends on `rust-sak`'s codec and RAW features, and its callers
  are the front ends through `opai`'s public API.
- Enforcing "models never name `inference`" mechanically inside `opai`. The crate boundary covers the primitives; the
  rule for the rest is stated in the tier header, and a lint or CI check for it can follow separately if drift appears.
- Separating `models`' request descriptions (`Operation`, variants, `ArtifactId`) from its pipeline implementations.
  The contract uses `ArtifactId` and the implementations use the contract, so at module level `models` still contains
  both ends of that edge. It is acyclic between layers, and splitting `models` is a larger change with no current
  pressure.
- Any behavioural change, rename of a public item, or new public API.

## Decisions

### D1. The primitives become a crate, `imaging`; the contract stays a module

The primitives go into `crates/imaging`, a workspace member with `publish = false`, depending on `image` (pinned exactly
as `opai` pins it, `default-features = false`, so `DynamicImage` resolves to one type) and `thiserror`. Their
`pub(crate)` items become `pub`; that is the crate's interface to `opai`, not a published API.

*Alternative: move the contract into the crate too.* Rejected: it would bring `sessions`, `providers`,
`InferenceError`, `ArtifactId` and `ort` with it — a second core crate rather than a leaf, and a boundary that
enforces nothing because everything is on the same side.

*Alternative: modules only, with a CI grep.* Rejected by choice: the crate makes the primitives' independence a
compile error rather than a convention.

The crate is named `imaging` to match the workspace's unprefixed member names (`opai`, `cli`, `gui`, `perf`).

### D2. The contract module is `opai::pipeline`

`inference/pipeline.rs`, `inference/session.rs` and the `Backend` trait from `inference/process.rs` move to a
top-level `opai::pipeline` module (`pipeline/mod.rs` for the traits, `pipeline/backend.rs`, `pipeline/session.rs`).
Named after what a model implements. `runtime` was considered and is taken: `opai::runtime` loads the ONNX Runtime
shared library.

Ownership and lifetime are unchanged by the move. The chain driver in `inference` acquires every session a pipeline
names through `Backend::acquire` and holds the `SessionHandle`s for the operation's duration. A pipeline receives them
as a borrowed slice, so it cannot retain, drop or reorder one, and the session cache's reference counts stay the
driver's. `Backend` stays generic, not `dyn`, so `SessionHandle<B::Session>` stays typed.

One function in `inference/pipeline.rs` does not move with the contract: `acquire`, which opens every session a
model names before the operation runs. It reports through `inference::progress::OperationProgress`, so in
`opai::pipeline` it would point the contract back at the orchestrator. It moves to `inference/acquire.rs` instead,
and `process` and `execute` both call it from there. Found during implementation.

### D3. Dispatch from a request to its implementation stays in `models`

`Operation::pipeline`, `DataModel::pipeline` and `as_subject` stay in `models/operation.rs`. Once the contract is in
`opai::pipeline` (D2) and `Subject` is in `models` (D8), they name only `models` and `crate::pipeline`, so nothing in
them points back at `inference`, and `inference` → `models` holds without moving them.

*Alternative, planned in the first draft and dropped during implementation: move the dispatch into `inference` as free
functions.* It does not work for the data path. `execute` is generic over the public bound `O: DataOperation`, so the
method that turns an `O` into a `SharedData<B, O::Output>` has to be on the sealed trait for a generic `O` to reach it.
Moving it out means moving the whole seal (`sealed::DataModel` and `DataOperation`) into `inference`. That keeps the
`private_bounds` allowance, which the draft expected to retire, and forces every `models` test that calls
`operation.pipeline::<NoBackend>()` to change, to buy a direction that D2 and D8 already give.

Consequences: adding a model touches only `models`. The sealed `DataModel` keeps its `pipeline` method and its
`private_bounds`/`private_interfaces` allowance, whose vocabulary is now `crate::pipeline`'s.

### D4. `filter` moves to `models/filter.rs`

It is the contract two families' models run, so it belongs at the cross-family tier inside `models`. It calls
`imaging::mix::blended` and the `opai::pipeline` contract, neither of which is `inference`. `GUARDED` moves with it;
the denoise and sharpen live checks import it from its new path.

### D5. Test fixtures follow their subject

- `imaging` gains a `test-support` feature exposing the picture fixtures (`gradient`, `nearest_neighbour`, the
  canvas `fixtures::flat`), and `opai` enables it from `[dev-dependencies]`. `cfg(test)` does not cross a crate
  boundary, which is why it is a feature.
- `NoBackend`, `Fake`, `stub_backend_runs`, `session` and the paired originals stay in `opai`, under
  `pipeline::test_support`, because they are backends and model-shaped fixtures.
- `photograph` moves to `imaging` with `gradient`: it is a pure picture generator, and the tests of `present` and `mix`,
  which move to `imaging`, use it. Decided during implementation; the first draft kept it in `opai`, which a crate
  boundary does not allow.

### D6. `depth.rs` stays; `ChannelDepth` moves

`OutputDepth` is public (`opai::OutputDepth`) and is a run option the orchestrator resolves, so `depth.rs` stays in
`inference`. `ChannelDepth`, which `run_tiled` and every primitive writing pixels takes, moves to `imaging`, and
`OutputDepth::resolve` returns `imaging::ChannelDepth`.

### D7. Performance is measured before the move and gated after it

Several primitives called per pixel are not generic — `Sampler::rgb`, `bilinear::sample`, `Tap`,
`Normalisation::encode_unit`/`decode`, `ramp_weight`. Inside one crate LLVM inlines them freely. Across a crate
boundary rustc only inlines small leaf functions on its own, the release profile has no LTO, and there is not one
`#[inline]` in `inference/` today. Generic ones (`blend_tile`, `chw_to_image`, `blended`, `for_each_row`) are
monomorphised in `opai` and unaffected.

The measurement uses `perf`, release build, on this machine:

- **Models**: `kyoto` at `--scale 4` (`run_tiled`, `grid`, `pad`, `tensor`, `blend`), `paris` (`present`, `mix`)
  and `stockholm` (the tiled filter, `mix`).
- **Precisions**: FP32 and FP16.
- **Provider**: `-p coreml` only.
- **Images**: the embedded 640x640 sample for all three, and for `paris` also one photograph of at least 12 MP
  through `--image`. A whole-image model's graph runs on a fixed square whatever the photograph's size, while
  presentation, read-back and blending run at full resolution, so on the large photograph the primitives are a large
  share of the wall time — that is the sensitive signal. A tiled model has no such effect: every tile pays the same
  model run and the same conversion and blend, so the primitives' share is the same at any size, and a large
  photograph adds only wall time. `kyoto` and `stockholm` are therefore measured on the sample alone.
- **Runs**: `-n 10 -w 1 --json`, run cache off (the default), with the JSON saved per run.

The first draft measured seven models (adding `tokyo`, `osaka`, `moscow`, `saopaulo` and `delhi`) on CPU as well.
One Tokyo 4x run on the 640x640 sample took 100 s on CPU, which put the 12 MP CPU cells at hours each, and the matrix
has to run twice. It was cut to the three models above on CoreML. A second cut followed: the 12 MP Kyoto runs took
62 s each at first and drifted to 115-186 s partway through the cell on an otherwise idle machine, which would have
inflated the baseline deviation the gate is measured against, while adding no signal a tiled model can give. `canvas`, `bilinear` and `rows` are therefore on
no measured path. The `#[inline]` remedy below still names their hot functions.

**Gate:** the move is done when no model/precision/provider/image combination regresses its median by more than the
larger of 3% and one baseline standard deviation. If one does, mark the hot non-generic primitives `#[inline]` and
re-measure. If that does not recover it, add `lto = "thin"` to `[profile.release]` and re-measure. Record the
baseline and the final table under **Measurements** at the end of this document.

### D8. `Subject` and `PassWeights` move into `models`

Found during implementation: two items the table first listed as orchestration vocabulary are used from `models`.

- `Subject` is the `Operation | Analysis` union with forwarding methods, and the public catalogue's
  `models::build` returns it. It moves into `models` beside the two carriers. `inference::progress` imports it from
  there, and the public path `opai::Subject` does not change.
- `PassWeights` is arithmetic whose only non-test caller is `models/upscale/passes.rs`, so it moves there at the
  family tier. The chain-progress test in `inference::progress` imports it from `models`, which is the allowed
  direction.

## Risks / Trade-offs

- [Per-pixel functions stop inlining across the new crate boundary] → D7's gate, with `#[inline]` and then thin LTO as
  the remedies, in that order.
- [Wide diff made mostly of prose and intra-doc links] → `cargo doc` with `-D rustdoc::broken_intra_doc_links` finds
  every stale `crate::inference::` link; a grep for `inference::` in comments finds the prose. Doc links inside
  `imaging` cannot point into `opai`, so the ones naming model callers (as `bilinear`'s and `rows`' headers do) become
  plain prose.
- [A behavioural change hides inside a "pure move"] → no function body changes except import paths and visibility.
  The existing fake-backend suite, the pixel-equality tests and the ignored live checks are the oracle, and all three
  must pass unchanged.
- [`models` drifts back to importing `inference` in a later slice] → the rewritten tier header states the rule. A
  mechanical check is a deliberate non-goal (see above) and can be added if it happens.

## Migration Plan

1. **Baseline.** On the commit before any of this change, run D7's measurement and save the JSON.
2. Create `crates/imaging`, add it to the workspace, and move the primitives, `ChannelDepth`, `run_tiled` and their
   unit tests. Add the `test-support` feature.
3. Create `opai::pipeline` from `inference/pipeline.rs`, `inference/session.rs` and `Backend`, with the backend
   fixtures.
4. Move `inference/filter.rs` to `models/filter.rs`.
5. Move `Subject` into `models` and `PassWeights` into `models/upscale/passes.rs` (D8). The dispatch stays (D3).
6. Update every import. Confirm `grep -rn "crate::inference" crates/opai/src/models` finds nothing.
7. Rewrite the tier header in `models/mod.rs` and the headers of `inference/mod.rs` and `imaging`'s `lib.rs`.
8. Run the full suite, `clippy`, `cargo doc` with broken links denied, and the ignored live checks on CPU and CoreML.
9. **Gate.** Re-run D7's measurement on the result and compare. Apply the remedies in order until it passes, and record
   both tables.

Rollback: the change is a single revertable commit series with no persisted state, cache key or public API change.
Run-cache entries on disk are keyed on operation identity, which does not move.

## Validation

Validated on macOS (Apple silicon) with the CPU and CoreML execution providers. CUDA and TensorRT are untested here.
Nothing in the change is provider-specific, but D7's timings say nothing about them.

## Measurements

### Baseline (task 1.1)

- **Commit:** `85a8e9e03d780a14d391dc2e75bcd8a13efa7d37`, release build, `perftest` 26.9.0.
- **Machine:** macOS on Apple silicon, `-p coreml`; every run reports `as_requested` on CoreML.
- **Photograph:** `target/perf-baseline/photo.png`, 4032x3136 (12.6 MP), SHA-256
  `524be527357aaa0214dbb1a88557318beeca0cc3c25c8c3ed413a3821b081f64` — `~/Desktop/test.png` (1152x896, SHA-256
  `8413582281f7ea58773c7a8000b868c691ba4a1efe3c4d537dc0614b471aa547`) enlarged 3.5x with `sips -z 3136 4032`.
- **Runs:** `-n 10 -w 1 --json`, cache off; the JSON per combination is under `target/perf-baseline/before/`, and
  `target/perf-baseline/run.sh` reproduces it.
- `kyoto` FP32 on the sample was measured first on a cool machine; the other seven after a pause following the
  abandoned 12 MP Kyoto cells.

| Model | Precision | Image | Median (ms) | Std dev (ms) | Gate: after median must be ≤ (ms) |
|---|---|---|---:|---:|---:|
| `kyoto` | FP32 | sample 640x640 | 2464.6 | 29.6 | 2538.5 |
| `kyoto` | FP16 | sample 640x640 | 1578.9 | 2.6 | 1626.2 |
| `paris` | FP32 | sample 640x640 | 65.6 | 2.6 | 68.1 |
| `paris` | FP16 | sample 640x640 | 65.1 | 7.4 | 72.5 |
| `paris` | FP32 | photo 4032x3136 | 900.2 | 8.1 | 927.2 |
| `paris` | FP16 | photo 4032x3136 | 896.7 | 11.3 | 923.6 |
| `stockholm` | FP32 | sample 640x640 | 346.5 | 1.5 | 356.8 |
| `stockholm` | FP16 | sample 640x640 | 329.6 | 2.9 | 339.5 |

The gate column is the median plus the larger of 3% and one baseline standard deviation.

### After the move (task 6.3)

Same binary flags, same photograph, same script; each sweep started only once macOS reported the `nominal` thermal
state and the 1-minute load average was under 1.5. Medians in ms.

| Model | Precision | Image | Before median | Gate | After, no remedy | After, `#[inline]` | Std dev | Gate |
|---|---|---|---:|---:|---:|---:|---:|---|
| `kyoto` | FP32 | sample | 2464.6 | 2538.5 | 2475.1 (+0.4%) | 2441.7 (-0.9%) | 5.2 | pass |
| `kyoto` | FP16 | sample | 1578.9 | 1626.2 | 1579.1 (+0.0%) | 1546.4 (-2.1%) | 1.6 | pass |
| `paris` | FP32 | sample | 65.6 | 68.1 | 66.3 (+1.1%) | 57.4 (-12.5%) | 2.9 | pass |
| `paris` | FP16 | sample | 65.1 | 72.5 | 65.6 (+0.7%) | 60.2 (-7.6%) | 3.8 | pass |
| `paris` | FP32 | photo | 900.2 | 927.2 | 941.5 (+4.6%) | 869.9 (-3.4%) | 7.2 | pass |
| `paris` | FP16 | photo | 896.7 | 923.6 | 941.6 (+5.0%) | 870.3 (-2.9%) | 7.8 | pass |
| `stockholm` | FP32 | sample | 346.5 | 356.8 | 350.9 (+1.3%) | 345.3 (-0.3%) | 1.5 | pass |
| `stockholm` | FP16 | sample | 329.6 | 339.5 | 331.1 (+0.5%) | 327.5 (-0.6%) | 1.2 | pass |

- **Without a remedy** the tiled models were unchanged, and `paris` on the 12 MP photograph was steadily 4.6% (FP32)
  and 5.0% (FP16) slower — every run about 40 ms above the baseline's, the full-resolution presentation and blend
  paying for per-pixel calls that no longer inlined across the crate boundary. It failed the gate.
- **Remedy 1 was enough.** `#[inline]` on the hot non-generic primitives — `Normalisation::encode`, `encode_unit`
  and `decode`, `Sampler::rgb`, the `u8`/`u16` `Channel` impls' `from_unit`, `to_unit` and `matching`,
  `bilinear::Axis::tap` and `bilinear::sample`, and `blend::ramp_weight` — brings every combination to or below its
  baseline. The `Channel` impls were not on D7's list; they are the same kind of function, concrete and called per
  pixel from generic code monomorphised in `opai`. Thin LTO was not needed and was not added.
- A first attempt at this measurement ran while another application was using the GPU. Its runs climbed within a
  cell (Stockholm FP32 from 372 ms to 685 ms) and it was discarded; the table above is the re-run.
- The JSON is under `target/perf-baseline/after/` (no remedy) and `target/perf-baseline/after-inline/` (final).
