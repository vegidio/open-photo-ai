## Slices

`opai::initialize()` - download and install the pinned ONNX Runtime, then the NVIDIA libraries on a
machine that has an NVIDIA GPU, reporting progress throughout - estimates well past the 15-task
ceiling as one change. It is split into four slices. Each ends with the workspace building, testing
and passing CI on its own.

| # | Slice | Delivers | Depends on | Status |
|---|-------|----------|------------|--------|
| 0 | `add-init-0-rust-sak` | `rust-sak`: `.part` + identity sidecar, atomic rename and `If-Range` in `fetch::download`; `resume_key`; progress hook and header totals in `fs::extract`. CalVer alignment and the `26.9.0` tag. Here: `opai`'s first dependency, pinned to that tag. | nothing | **done** (archived `2026-09-11`) |
| 1 | `add-init-1-runtime` | `rust-sak`: an opt-in content digest computed during a transfer, released as `26.9.1`. Here: the config directory and app name, the pinned artifact table (tag + SHA-256 + size + library name per platform), `Progress`/`Dependency`/`Phase`, manifest bookkeeping (`fingerprint` -> `intact` -> `remove`), the download/verify/extract/record pipeline, and `Opai::initialize(name, on_progress)` installing the ONNX Runtime into `<name>/runtime/`. CI widens to a three-OS matrix. | slice 0 | **done** (archived `2026-09-11`) |
| 2 | `add-init-2-gpu` | `opai`: NVIDIA detection via `sysinfo::gpu_info`, CUDA + cuDNN into `<name>/libs/{cuda,cudnn}/` and TensorRT into `<name>/libs/tensorrt/` when supported, loader search-path handling including the Linux `LD_LIBRARY_PATH` re-exec, and the supported-execution-provider report. | slice 1 | **current** |
| 3 | `add-init-3-ort-env` | `opai`: the `ort` crate, the ONNX environment started against the downloaded library, its log level, and the teardown/re-initialize lifecycle. | slice 2 | pending |

Only slice 2 is proposed. Slice 3 is proposed when it is reached.

Two things the series does not cover, recorded so they are not lost: wiring the front ends to
`initialize()`, to its progress and - new in this slice - to `prepare_library_path` and the provider
report; and the model download path, which reuses slice 1's pipeline with several sources per
dependency. Both are their own changes after slice 3.

## Context

See proposal.md - Why.

Slice 1 left a working pipeline and one dependency going through it. What this slice adds sits
almost entirely *around* that pipeline rather than inside it:

- `deps::install::install(dir, descriptor, reporter)` already takes an arbitrary directory and an
  arbitrary descriptor, and `config::sub_dir` already accepts a nested `sub` (`libs/cuda`) and
  validates it against escaping. Neither has to change.
- `Progress`, `Phase` and `Dependency` are `#[non_exhaustive]`, so three new variants are additive.
- `Reporter` is per dependency and already lands on exactly `1.0`, which is what makes four
  sequential installs four clean bars rather than one composite.
- `Artifact::lib` is already documented as "empty for a dependency with none", which is what the
  NVIDIA archives are - they carry no single library the loader is pointed at.

What is genuinely new is hardware detection, the loader search path, and the provider report.

`rust-sak` 26.9.1's `sysinfo` module covers the first completely: `gpu_info() -> Result<Vec<GpuInfo>,
SysinfoError>` with `{ name: String, vendor: Option<String>, memory: Option<u64> }`, asked through
IOKit on macOS, DXGI on Windows and `sysfs` plus `pci.ids` on Linux - no subprocess, where go-sak
shells out to `system_profiler`, a PowerShell CIM query and `lspci`/`nvidia-smi`. `GpuInfo` has
public fields and is not `#[non_exhaustive]`, so a test builds adapters directly and every
classification below is checkable on a runner with no GPU at all.

