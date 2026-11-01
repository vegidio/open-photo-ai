## Context

See `proposal.md` for the motivation. Today `opai` owns the whole path from "nothing on disk" to "runtime loaded":

```
setup.rs (Plan, select, install)
  |-- deps/artifact.rs   pinned table: tag, SHA-256, size, lib, webgpu per platform
  |-- deps/release.rs    Release -> Dependency descriptor (shared with models)
  |-- deps/install.rs    check / acquire / transfer / expand (archive + bare)
  |-- deps/manifest.rs   .manifest.json: fingerprint + file list with sizes
  |-- gpu.rs             CudaSupport / TensorRtSupport with reasons
libpath.rs               library_dirs + search_path + execvp / PATH
runtime.rs               ort::init_from, logger, atexit silence, WebGPU plugin
runtime/diagnostics.rs   ORT logger: OOM marks, burst tallies
```

`rust-sak` `ai` (tag `26.11.0`) provides the same steps, generalised: `Source`, `Dependency::{runtime,libraries}`,
`install`, `is_present`, `load`, `libpath::prepare`, `gpu::meets`, `hf::Hub`, and the `ort` re-export. Its archives on
Hugging Face are byte-identical to OPAI's GitHub release assets: same versions, same sizes.

Constraints:

- `rust-sak` `ai` builds `ort =2.0.0-rc.15` with `api-30`. `opai` builds `=2.0.0-rc.13` with `api-28`. Two `ort-sys`
  versions cannot link into one binary, so turning on `ai` forces the `ort` bump in this slice, not in slice 2.
- `rust-sak`'s `load` builds its own `Environment` and keeps it private. OPAI's session builder (replaced only in
  slice 2) needs that environment and its WebGPU devices.
- The `initialization` spec's GPU-selection text (RTX branding, "driver doesn't matter") is stale. The code already
  uses the driver's CUDA version and the device's compute capability. The delta restates the rule as the code and
  `rust-sak` implement it.

## Slices

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `adopt-rust-sak-ai-1-deps` | Runtime and NVIDIA installs, GPU selection, library path and runtime load on `rust-sak` `ai`; `ort` rc.15; shared `io.vinicius.ai` root; old folders removed | - | **current** |
| 2 | `adopt-rust-sak-ai-2-sessions` | Session building, provider options, ladder and tuning on `rust-sak` (single-rung build with OOM classification, shared serialised TensorRT timing cache); OPAI keeps rung policy, resident cache, OOM retry, decline latch | 1 | pending |
| 3 | `adopt-rust-sak-ai-3-models` | Generic Hugging Face model installer in `rust-sak` `ai` (named files from a folder, API-hash verified, resumable, progress, trust-local option); old model files deleted and re-downloaded (TODO-marked); compiled-in and cached listings dropped; `ModelTrust::LocalFiles` mapped onto trust-local | 1 | pending |

Each slice ships its `rust-sak` additions under a new tag first, then the OPAI migration that uses them.

## Goals / Non-Goals

**Goals:**

- OPAI holds no code for installing, selecting, locating or loading the runtime and NVIDIA libraries. What remains in
  OPAI is policy: which folders to name, when to report what, error mapping, and the old-folder cleanup.
- Every user-visible behaviour in `initialization`, `gui-setup` and the CLI's setup output stays as it is. The
  exceptions are the ones the delta spec states: install location, source, one re-download, and a faster offline
  launch.

**Non-Goals:**

- Session building, provider option maps, the ladder, `EpProfile` → `Tuning` (slice 2). `opai`'s
  `ExecutionProvider` and `SupportedProviders` stay as OPAI types in this slice.
- Model installs (slice 3). The bare-file half of `deps/install.rs` and `deps/manifest.rs` stays, used only by models.
- Pinning hashes in the binary. Integrity of the runtime and NVIDIA archives now rests on the Hugging Face API's hash
  and HTTPS. This was decided explicitly.

## Decisions

### D1. Extend `rust-sak`, never keep a parallel OPAI implementation

Each gap between `rust-sak` `ai` and OPAI's current behaviour is filled in `rust-sak`, because the gap is
infrastructure that any app installing from the same folders needs:

