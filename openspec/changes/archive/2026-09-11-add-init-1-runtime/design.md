## Slices

`opai::initialize()` - download and install the pinned ONNX Runtime, then the NVIDIA libraries on a
machine that has an NVIDIA GPU, reporting progress throughout - estimates well past the 15-task
ceiling as one change. It is split into four slices. Each ends with the workspace building, testing
and passing CI on its own.

| # | Slice | Delivers | Depends on | Status |
|---|-------|----------|------------|--------|
| 0 | `add-init-0-rust-sak` | `rust-sak`: `.part` + identity sidecar, atomic rename and `If-Range` in `fetch::download`; `resume_key`; progress hook and header totals in `fs::extract`. CalVer alignment and the `26.9.0` tag. Here: `opai`'s first dependency, pinned to that tag. | nothing | **done** (archived `2026-09-11`) |
| 1 | `add-init-1-runtime` | `rust-sak`: an opt-in content digest computed during a transfer. Here: the config directory and app name, the pinned artifact table (tag + SHA-256 + size + library name per platform), `Progress`/`Dependency`/`Phase`, manifest bookkeeping (`fingerprint` -> `intact` -> `remove`), the download/verify/extract/record pipeline, and `Opai::initialize(name, on_progress)` installing the ONNX Runtime into `<name>/runtime/`. CI widens to a three-OS matrix. | slice 0 | **current** |
| 2 | `add-init-2-gpu` | `opai`: NVIDIA detection via `sysinfo::gpu_info`, CUDA + cuDNN into `<name>/libs/{cuda,cudnn}/` and TensorRT into `<name>/libs/tensorrt/` when supported, loader search-path handling including the Linux `LD_LIBRARY_PATH` re-exec, and the supported-execution-provider report. | slice 1 | pending |
| 3 | `add-init-3-ort-env` | `opai`: the `ort` crate, the ONNX environment started against the downloaded library, its log level, and the teardown/re-initialize lifecycle. | slice 2 | pending |

Only slice 1 is proposed. Each later slice is proposed when it is reached.

Two things the series does not cover, recorded so they are not lost: wiring the front ends to
`initialize()` and its progress (the GUI splash and the CLI bar), and the model download path, which
reuses this slice's pipeline with several sources per dependency. Both are their own changes after
slice 3.

## Context

See proposal.md - Why.

The workspace is three crates; `opai` exposes `version()` and depends on `rust-sak` 26.9.0 with the
`fetch` and `fs` features, unused by any source file. CI runs fmt, clippy and test on
`ubuntu-latest` only, with a comment saying the three-OS matrix "belongs to the first change that
introduces platform-specific code". This is that change.

What `rust-sak` 26.9.0 already provides, read rather than assumed:

- `fetch::Fetch::download_with_options` - retrying (Fibonacci backoff), resumable via `<path>.part`
  plus an identity sidecar, `connect_timeout`/`read_timeout` bounding both a dead host and a stalled
  one, progress coalesced to ~256 KiB or 100 ms, and `RequestOptions::resume_key` for an identity
  stronger than the URL. A file at the target path means complete, always.
- `fs::mk_user_config_dir(name, sub_path)` - the platform config directory, created, with `name` and
  `sub_path` validated by the same rules as archive entries, so `../../.ssh` is refused rather than
  resolved. The Go original had to write that check itself (`internal.ConfigDir`).
- `fs::extract` - hardened 7z/ZIP/TAR.XZ expansion with `ExtractOptions::on_progress` reporting
  entries and uncompressed bytes from inside the copy loop, and header totals where the format
  carries them (7z does).
- `fs::list_path` with `ListOptions::recursive(true)` - reading the extracted tree back.
- `crypto::sha256_bytes` / `sha256_string` / `sha256_file`.

The Go implementation is the behavioural reference: `init.go` for the entry point, and
`internal/deps/{install,manifest,download,dependency,release}.go` plus `internal/artifact*.go` for
the pipeline and the pinned table. Its retry loop, stall watchdog and resume bookkeeping are all
things `rust-sak` now owns, so roughly half of that package has no counterpart here.

## Goals / Non-Goals

**Goals:**

