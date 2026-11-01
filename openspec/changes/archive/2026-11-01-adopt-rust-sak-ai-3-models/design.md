## Context

See `proposal.md` for the motivation. After slices 1 and 2, `opai`'s `deps/` holds only the model path:

```
deps/model/mod.rs         install(): Listing::resolve, descriptor_at, deps::install::install, discard the outcome
deps/model/manifest.rs    Listing: live tree (paged) -> cached .listing.json -> compiled-in published.json -> refuse
deps/model/descriptor.rs  <artifact>.onnx* files from the listing -> Dependency { sources, derived: engines/<artifact> }
deps/model/generate.rs    test-only generator for published.json
deps/install.rs           check (fingerprint + intact | trust stamp), empty dir + derived, acquire (resume, verify,
                          retry once clean), record; Outcome; record_ended; cancel token -> InitError::Stopped
deps/manifest.rs          .manifest.json: fingerprint + files with sizes; empty_dir; record_tree
deps/release.rs           Dependency / Source descriptor (now models only)
```

Its callers are `sessions/mod.rs` (`Sessions::install`, with a test seam `ModelSource::At { base_url, listing }`),
`setup.rs` (`record_ended` for the runtime), `progress.rs` (`Reporter::for_dependency`) and `sessions/caches.rs`
(`ENGINES_DIR`, `engine_dir`).

`rust-sak` `ai` (tag `26.11.0`) has `hf::Hub` (`resolve`, a paged `list`, `url`), `hf::Hash` (`Sha256` for LFS,
`GitBlobSha1` otherwise, `verify_file`), `fetch`'s resumable downloads, an OS advisory lock per folder, the
`.installed` marker (hash, size, files with sizes), `Event`/`Phase`, and `InstallError`/`Failure`. All of it is built
for one archive per version folder:

| OPAI's model path needs | `rust-sak` today |
|---|---|
| Bare files installed into a folder the caller chooses | One archive, extracted into `<root>/<name>/<version>` |
| Several files per install, chosen from a shared folder (`<artifact>.onnx`, `<artifact>.onnx.data`) | One asset per platform, picked by `asset_for` |
| A folder of one segment (`models`) | `Source` needs `<name>/<version>`, so two segments |
| A file republished at the same path replaces the install | Local-first: a present install is current with no request |
| One listing shared by every install in a process | `plan` lists per call |
| Empty a derived folder (`engines/<artifact>/`) when the files are replaced | Nothing derived |
| Trust the files on disk (`ModelTrust::LocalFiles`) | No trust option |
| Know whether the install replaced files or was current | `install` returns `()` |

The user decided three things when this slice was proposed. Models stay in `<config>/io.vinicius.opai/models/` and
are not moved to the shared `io.vinicius.ai` root. They stay in `vegidio/open-photo-ai`, folder `models`. And a model
keeps the same path when it is republished, so the installer has to compare against the listing and cannot be
local-first the way the runtime is.

## Slices

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `adopt-rust-sak-ai-1-deps` | Runtime and NVIDIA installs, GPU selection, library path and runtime load on `rust-sak` `ai`; `ort` rc.15; shared `io.vinicius.ai` root; old folders removed | - | done |
| 2 | `adopt-rust-sak-ai-2-sessions` | Session building, provider options, ladder and tuning on `rust-sak` (single-rung build with OOM classification, shared serialised TensorRT timing cache, explicit cache paths); `ExecutionProvider`/`SupportedProviders` re-exported; `EpProfile` → `Tuning`; per-provider engine folders | 1 | done |
| 3 | `adopt-rust-sak-ai-3-models` | Generic Hugging Face file installer in `rust-sak` `ai` (selected files from a folder, API-hash verified against a shared listing, republish-aware, adopt in place, derived folders, resumable, progress, trust-local, offline use of an intact install); OPAI's model pipeline, cached and compiled-in listings deleted; `ModelTrust::LocalFiles` mapped onto trust-local | 1 | **current** |

Each slice ships its `rust-sak` additions under a new tag first, then the OPAI migration that uses them.

**Changed from the plan in slices 1 and 2:** those described this slice as "old model files deleted and re-downloaded
(TODO-marked)". This design adopts them by hash instead (D4). Models stay in the same folder, so the files an earlier
release verified are already where they belong, and hashing them is seconds against gigabytes of download. The
TODO-marked cleanup shrinks to the orphaned `models/.listing.json`.

