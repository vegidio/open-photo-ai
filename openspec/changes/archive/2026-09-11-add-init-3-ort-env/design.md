## Slices

`opai::initialize()` - download and install the pinned ONNX Runtime, then the NVIDIA libraries on a
machine that has an NVIDIA GPU, reporting progress throughout, and start the inference environment
against what was installed - estimates well past the 15-task ceiling as one change. It is split into
four slices. Each ends with the workspace building, testing and passing CI on its own.

| # | Slice | Delivers | Depends on | Status |
|---|-------|----------|------------|--------|
| 0 | `add-init-0-rust-sak` | `rust-sak`: `.part` + identity sidecar, atomic rename and `If-Range` in `fetch::download`; `resume_key`; progress hook and header totals in `fs::extract`. CalVer alignment and the `26.9.0` tag. Here: `opai`'s first dependency, pinned to that tag. | nothing | **done** (archived `2026-09-11`) |
| 1 | `add-init-1-runtime` | `rust-sak`: an opt-in content digest computed during a transfer, released as `26.9.1`. Here: the config directory and app name, the pinned artifact table (tag + SHA-256 + size + library name per platform), `Progress`/`Dependency`/`Phase`, manifest bookkeeping (`fingerprint` -> `intact` -> `remove`), the download/verify/extract/record pipeline, and `Opai::initialize(name, on_progress)` installing the ONNX Runtime into `<name>/runtime/`. CI widens to a three-OS matrix. | slice 0 | **done** (archived `2026-09-11`) |
| 2 | `add-init-2-gpu` | `opai`: NVIDIA detection via `sysinfo::gpu_info`, CUDA + cuDNN into `<name>/libs/{cuda,cudnn}/` and TensorRT into `<name>/libs/tensorrt/` when supported, loader search-path handling including the Linux `LD_LIBRARY_PATH` re-exec, and the supported-execution-provider report. | slice 1 | **done** (archived `2026-09-11`) |
| 3 | `add-init-3-ort-env` | `opai`: the `ort` crate at `load-dynamic`/`api-26`, the runtime library path carried onto the install descriptor, the ONNX environment started eagerly against the downloaded library with telemetry off and its log level at warning, and the process-lifetime / repeat-initialize semantics that replace Go's `Destroy`. | slice 2 | **current** |

This is the last slice of the series. Nothing after it belongs to `initialize()` itself; what is
recorded so it is not lost is, in the order it is likely to be picked up: wiring the front ends to
`initialize`, `prepare_library_path` and the provider report; the model download path, which reuses
slice 1's pipeline with several sources per dependency; and the session/execution-provider work,
which brings `ExecutionProvider`, the CPU fallback, and the engine-cache invalidation Go performs
when the pinned runtime tag changes. Each is its own change.

## Context

See proposal.md - Why.

Three facts about the installed state and about `ort` shape everything below. They were read out of
`ort` `2.0.0-rc.13`'s own source (`src/lib.rs`, `src/environment.rs`, `src/session/mod.rs`,
`ort-sys/src/version.rs`), not inferred from its documentation, because two of them are stated
nowhere else.

**The pinned runtime is 1.26.0 and `ort` targets 1.28.** `ort`'s `load-dynamic` path calls
`GetVersionString()` on the library it opened and **refuses one whose minor version is lower** than
`ort_sys::ORT_API_VERSION`. That constant is not fixed per release: it is `17` plus one for each
enabled `api-NN` feature, and the features are additive. So the version `ort` demands is chosen by
the feature set, and `api-26` makes 1.26.0 an exact match rather than a rejection. The default
feature set includes `api-27`, which would refuse the pinned runtime outright with
`LoadError::BadVersion`.

