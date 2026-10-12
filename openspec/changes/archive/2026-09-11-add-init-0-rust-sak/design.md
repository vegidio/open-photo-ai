## Slices

`opai::initialize()` - download and install the pinned ONNX Runtime, then the NVIDIA libraries on a
machine that has an NVIDIA GPU, reporting progress throughout - estimates well past the 15-task
ceiling as one change. It is split into four slices. Each ends with the workspace building, testing
and passing CI on its own.

| # | Slice | Delivers | Depends on | Status |
|---|-------|----------|------------|--------|
| 0 | `add-init-0-rust-sak` | `rust-sak`: `.part` + identity sidecar, atomic rename and `If-Range` in `fetch::download`; `resume_key`; progress hook and header totals in `fs::extract`. CalVer alignment and a new tag. Here: `opai`'s first dependency, pinned to that tag. | nothing | **current** |
| 1 | `add-init-1-runtime` | `opai`: config directory and app name, the pinned artifact table (tag + SHA-256 + size + library name per platform), `Progress`/`Dependency`/`Phase` types, manifest bookkeeping (`fingerprint` -> `intact` -> `remove`), the download/verify/extract/record pipeline, and `pub async fn initialize(name, on_progress)` installing the ONNX Runtime into `<name>/runtime/`. | slice 0 | pending |
| 2 | `add-init-2-gpu` | `opai`: NVIDIA detection via `sysinfo::gpu_info`, CUDA + cuDNN into `<name>/libs/{cuda,cudnn}/` and TensorRT into `<name>/libs/tensorrt/` when supported, loader search-path handling including the Linux `LD_LIBRARY_PATH` re-exec, and the supported-execution-provider report. | slice 1 | pending |
| 3 | `add-init-3-ort-env` | `opai`: the `ort` crate, the ONNX environment started against the downloaded library, its log level, and the `Destroy`/re-initialize lifecycle. | slice 2 | pending |

Only slice 0 is proposed. Each later slice is proposed when it is reached.

## Context

See proposal.md - Why.

The workspace today is the output of the two scaffolding slices: three crates, `opai` exposing a
single `version()` **with no dependencies at all**, and CI running fmt, clippy and test with
`--locked`. This slice gives `opai` its first dependency.

Three facts about `../rust-sak` shape everything below, and all three were checked rather than
assumed:

- **It is not distributable today.** Not on crates.io (`index.crates.io/ru/st/rust-sak` -> 404), no
  GitHub releases (`gh api repos/vegidio/rust-sak/releases` -> `0`).
- **Its version scheme contradicts itself.** One tag, `26.7.0` (CalVer, 2026-07-15); the manifest at
  that tag reads `1.0.0`; `HEAD` reads `2.0.0` and is untagged.
- **`fetch::download` writes to the final path.** `DownloadMode::Resume` opens the target with
  `append(true)` and sends `Range` from its on-disk length. There is no `.part` file and no record of
  what a partial belongs to.

The Go original is the behavioural reference for the *problem*, not for the solution:
`internal/deps/partial.go` and `manifest.go` carry `.part`, `.part.json`, `.tmp` and `.old` suffixes,
and `internal/deps/archive.go` fakes extraction progress with a 250 ms ticker over a directory walk,
saying in its own comment that the right home for it is the utility library.

## Goals / Non-Goals

**Goals:**

- Make a resumed transfer provably a resume *of the right thing*, before any byte is appended.
- Make a file at a download's target path mean "complete", always.
- Report extraction progress from inside the extraction, with real byte counts.
- Leave `rust-sak`'s extractor security guards untouched - extended, never forked or re-derived.
- Give this workspace a dependency pin that resolves in CI and in a fresh clone.
- Leave `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` green in both repositories.

**Non-Goals:**

- Any `opai` source code. This slice changes one manifest and one lock file here.
- Publishing `rust-sak` to crates.io, or designing its release process beyond cutting one tag.
- Concurrent transfers to the same target path, and cross-process locking. Both repositories' callers
  are single-writer per path; the Go original reaches the same conclusion explicitly.
- Progress for `text`/`json` requests, and cancellation semantics - unchanged.

## Decisions

### D1. `<path>.part` plus a sidecar, renamed into place on success

```
  during transfer                         on success
  --------------                          ----------
  onnx_darwin_arm64.7z.part        -->    fsync, drop sidecar,
  onnx_darwin_arm64.7z.part.json   -->    rename .part -> onnx_darwin_arm64.7z
  { "url": "...runtime/1.26.0/...",
    "etag": "\"a1b2\"",                   a file at the final path is now,
    "total": 174834737,                   by construction, complete
    "resume_key": "5cafbaae..." }
```

