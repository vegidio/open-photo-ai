## Why

The first slice of `opai::initialize()` has to download a pinned ONNX Runtime archive, verify it,
expand it and record what it wrote. `rust-sak` already covers most of that - `fetch` for the
resumable download with retries, `crypto` for the SHA-256, `fs` for the config directory and the
hardened 7z extractor - but two gaps sit directly on that path, and both are general-purpose
failures of the modules rather than anything specific to this application:

1. **A resumable download cannot tell what its partial file is of.** `fetch::download` writes
   straight to the target path and appends on resume. Release assets here are tag-namespaced
   (`.../download/runtime/1.26.0/onnx_darwin_arm64.7z`), so a version bump is a *different URL*
   writing to the *same local filename* - and the resume appends new bytes onto a stale prefix. The
   pinned hash catches the corruption afterwards, which is a loop rather than a fix. The same write
   pattern leaves a **truncated file at the final path** when a transfer is cancelled, after which
   `DownloadMode::Skip` reports it complete forever.
2. **Extraction reports no progress.** A couple of gigabytes of LZMA2 is a minute or more of work
   with nothing arriving over the network, and the bar has already been told 100%.

Both belong in `rust-sak`, not here. The project's own rule is that a close-but-incomplete module is
extended rather than worked around, and neither gap is about photo editing: any caller resuming a
download to a stable path has bug 1, and any caller expanding a large archive behind a progress bar
has bug 2. The Go original reached the same conclusion from the other side - `internal/deps/archive.go`
polls the destination directory's byte total every 250 ms and carries a comment saying "If go-sak
ever grows a progress variant, this collapses into a call to it."

Doing this first, as its own slice, is what keeps the three slices that follow inside the 15-task
ceiling and keeps the `rust-sak` work reviewable instead of squeezed into a change about something
else.

## What Changes

Task estimate: **11**. The full initialization feature estimates well past 15, so it is split into
four slices (see design.md - Slices). This proposal covers slice 0 only.

**In `../rust-sak` (a separate repository):**

- **`fetch::download` gains partial-file bookkeeping.** Transfers write to `<path>.part` with a
  `<path>.part.json` sidecar recording the URL, the discovered total and the response's `ETag`; on
  success the file is renamed into place. A sidecar that does not match the current request
  invalidates the partial rather than resuming onto it. Where an `ETag` was captured, the resume
  request carries `If-Range`, so a server that changed the bytes at an unchanged URL answers `200`
  and the transfer restarts on its own.
- **`RequestOptions::resume_key`** - an opt-in caller-supplied identity, stronger than the URL. This
  slice's consumer passes the pinned SHA-256, so a partial is invalidated even if a release were
  re-cut at the same URL.
- **BREAKING (behavioural):** `DownloadMode::Resume` now resumes `<path>.part`, and a file at
  `<path>` means "complete" rather than "partially transferred". `DownloadMode::Skip` becomes correct
  as a consequence - it can no longer mistake a truncated file for a finished one. Nothing consumes
  `rust-sak` from a registry (see Impact), so the blast radius is local checkouts only.
- **`fs::extract` gains a progress hook.** `ExtractOptions::on_progress` receives entries and
  uncompressed bytes written so far, plus the totals read from the archive header where the format
  carries them (ZIP and 7z do; TAR.XZ does not, and reports `None`). It fires within a large entry,
  not only between entries - the ONNX Runtime archive is one ~170 MB shared library, which per-entry
  reporting would render as a single jump from 0 to 100%.
- **Versioning is settled.** The repository's only tag is CalVer `26.7.0` while the manifest at that
  tag says `1.0.0` and `HEAD` says `2.0.0` untagged. The manifest moves to CalVer to agree with the
  tags and with this workspace's own scheme, and a new tag is cut.

**In this repository:**

- `opai/Cargo.toml` takes its first dependency: `rust-sak` as a **git dependency pinned to the new
  tag**, with the `fetch` and `fs` features - the two modules this slice extends, so the build proves
  both new APIs resolve and compile. `crypto` is enabled in slice 1, where verification is written.
- `Cargo.lock` is updated and committed; CI already runs `--locked`.

## Capabilities

### New Capabilities

None. This slice creates no user-observable behaviour in Open Photo AI. Its output is a dependency
that can do two things it could not do before, plus the pin that reaches it; no function is added to
`opai`, and neither front end changes. `.openspec.yaml` therefore sets `skip_specs: true`.

The two behaviours this slice enables - a download that invalidates a stale partial, and an
extraction that reports progress - *are* specifiable, and they will be specified in
`add-init-1-runtime`, where `opai::initialize()` makes them observable from this application and a
requirement can actually be verified against it. Writing them here would describe `rust-sak`'s
behaviour rather than OPAI's, and would then be duplicated by the slice that owns them.

### Modified Capabilities

None. `openspec/specs/` is empty.

## Impact

**Crates touched:** `opai` (manifest only - one dependency and its features; no source file changes).
`gui` and `cli` are untouched. No logic is placed in a front end, because no logic is placed anywhere
in this repository by this slice.

**`rust-sak`:** this slice *is* the `rust-sak` work, so the usual "what did you find in it" answer is
inverted. What it already provides and is **not** rebuilt here: the retrying reqwest client, the
Fibonacci backoff, the `Range` resume, the SHA-256 helpers, the config-directory resolution, and the
hardened cap-std-anchored extractor with its name validation, symlink re-resolution and declared-size
limits. Both changes are additive extensions to those, sharing their existing code paths - in
particular the extractor's security guards are not touched, re-derived or forked.

**Dependency distribution - confirmed, not assumed.** `rust-sak` is **not published on crates.io**
(`https://index.crates.io/ru/st/rust-sak` returns 404) and has **no GitHub releases**
(`gh api repos/vegidio/rust-sak/releases` returns 0), despite its README instructing `cargo add
rust-sak`. Its single tag `26.7.0` predates the current `HEAD` by several commits. So the dependency
must be a git dependency pinned to a tag: a path dependency would break CI and anyone cloning this
repository alone, and a version dependency has nothing to resolve against. Publishing to crates.io
was considered and deferred - it is a one-way door on the crate name and belongs in a change about
`rust-sak`'s release process, not in one about two of its features.

**Parity divergence:** none in this slice - no OPAI behaviour exists to diverge from yet. One
divergence in *approach* is worth recording because it is deliberate and the Go code invites it:
where `internal/deps/archive.go` samples the destination directory's size on a 250 ms ticker to fake
extraction progress, this reports exact byte counts from inside the extraction loop. That is the
"collapses into a call to it" the Go comment anticipated, and it removes a goroutine, a per-tick
directory walk and a second read of the archive header.

**Not in scope:** any `opai` source code; `initialize()` and its signature; the pinned artifact table;
manifest bookkeeping; GPU detection; the `ort` crate; publishing `rust-sak` to crates.io; and the
`crypto` and `sysinfo` features, which are enabled by the slices that first need them.