**The environment cannot be destroyed and rebuilt.** `ort`'s own comment in `src/environment.rs`
records why its global went from a `Weak` to a strong `Arc`: "apparently you can only ever call
`CreateEnv` once throughout the lifetime of the process, *even if* the last env was `ReleaseEnv`'d.
So once all `Session`s fell out of scope, if you ever tried to create another one, you'd crash."
`ort` therefore holds the environment in a process-global `Mutex<Option<Arc<Environment>>>` and
releases it from a `.fini_array` / `.CRT$XLB` / `__cxa_atexit` destructor at process exit. This is
the ONNX Runtime C API's limitation, not `ort`'s policy, so no other binding could do better.

**`commit()` is lazy and `Session` owns a strong reference.** `EnvironmentBuilder::commit()` only
stores the configuration in a `OnceLock`, returning `false` if one is already stored; the
`CreateEnv` happens at the first `Environment::current()`, which is what session construction calls.
`ort::session::Session` holds an `Arc<Environment>` field, so a session provably cannot outlive the
environment it was built under.

On this side, slice 1 left `Artifact::lib` pinned per platform - `onnxruntime.1.26.0.dylib`,
`onnxruntime.so.1.26.0`, `onnxruntime-1.26.0.dll` - with a comment saying nothing loads a library
until this slice. Slice 2 left `prepare_library_path`, which is what makes the runtime's own
`dlopen` of the CUDA, cuDNN and TensorRT libraries resolve; nothing here repeats that work.

The Go behavioural reference is `init.go`: `startRuntime` for the load, the level and the
already-initialized guard, `Initialize` for the ordering and the lifecycle re-arm, and `Destroy` for
the teardown this slice deliberately does not port.

## Goals / Non-Goals

**Goals:**

- One place that turns an install into a running environment, called from `initialize` and from
  nowhere else.
- A feature set for `ort` that is exactly what is needed, so nothing downloads a second runtime and
  nothing links against one at build time.
- Ownership that makes "a session outliving its environment" unrepresentable rather than avoided by
  discipline, since that is the sharpest edge the Go original carries into every later slice.
- Repeat initialization that succeeds without surprises, because the GUI's error boundary reaches
  it.
- `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` green on all three OSes, none
  of which has a runtime to load.

**Non-Goals:**

- Building a session, registering an execution provider, or the CPU fallback when one fails.
  `ort`'s `cuda`, `tensorrt` and `coreml` features stay off until the slice that needs them; the
  environment is committed with no default execution providers.
- Any public teardown API. See D5.
- Routing ONNX diagnostics anywhere. See D6.
- Honouring `ORT_LOG`. `ort` reads it for the level the environment is *created* with, and this
  slice overrides that immediately afterwards, so the variable has no effect. A debug override is
  the logging change's to design, alongside where the records go.
- Invalidating the execution-provider engine cache on a runtime version bump, which Go does in
  `cleanEngineCache` before installing. It is about caches of compiled provider engines, and nothing
  compiles one until sessions exist.
- Upgrading the pinned runtime to 1.28 so `ort`'s defaults apply. See D1.

## Decisions

### D1. `ort = "=2.0.0-rc.13"`, `default-features = false`, features `load-dynamic` and `api-26`

```toml
ort = { version = "=2.0.0-rc.13", default-features = false, features = ["load-dynamic", "api-26"] }
```

Four separate choices, each of which changes what compiles or what loads:

**`load-dynamic`** is the whole point: it turns `ort-sys`'s linking off and adds
`ort::init_from(path)`, which `dlopen`s a library at a path decided at runtime. Without it `ort`
links against a runtime at build time, and the 2.8 GB the last three slices exist to download would
be beside the point. It implies `std` and `preload-dylibs` (`libloading`).

**`api-26`** is what makes the pinned 1.26.0 acceptable, per Context. It is not a
lowest-common-denominator hedge: it is the exact version pinned in `deps/artifact.rs`, and the two
have to be bumped together. A comment in the manifest says so, because the failure if they drift is
a startup error on every machine, phrased in `ort`'s words rather than ours.