The Go implementation is the behavioural reference: `utils/nvidia.go` for the detection and the
install, `cmd/gui/services/app_service.go` (`Initialize`, `initializeCuda`, `initializeTensorRT`) for
the ordering and the provider report, `cmd/gui/main.go` (`setLibPathAndRestart`) for the Linux
re-exec, `utils/apple_darwin.go` for the CoreML check, and `internal/artifacts.go` for the pinned
table. Roughly half of `utils/nvidia.go` is the install, which is slice 1's pipeline here.

## Goals / Non-Goals

**Goals:**

- Three more dependencies that are descriptors in the existing table, not a second code path.
- Detection that is a pure function of a list of adapters, so every branch is tested on hardware
  nobody has.
- One entry point for the loader search path, covering both platforms' mechanisms, called once at
  the top of `main`.
- A provider report that is the library's answer rather than a front end's, so the CLI gets the GPU
  support the Go CLI never had.
- `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` green on all three OSes with
  no GPU on any of them.

**Non-Goals:**

- Building a session on any provider, or the CPU fallback when one fails to build. Slice 3.
- `ExecutionProvider` as a type. The report is four booleans here; the enum with `Auto` belongs to
  the slice that selects one, and `SupportedProviders` is `#[non_exhaustive]` so it can grow a
  `supports(ExecutionProvider)` method then without breaking anyone.
- Driver version checks. Both probes are hardware proxies, as they are in Go; a card whose driver is
  missing or too old still installs the libraries and fails later at session build.
- Parallel installs. Four sequential transfers, in dependency order, as in Go.
- Calling `prepare_library_path` from `gui` or `cli`. It belongs in their `main`, and they are wired
  up in their own change.
- Removing a GPU library when the hardware stops supporting it (a card pulled from a machine). The
  manifest makes it possible; nothing asks for it, and an uninstall path is its own change.

## Decisions

### D1. The three libraries are rows in the existing table, with two new fields on `Release`

```rust
pub(crate) struct Release {
    name: &'static str,        // "onnx-runtime", "cuda", "cudnn", "tensorrt" - the manifest's name
    prefix: &'static str,      // the asset family: "onnx", "cuda", ...
    tag: &'static str,         // "cuda/13.3.0"
    dir: &'static str,         // NEW: "runtime", "libs/cuda", "libs/cudnn", "libs/tensorrt"
    progress: Dependency,      // NEW: which variant its reports carry
    archives: &'static [Artifact],
}
```

Values copied verbatim from `internal/artifacts.go`: `cuda/13.3.0`, `cudnn/9.23.1`,
`tensorrt/10.14.1`, each with `linux_amd64`, `linux_arm64` and `windows_amd64` and an empty `lib`,
re-derivable with the `gh api .../releases/tags/...` command already recorded there and in
`artifact.rs`.

`dir` and `progress` move onto the row because the alternative is a `match` somewhere else that has
to be kept in step with the table - and the failure mode of getting it wrong is installing cuDNN's
files into CUDA's directory, where the next CUDA bump deletes them. Go keeps the destination at the
call site (`ReleaseDependency(libName, libName, path.Join("libs", libName))`) and gets away with it
because the two strings are the same variable; here they are four rows, so the row owns both.

`Release::pinned_for` is unchanged, including its `UnsupportedPlatform` error - what changes is that
one caller now treats that error as "not offered" rather than propagating it (D3).

### D2. Detection is a pure classifier over `&[GpuInfo]`, probed once per process

```rust
fn is_nvidia(gpu: &GpuInfo) -> bool;           // vendor == "nvidia" (ci) or name contains "nvidia"
fn is_tensorrt_capable(gpu: &GpuInfo) -> bool; // is_nvidia && name contains "rtx"

pub(crate) fn adapters() -> &'static [GpuInfo]; // OnceLock over sysinfo::gpu_info(), [] on error
```

The split is what makes this testable: the classifiers take adapters, the probe supplies them, and
the suite feeds constructed `GpuInfo` values - an RTX 4090, a GTX 1080 Ti, a Radeon, an Apple M2, an
adapter with `vendor: None` whose name still says NVIDIA - through every branch on a runner that has
none of them.