## Goals / Non-Goals

**Goals:**

- OPAI holds no code for listing, downloading, resuming, verifying, recording or invalidating model files. What stays
  is policy: which folder, which prefix, where to install, what is derived from it, the trust declaration, the
  once-per-process listing, the records and the `Reporter`.
- Every `model-install` scenario not changed by the delta behaves as today, including stop and resume, the
  non-decreasing fraction that lands on 1, separability between models, and engine discard on replacement only.

**Non-Goals:**

- Moving models to the shared root or to `vegidio/general`. The user decided against both.
- Changing `ModelTrust`'s public shape, `InitError`'s variants, or `Progress` for models.
- Sharing a model between applications. Nothing here stops a later app pointing at the same folder, but that is
  not designed for.

## Decisions

### D1. A file installer in `rust-sak` `ai`, beside the archive installer

New public API in `rust-sak` `ai`. The behaviour is fixed here, and names are settled in implementation:

```text
Folder::parse(url)                      a Hugging Face folder URL of one or more segments (Source keeps its own
                                        two-segment rule and is built on Folder)
Listing::read(&Folder) -> Result<Listing, HfError>
                                        resolve the revision to a commit, then list every page: the files, sizes
                                        and hashes, and the commit every download uses
Files { folder, select, dir, derived, trust }
  select:  Select::Names(Vec<String>) | Select::Prefix(String)
  dir:     the folder the files install into (caller-chosen; created if missing)
  derived: Vec<PathBuf>, folders computed from these files
  trust:   Trust::Published | Trust::LocalFiles
install_files(&files, listing: Option<&Listing>, on_event) -> Result<Outcome, InstallError>
  Outcome: Installed | Current | Adopted | Offline | Trusted
  Failure: + NotListed (the selection matched nothing in the listing), + Unlisted (no listing and nothing intact)
```

It lives in `rust-sak` because none of it is about OPAI: any app that installs models from a Hugging Face folder,
which mostly hold several models and republish in place, needs the selection, the republish check, the derived
folders and the offline use. `vegidio/general`'s `models/insightface` and `models/siglip2` are folders of exactly that
shape. Trust-local is the debugging workflow of any app that runs models it re-exports.

Alternative considered: make the archive installer take bare files. Rejected. Its contract is "one version per
folder, local-first", and models break both halves. One installer with two contracts selected by flags would be
harder to read than two that share their pieces (lock, marker, hashing, events).

### D2. The current check, with and without a listing

Inside the folder's lock (`dir/.lock`, the existing advisory lock):

```text
trust == LocalFiles and every selected file is present as a regular file
    -> stamp = sha256 over (name, size, mtime ns) of each, in name order
    -> marker is Trusted{stamp}?  Trusted, nothing touched
       otherwise: delete marker, empty derived, write Trusted{stamp} -> Trusted
listing present:
    want = files the listing names for `select` (a file with no hash counts as absent);
    empty -> Failure::NotListed, nothing transferred
    marker is Files{..} with exactly `want`'s names, each hash equal, each file on disk at its recorded size
        -> Current, nothing touched
    otherwise -> install (D3)
listing absent:
    marker is Files{..} and every recorded file is on disk at its recorded size -> Offline
    otherwise -> Failure::Unlisted, nothing transferred
```

The selection under trust comes from the listing when there is one, and otherwise from the marker's names. A `Prefix`
with neither falls back to matching the folder's own files. A `Trusted` marker never satisfies the `Files` check, so
a process without trust reinstalls (and, through D4, keeps whatever happens to match). The marker gains a variant:
`Files { files: [{ path, size, hash }] }` or `Trusted { stamp }`. The archive marker is unchanged.

The "present and the right size" test is the existing one: `symlink_metadata`, a regular file, never a hash.

### D3. The install

1. Delete the marker. Empty every `derived` folder, keeping the folder itself. Only now can new bytes move, so an
   interruption leaves new weights beside no compiled artifact, never beside a stale one.
2. For each wanted file: if it is present at the listed size, hash it (`Event::Verifying`). A match is kept (D4).
   Otherwise download `resolve/<commit>/<path>` into `dir/.staging/<name>` with `fetch`'s resume, verify it against
   the listing's hash, and rename it into `dir`. A mismatch after resuming a partial is retried once from nothing,
   and fails with `Failure::Damaged` after that, as OPAI does today. A mismatch on a clean download fails at once.