**`default-features = false`** removes `download-binaries` - which would run a build script that
fetches its own ONNX Runtime, a second copy of a 175 MB artifact - along with `copy-dylibs`,
`tls-native` (and with it `ureq` and a TLS stack, where the project already has `reqwest` through
`rust-sak`), `ndarray`, `tracing` (D6) and the default `api-27`.

**The `=` pin** is because `2.0.0-rc.13` is a pre-release, and Cargo's default caret requirement
would accept `2.0.0-rc.14`. The three behaviours this design is built on - the minor-version refusal,
the once-per-process `CreateEnv`, the lazy `commit` - are unstated in the crate's documentation and
have already changed shape between release candidates (`rc.13` and the current development tree
disagree about all three). An exact pin makes an `ort` bump a deliberate act with this design
re-read against it, which is what a pre-release dependency at the centre of the application
deserves.

**Alternative: bump the pinned runtime to 1.28 and take `ort`'s defaults.** It would keep the
project on the version `ort` is developed against, and the compatibility note it emits for a
*newer*-than-expected runtime would go away. Rejected for this slice: the runtime archives are
published as GitHub release assets on `vegidio/open-photo-ai` with pinned hashes and sizes for five
platforms, so a bump is a publishing job plus a re-pin of the table, and it is orthogonal to
starting an environment. `api-26` costs one feature flag and nothing else. When the runtime is next
bumped, the feature moves with it.

**Alternative: the `ORT_DYLIB_PATH` environment variable** instead of `init_from`. It is `ort`'s
other way to point `load-dynamic` at a library, and it would need `std::env::set_var` - `unsafe` in
Rust 2024, from inside an async `initialize` with a Tokio runtime already up, which is exactly the
race slice 2's D6 moved `prepare_library_path` to the top of `main` to avoid. `init_from` takes the
path as an argument. Not close.

### D2. The library name rides the descriptor, like `dir` and `progress` before it

```rust
pub(crate) struct Dependency {
    name: &'static str,
    version: String,
    dir: &'static str,
    progress: Which,
    lib: &'static str,    // NEW: "onnxruntime.1.26.0.dylib"; empty for a dependency with none
    sources: Vec<Source>,
}
```

`Artifact::lib` has been pinned per platform since slice 1 and read by nobody. Carrying it onto the
descriptor in `from_release_at` is the same move slice 2's D1 made for `dir` and `progress`, for the
same reason: the alternative is a second resolution of the platform at the call site - `pinned_for`
again, or a `match` on `std::env::consts` - that can disagree with the one the install actually
used. After the install, `initialize` has the descriptor it installed from and the directory it
installed into, and the library path is the join of the two. Nothing else can name it.

Empty for the three NVIDIA libraries, which is already what the table says and what slice 2's tests
assert. `initialize` only ever asks the runtime descriptor - `plan[0]`, which slice 2's `select_at`
guarantees is the runtime - so the empty case is not reachable, and the code treats an empty `lib`
on the runtime row as the `UnsupportedPlatform` error it already is rather than joining an empty
path onto a directory. Go has the same guard (`!found || pinned.Lib == ""`).

### D3. The environment is started eagerly, in one function, at the end of `initialize`

```rust
// opai/src/runtime.rs
pub(crate) fn start(name: &str, library: &Path) -> Result<(), InitError>;
```

```rust
ort::init_from(library)?          // dlopen + version check, first call wins for the process
    .with_name(name)              // the environment's identity in a diagnostic
    .with_telemetry(false)        // proposal.md - divergence
    .commit();                    // stores the config; returns false if one is already stored
let env = Environment::current()?; // forces CreateEnv here rather than at the first session
env.set_log_level(LogLevel::Warning);
```

**Eager, because `commit()` is lazy.** On its own, `commit()` only records the configuration; the
`CreateEnv` would happen inside the first session construction, some slices from now, in the middle
of a user's first enhancement. Calling `Environment::current()` here moves that failure to
initialization, which is what the spec requires and what Go does - `InitializeEnvironment` creates
the environment on the spot. It is also the only way to reach `set_log_level`, which is a method on
the environment rather than on the builder.