`OnceLock` rather than a probe per question, matching Go's `sync.OnceValues` and for the same
reasons: three callers want the answer, and short of hot-plugging an eGPU it cannot change while the
process runs. A probe that errors is memoised as an empty list, so a machine whose adapters cannot be
enumerated initializes on the CPU instead of failing - Go's behaviour, and the spec's.

**One deliberate difference from Go.** Go's TensorRT check requires `vendor == "nvidia"` *exactly*,
where its CUDA check also accepts a name containing "nvidia". That asymmetry is not intentional -
nothing in the file argues for it - and it means a Linux machine with no `pci.ids` database, where
the vendor string can be absent but the model name is still `NVIDIA GeForce RTX 4090`, would be
offered CUDA and not TensorRT. Both predicates use the same `is_nvidia` here.

**A second, larger difference: the TensorRT test is `rtx`, not `rtx 20|30|40|50`.** Go's four
substrings match the GeForce naming and miss every professional card - `RTX A6000`,
`RTX 5000 Ada Generation`, `RTX PRO 6000 Blackwell`, `Quadro RTX 6000`, `TITAN RTX` - all of which
are TensorRT-capable and are today offered CUDA only. One substring covers all of them.

It is not a loosening of the requirement, it *is* the requirement: the pinned TensorRT 10.14 needs
compute capability 7.5 or newer - Turing and up, Volta having been deprecated in 10.0 and removed in
10.5 - and NVIDIA introduced the RTX brand with Turing. "Named RTX" and "at least Turing" are
therefore the same set for RTX-branded hardware, which is why a single substring is more accurate
than an enumeration and needs no edit when the next product line ships. There is no pre-Turing NVIDIA
card carrying the RTX name for it to over-match.

It reads correctly on all three backends, which matters because they name cards differently: DXGI
reports `NVIDIA RTX A6000`, while Linux takes the model name from `pci.ids` and reports
`GA102GL [RTX A6000]` with the vendor carried separately - so a rule anchored on a leading vendor
word or on a `GeForce`/`Quadro` prefix would work on one platform and not the other. A bare
case-insensitive substring works on both.

**What it still misses, recorded rather than fixed.** TensorRT-capable NVIDIA hardware whose name
carries no RTX at all: the GTX 16-series (Turing, compute 7.5), the data-centre parts (T4, A100,
L40S, H100) and the Jetson modules that `linux_arm64` exists for. Each would need its own rule with
its own failure mode, none of them is a photo-editing machine except the GTX 16-series, and none can
be confirmed without the hardware in hand to see what `gpu_info` actually reports. They stay on CUDA,
which works.

### D3. An unpublished platform is an error for the runtime and a non-offer for a GPU library

The NVIDIA archives exist for `linux_amd64`, `linux_arm64` and `windows_amd64`. A machine with an
NVIDIA card on `windows_arm64` therefore detects as CUDA-capable and has nothing to install.

Go fails the launch there: `ReleaseDependency` errors, `InitializeNvidiaLib` wraps it, and the GUI's
`Initialize` returns it. **This slice reports the provider unsupported and continues.** The runtime
is what the application cannot run without, so a platform it is not published for stays a reported
failure; a GPU library is an optimisation, so a platform it is not published for is an optimisation
that is not offered. Failing a launch over a provider the user was never going to get is not a
behaviour worth porting.

This is narrowly scoped and is *not* the same question as a GPU install that fails, which does fail
initialization exactly as in Go (see proposal.md - Parity kept). Selection asks "is there anything to
install here?"; once the answer is yes, every later failure is fatal.

### D4. `SupportedProviders` hangs off the handle rather than changing `initialize`'s signature

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportedProviders {
    pub cpu: bool,       // always true
    pub coreml: bool,
    pub cuda: bool,
    pub tensorrt: bool,
}

