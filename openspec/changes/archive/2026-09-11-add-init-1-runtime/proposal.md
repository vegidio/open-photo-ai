## Why

`opai` has a dependency and no behaviour. Slice 0 landed `rust-sak` 26.9.0 - a download that knows
what its partial file is of, and an extraction that reports progress - and pinned it into
`opai/Cargo.toml`, but nothing in this repository calls either API. The crate still exposes
`version()` and nothing else.

This slice turns that dependency into the first thing the application actually does: on first launch,
fetch the pinned ONNX Runtime for this platform, prove it is the published archive, expand it into
the user's config directory, and record every file it wrote - reporting progress across the whole
operation rather than going quiet for the minute the expansion takes. Nothing else in the project can
start until the runtime is on disk, so this is the slice every later one waits on.

## What Changes

Task estimate: **14**. The full `opai::initialize()` feature estimates well past 15 and is split into
four slices (see design.md - Slices); this proposal covers slice 1 only.

**In `../rust-sak` (a separate repository):**

- **`fetch::download` gains an opt-in content digest.** `RequestOptions::digest(DigestAlgorithm)`
  makes the transfer hash the bytes as they are written, and `Download::digest()` returns the hex
  digest once the transfer has completed successfully. A resumed transfer hashes the existing
  `.part` prefix before appending, so the digest always covers the whole artifact rather than the
  tail this attempt happened to carry. Additive: a download that does not ask for a digest behaves
  exactly as it does today, and `Download::join`/`track` keep their signatures.
- Released as a new CalVer tag, which this repository re-pins to.

**In `opai`:**

- **`Opai::initialize(name, on_progress)`** - the application's entry point, `async`, returning an
  owned handle rather than the free function slice 0's table sketched (see design.md - D1). It
  resolves the config directory for `name`, installs the ONNX Runtime into `<config>/runtime/`, and
  reports progress throughout.
- **The pinned artifact table** - the release tag, SHA-256, published size and shared-library file
  name for each platform the runtime is built for, in exactly one place, as in the Go original. A
  platform with no entry is an error naming the platform, never a silent unverified install.
- **The install pipeline** - fingerprint the dependency, skip when the recorded install is already
  current and intact, otherwise clear what the previous version left, download, verify against the
  pinned hash, expand, record every extracted file, and write the manifest last.
- **Manifest bookkeeping** - a versioned on-disk record of what a dependency installed, written
  atomically and only once every file is in place, so an interruption is always detected as
  mid-install rather than mistaken for a finished one. It is also the delete list that makes
  replacing a version a replacement instead of a merge.
- **Public progress types** - `Progress`, `Phase` and `Dependency`, spreading one 0-100% report
  across the transfer and the expansion that follows, throttled so a front end is not driven by the
  download loop.
- `opai/Cargo.toml` re-pins `rust-sak` to the new tag, adds its `crypto` feature, and takes
  `tokio`, `serde`, `serde_json` and `thiserror`.

## Capabilities

### New Capabilities

- `initialization`: what the application does before it can enhance anything - installing the pinned
  ONNX Runtime for the running platform into the user's config directory, verifying it against its
  published hash, keeping a record of what it installed so a version bump replaces rather than
  merges, and reporting progress across the download and the expansion. Slices 2 and 3 extend this
  same capability with the NVIDIA libraries and the ONNX environment.

This is also where slice 0's two `rust-sak` behaviours become specifiable, as that slice said they
would: a resumed transfer that is provably a resume of the right artifact, and an expansion that
reports progress, are requirements here because `initialize()` is what makes them observable from
this application.

### Modified Capabilities

None. `openspec/specs/` is empty - this is the project's first capability.

## Impact

**Crates touched:** `opai` only. `gui` and `cli` are untouched, and no logic is placed in a front
end - `initialize()` and everything under it is library code both front ends will call unchanged.

**Front ends are deliberately not wired up in this slice.** The GUI splash screen and the CLI's
progress bar are a parity gap this change knowingly leaves open: they need slices 2 and 3 as well to
show anything meaningful, and wiring a progress bar to an initialization that cannot yet start a
session would be scaffolding rebuilt twice. `initialize()` is exercised from tests in this slice.

**`rust-sak` - what was found before anything was proposed.** Every piece of infrastructure this
slice needs already exists there and is **not** rebuilt: `fetch` for the retrying, resumable,
progress-reporting transfer and the `.part` bookkeeping from slice 0; `fs::mk_user_config_dir` for
the config directory, including the path-segment validation the Go original had to write itself;
`fs::extract` for the hardened 7z expansion and its progress hook; `fs::list_path` for reading the
extracted tree back; `crypto::sha256_*` for the fingerprint and for the one verification path the
transfer cannot cover. The single gap is in-stream hashing during the download, which is why the
change above is proposed in `rust-sak` rather than worked around here - any caller downloading an
artifact with a published checksum has it, and it is not about photo editing.

**New third-party crates:** `tokio` (the async runtime `fetch` already requires, plus
`spawn_blocking` for the synchronous extraction), `serde` + `serde_json` (the manifest's on-disk
format, matching the Go layout), `thiserror` (the project's stated error convention). None of them
duplicates a `rust-sak` module.

**Parity divergences from the Go implementation, deliberate:**

- `Opai::initialize` returns an owned handle where `opai.Initialize` is a free function writing
  process globals. Structural, and it removes the `lifecycleMu`/`destroyed` flag pair that exists
  only because a `sync.Once` cannot be re-armed. See design.md - D1.
- Extraction progress is exact byte counts read from inside the extraction, not a 250 ms ticker
  sampling the destination directory - the collapse `internal/deps/archive.go` says it is waiting
  for.
- Transfer retries, backoff and stall bounding are `rust-sak`'s, not a local retry loop. The Go
  package's `retry.go` and its stall watchdog have no counterpart here.

**Not in scope:** GPU detection and the NVIDIA libraries (slice 2); the `ort` crate and the ONNX
environment (slice 3); models, the model manifest and its budgets; the image cache; the engine
cache and its invalidation; the legacy-layout pruning; the `SkipVerify` debug override; and any
`gui` or `cli` change.