**The return value of `commit()` is deliberately ignored.** `false` means an environment was already
configured in this process, which on a second `initialize` is the expected case, not a failure - see
D5. Nothing else can be learned from it: the configuration that lost is identical to the one that
won, because the only inputs are the application name and two constants.

**Ordering inside `initialize`:** after every install, before the handle is constructed. Go is the
same shape (`deps.Install` ... `startRuntime`), and it is the only order that works - the library
has to exist before it can be loaded, and the environment has to be running before `Opai` can hold
it. The provider report is decided before the installs and is unaffected; nothing in this slice
touches it.

**On `spawn_blocking`.** `initialize` is `async`, and `dlopen` of a ~175 MB shared library plus
`CreateEnv` is a blocking, syscall-heavy operation that can run into tens or hundreds of
milliseconds on a cold page cache. Slice 1 already established the precedent by putting the archive
expansion on `spawn_blocking`, and `Environment` is `Send + Sync`, so this goes the same way. In a
Tauri process the async runtime is shared with window and IPC plumbing, so "nothing else is running
during initialize" is not true there.

### D4. The lifetime model is `ort::Session`'s `Arc<Environment>`, and `Opai` stores nothing new

This is the decision the conventions in the project context are about - the concurrency edge where
Go can free an ONNX session while another thread is running inference on it, which is a native
use-after-free rather than a panic. The shape that removes it is already inside `ort`:
`ort::session::Session` holds an `Arc<Environment>` field. A session keeps its environment alive by
construction, with no lock, no drain and no ordering rule for anyone to get wrong. The model
registry slices inherit that: a lease can hand out something holding a session, and the environment
underneath it cannot go away while the lease exists.

So `Inner` gains no field. `runtime::start` holds the handle exactly long enough to set the log
level and lets it drop; `ort`'s process-global holds a strong reference until process exit, so that
drop releases nothing and the environment the next session finds is the one this slice configured.

**Alternative: `Inner { env: Arc<Environment>, .. }`**, to express that an initialized application
owns what it was initialized with. Rejected on a concrete cost rather than on taste. `Inner` is
built by `Opai::install`, the private seam that every slice-1 and slice-2 test drives a **real**
install through - against a local server and a committed fixture archive that is emphatically not an
ONNX Runtime. An `Environment` cannot be constructed without a real ~175 MB runtime, and `ort` keeps
its own test environment helper `pub(crate)`, so the field would have to be an
`Option<Arc<Environment>>` whose `None` case exists only to let those tests compile - and every
later slice that builds a session would carry the unwrap. A field that adds no safety is not worth
an `Option` in the state every future slice reads.

What that field would have bought - "you cannot reach a session without having gone through
`start`" - is bought instead by session construction taking `&self` on `Opai` in the slice that adds
it: outside this crate's own tests, `initialize` is the only thing that hands one out, and
`initialize` always starts the environment.

### D5. No teardown API, and the repeat-initialize path is where that is proved

Go's `Destroy()` does three things: closes the image cache, drains the model registry, and calls
`ort.DestroyEnvironment()` - skipping the last one, with an error logged, if the drain times out,
because tearing the environment down under a live session is the use-after-free above. Of the three,
two have nothing to act on yet and the third is impossible: per Context, the ONNX Runtime C API
supports one `CreateEnv` per process even after a `ReleaseEnv`, and `ort` enforces that by keeping
the environment alive until the process exits.

So the honest slice-3 answer is that **nothing is added**. A public `shutdown()` that does nothing to
the environment would be a signature making a promise the runtime cannot keep, and the drain it
would exist to make safe is unnecessary once the environment cannot be destroyed. The cache and the
registry get their teardown with the slices that add them, and `Opai` is `Arc`-backed, so a `Drop`
on `Inner` is where that will go without a public API change.

