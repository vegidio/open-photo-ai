## Why

OPAI still carries its own session builder: the provider vocabulary, the machine report, the fallback ladder, each
provider's baseline options, the per-model tuning type, the builder calls, the TensorRT serialisation and the
out-of-memory detection. `rust-sak`'s `ai` feature (tag `26.11.0`) already has a generalised copy of most of it
(`ExecutionProvider`, `Supported`, `ladder`, `Tuning`, `provider_options`, `session`), written from OPAI's code. Two
copies means two places to fix, and they have already started to drift: `rust-sak`'s timing cache is per model, while
OPAI's is shared.

This is **slice 2 of 3** of the `adopt-rust-sak-ai` series:

1. `adopt-rust-sak-ai-1-deps` (done): runtime and NVIDIA installs, GPU selection, library path and runtime load on
   `rust-sak`.
2. **`adopt-rust-sak-ai-2-sessions`** (this change): session building, provider options, the ladder and tuning on
   `rust-sak`.
3. `adopt-rust-sak-ai-3-models`: model downloads, on a generic Hugging Face model installer in `rust-sak`.

User-visible behaviour stays the same. The one exception is a single recompile of every model's compiled engines,
caused by the cache layout change below. Where `rust-sak` cannot do what OPAI does today, `rust-sak` is extended
instead of OPAI keeping its own code.

## What Changes

**In `rust-sak` (`ai` feature), released under a new tag before OPAI moves:**

- **Single-rung build.** A public function builds a session on exactly one rung (an ordered list of accelerators, or
  none for the CPU) and returns the session or a rung error. The caller walks the ladder. `session()` keeps its
  behaviour and is rebuilt on top of it.
- **Out-of-memory classification.** A rung error says whether the device ran out of memory. That is the case when the
  runtime logged an out-of-memory record on the building thread during the build, or when its error message says so.
  The CPU rung is never classified as out of memory. `load_with` always installs `rust-sak`'s own logger wrapper,
  which counts those records and then forwards each one to the caller's logger, or to `tracing` when there is none.
- **Shared, serialised TensorRT timing cache.** The session request takes explicit `CachePaths`, so a caller can
  point every model at one timing folder. `CachePaths::under` stays as a convenience. Every rung that attaches
  TensorRT is serialised process-wide. A failed TensorRT rung empties the timing folder and logs that it did.
- The CUDA device-memory probe that sizes TensorRT's workspace runs at most once per process.

**In Open Photo AI:**

- **BREAKING (public API of `opai`):** `ExecutionProvider` and `SupportedProviders` become re-exports of
  `rust_sak::ai::ExecutionProvider` and `rust_sak::ai::Supported`. Spellings, serde form and fields are identical, so
  stored settings and the front-end JSON do not change. `ExecutionProvider::from_str` now fails with `rust-sak`'s
  `UnknownProvider`. `InitError::UnknownProvider` is kept and built from it.
- `EpProfile` and its `CoreMlComputeUnits` / `CoreMlSpecialization` / `ExecutionMode` enums are deleted. Every model
  declares a `rust_sak::ai::Tuning` instead. The fields and values are the same, and each model's measured settings
  are unchanged.
- `providers/options.rs` (chain resolution, ladder, option maps, session plan) and the builder half of
  `sessions/build.rs` (attaching providers, settings, the TensorRT mutex, timing-cache discard, OOM matching) are
  replaced by `rust_sak::ai::ladder` and the single-rung build.
- OPAI keeps the policy: which rungs to skip (the `Auto` decline latch), the resident session cache and its single
  flight, the one out-of-memory retry after releasing unused sessions, the model-file pre-check that tells an install
  failure apart from a build failure, the `session_build` span, the downgrade records and the fallback metric.
- **Cache layout (on disk):** a model's compiled output moves from `engines/<artifact>/` to
  `engines/<artifact>/tensorrt/` and `engines/<artifact>/coreml/`, one folder per provider. The shared timing cache
  stays at `engines/.timing/`. The `engines/.version` stamp now records the cache layout beside the runtime version,
  so the first launch after updating discards the old flat engines once.
- `runtime/diagnostics.rs` drops its out-of-memory counter, which moves to `rust-sak`. Its severity correction,
  repeat throttle and flush stay.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `model-sessions`: what a provider compiles from a model is kept in a folder of that provider's own, inside the
  model's folder.
- `execution-providers`: the supplied cache directories are one per provider for each model, plus the one shared
  timing directory.
- `initialization`: compiled provider artifacts are also discarded when the cache layout they were written in
  changes, not only when the runtime version does.

## Impact

- **Crates:** `opai` (mostly `providers/`, `sessions/build.rs`, `sessions/caches.rs`, `runtime/diagnostics.rs`,
  `error.rs`, `lib.rs`, plus about 22 model files that declare a tuning). `gui` and `perf` get small edits where they
  match the `from_str` error or name `InitError::UnknownProvider`. Neither front end gains logic. `cli` and `perf` take
  the `rust-sak` tag bump.
- **What `rust-sak` already has:** `ExecutionProvider`, `Supported`, `AUTO_ORDER`, `ladder`, `Tuning` and its enums,
  `provider_options`, `trt_workspace_bytes`, `CachePaths` and `session`. They match OPAI's behaviour, including
  graph capture being off, `error_on_failure`, WebGPU attached through its first device, and disabled optimizers
  joined by `;`. What it lacks is listed under "What Changes" and is added there, not in OPAI.
- **Dependencies:** a new `rust-sak` tag, bumped in `opai`, `gui`, `cli` and `perf` together. No new crates.
- **Disk:** a one-time discard of every compiled engine. TensorRT engines rebuild in minutes per model on first use;
  CoreML recompiles in seconds to minutes.
- **Risk:** the CUDA and TensorRT rungs, the TensorRT serialisation and the OOM classification cannot run on macOS.
  An NVIDIA hardware test gates the release, as it did for slice 1.
- **Estimated tasks:** 12.
