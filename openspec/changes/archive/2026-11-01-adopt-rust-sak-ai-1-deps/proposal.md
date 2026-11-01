## Why

Open Photo AI carries its own implementation of everything needed to get ONNX inference running: the pinned table of
runtime and NVIDIA archives, their download/verify/extract pipeline, the GPU probe that picks them, the loader search
path, and the runtime load. That code now lives, generalised, in `rust-sak`'s `ai` feature, which installs the same
archives (byte-identical, same versions) from Hugging Face (`huggingface.co/vegidio/general/tree/main/deps`). Keeping a
second copy in OPAI means two implementations to fix, and the GitHub release assets it uses are at the 2 GB per-asset
ceiling (`tensorrt_linux_arm64.7z` is 1.99 GB).

This is **slice 1 of 3** of the `adopt-rust-sak-ai` series:

1. **`adopt-rust-sak-ai-1-deps`** (this change): the non-model dependencies: install, GPU selection, library search
   path, runtime load.
2. `adopt-rust-sak-ai-2-sessions`: session building, provider options and the ladder, on a single-rung `rust-sak`
   build with OOM classification and a shared TensorRT timing cache.
3. `adopt-rust-sak-ai-3-models`: model downloads, on a generic Hugging Face model installer added to `rust-sak`'s `ai`.

The user-visible behaviour must stay the same. Where `rust-sak` cannot do what OPAI does today, `rust-sak` is extended
rather than OPAI keeping its own code.

## What Changes

**In `rust-sak` (`ai` feature), released under a new tag before OPAI moves:**

- Local-first install: a dependency that is present and intact is used with no network request.
- The install marker records every installed file and its size. The presence check verifies them locally, so a
  deleted or truncated file causes a reinstall (what OPAI's `.manifest.json` does today).
- A cross-process lock per dependency folder, held for the whole install, because the install root is now shared
  between applications.
- Installing a version of a dependency removes that dependency's other version folders, skipping any that another
  process holds locked.
- A plan step reports each dependency's state and published size from one listing, before anything moves. Progress
  events gain a phase (download / extract), bytes and total, and an "already current" event.
- `gpu::meets` / `meets_with` report why a machine does not qualify (no driver, driver too old, no device, compute
  capability too low), not just `false`.
- `load` takes the application name for the environment and a logger hook, silences the runtime's logging at process
  exit, and exposes the loaded environment and its WebGPU devices to the caller.

**In Open Photo AI:**

- **BREAKING (on-disk):** the ONNX Runtime and the NVIDIA libraries install under `<user config>/io.vinicius.ai/<name>/<version>`,
  shared with other applications, instead of `<user config>/io.vinicius.opai/runtime` and `.../libs/{cuda,cudnn,tensorrt}`.
  The old folders are deleted best-effort on startup (code marked with a TODO for later removal). Each user therefore
  downloads the runtime, and on NVIDIA machines the GPU libraries, once again after updating.
- Dependencies are named by their Hugging Face folder URL. The pinned table (tags, SHA-256, sizes, library names) and
  the GitHub release download path are removed. Integrity comes from the hash the Hugging Face API reports.
- `ort` moves from `=2.0.0-rc.13` (`api-28`) to `rust-sak`'s `=2.0.0-rc.15` (`api-30`), used through
  `rust_sak::ai::ort`. Two `ort` versions cannot link into one build, so this lands here.
- The archive half of OPAI's install pipeline, its GPU-selection code, its library-search-path code and its runtime
  loader are replaced by `rust-sak` calls. The bare-file half of the pipeline stays for models until slice 3.
- Models remain in `<user config>/io.vinicius.opai`, untouched by this slice.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `initialization`: where the runtime and GPU libraries live and where they come from. Dependencies are named by
  Hugging Face folder and verified against the API-reported hash, not a pinned one. The GPU-selection rule is restated
  as the driver/compute-capability rule the code already uses. A present install needs no network. Installs are
  serialised across processes. A new version replaces the old version folder. The old install folders are removed.
  The provider report includes WebGPU.

## Impact

- **Crates:** `opai` (almost all of it: `deps/`, `gpu.rs`, `libpath.rs`, `runtime.rs`, `setup.rs`, `config.rs`, the
  `ort` bump across inference code). `gui` and `cli` call `opai::prepare_library_path` and consume
  `PlannedDependency`/`Progress`. Their signatures stay, so neither front end gains logic. `perf` follows the
  `rust-sak` tag bump.
- **`rust-sak` checked first:** `ai` already provides `Source`, `Dependency`, `install`, `is_present`, `load`,
  `libpath::prepare`, `gpu::meets`, `hf::Hub` and the `ort` re-export. What it lacks for identical behaviour is listed
  under "What Changes" and is added there, not in OPAI.
- **Dependencies:** `rust-sak` gains a new tag, with the `ai` feature enabled in `opai`. `ort` is dropped as a direct
  dependency of `opai`.
- **Network:** downloads move from `github.com/vegidio/open-photo-ai/releases` to `huggingface.co/vegidio/general`.
- **Disk:** a one-time re-download per user, and old folders deleted.
- **Risk:** `rust-sak`'s Linux restart, WSL2 driver folder, Windows `PATH` step and NVIDIA installs have never run on
  real hardware. Testing on Linux+NVIDIA and Windows+NVIDIA machines gates the release.
- **Estimated tasks:** 15.