What is left of the lifecycle is the **re-initialize** half, which is real and reachable - the GUI's
error boundary reloads the frontend, which calls `initialize` again inside a process that is already
initialized. Three mechanisms carry it, none of them ours:

- `ort::init_from` inserts into a `OnceLock`, so the *first* library a process loads is the one it
  uses. A second `initialize` under a different application name installs into that name's
  directories and keeps the first library. Go behaves identically, for the identical reason, and
  documents it nowhere; the spec states it. This holds only when the first load **succeeds**; what
  happens when it fails is a defect in `ort`, and D9 is what this slice does about it.
- `commit()` returns `false` and changes nothing.
- `Environment::current()` returns the live environment.

The result is that a repeat `initialize` succeeds and leaves everything usable, which is Go's
observable behaviour without Go's re-arm flag. Go needed `destroyed = false` and `Registry.Reopen()`
because its `Destroy` latched state closed; with no `Destroy`, there is no latch to re-arm.

### D9. A failed load latches the process, because `ort`'s does not

This was found while writing the tests for D7 and it invalidates half of D5, so it is recorded as
its own decision rather than as a note on either.

`ort::load_dynamic::init` loads through `G_ORT_LIB.get_or_try_init(..)`, and `ort`'s `OnceLock`
implements that with `std::sync::Once::call_once_force`
(`src/util/once_lock_std.rs`). `call_once_force` marks the `Once` **completed** whenever its
closure returns without panicking — a returned `Err` included. Nothing is written into the slot, but
`OnceLock::get` consults `is_completed()` and hands back `&*(*self.data.get()).as_ptr()`: a
reference to uninitialized memory. Measured, three calls in one process:

```text
FIRST : Err("failed to load from `/no/such/lib-a.so`: dlopen failed")
SECOND: Ok(builder)          // a different path that also does not exist
THIRD : Ok(builder)          // /etc/hosts, not a library at all
```

So after one failed load, **every** later `ort::init_from` in that process returns `Ok` having
loaded nothing, whatever path it is given. Using what it hands back is undefined behaviour; in
practice it surfaced as a panic from inside `ort` — ``` `OrtGetApiBase` must be present in ONNX
Runtime dylib: DlSym { source: "dlsym(0x0, OrtGetApiBase): invalid handle" } ```.

**Why this is a product decision and not a test problem.** D5 is written for the GUI error
boundary, which reloads the frontend and calls `initialize` a second time in a process that is
already initialized. On the path D5 describes — a first call that succeeded — `ort`'s own `OnceLock`
carries it. On a first call whose runtime *failed to load*, which is the case the user is most
likely to retry from, the second `initialize` would return `Ok` having loaded nothing and the first
session built under it would panic. The spec's "SHALL NOT fail because one is already running" was
written for the success case only.

**What is added:** one process-global in `runtime.rs` recording the first failed load, and a third
error variant reporting it.

```rust
/// The first load failure in this process, if there has been one.
static FAILED_LOAD: Mutex<Option<Failure>> = Mutex::new(None);
struct Failure { path: PathBuf, reason: String }
```

`start` takes that lock for its whole body. A process with a recorded failure never reaches `ort`
again and returns `InitError::RuntimeUnavailable`; otherwise it calls `ort::init_from` and, on an
error, records the attempt before returning `RuntimeLoad`. The lock is held across the load rather
than only around the check because the same defect is reachable concurrently: if two threads call
`init_from` at once and the one that runs the closure fails, the other's `get_or_try_init` sees a
completed `Once` and returns the same uninitialized handle.

`Failure` keeps the original error's `Display` rather than the error, because `LoadDynamicError` is
neither `Clone` nor reconstructible without inventing a variant that did not happen. That is a
deliberate exception to D7's "do not flatten a source to a `String`": the original error is gone,
and a recorded message is the honest thing to report in its place.

