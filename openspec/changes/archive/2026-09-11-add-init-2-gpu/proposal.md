## Why

Slice 1 put the ONNX Runtime on disk, and on an NVIDIA machine that runtime can do nothing with the
card. The CUDA and TensorRT execution providers shipped inside the runtime archive are stub loaders:
each one `dlopen`s the CUDA, cuDNN and TensorRT shared libraries at session-build time, and neither
is installed, so today every machine runs on the CPU regardless of what hardware it has.

This slice installs those libraries when the hardware can use them, puts their directories where the
loader will actually look, and answers the question a front end has to ask before it can offer the
user a choice of AI processor: which execution providers does this machine support?

## What Changes

Task estimate: **14**. The full `opai::initialize()` feature is split into four slices (see
design.md - Slices); this proposal covers slice 2 only. Nothing in `../rust-sak` changes.

**In `opai`:**

- **NVIDIA detection over `rust_sak::sysinfo::gpu_info`** - `cuda` when any adapter reports NVIDIA,
  `tensorrt` when one of them is RTX-branded, consumer or professional. Both are rough proxies, as
  they are in Go: they say the hardware is right, not that the driver will load.
- **Three more pinned dependencies** - CUDA `cuda/13.3.0`, cuDNN `cudnn/9.23.1` and TensorRT
  `tensorrt/10.14.1`, each published for `linux_amd64`, `linux_arm64` and `windows_amd64` only,
  installed into `<config>/libs/{cuda,cudnn,tensorrt}/`. They go through slice 1's pipeline unchanged
  - the same fingerprint, verify, extract, record ordering - because that is what it was shaped for.
- **`Dependency` gains `Cuda`, `Cudnn` and `TensorRt`**, so a progress report names which of the four
  installs it belongs to without a front end learning the names. The enum is already
  `#[non_exhaustive]`, so this is additive.
- **Loader search-path handling.** On Windows the library directories are appended to `PATH`. On
  Linux they have to be on `LD_LIBRARY_PATH` *before* the process starts, because glibc reads that
  variable once at exec time - so `opai` exposes a function a front end calls as the first thing in
  `main`, which creates the four directories and re-execs the process once with them prepended.
- **`SupportedProviders`** - the report of which execution providers this machine can offer: CPU
  always, CoreML on macOS 12 or newer, CUDA and TensorRT on the hardware that has them. Returned
  from `initialize()`.
- `opai/Cargo.toml` adds `rust-sak`'s `sysinfo` feature and, on macOS only, `objc2-foundation` for
  the operating-system version.

**Deliberately not in this slice:** nothing calls the re-exec function yet. It belongs in a front
end's `main`, and the front ends are wired to `initialize()` in their own change after slice 3.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `initialization`: extends what the application installs before it can run a model. New
  requirements for GPU-library installation gated on detected hardware, for the loader search paths
  those libraries have to be found on, and for the supported-execution-provider report. The existing
  requirements about verification, manifests, replacement and progress are unchanged and apply to
  the three new dependencies as written - which is the point of reusing slice 1's pipeline.

## Impact

**Crates touched:** `opai` only. `gui` and `cli` are untouched. No logic goes into a front end: the
detection, the install and the search-path construction are all library code, and the one thing a
front end must do itself - calling the re-exec before its runtime starts - is a single call to an
`opai` function rather than a reimplementation of it.

**`rust-sak` - what was found before anything was proposed.** `sysinfo::gpu_info()` covers the GPU
probe completely, and covers it better than the Go original: it asks IOKit, DXGI and sysfs directly
where go-sak shells out to `system_profiler`, a PowerShell CIM query and `lspci`/`nvidia-smi`. It is
enabled here and nothing about hardware detection is written locally. `fetch`, `fs` and `crypto` are
already in use through slice 1's pipeline, which the three new dependencies pass through unchanged.

The one gap is the process re-exec and the environment-variable append that go-sak provides as
`os.ReExec` and `os.AppendEnvPath`. **It is not proposed as a `rust-sak` addition**: Rust's standard
library already has both halves - `std::os::unix::process::CommandExt::exec` is the `execvp` that
never returns, and `std::env` covers the variable - so the only thing left is this application's own
rule about which four directories go on the path in which order. That is not general-purpose, so it
lives in `opai`. See design.md - D5.

**New third-party crates:** `objc2-foundation` 0.3.2, macOS-only, feature-gated to `NSProcessInfo`,
for the operating-system version behind the CoreML check. It is the current standard Foundation
binding (the `objc2` family that replaced the unmaintained `objc`/`cocoa` crates; ~12M downloads a
month, last released October 2025) and it makes exactly the call the Go cgo shim makes. The
alternative, `libc`'s `sysctlbyname("kern.osrelease")`, needs an `unsafe` block and an undocumented
Darwin-to-macOS version mapping to save one dependency on one platform. It duplicates no `rust-sak`
module.

**Parity divergences from the Go implementation, deliberate:**

- **The NVIDIA install moves from the GUI into the library.** In Go it lives in
  `cmd/gui/services/app_service.go`, so the Go CLI never installs CUDA or TensorRT and never re-execs
  - GPU inference is GUI-only there, which is precisely the "logic trapped inside a front end" the
  project's layering rule exists to prevent. Here `initialize()` does it for both front ends.
- **The supported-provider report is the library's answer, not the GUI service's.** Same reason: the
  CLI needs to know what it can offer too.
- Detection is one probe through `rust-sak`, not three shell-outs. Behaviourally identical answers,
  minus the second-or-more `system_profiler`/`lspci` cost and minus a `PATH` dependency on tools that
  may not be installed.
- **TensorRT is offered on professional RTX cards, which Go leaves on CUDA.** Go tests the name
  against `rtx 20|30|40|50`, so an `RTX A6000`, an `RTX 5000 Ada Generation`, an `RTX PRO 6000
  Blackwell`, a `Quadro RTX 6000` or a `TITAN RTX` is treated as TensorRT-incapable when all of them
  are Turing or newer. The test here is the RTX brand itself, which is the same set as "compute
  capability 7.5 or newer" for RTX-named hardware and needs no edit per product line. See
  design.md - D2.

**Parity kept, including where it is arguable:** a GPU dependency that fails to install fails
`initialize()`, exactly as the Go GUI service's `Initialize` returns an error. The alternative -
logging it, reporting the provider unsupported and launching on the CPU - is a better behaviour for a
transient failure on a 1.4 GB archive, but it is a user-visible change to what a failed launch means
and it belongs in a change of its own with the front ends in view. Recorded here so it is not lost.

**Not in scope:** the `ort` crate and the ONNX environment, including actually *building* a session
on any of these providers and the CPU fallback when one fails (slice 3); wiring either front end to
`initialize()` (its own change); models; the engine cache and its invalidation on a runtime bump; the
legacy-layout pruning; OpenVINO and any provider beyond Auto/CPU/CoreML/CUDA/TensorRT.