| Gap | `rust-sak` addition | Why not in OPAI |
|---|---|---|
| Every launch calls the HF API (30 s × 5 retries before falling back) | **Local-first install**: a dependency that `is_present` and is intact is skipped with no request | Every app sharing the root wants a no-network launch |
| The marker only proves the install finished, not that its files are still there | **Marker records files and sizes** (with the archive hash), checked locally by `is_present` / install, symlinks inspected as links | Antivirus quarantining a cuDNN DLL is a property of the install, not of OPAI |
| Shared root, no lock | **Exclusive lock per version folder** (`<root>/<name>/<version>/.lock`, OS advisory lock so a dead process releases it), held for download+verify+extract. A waiter re-checks the folder after acquiring | Only the installer knows when it writes |
| Old versions accumulate | **Replace on install**: after a version's marker is written, its sibling version folders under `<root>/<name>/` are removed. A folder whose lock is held by another process is skipped | Same |
| Events lack sizes, phases, bytes and an "already current" signal | **`plan(root, &deps)`** returning per-dependency `{ state: Present \| Missing, size }`: sizes from the marker for present deps, so no network, and from one listing for missing ones, which `install` reuses. **Events**: `Progress { dependency, phase: Download \| Extract, bytes, total, fraction }`, `Verifying`, `Current { dependency }` | OPAI would otherwise guess the phase from `fraction < 0.8` and estimate bytes |
| `gpu::meets` returns `bool` | **`gpu::check(&req) -> Result<(), GpuRefusal>`** with `NoDriver`, `DriverTooOld { driver, required }`, `NoDevice { driver }`, `DeviceTooOld { name, compute, required }`, `ProbeFailed`; `meets` kept as a wrapper | The reasons are about the requirement, which `rust-sak` owns |
| `load` names the environment `"rust-sak"`, uses the default logger, no exit silencing, environment private | **`load_with(runtime_dir, &LoadOptions { name, logger })`**, `atexit` that drops the log level to fatal, **`ai::environment()`** and **`ai::webgpu_devices()`** public | Every app wants its name on runtime diagnostics. OPAI's OOM counting (`runtime/diagnostics.rs`) stays OPAI's, plugged in through the logger hook |

The exact signatures are settled in `rust-sak` during implementation. The table fixes their behaviour.

Alternative considered: keep OPAI's `check` and manifest in front of `rust-sak` `install`. Rejected, because that is
exactly the duplicated code this series removes.

### D2. Dependencies are named by Hugging Face folder URL in one place

`opai` holds one small table of four folder URLs (`deps/onnx/1.30.0`, `deps/cuda/13.3.0`, `deps/cudnn/9.23.1`,
`deps/tensorrt/10.14.1`) and the CUDA/TensorRT `GpuRequirement`s (driver `(13, 0)`, compute `(7, 5)`). A version bump
is a one-line URL change. The display names (`ONNX Runtime`, `NVIDIA CUDA`, …) stay mapped from OPAI's public
`progress::Dependency` enum, so the front ends do not change.

The install root is `rust_sak::fs` user config dir + `io.vinicius.ai`, a constant beside `APP_NAME`.

### D3. OPAI translates; it does not interpret

`setup.rs` becomes:

1. `gpu::check` for CUDA, then TensorRT, logging each refusal at `info` with the reason (as today).
2. Build the dependency list in install order: runtime, CUDA, cuDNN, TensorRT.
3. `ai::plan`, which gives `PlannedDependency { dependency, size }` rows and the single `on_plan` call.
4. `ai::install`, with events mapped onto `Progress`: `Download` → `Phase::Downloading`, `Extract` →
   `Phase::Extracting`, `Current` → `Phase::AlreadyInstalled` with fraction 1 and bytes 0. `InstallError`/`Failure` is
   mapped onto `InitError`, keeping the variants `gui-setup` uses to name a known cause (disk full, network, damaged).
5. `ai::load_with(name = "io.vinicius.opai", logger = diagnostics::logger())`.
6. `SupportedProviders` is built from `rust-sak`'s `Supported::detect(cuda, tensorrt)` field by field. OPAI keeps its
   own public type until slice 2.

Install-duration and failure records (`record_ended`) stay in OPAI, driven from the events and the result, so
Grafana keeps the same `dependency` / `version` / `duration` fields.

### D4. Policies that stay in OPAI

- **Failed-load latch.** The spec requires every later initialization in a process to fail after a failed load.
  `rust-sak` allows a retry. OPAI keeps its `FAILED_LOAD` latch around `load_with`. This is app policy, not
  infrastructure.