**Alternative: fix it upstream and pin a patched `ort`.** It is a real bug with a one-line fix, and
`ort` is third-party rather than first-party like `rust-sak`, so it means a PR to `pykeio/ort` and
waiting, or a fork of a crate that sits at the centre of the application. Rejected as a blocker for
this slice; the guard here is small, needs no pin change, and stays correct if the bug is fixed —
`start` would simply stop being the thing that catches it.

**Alternative: document it and treat `RuntimeLoad` as fatal in the front ends.** Cheapest, and
rejected: it leaves reachable undefined behaviour behind a retry that the GUI already performs, and
it puts the guarantee in two front ends rather than in the one place both of them go through.

**What it costs the tests.** `ort`'s load global is process-wide, so a process gets exactly one
first load attempt, and Cargo runs every unit test of a crate in one binary on concurrent threads.
No unit test may touch `ort::init_from` at all — which of the two failure paths ran "first" would be
whichever thread won. Each one therefore owns a process: the tests re-run the unit-test binary
against a single `--exact` test name under a marker variable, which is the arrangement
`tests/reexec_linux.rs` already uses for the `execvp` it cannot reach in-process, and it keeps the
crate-internal access a unit test has. `library_path`, the latch's decision and every `Display` that
does not need a real `ort::Error` stay ordinary unit tests.

### D6. `tracing` stays off; the level is set, not the destination

`ort`'s `tracing` feature replaces ONNX Runtime's C++ logger with a callback that forwards every
record into the `tracing` crate. There is no `tracing` subscriber anywhere in this workspace - no
`gui`, `cli` or `opai` code initializes one - so enabling it would send every ONNX diagnostic into a
callback that drops it. The runtime's own logger writes to the process's stderr, which is where Go's
records go too before `shared.SetupLogging` redirects that stream into `opai.log`.

So: feature off, level set to warning through `Environment::set_log_level`, exactly Go's
`SetEnvironmentLogLevel(ort.LoggingLevelWarning)` and for the reason Go's comment gives - warning is
where the node-assignment and provider-fallback diagnostics live, without the per-node flood that
info and verbose produce. Those two messages are what will explain "why is this running on the CPU"
in a later slice, so the level is not arbitrary.

When logging lands as its own change, it decides three things together: a subscriber, whether
stderr is redirected to a file as in Go, and whether `tracing` goes on so ONNX records join the same
stream. Deciding one of them here in isolation would be guessing at the other two.

### D7. Two error variants, each naming the library path

```rust
#[error("failed to load the ONNX Runtime at {path}: {source}")]
RuntimeLoad { path: PathBuf, #[source] source: ort::LoadDynamicError },

#[error("failed to start the ONNX Runtime environment for {path}: {source}")]
RuntimeStart { path: PathBuf, #[source] source: ort::Error },
```

`InitError` is `#[non_exhaustive]`, so this is additive. Two variants rather than one because they
fail differently and a front end will say different things: `RuntimeLoad` means the file is missing,
unreadable, exports no `OrtGetApiBase`, or is a runtime older than `api-26` - `ort`'s `BadVersion`
carries the version string it found, which is what the spec's third scenario asserts on - and it is
recoverable by reinstalling. `RuntimeStart` means the library loaded and the runtime refused to
create an environment, which is not.

Neither error carries the path today: `LoadDynamicError` names it in its `Display` but not as a
field, and `ort::Error` does not know it at all. Adding it as a field on both matches what slice 1
did for every download error, where the URL is on the variant rather than on `rust-sak`'s error.

This makes `ort` a public dependency of `opai`. The alternative - flattening both sources to a
`String` - would hide that, at the cost of a caller's `source()` chain. `opai` is a workspace
library consumed by `gui` and `cli` in the same repository and is not published, so the exposure
costs nothing that the exact version pin does not already control.

### D8. Module layout and what is actually tested

