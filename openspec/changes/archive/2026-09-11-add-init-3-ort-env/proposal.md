## Why

Slices 1 and 2 put the ONNX Runtime and the NVIDIA libraries on disk and on the loader's search
path, and nothing in the project has ever opened any of them. `opai` today is an installer: it
downloads several hundred megabytes of inference runtime, verifies it, records it, and then returns
a handle that cannot run a single graph, because no line of code has loaded `onnxruntime` or created
the environment every session is built under.

This slice is where the installed runtime becomes a loaded one. It adds the `ort` crate, points it
at the library slice 1 downloaded rather than at one of its own, starts the ONNX environment against
it, sets the level its diagnostics are reported at, and settles the lifetime question that decides
everything after it: how long that environment lives, and what happens when initialization runs a
second time in the same process.

## What Changes

Task estimate: **12**. The full `opai::initialize()` feature is split into four slices (see
design.md - Slices); this is slice 3, the last of them. Nothing in `../rust-sak` changes.

**What `rust-sak` covers here: nothing.** Its seven modules are `crypto`, `fetch`, `fs`, `image`,
`memo`, `o11y` and `sysinfo`; none of them loads a shared library or wraps an inference runtime, and
none should - an ONNX binding is not general-purpose infrastructure. `o11y` is an OTLP log exporter
and is not what routes ONNX Runtime's own C++ diagnostics. The one new crate is `ort`, which the
project context already names as the chosen binding.

**In `opai`:**

- **The `ort` crate, at `=2.0.0-rc.13`, with `default-features = false` and exactly two features:
  `load-dynamic` and `api-26`.** `load-dynamic` is what lets the runtime be a file we downloaded at
  a path we choose instead of something linked at build time or fetched by a build script. `api-26`
  is not cosmetic: `ort` refuses to load a runtime older than the ONNX Runtime minor version its
  enabled `api-NN` feature names, and its default is `api-27`, which would reject the pinned
  1.26.0 outright. Turning the defaults off also turns off `download-binaries`, which would
  otherwise have a build script fetch a second copy of the runtime this project spent a slice
  installing.
- **The runtime library's path resolved from the pinned row.** `Artifact::lib` has carried the
  per-platform library name since slice 1 - `onnxruntime.1.26.0.dylib`, `onnxruntime.so.1.26.0`,
  `onnxruntime-1.26.0.dll` - and nothing has read it. It moves onto the install descriptor beside
  `dir` and `progress`, for the same reason those did: the row owns what the row knows.
- **The environment started inside `Opai::initialize`, after the installs and before it returns.**
  The library is loaded from `<config>/runtime/<lib>`, the environment is committed with the
  application's name and telemetry off, it is created eagerly rather than lazily so that a runtime
  that cannot start is a failed initialization rather than a failed first enhancement, and its log
  level is set to warning.
- **The lifetime question settled, in `ort`'s favour.** `ort::Session` holds an `Arc<Environment>`,
  so a session cannot outlive the environment it was built under - the native use-after-free that
  the Go registry's locking discipline exists to prevent is a refcount here, and the model registry
  slices inherit that for free. `Opai` therefore stores no new state.
- **Two new `InitError` variants**, one for a runtime library that will not load and one for an
  environment that will not start, both naming the library path.

**Deliberate divergence from the Go behaviour, and why:**

- **`Destroy()` has no counterpart in this slice, and the ONNX half of it never will.** Go tears the
  environment down at shutdown, after draining the model registry, and deliberately skips the
  teardown when the drain times out - because destroying an environment under a running session is a
  native use-after-free. `ort` forbids the operation outright: the ONNX Runtime C API only supports
  one `CreateEnv` per process even after a `ReleaseEnv`, so `ort` holds the environment until the
  process exits and releases it from a linker-section destructor. There is therefore nothing for a
  teardown to do to the environment, and the drain it exists to make safe is unnecessary. The rest
  of `Destroy()` - closing the image cache, releasing models - is real work that belongs to the
  slices that add a cache and a registry. No public teardown API is added here rather than an empty
  one that pretends otherwise.
- **A second `initialize()` in the same process does not restart the environment.** Go re-arms its
  lifecycle flag and guards on `ort.IsInitialized()`; the observable result is the same - the second
  call succeeds and the environment stays up - but the reason is now structural rather than a guard.
  A consequence worth stating plainly: a second `initialize()` with a *different* application name
  installs into a different directory and still uses the runtime library loaded by the first call.
  Go has exactly the same behaviour for exactly the same reason, and neither documents it; this one
  is specified.
- **Telemetry is switched off; Go leaves it at the runtime's default.** ONNX Runtime's
  Microsoft-provided builds send graph version, model producer, operator domains, execution provider
  names, error messages and inference counts by default, and Go never touches the setting - not as a
  decision, but because `InitializeEnvironment` exposes none. A privacy-respecting open-source image
  editor should not be quietly reporting what its user is editing, and `ort` makes it one call.
- **ONNX diagnostics keep ONNX's own logger.** Go redirects the process's stderr into `opai.log` and
  sets the ONNX level to warning; this project has no logging infrastructure yet, so the level is
  set identically and the records go where the runtime puts them. `ort`'s `tracing` feature stays
  off: with no subscriber anywhere in the workspace it would silently discard every ONNX
  diagnostic, which is worse than the terminal. Routing them is the logging change's job.

**Not in this slice**, recorded so it is not lost: building a session on any execution provider and
the CPU fallback when one fails to build; the `ExecutionProvider` type; the model registry, its
memory budgets and its leases; the image cache; the engine-cache invalidation Go performs when the
pinned runtime tag changes, which is about provider caches compiled against a runtime and belongs
with the slice that builds sessions; and wiring the front ends to `initialize`, `prepare_library_path`
and the provider report.

## Capabilities

### New Capabilities

None. This extends the behaviour of an existing capability.

### Modified Capabilities

- `initialization`: adds requirements for loading the installed ONNX Runtime and starting its
  environment as part of initialization, for the environment's process lifetime and the behaviour
  of a repeated initialization, and for the level ONNX Runtime's own diagnostics are reported at.

## Impact

- **Crates touched: `opai` only.** Nothing in `gui` or `cli` changes - they are wired to `opai` in
  their own change, and everything here is library behaviour both of them would need.
- **New dependency:** `ort` `=2.0.0-rc.13` (`default-features = false`, features `load-dynamic` and
  `api-26`), which brings `ort-sys` with linking disabled and `libloading`. No build script
  downloads anything.
- **Public API:** `Opai::initialize` gains a failure mode - a runtime that will not load or an
  environment that will not start - expressed as two new `InitError` variants on an already
  `#[non_exhaustive]` enum. No signature changes.
- **Internal:** `deps::release::Dependency` gains a `lib` field carried from the pinned row, and a
  new `runtime` module owns the library path and the environment start. `Opai`'s own state is
  unchanged.
- **Tests:** the environment start needs a real ~175 MB runtime, which no CI runner has. The pure
  parts are unit tested; the real load-and-start is one `#[ignore]`d integration test run by hand,
  the same arrangement as Go's `internal/deps/live_manual_test.go`.