State machine on a `Resume` request:

| On disk | Sidecar | Action |
|---------|---------|--------|
| final path exists | - | complete; nothing transferred |
| `.part` + matching sidecar | url **and** `resume_key` match | `Range` from `.part`'s length, `If-Range` when an etag was recorded |
| `.part` + sidecar | either differs | delete both, start fresh |
| `.part`, no sidecar | - | delete, start fresh - identity is unknowable, and guessing is the bug being fixed |
| nothing | - | fresh |

Alternatives considered. **Keep writing to the final path and delete-plus-retry on a hash mismatch**
(the original recommendation): one task instead of several, and the pinned hash does eventually catch
it - but it catches it only after transferring gigabytes onto a prefix that was known-stale before
the first byte, and it leaves the truncated-file bug at the final path untouched. **A temp directory
per install**: the stale-prefix hazard cannot exist, but a 2 GB TensorRT download in slice 2 would
restart from zero after every interruption, which is the property the resume exists for. **Carry
Go's suffixes into `opai` instead**: rejected by the project's rule against forking a close-but-
incomplete `rust-sak` module, and it would leave every other caller of `fetch` with both bugs.

### D2. Identity is the URL, with `resume_key` as an opt-in strengthening

The URL alone closes the case this project actually has: release assets are tag-namespaced, so
`runtime/1.26.0` -> `runtime/1.27.0` is a different URL writing to the same local filename, and the
sidecar sees the difference before the `Range` request is built.

`RequestOptions::resume_key(impl Into<String>)` covers what the URL cannot - the same URL serving
different bytes because a release was re-cut. Slice 1 passes the pinned SHA-256, which it has in hand
before the transfer starts. It is opt-in because most callers have no such value and should not be
made to invent one.

`If-Range` is the third layer and costs almost nothing: the sidecar already records the response's
`ETag`, so the resume request sends `If-Range: <etag>` and a server whose bytes changed answers `200`
instead of `206`, which the existing code already handles as a fresh download. It is a complement,
not a replacement - it cannot see D2's URL case at all, because that request is never sent against the
old URL.

### D3. `ExtractOptions::on_progress`, fired inside the copy loop

```rust
pub struct ExtractProgress {
    pub entries: u64,                 // entries written so far
    pub bytes: u64,                   // uncompressed bytes written so far
    pub total_entries: Option<u64>,   // from the archive header
    pub total_bytes: Option<u64>,     // from the archive header
}

impl ExtractOptions {
    pub fn on_progress(self, f: impl Fn(&ExtractProgress) + Send + Sync + 'static) -> Self;
}
```

`Fn` rather than `FnMut` so the options struct stays `Clone` and `Sync`, matching the rest of the
builder; a caller needing mutation uses interior mutability, as it would for any shared callback.

**It must fire within an entry, not only between entries.** The ONNX Runtime archive for macOS is
essentially one ~170 MB shared library: per-entry reporting would render the whole expansion as a
single jump from 0 to 100%, which is the exact failure this is being added to fix. The extractor
already copies entry bodies in chunks, so the hook goes there, with the same throttle discipline the
counters already have.

Totals come from the archive header, which is why they are `Option`: ZIP's central directory and 7z's
header both carry uncompressed sizes, so both report `Some`. **TAR.XZ carries none** - the sizes are
inside the compressed stream, and producing a total would mean decompressing twice. It reports `None`
rather than a guess, and a caller renders an indeterminate bar. This is also strictly better than the
Go workaround, which needs a second full open of the archive to compute the same number that the
extractor is already holding.

### D4. The extractor's security guards are extended, never re-derived

Nothing in D3 touches name validation, the cap-std anchored directory handle, symlink re-resolution,
mode masking, deferred directory modes, or the reserve-before-write limit accounting. The hook reads
counters that already exist and is called from a loop that already runs. This is the whole reason the
change belongs in `rust-sak`: the Go comment on `extractArchive` says reimplementing the loop to get a
hook "would mean re-deriving its Zip-Slip and symlink-escape guards, which is the wrong thing to fork
for a progress bar."

### D5. `rust-sak` moves to CalVer, agreeing with its own tags

The manifest goes to `26.9.0` and a tag `26.9.0` is cut. The tags were always CalVer (`26.7.0`); it is
the manifest that drifted, and this workspace documents the same `YY.M.MICRO` scheme in its root
`Cargo.toml`. Nothing resolves `rust-sak` by version today - it is on no registry and has no releases
- so the nominal `2.0.0` -> `26.9.0` move breaks no resolver anywhere.