- One installed runtime per pinned identity, replaced rather than merged when the pin moves.
- Nothing on disk is trusted that is not recorded, and nothing is recorded before it is in place.
- Progress that never goes backwards, never reaches 1 early, and always ends at 1.
- A pipeline shaped so slice 2's three GPU dependencies and the later model downloads are new
  descriptors rather than a second pipeline.
- `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` green on all three OSes.

**Non-Goals:**

- Concurrent downloads of several sources in one dependency. The runtime is one archive; the fan-out
  belongs to the first dependency that has more than one source.
- Cross-process locking. As in the Go original, two copies of the app racing would download the same
  bytes twice, which is not worth a lock file and its staleness rules.
- Cancellation as a first-class API. `initialize()` is a future; dropping it stops polling and
  `rust-sak` leaves the `.part` behind for the next call, which is the useful behaviour. A `cancel`
  handle is a later change if a front end needs one.
- The `SkipVerify` debug override and the unverified-install path. Those exist in Go for hand-dropped
  model weights, which arrive with the model work.

## Decisions

### D1. `Opai::initialize` returns an owned handle, superseding slice 0's sketched signature

```rust
pub struct Opai { /* Arc<Inner> */ }

impl Opai {
    pub async fn initialize(
        name: &str,
        on_progress: Option<Arc<dyn Fn(&Progress) + Send + Sync>>,
    ) -> Result<Self, InitError>;

    pub fn config_dir(&self) -> &Path;
    pub fn sub_dir(&self, sub: &str) -> Result<PathBuf, InitError>;
}
```

The directory accessor is two methods rather than one. Reading the application's own directory cannot
fail once the handle exists - it was resolved and created during `initialize` - so returning a
`Result` there would be a lie every caller has to unwrap. Resolving a *subdirectory* genuinely can
fail, because `sub` is caller-supplied and must not escape its parent, so that keeps the `Result`.

Slice 0's table wrote this as a free `initialize(name, on_progress)`, mirroring Go's
`opai.Initialize(ctx, name, onProgress)`. It changes here, deliberately, and the change is structural
rather than behavioural - the context explicitly allows that, and the Go shape is the one thing here
that exists to work around a language limitation.

Go needs `lifecycleMu` plus a `destroyed` flag purely because `sync.Once` cannot be re-armed, and it
needs `internal.SetAppName` as a process global because there is nothing to hang the name on. A
handle removes both: the app name, the resolved config directory and - in slices 2 and 3 - the model
registry and the ONNX environment are fields, and teardown is the handle going away. It also matches
what current Rust libraries do (`reqwest::Client`, `sqlx::Pool`), and it is what makes slice 3's
"a session must not outlive the environment" expressible as a borrow rather than as a locking
discipline, which the project's conventions ask for.

`Arc`-backed and `Clone`, so the GUI holds one in `app.manage()` and the CLI in a local, and cloning
is not a second initialization.

Alternatives. **Free function plus globals**, as pinned: a direct translation of the Go call sites,
but it carries the flag pair into Rust and makes every later lease a runtime check. **A handle with
an async `shutdown()` and `Drop` as a backstop**: still the likely end state, but the teardown
semantics belong to slice 3, where there is something to tear down; this slice has nothing that
needs closing, so it defines no destructor rather than defining one that does nothing.

`on_progress` is an `Option<Arc<dyn Fn>>` rather than a generic `impl Fn`: the callback is handed to
`fs::extract`'s hook, which requires `Fn + Send + Sync + 'static`, and it crosses a `spawn_blocking`
boundary (D6). An `Arc<dyn ...>` is what both need anyway, and it keeps `initialize` from being
monomorphised per call site for no gain.

### D2. In-stream hashing is added to `rust-sak`'s `fetch`, not worked around here

```rust
// rust-sak
pub enum DigestAlgorithm { Sha256 }

impl RequestOptions {
    pub fn digest(self, algorithm: DigestAlgorithm) -> Self;
}