```
opai/src/
  runtime.rs        NEW: library_path(app_dir, descriptor), start(name, library), the D9 latch
  deps/release.rs   + `lib` on the descriptor, carried from the pinned row
  deps/artifact.rs  unchanged - `lib` has been pinned since slice 1
  error.rs          + RuntimeLoad, RuntimeStart, RuntimeUnavailable (D9)
  lib.rs            the call into `runtime::start` at the end of `initialize`; `Inner` unchanged
  gpu.rs, libpath.rs, providers.rs, progress.rs, config.rs   unchanged
```

The split is the same one slice 2 used for the re-exec, and for the same reason: everything that can
be tested without the thing no runner has is a function of its arguments.

- `library_path(app_dir, descriptor)` is pure - the join, and the empty-`lib` rejection. Covered for
  all five published platforms against constructed descriptors, on any runner.
- `start` cannot be covered without a real ONNX Runtime, so its failure paths are covered where they
  can be: loading a file that is not a shared library at all, and loading a path that does not
  exist, both produce `RuntimeLoad` naming the path, and both are reachable from a `tempfile` on
  every runner. What no runner can reach is the success path and `BadVersion`.
- Per D9, each of those two owns a **process**: the unit-test binary re-runs itself against one
  `--exact` test name under a marker variable, because `ort`'s load global is process-wide and a
  process gets one first load attempt. The third case a process can be spent on is the latch itself -
  a failed load followed by a second `start` against a different path, which must answer
  `RuntimeUnavailable` rather than the `Ok` that `ort` alone would give.
- `RuntimeStart`'s `Display` is **not** unit-tested, and cannot be: every `ort::Error` constructor
  (`new`, `new_with_code`, `wrap`) goes through `ortsys![CreateStatus(..)]`, so constructing one
  needs a runtime already loaded. It is asserted in the live test instead. `RuntimeLoad`'s and
  `RuntimeUnavailable`'s are ordinary unit tests: `ort::LoadDynamicError`'s variants are public with
  public fields, so `MissingApi { path }` builds without touching the load global, and
  `RuntimeUnavailable` carries only a `String`.
- One `#[ignore]`d integration test does the real thing: install the pinned runtime into a temporary
  directory, start the environment against it, assert it comes up and that a second `start` in the
  same process succeeds and does not replace it. Run by hand with `cargo test -- --ignored`, which
  is the arrangement Go uses for `internal/deps/live_manual_test.go`, and for the same reason - it
  is a ~175 MB download per run.

The repeat-initialize semantics of D5 are partly reachable without a runtime: a second `init_from`
against a *different* non-library path still returns the first call's outcome, which proves the
`OnceLock` behaviour the spec's "different application name" scenario rests on, without a runtime
ever loading. That test has to own the process, since the global is process-wide - it goes in its
own integration test file rather than in the unit suite, as slice 2's re-exec test did.

## Risks / Trade-offs

- **The core of this slice - a runtime that actually loads and an environment that actually starts -
  is green on no CI runner.** -> Accepted deliberately (the alternative was a three-OS CI job
  downloading 175 MB per runner). The pure parts and both reachable failure paths are covered
  everywhere; the success path is the `#[ignore]`d live test plus a manual run on macOS/aarch64
  before the change is called done. The exposure is real and it is what D1's exact `ort` pin
  mitigates: a version bump cannot silently change the load behaviour, because it cannot happen
  without someone editing the manifest.
- **`api-26` and the pinned runtime tag are two places that have to say the same thing, in two
  different files.** -> A comment in `opai/Cargo.toml` naming `deps/artifact.rs` and vice versa, and
  a failure mode that is loud rather than subtle: every launch fails at `init_from` with `ort`'s own
  message naming both versions. A test cannot catch it, since the feature is not readable from code.