Alternative considered: **retag as semver `2.1.0`**, treating `26.7.0` as a mistake. Semver is the
ecosystem norm and communicates breakage, which CalVer cannot - and this slice does ship a behavioural
break (D1). But the break is invisible: no consumer resolves by version, so the signal has no
receiver, while the tag/manifest disagreement is a live ambiguity every later slice would re-hit.

### D6. Consumed as a git dependency pinned to a tag

```toml
[dependencies]
rust-sak = { git = "https://github.com/vegidio/rust-sak", tag = "26.9.0", features = ["fetch", "fs"] }
```

No `version` key: with `git` + `tag` there is nothing to constrain, and adding one only creates a
second place for D5's number to drift. `Cargo.lock` pins the exact revision, so the build is
reproducible even if the tag were ever moved, and CI's existing `--locked` enforces it.

Alternatives: **a path dependency** breaks CI and any clone of this repository alone - `add-scaffold-1-workspace`
already recorded this. **A rev pin** is equally reproducible and sidesteps D5 entirely, but reads as an
opaque SHA and makes every `rust-sak` change a manual hash edit here. **Publishing to crates.io** is
the clean long-term answer and is what `rust-sak`'s own README already tells people to do, but it is a
one-way door on the crate name and belongs in a change about that repository's release process.

### D7. Only `fetch` and `fs` are enabled now

Those are the two modules this slice extends, so enabling exactly them makes the build here a proof
that both new APIs resolve, compile and are public. `crypto` arrives in slice 1 with the verification
that needs it, and `sysinfo` in slice 2 with GPU detection. `rust-sak` has no default features, so
this is a deliberate list rather than a trimming.

## Risks / Trade-offs

- **D1 changes `fetch::download`'s on-disk behaviour for existing callers.** -> The blast radius is
  local checkouts only: nothing resolves `rust-sak` from a registry, and `gh` reports zero releases.
  The change is called out as BREAKING in the proposal and belongs in `rust-sak`'s own release notes.
  It also *fixes* more than it breaks - `DownloadMode::Skip` currently treats a truncated file as
  complete forever.
- **A rename over an existing file is not portable by default.** -> `std::fs::rename` replaces an
  existing file on both Unix and Windows (`MoveFileEx` with `MOVEFILE_REPLACE_EXISTING`), and `.part`
  sits beside its target, so the rename is always within one filesystem and cannot degrade to a copy.
  Worth an explicit test on Windows in CI rather than an assumption.
- **A rename is only as durable as what preceded it.** -> The `.part` file is flushed and synced
  before the rename, so a crash cannot leave a complete-looking file with unwritten tail bytes.
- **The extraction hook runs inside the copy loop, so a slow callback slows extraction.** -> Documented
  as a contract - the callback must not block - matching what `Download::track` already asks of
  callers. Slice 1's consumer throttles to ~100 ms before anything reaches a front end.
- **A sidecar is one more file that can be left behind.** -> It is removed on success and on every
  invalidation path, and an orphan is harmless: a `.part` with no sidecar is deleted and restarted,
  which is the safe default in D1's table.
- **TAR.XZ reports no total.** -> Accepted and documented. Neither this project nor the Go original
  ships a `.tar.xz` dependency; every archive here is `.7z`.
- **A git dependency needs network at build time and cannot be vendored by `cargo vendor` the way a
  registry crate can.** -> Accepted for now; revisiting it is what publishing `rust-sak` would be for.

## Migration Plan

1. Land both extensions in `../rust-sak` on its default branch, with tests, and the full gate green.
2. **Confirm with the repository owner before pushing** - `rust-sak` is a shared first-party crate, and
   the project's rules require an explicit go-ahead.
3. Align the manifest version (D5) and cut the `26.9.0` tag.
4. Add the pin here (D6), update `Cargo.lock`, verify a cold `cargo build --locked` and CI.

Rollback is the pin: reverting `opai/Cargo.toml` and `Cargo.lock` returns `opai` to having no
dependencies, since no source file in this repository uses either new API in this slice. The
`rust-sak` tag is immutable and harmless if unreferenced.

## Open Questions

- The exact CalVer number in D5 assumes the tag is cut in September 2026. If it slips into a later
  month, the number moves with it and D6's `tag` follows. This changes no approach, no requirement and
  no task - only a literal in two files.