impl Opai {
    pub fn providers(&self) -> SupportedProviders;
}
```

Go returns `SupportedEPs` from the GUI service's `Initialize`. Returning `(Opai,
SupportedProviders)` here would work and would mirror that, but the report is a property of the
initialized application, not a second result of initializing it - and `Opai` is already the thing
both front ends hold. Keeping `initialize`'s signature also keeps slice 1's tests and doc comment
honest, and the field lives beside the model registry and the ONNX environment that slices 3 and
later add to `Inner`.

Four booleans rather than a set of an `ExecutionProvider` enum, for the reason in Non-Goals: nothing
selects a provider until slice 3, and `#[non_exhaustive]` keeps that addition additive. `cpu` is a
field rather than an implicit truth so a front end can render the list without a special case.

`cuda` and `tensorrt` are true only when the library was actually installed, not when the hardware
was merely detected, which is what makes the report honest on the `windows_arm64` case in D3.

### D5. The macOS version comes from `NSProcessInfo` via `objc2-foundation`

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2-foundation = { version = "0.3.2", default-features = false, features = ["std", "NSProcessInfo"] }
```

Go asks `[[NSProcessInfo processInfo] operatingSystemVersion].majorVersion >= 12` through a cgo
shim, because macOS 12 is what the MLProgram model format the CoreML provider is configured with
requires. `objc2-foundation` makes the identical call from safe Rust.

It is the current standard Foundation binding - the `objc2` family that replaced the unmaintained
`objc`/`cocoa` crates, ~12M downloads a month, 0.3.2 released October 2025 - and it is close to free
here: `rust-sak`'s `sysinfo` feature, which this slice enables anyway, already pulls
`objc2-core-foundation` and `objc2-io-kit` into the macOS tree, so the `objc2` runtime is compiled
either way. Feature-gated to `NSProcessInfo` so the rest of Foundation is not.

Alternatives. **`libc`'s `sysctlbyname("kern.osrelease")`**, mapping Darwin major >= 21 to macOS 12:
one fewer dependency, at the cost of an `unsafe` block and a version mapping written nowhere in the
call - and the workspace lints demand a `SAFETY` comment that would have to explain a table. **No
version check at all**, reporting CoreML on every macOS build: simplest, and a user on macOS 11 would
be offered CoreML and get slice 3's CPU fallback - but it makes the report a guess on exactly the
platform the project is developed on. Both rejected; the user chose this one.

Non-macOS builds have no such dependency and report `coreml: false` from a `cfg`, as
`utils/apple_others.go` does.

### D6. One entry point owns the loader search path, and it is `unsafe`

```rust
/// # Safety
/// Must be called before any other thread exists - the first statement of `main`.
pub unsafe fn prepare_library_path(name: &str) -> Result<Vec<PathBuf>, InitError>;
```

It creates `<config>/runtime`, `<config>/libs/cuda`, `<config>/libs/cudnn` and
`<config>/libs/tensorrt`, then:

- **Linux**: prepends them to the inherited `LD_LIBRARY_PATH` and replaces the process image with one
  started under it, guarded by `OPAI_REEXEC=1` in the child so it happens at most once. glibc reads
  that variable once at exec time, so appending to it later - after the libraries have been
  downloaded - changes nothing about the `dlopen` calls the runtime makes.
- **Windows**: prepends them to `PATH`, which `LoadLibrary` consults at call time, so no re-exec is
  needed.
- **macOS**: creates the directories and returns. No NVIDIA library is published for it and the
  CoreML provider ships inside the runtime archive.

**Go splits this in two and it costs it the CLI.** The re-exec is in `cmd/gui/main.go` and the `PATH`
/ `LD_LIBRARY_PATH` append is inside `utils.InitializeNvidiaLib`, which only the GUI calls - so the
Go CLI has neither, and CUDA is unusable from it. One function, in `opai`, called by both front ends,
is the project's layering rule applied to the thing that most obviously broke it.

**Why `unsafe`, and why not inside `initialize()`.** Rust 2024 makes `std::env::set_var` unsafe
because changing the environment races with any other thread reading it, and `initialize()` is async
with a Tokio runtime already up. Doing the `PATH` append there would be exactly the race the rule
exists for. At the top of `main` there is no other thread, which makes it sound - and it is where the
Linux re-exec has to be anyway, since descriptors survive `execvp` and a re-exec after the logging
setup has redirected stderr into a pipe would leave the new image writing into a pipe whose reader
died with the old process (the hazard `cmd/gui/main.go` documents). One place, one precondition,
stated in the signature rather than in prose a caller can skip.

The cost is that a front end which forgets the call gets CPU-only inference on Windows as well as
Linux, where Go's arrangement at least covered Windows from inside the install. It is one call, the
front-end change adds it, and the alternative is a footgun with no signature to warn about it.

Windows' `SetDllDirectory` / `AddDllDirectory` were considered as a thread-safe way to avoid touching
`PATH` at all: `SetDllDirectory` holds only one directory and we need four, and `AddDllDirectory`
only affects loads that pass `LOAD_LIBRARY_SEARCH_*` flags unless `SetDefaultDllDirectories` changes
the default search semantics process-wide - which would alter how every other `LoadLibrary` in the
process resolves, including Tauri's and WebView2's. Not worth it to avoid an assignment made before
any thread exists, and it would add a `windows-sys` dependency.

### D7. The re-exec is split so everything except the `execvp` is a unit test

```rust
fn library_dirs(app_dir: &Path, name: &str) -> Result<Vec<PathBuf>, InitError>;  // creates, ordered
fn search_path(dirs: &[PathBuf], inherited: Option<&OsStr>) -> OsString;          // pure
```

`search_path` is where every rule in the spec lives - ours first, inherited preserved and appended,
duplicates dropped keeping the first occurrence, empty entries dropped - and it is a pure function of
two arguments, so all of it is covered without an exec, an environment or a GPU.

What is left is `Command::new(current_exe()?).args(args_os().skip(1)).env(...).exec()`, which on
success never returns. It is covered on Linux by one integration test that re-runs the test binary
itself with a marker variable set, and asserts the child comes up with the composed
`LD_LIBRARY_PATH` and does not re-exec again. `exec` returning at all is an error, and per the spec
it is reported and startup continues rather than aborting - a machine that cannot exec its own binary
has worse problems than an unfindable CUDA.

`OPAI_REEXEC` rather than Go's `APP_REEXEC`: go-sak's `os.ReExec` owned that name, nothing here does,
and an application-specific variable cannot collide with another program's re-exec guard in an
inherited environment.

### D8. Four sequential installs, in dependency order, each with its own reporter

`initialize()` builds the list - runtime always, then CUDA, cuDNN and TensorRT as the hardware and
the published platforms allow - and runs them in that order through the existing
`deps::install::install`, one `Reporter` each.

cuDNN after CUDA because cuDNN's libraries link against the CUDA runtime, and TensorRT last because
it is the largest by a wide margin (1.4-2 GB against 0.4-0.7 GB): a user watching a first launch gets
the two that unlock CUDA before the one that unlocks TensorRT, and an interrupted first launch has
more of the useful part already on disk.

Sequential rather than concurrent, as in Go. Three concurrent multi-hundred-megabyte transfers
compete for the same link and the same disk, turn one honest progress bar into three, and make the
"which dependency is being installed" field of a report ambiguous for the whole of a first launch.

### D9. Module layout

```
opai/src/
  gpu.rs            the adapter probe (OnceLock) and the NVIDIA / RTX classifiers
  providers.rs      SupportedProviders, incl. the cfg'd CoreML check
  libpath.rs        library_dirs, search_path, prepare_library_path
  deps/artifact.rs  + CUDA, CUDNN, TENSORRT; `dir` and `progress` on Release
  progress.rs       + Dependency::{Cuda, Cudnn, TensorRt} and their display names
  lib.rs            the selection, the sequential installs, Opai::providers
  error.rs          unchanged