3. Remove every entry of `dir` that is neither wanted nor the installer's own (`.installed`, `.lock`, `.staging`).
   That drops a previous version's extra file and an earlier release's `.manifest.json`.
4. Write the `Files` marker last.

The outcome is `Adopted` when every file was kept, and `Installed` otherwise. `.staging` sits inside `dir`, so the
rename is never across filesystems and peak disk is one copy plus the file in flight, as it is today.

**Events:** `Progress { phase: Download, bytes, total, fraction }` over the sum of the wanted files' listed sizes
(no `Extract` phase), `Verifying` while a present file is hashed, and nothing for `Current`/`Offline`/`Trusted`. The
fraction never decreases. It is exactly 1 only after the marker is written, and a final `Progress` at 1 is always
sent for an install that ran. **Cancellation** is by dropping the future, as for archives. The partial stays in
`.staging` to resume onto.

### D4. Adopt in place, not delete and re-download

A selected file already in `dir` at its listed size is hashed and kept when it matches. Once, after updating, every
installed model goes down this path: the old `.manifest.json` is not a `rust-sak` marker, the files are there and
they match. The cost is one hash per file, about 1.7 GB for the largest, on the first acquisition of each model. The
alternative, the "delete and re-download" in slices 1 and 2, costs every user tens of GB for nothing. The same path
also serves a user who copied a model folder in by hand, and the trust path never hashes.

Offline at the first acquisition after updating, an old install has no `rust-sak` marker and no listing to verify
against, so it fails as "not installed" until the network is back (the delta states this). The alternative is to
accept the old `.manifest.json` as proof, which would keep OPAI code alive only for this case, and it holds a
combined fingerprint, not per-file hashes, so it could not seed the new marker anyway. Rejected.

### D5. What OPAI keeps, and where

```text
deps/model/mod.rs     FOLDER = "https://huggingface.co/vegidio/open-photo-ai/tree/main/models"
                      LISTING: process-wide tokio OnceCell<Arc<Listing>> (success memoized)
                               + AtomicBool OFFLINE (a read failed this process)
                      install(app_dir, name, id, trust, installing):
                        listing = if OFFLINE and the model is intact locally { None } else { read or memo }
                        Files { folder, Prefix("<id>.onnx"), dir: models/<id>, derived: [engines/<id>], trust }
                        select! { install_files(..) , installing.cancel.cancelled() => InitError::Stopped }
                        events -> Reporter (Downloading bytes/total; finish at the end)
                        Outcome -> records; Trusted -> warn naming the model (kept from today);
                                   Offline -> the once-per-run warn (logging delta)
                        InstallError -> InitError (below)
deps/model/paths.rs   MODELS_DIR, ENGINES_DIR, model_dir, engine_dir  (what descriptor.rs shrinks to)
deps/records.rs       record_ended and the install start/ready/failed records, moved out of install.rs
```

The `OFFLINE` short-cut needs "intact locally" without a listing. That is `rust-sak`'s offline check (D2), exposed as
`files_present(&files) -> bool` so OPAI never reads the marker itself. A model that is not intact tries the
listing again, which is the delta's "MAY try again".

**Error mapping:** `Failure::Network | Http` → `InitError::Download`, `Damaged` → `HashMismatch`,
`NotListed` → `UnpublishedModel`, `Unlisted` → `UnlistedModel`, a sibling variant whose message says the listing
could not be read and that the model is not installed (kind `unlisted_model`; `InitError` stays `#[non_exhaustive]`),
`DiskFull | Io` → `Io`. A listing read error is an `HfError`, kept to decide `Unlisted` and the record.

**Deleted:** `deps/install.rs`, `deps/manifest.rs`, `deps/release.rs`, `deps/model/manifest.rs`,
`deps/model/generate.rs`, `deps/model/published.json`, the model half of `deps/test_server.rs`, and
`ModelSource::At { base_url, listing }`. The test seam becomes `ModelSource::At { folder }`, pointing at
`deps/hf_server.rs` (an HF-shaped test server that already exists), so session tests install real files through
`rust-sak`. `Reporter::for_dependency` takes `(Which, total)` in place of a descriptor.

**Old cached listing:** `config.rs` gains a best-effort removal of `models/.listing.json` beside the slice 1 cleanup,
marked `// TODO: remove once users have upgraded past this release`.