impl Download {
    /// The hex digest of the artifact, once the transfer has completed successfully.
    pub fn digest(&self) -> Option<String>;
}
```

The download path is the only place where the bytes are already in hand, so hashing them there costs
nothing, while hashing afterwards is a second full read of the archive. A resumed transfer hashes the
existing `.part` prefix before it appends, so the digest always describes the whole artifact rather
than the tail this attempt carried - which is what Go's `fill` does by falling back to `hashFile`
when its in-stream hash is unusable.

It belongs upstream for the same reason slice 0's two fixes did: any caller downloading an artifact
published with a checksum needs it, and nothing about it is specific to photo editing. Additive -
`digest()` is `None` unless asked for, and `join`/`track`/`Progress` are untouched.

**One path still needs `crypto::sha256_file`, and it is defensive rather than routine.**
`DownloadMode::Resume` short-circuits when a file already sits at the target path: it reports complete
without contacting the server, and therefore without a digest. The pipeline treats "complete, no
digest" as "hash the file on disk", so verification is unconditional however the transfer reported -
which is the property worth having, and it is covered by tests driving `acquire` directly.

`install` does not reach that state through its own front door, and the reason is D4's ordering. An
archive sitting un-extracted at its final path means a run was killed between the rename and the
extraction - and that run cleared the manifest before it wrote anything, so the next run reads no
record, empties the undescribed directory (keeping only `.part`/`.part.json`) and transfers again. The
cost is a repeated download of an archive that had already been verified, and the window is the whole
minute the expansion takes rather than a narrow one. Sparing the source's own final path in that
emptying would close it, at the price of a rule the spec does not state and of a stale same-named
archive from a previous release being hashed before it can be discarded. Left as it is, and recorded
here rather than lost, so the slice that next touches the pipeline decides it with the model downloads
in view.

The `crypto` feature is enabled for that fallback and, unconditionally, for the fingerprint (D4).

Alternative considered: **`crypto::sha256_file` on every install**, no upstream change. One less
repository to touch and one less release, and the cost is under a second on any SSD even for the
175 MB Linux archive. Rejected on the same grounds as slice 0's two fixes - the read is avoidable,
the fix is general, and the upstream API is the honest place for it.

### D3. `Progress` carries the dependency and the phase; the expansion owns the last fifth

```rust
#[non_exhaustive]
pub enum Phase { Downloading, Extracting }

#[non_exhaustive]
pub enum Dependency { Runtime }   // slice 2 adds Cuda, Cudnn, TensorRt

pub struct Progress {
    pub dependency: Dependency,
    pub phase: Phase,
    pub bytes: u64,          // bytes moved in the current phase
    pub total: Option<u64>,  // the phase's total, where one is known
    pub fraction: f64,       // 0.0..=1.0 across the whole install of this dependency
}
```

Go's callback is `func(downloaded, total int64, percent float64)`, and its GUI service supplies the
dependency name itself - `DownloadProgress{Dependency: "ONNX Runtime", ...}` hardcoded at the call
site, once per dependency it installs. Carrying it in the report instead is the same user-visible
result with the knowledge in the one place that has it; slice 2 adds three more dependencies without
either front end learning their names.

`fraction` reserves the last fifth of the bar for the expansion, as `downloadShare = 0.8` does in Go:
a couple of gigabytes of LZMA2 is a minute of work with nothing arriving over the network, and
without the split the bar sits at 100% throughout it. `fraction` is clamped monotonic per dependency
and forced to exactly 1 by a final report, so a throttled tick can never be the last thing a front
end sees.

Throttling is mostly inherited - `rust-sak` coalesces both the download and the extraction hooks to
~256 KiB or 100 ms - so this layer adds phase composition, the monotonic clamp and the terminal
report rather than a second throttle of its own.

### D4. The manifest is Go's, format and ordering both

```jsonc
// <config>/runtime/.manifest.json
{ "schema": 1, "name": "onnx-runtime", "version": "runtime/1.26.0",
  "fingerprint": "<sha256 of tag + each source URL and expected hash>",
  "files": [ { "path": "onnxruntime.1.26.0.dylib", "size": 32109216 } ] }