```

Tests beside the code they cover, as in slice 1, `rust-sak` and the Go original. `InitError` needs no
new variant: a GPU library that fails to install fails with the same `Download`/`HashMismatch`/`Fs`
variants the runtime does, all of which already name the URL, and selection never produces an error
at all.

## Risks / Trade-offs

- **No CI runner has an NVIDIA GPU, so the install path for these three dependencies is never
  exercised end to end against real hardware.** -> The classifiers are pure and fully covered against
  constructed adapters; the install itself is slice 1's pipeline, already covered end to end against
  the local server and the committed fixture, and the suite drives the four-dependency selection
  through that same server with a forced selection rather than a probe. What stays a manual check, as
  `internal/deps/live_manual_test.go` is in Go, is a real 2.8 GB install on a real RTX machine.
- **The dependency archives are enormous** - CUDA 587 MB, cuDNN 407 MB, TensorRT up to 2 GB, so a
  first launch on an RTX machine is ~2.8 GB before the app does anything. -> This is what the Go app
  does today and what the hardware gating exists to bound. Slice 1's resume, fingerprint and
  skip-when-intact all apply unchanged, so it is paid once and an interruption is not paid twice.
- **Extending the user's `PATH` in-process on Windows is a global mutation of the process
  environment.** -> Confined to `prepare_library_path`, whose `unsafe` contract is that it runs
  before any thread exists; the variable is extended rather than replaced, ours first, inherited
  preserved.
- **`prepare_library_path` has no caller in this slice**, so the re-exec is proved by tests rather
  than by use. -> Deliberate: the front ends are wired in their own change, and shipping the function
  with the slice that needs it keeps that change to plumbing. The integration test does run the real
  `execvp` on Linux.
- **A GPU install failure fails the whole launch**, so a flaky transfer of the largest archive in the
  project blocks an app that would otherwise run on the CPU. -> Go parity, chosen deliberately;
  slice 1's resume means the retry starts from where it stopped, and the proposal records the
  alternative for the change that wires the front ends, where a degraded launch can actually be shown
  to a user.
- **`Dependency` and `SupportedProviders` are types a later slice extends.** -> Both
  `#[non_exhaustive]`, so slice 3 adding `ExecutionProvider` and any front end matching on a
  dependency name stay source-compatible.