- **`ort` mistakes a failed load for a completed one, and hands out a reference to uninitialized
  memory afterwards (D9).** -> Guarded here, in the one function both front ends reach the runtime
  through, and covered by a test that owns its own process. The residual exposure is that the guard
  is `opai`'s rather than `ort`'s: anything in a later slice that calls `ort::init_from` directly
  walks around it. `runtime::start` being the only caller is what the Goals section asks for
  anyway, and the module's own documentation says why it matters. Not reported upstream as part of
  this slice; worth doing, and its own piece of work.
- **`ort` is a release candidate at the centre of the application, and three behaviours this design
  depends on are undocumented and already differ in its development tree.** -> The exact pin, and
  this document naming the three so a bump is read against them rather than merged. D9 is a fourth,
  found by writing the tests rather than by reading the source, which is the argument for the pin
  restated. There is no
  alternative binding: `ort` is what the project context selected, and the Go original's
  `yalue/onnxruntime_go` has no Rust equivalent with anything like its maintenance.
- **The environment can never be restarted, so a process that loses its runtime - a user deleting
  the config directory while the app runs - cannot recover without restarting.** -> The ONNX Runtime
  C API's limitation, not a choice; Go has the same exposure through its timed-out-drain path, where
  it skips the teardown and carries on with an environment it meant to destroy. Nothing in the
  product asks to recover from this.
- **Telemetry off is a divergence from what the Go app actually ships.** -> Stated in proposal.md and
  specified, so it is a decision rather than drift. It cannot break anything: telemetry produces no
  application-visible behaviour.
- **`ORT_LOG` silently stops working.** -> `ort` reads it only for the creation level, which this
  slice overwrites one statement later. Nobody depends on it yet - this is `ort`'s first appearance
  in the project - and a debug override belongs with the logging change, where it can be one
  variable for the whole application rather than one for its ONNX component.

**Validation.** The live test of task 4.2 was run on **macOS/aarch64** on **2026-09-11**, and passed:
it installed `runtime/1.26.0`'s `onnx_darwin_arm64.7z` into a temporary directory, loaded
`onnxruntime.1.26.0.dylib` from it, created the environment, and confirmed that a second
`runtime::start` in the same process succeeded and left the first environment in place - asserted by
pointer identity on the `Arc<Environment>`, not merely by both calls returning `Ok`. The runtime
reported `ORT Build Info: git-branch=HEAD, git-commit-id=8c546c37b4, fp8-kv-cache=1, build type=Release`,
and `ort`'s version check passed silently, which is itself the evidence that the loaded library's
minor version matches the `api-26` feature - a 1.25 or lower would have been refused with
`BadVersion` and a 1.27 or higher would have logged a compatibility note. The darwin archive is
6.4 MB rather than the ~175 MB the linux/windows amd64 ones are, so the run cost seconds on this
platform and will cost minutes on those.

So the real load, the real `CreateEnv` and `onnxruntime.1.26.0.dylib` are exercised on exactly one
platform. Linux/x86_64 and Windows/x86_64 through the three-OS CI matrix,
which compiles `ort` with this feature set, runs the pure and failure-path tests, and never loads a
runtime. Linux/aarch64 and Windows/aarch64 are pinned and untested, as in slice 2. No execution
provider is registered and no session is built anywhere in this slice, so CUDA, TensorRT and CoreML
remain entirely unexercised - the environment is committed with no execution providers at all.

## Migration Plan

Single-repository, single-crate: add `ort` to `opai/Cargo.toml` with the feature set in D1, and
build. No `rust-sak` change, nothing upstream to land first, and no published artifact to re-publish
- the pinned runtime is the one already in the table.

The first build is the slow one: `ort-sys` compiles with linking disabled and downloads nothing, but
`Cargo.lock` gains `ort`, `ort-sys` and `libloading`, and CI's `--locked` requires it committed.

Rollback is a revert. Nothing here writes to disk, changes a format, or leaves state behind: the
environment lives in process memory and dies with the process, and the installed runtime a reverted
build leaves in place is inert exactly as it was before this slice.