### D6. Trust stays a property of the handle

`ModelTrust` stays OPAI's public enum, read once at `initialize` and stored on `Sessions`, and maps 1:1 onto
`rust_sak::ai::Trust`. Nothing about who can set it changes. The `warn` on each trusted acquisition stays in OPAI,
since what an app logs is the app's. The stamp moves to `rust-sak` unchanged: size and mtime, never a hash.

### Division across crates

All logic is in `opai` and `rust-sak`. `gui`, `cli` and `perf` change only the `rust-sak` tag. `perf --skip-verify`
keeps passing `ModelTrust::LocalFiles`. No front end gains logic, and none needs to: `Progress` for a model is
unchanged.

### `rust-sak` features and crates

`ai` only (already enabled in `opai`), which brings `fetch`, `crypto` and `fs`. No new crate in either repository.
`tokio::select!` and `tokio::sync::OnceCell` are already used in `opai`. Every gap in the table under Context is
filled in `rust-sak` (D1), for the reason given there. The only OPAI-local pieces are the folder URL, the prefix rule,
the paths, the memo and the records, which are the app's choices.

### Ownership and lifetime

- **Listing:** an `Arc<Listing>` in a process-wide `OnceCell` in `opai`, filled by the first acquisition that reads
  it, borrowed by every later `install_files`, never refreshed within a process. It pins the commit, so every file of
  one process comes from one revision. `OFFLINE` is set once and never cleared.
- **Folder lock:** taken by `install_files` for its whole run, released on return or when the future is dropped
  (stop), and by the OS if the process dies. The session cache's single flight already keeps two requests in one
  process from racing, and the lock covers a second process on the same configuration folder.
- **Derived folders:** owned by OPAI's `caches` (paths), emptied by `rust-sak` inside an install that replaces files,
  emptied wholesale by OPAI on a runtime or layout stamp change (slice 2), and filled by `rust-sak`'s session build.
- **Sessions:** unchanged. A session is built only after `install` returns the folder. Replacing a model's files
  while a resident session holds the old ones cannot happen in one process, since the acquisition that replaces them
  is the one that builds.

## Risks / Trade-offs

- [The first acquisition of every installed model after updating hashes it, up to ~1.7 GB] → Seconds per model,
  once. `Verifying` is reported, so the GUI's progress shows the work.
- [Offline at the first acquisition after updating, installed models fail as not installed] → Stated in the delta.
  The alternative keeps OPAI's manifest code alive for one launch (D4).
- [A republish is picked up only when a process reads the listing, so a long-lived GUI keeps the version it read at
  start] → The same as today. The next launch replaces it.
- [The compiled-in fallback is gone, so a first install needs the network] → It always did for the bytes. Only a
  listing outage with a working CDN is lost, and both are on the same host.
- [Hash integrity now rests on the HF API, as for the runtime since slice 1] → Already accepted in slice 1. Models
  were already verified against the live listing first.
- [A new marker format in model folders] → An older OPAI finds no `.manifest.json` and re-downloads in place, so a
  rollback costs bandwidth, not correctness.
- **Platforms validated on:** macOS arm64 (developer and `rust-sak` tests), CPU and CoreML for the model runs.
  **Untested until the release gate:** Windows (lock, rename over an existing file, removing a file another process
  maps) and Linux. Nothing here is provider-specific. CUDA, TensorRT and WebGPU only read the files, so the engine
  discard is the only provider-facing part and is covered on macOS through CoreML's folder.

## Migration Plan

1. `rust-sak`: D1–D3 with tests against its local HF test server, README updated, then tagged after the user's
   go-ahead.
2. OPAI: bump the tag in `opai`, `gui`, `cli` and `perf`. Replace `deps/model` with D5 and port the model-install tests
   onto the HF test server. Delete the old pipeline and listings. Add the `.listing.json` cleanup.
3. Verify on macOS. Run every model, FP32 and FP16, through Rust on CPU and CoreML. Launch the GUI over an existing
   install: models are adopted, not downloaded, and engines are kept. Do a second launch offline, and run a trusted
   `perf --skip-verify` with a re-exported model.
4. Windows and Linux gate: a first install, an adoption, a republish replacement and a stop/resume.

Rollback: revert the OPAI commit. The earlier release does not recognise `.installed`, finds no `.manifest.json`,
and re-downloads each model in place on first use. Engines are discarded with it.