- **Compiled-artifact invalidation** (`engines/.version`) is keyed on the runtime folder's version (`1.30.0`) in
  place of the GitHub tag (`runtime/1.30.0`). The first launch after the update sees a different string and discards
  the engines once, which costs one recompile and is correct, since the runtime install is fresh too.
- **Old-folder cleanup**: best-effort removal of `<config>/io.vinicius.opai/runtime` and `.../libs` on initialize,
  marked `// TODO: remove once users have upgraded past this release`. It runs after the single-instance claim, not
  beside `remove_roaming_dir` (which runs before it), so a still-running process of an earlier release never loses
  the runtime it loaded.

### D5. Library search path

`opai::prepare_library_path(name)` keeps its public signature, so `gui` and `cli`'s `main` are unchanged. It now
computes the four version folders with `ai::dirs::dir_for(root, &source)` (pure, no network) and calls
`ai::libpath::prepare(&dirs, "OPAI_REEXEC")`. The guard name is unchanged, so a running process upgraded in place
behaves the same.

### D6. `ort` through `rust_sak::ai::ort`

`opai` drops its direct `ort` dependency and imports `rust_sak::ai::ort`, so one `ort` is compiled. The rc.13 → rc.15
API differences (e.g. `Session::builder(&environment)` taking the environment explicitly) are absorbed in `opai`'s
inference and session code, which gets the environment from `ai::environment()`. No other crate is introduced.

### Ownership and lifetime

The runtime `Environment` is owned by `rust-sak`'s process-wide `load` state and lives until process exit. OPAI holds
clones obtained from `ai::environment()` for session builds and never tears it down. That matches the existing
"started once, lives as long as the process" requirement. The lock in D1 is held only for the duration of an
`install` future, and dropping the future releases it. Sessions, caches and memory budgets are unchanged in this
slice.

### Crates and features

`opai`: `rust-sak` gains `ai` (which pulls `crypto`, `fetch`, `fs`, `sysinfo`), bumped to the new tag in `opai`,
`gui`, `cli` and `perf` together so the lock file holds one `rust-sak`. `ort` is removed from `opai`'s
`[dependencies]` and `[dev-dependencies]`. `gui` and `cli` change only the tag.

## Risks / Trade-offs

- [Untested platform code: `rust-sak`'s Linux restart, WSL2 driver folder, Windows `PATH` step and NVIDIA installs
  have never run on hardware] → Release gate: run a first launch, a second (offline) launch and a CUDA/TensorRT
  enhancement on a Linux+NVIDIA and a Windows+NVIDIA machine before tagging the OPAI release.
- [Integrity now rests on the HF API hash, from the same host as the file] → Accepted. The "one version per folder"
  rule is documented in `rust-sak`. Revisit if the repository's access model changes.
- [Every user re-downloads the runtime, and on NVIDIA machines about 2.5 GB of libraries, once] → Accepted. It is
  the cost of the shared root, and the setup dialog already presents downloads.
- [Two apps pinning different versions of one dependency delete each other's version on every install ("replace
  immediately")] → Pin the same versions across first-party apps. A locked folder is never removed mid-install.
- [`ort` rc.13 → rc.15 API churn across inference code] → Mechanical changes. All ONNX models are tested on CPU and
  CoreML through Rust, as the project requires.
- [HF API rate limits for anonymous requests] → Local-first means only installs call the API, and `plan` and
  `install` share one listing.
- [Platforms validated on]: macOS arm64 with CPU, CoreML and WebGPU, by the developer and by `rust-sak`'s tests.
  Untested until the release gate: Linux x64/arm64 and Windows x64/arm64, with CUDA, TensorRT and WebGPU there.

## Migration Plan

1. Implement the `rust-sak` additions (D1). Tag `rust-sak`.
2. Bump `rust-sak` in all four crates, enable `ai` in `opai`, and drop `ort`.
3. Replace `opai`'s install, GPU selection, library path and load (D2–D6). Delete `deps/artifact.rs`, the archive
   branch of `deps/install.rs`, `gpu.rs`'s selection, `libpath.rs`'s mechanism and `runtime.rs`'s loader.
4. Add the old-folder cleanup.
5. Hardware release gate.

Rollback: revert the OPAI commit. The old release still installs into `io.vinicius.opai/runtime`, so it re-downloads
there. The `io.vinicius.ai` folder is harmless left behind.