```

The ordering is the part that matters and it is copied exactly: the previous record is deleted
*before* the first byte is written and the new one is written *after* every file is in place, so an
interruption always leaves a directory that is explicitly mid-install. That is also why there is no
staging directory - it would double peak disk for a multi-gigabyte archive to close a window this
ordering already closes. The write itself is atomic (temp file plus rename, the transient suffixes
kept out of the recorded tree).

`fingerprint` rather than `version` decides a reinstall, because the model dependencies that arrive
later have no tag and move when their published hash moves; one comparison covers both. `intact()`
stats each recorded file and compares its size - deliberately not a re-hash, which would re-read
gigabytes on every launch to catch what install-time verification already ruled out. It uses
`symlink_metadata`, not `metadata`: the Linux archives carry `libfoo.so -> libfoo.so.N` chains,
and following them makes every launch read the install as corrupt.

Kept from Go for the same reasons it states: the install directory itself survives `remove` (on
Linux it is on `LD_LIBRARY_PATH` from before the process re-execed, and the loader permanently skips
a search path it finds missing); an exclusive directory with no manifest is emptied rather than
merged into; `.part`/`.part.json` survive that emptying so a large interrupted download is still
resumable.

Not carried over: the `.old` rename-aside fallback for a file Windows has mapped into a running
process. It is real, but it only bites when two copies of the app run at once, and slice 3 is where
a runtime is actually loaded and can be held open. Recorded here so that slice picks it up.

### D5. The extracted tree is read back with `fs::list_path`, not by extending `ExtractSummary`

Extraction produces a file list nobody declared, so - as in Go's `recordTree` - the list is read off
the disk afterwards rather than diffed, since an archive overwriting an existing file would be
missing from a diff and the manifest has to name every file it owns for the next uninstall to be
exact.

`rust-sak`'s `ExtractSummary` counts files but does not name them, and `fs::list_path` returns paths
without sizes, so recording costs one `symlink_metadata` per file. Go avoids that by walking
`DirEntry` itself and calling this out as deliberate. Here it is a few dozen files for the runtime
and a few hundred for slice 2's CUDA tree - microseconds either way - so the third `rust-sak` change
it would take to return an entry list is not worth making. Revisit only if a measurement disagrees.

### D6. The extraction runs on `spawn_blocking`; everything else is async

`fs::extract` is synchronous and CPU-bound for a minute on a large archive, so running it on the
async runtime would stall every other task in the process - in the GUI, the whole Tauri event loop.
It goes to `tokio::task::spawn_blocking`, which is also why the progress callback must be
`Send + Sync + 'static` (D1): it is invoked from that thread.

`tokio` is enabled with `rt`, `fs` and `sync` only. `fetch` already requires a Tokio runtime, so this
adds a feature set rather than a runtime.

### D7. A hash mismatch is retried exactly once, from zero

On a mismatch the downloaded bytes are deleted - they must not survive as a resume point - and the
transfer is run again with `DownloadMode::Overwrite`. If the second result also mismatches, the
install fails with both hashes in the error. Two passes, as in Go's `maxTransferPasses`: one
transfer, plus one clean restart if what was resumed turned out not to hash. A third would only be
retrying the clean download that just failed.

`resume_key` is set to the pinned SHA-256, so slice 0's sidecar check normally catches a stale
partial before a byte moves and this path is the backstop rather than the mechanism.

### D8. The pinned table is a `const` keyed by the release asset's own platform names

The published assets are named from Go's `GOOS`/`GOARCH` - `onnx_darwin_arm64.7z`,
`onnx_linux_amd64.7z` - and those names are fixed in releases that already exist. Rust reports
`macos`/`x86_64`/`aarch64`, so the table maps `std::env::consts::{OS, ARCH}` onto the asset spelling
(`macos -> darwin`, `x86_64 -> amd64`, `aarch64 -> arm64`) in one function. An unmapped pair is an
error naming the platform, never a default.

The table itself is a `const` slice per dependency holding the tag once and the hash, size and
library name per platform - the shape of Go's `internal.Releases`, for the reason its comment gives:
the tag written once per dependency instead of once per platform means two versions of one dependency
cannot be expressed at all, so a bump is one edit. Resolution returns `Option`, so a missing entry is
a reported error rather than an empty expected hash, which is how the Go app once shipped the NVIDIA
libraries unverified.