**Validation.** Developed and validated on macOS/aarch64 - which exercises the CoreML branch, the
"no NVIDIA adapter" branch and the no-op search path, and nothing else. Linux/x86_64 and
Windows/x86_64 through the three-OS CI matrix, including the Linux re-exec integration test; neither
has a GPU, so on both the detection is the "no NVIDIA adapter" branch and only the classifier unit
tests cover the rest. Linux/aarch64 and Windows/aarch64 are in the pinned table and untested - no
runner covers them, and `windows_arm64` is the platform D3's non-offer path exists for. No execution
provider is built or loaded anywhere in this slice; nothing loads the runtime until slice 3.

## Migration Plan

No `rust-sak` change, so this is a single-repository slice: enable the `sysinfo` feature, add the
macOS-only `objc2-foundation` dependency, and build. Nothing upstream has to land first.

Rollback is a revert. Nothing here writes user data or a format anyone else reads; the three
`libs/*` directories a reverted build left behind are inert - nothing loads them without the search
path this slice establishes - and the next build that wants them finds their manifests matching the
pin and installs nothing.

## Open Questions

- Whether TensorRT should also be offered on the capable NVIDIA hardware that carries no RTX name -
  the GTX 16-series, the data-centre parts, the Jetson modules (D2). Each needs one of those machines
  in hand to confirm what `gpu_info` reports for it before a rule can be written that does not
  over-match, and being wrong costs a 2 GB download that can never build a session. The RTX rule is
  correct for everything it does match, so this only ever adds.
- Whether a GPU library should be *removed* when its hardware disappears from a machine. The
  manifests make it a two-line uninstall; nothing asks for it today, and it is meaningless until
  something can be uninstalled at all.