Values are copied verbatim from `internal/artifacts.go` (`runtime/1.26.0`, five platforms) and are
re-derivable with the `gh api .../releases/tags/...` command recorded there.

### D9. Module layout and error type

```
opai/src/
  lib.rs         Opai, initialize, re-exports
  error.rs       InitError (thiserror), wrapping rust-sak's FsError and DownloadError
  config.rs      config directory resolution over fs::mk_user_config_dir
  progress.rs    Progress, Phase, Dependency, and the reporter that composes the phases
  deps/
    mod.rs
    artifact.rs  the pinned table and platform resolution
    release.rs   the release URL and the dependency descriptor built from a pinned artifact
    manifest.rs  Manifest, fingerprint, read/write/intact/remove
    install.rs   the pipeline
```

Tests sit beside the code they cover, as in both the Go original and `rust-sak`. `InitError` is
`thiserror`, matching `rust-sak` and the project convention; the hash-mismatch and
platform-unsupported cases are named variants rather than strings, because tests and a front end both
need to tell them apart.

## Risks / Trade-offs

- **This slice depends on a `rust-sak` release that does not exist yet.** -> Same shape as slice 0:
  land and tag upstream first (with the owner's go-ahead before pushing), then re-pin here. Until
  then nothing in this slice compiles, which is why the upstream work is tasked first and separately.
- **Extraction progress and download progress are reported by two different mechanisms** (a
  `Download` handle polled from async, a hook invoked on a blocking thread). -> They meet in one
  reporter that owns the monotonic clamp and the phase split, so the composition is in one testable
  place rather than at each call site.
- **A hand-built 7z fixture is needed for the extraction tests.** -> A committed archive carrying one
  incompressible 64 KiB library beside two small providers, served from a local
  `tokio::net::TcpListener` - the same approach `rust-sak`'s own `fetch` tests use, so nothing
  reaches the network. **Deliberately not the 258 bytes `internal/deps/testdata/tree.7z` is in Go**:
  a body that fits inside a transfer's write buffer is never flushed to disk when the transfer is
  interrupted, so the `.part` is empty and cross-install resume cannot be observed at all. The large
  member is also what makes progress advance from *inside* one file rather than only between files.
  Do not shrink it back.
- **Widening CI to three OSes triples the test job's minutes.** -> It is what the existing comment
  asks for at exactly this point, and it is the only way the platform mapping in D8, the symlink
  handling in D4 and the rename-over-existing behaviour are checked rather than assumed. fmt and
  clippy stay on one runner.
- **The real archives are not downloaded in CI.** -> A live install is a 175 MB transfer against
  GitHub releases on every run. It stays a manual check, as `internal/deps/live_manual_test.go` is in
  Go; the automated tests cover the pipeline against the local server and a fixture archive.
- **`Dependency` and `Phase` are enums a later slice adds variants to.** -> Marked
  `#[non_exhaustive]` so slice 2 is additive for any consumer, including the front ends.

**Validation.** Developed and validated on macOS/aarch64; Linux/x86_64 and Windows/x86_64 through the
new CI matrix. Linux/aarch64 and Windows/aarch64 are in the pinned table but untested - no runner
covers them. No execution provider is exercised at all in this slice: nothing loads the runtime until
slice 3.

## Migration Plan

1. Land the digest extension in `../rust-sak` on its default branch, with tests and the full gate
   green.
2. **Confirm with the repository owner before pushing** - `rust-sak` is a shared first-party crate.
3. Cut the next CalVer tag there.
4. Re-pin here, add the `crypto` feature, and build the slice against it.

Rollback is the pin plus a revert: no user data, no on-disk format anyone else writes, and an
installed `runtime/` directory left by a reverted build is simply reinstalled by the next one, since
the manifest it wrote either matches the pin or does not.

## Open Questions

- The CalVer number for the `rust-sak` release follows whichever month it is cut in; a slip changes a
  literal in two files and nothing else.
- Whether `initialize()` should expose cancellation as an explicit handle rather than relying on the
  future being dropped. It cannot be answered without a front end consuming it, and the answer changes
  no requirement here - dropping the future already leaves a resumable `.part`.
