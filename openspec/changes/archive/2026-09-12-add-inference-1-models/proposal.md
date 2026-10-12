## Why

`opai` can name every model it will ever run and cannot fetch one. `models/` resolves an operation
to the artifacts it needs — `Upscale::required_artifacts()` answers `up_kyoto_4x_fp32`,
`up_kyoto_2x_fp32` for an 8x Kyoto — and nothing in this repository can turn either of those names
into a file on disk. `deps/` installs the ONNX Runtime and the NVIDIA libraries from pinned GitHub
release assets; the models are published somewhere else, under different rules, and no code here
reaches them.

Inference is blocked on exactly that gap. This slice closes it: given an `ArtifactId`, put the
verified `.onnx` on disk, and prove it is the published one. Nothing runs yet — the first inference
is slice 5 — but every slice after this one assumes the weights are reachable.

## What Changes

Task estimate: **15**. The inference feature estimates far past 15 and is split into six slices
(see design.md - Slices); this proposal covers slice 1 only.

**In `opai`:**

- **Model artifact resolution.** An `ArtifactId` resolves to the files that back it — the graph, plus
  the external-data blob a model too large for the 2 GB protobuf limit keeps beside it — and to the
  directory it installs into. Kyoto has no blob; Osaka's is 6.8 GB, and a model that charged only for
  its graph would look free.
- **The published model manifest**, giving each file its size and its SHA-256. It is fetched from the
  Hugging Face tree listing for the models directory, where the Git LFS object id *is* the SHA-256 of
  the contents and so compares directly against a hash computed over the download.
- **A four-step resolution for that manifest**: the live listing, then the copy cached from the last
  successful fetch, then a manifest compiled into the binary, then refusal. **There is no unverified
  install path.** See the divergence note below — this is the one place this slice deliberately
  refuses to do what the Go application does.
- **A generator for the compiled-in manifest**, plus the data file it produces, checked in. The
  hashes are content hashes of published files and never change, so the compiled-in copy stays
  correct for every model that existed when the binary was built; the live fetch exists to cover
  models published after it.
- **Lazy fetching.** `Opai::initialize` gains nothing and keeps its current failure modes: the
  manifest is fetched the first time a model is actually installed, inside an operation that is
  already transferring tens of megabytes.
- **A no-extraction path through the existing install pipeline.** `deps::install` currently expands
  every source it acquires; a model is a bare file and has nothing to expand. Everything else the
  pipeline earned — the fingerprint skip, the manifest bookkeeping, the resume, the one-retry hash
  recovery — applies unchanged.
- **A directory per model** under `models/`, rather than the flat directory the Go application uses.
  The install pipeline assumes a dependency owns its directory exclusively, and it empties that
  directory when it replaces a version; installing `up_kyoto_4x_fp32` into a shared `models/` would
  delete `up_kyoto_2x_fp32` beside it.

**Not exposed publicly in this slice.** The install path is `pub(crate)` and is exercised through
tests against the local test server, the same way slice 1 of the initialization series was. It
becomes reachable in slice 5, when there is something to run.

## Capabilities

### New Capabilities

- `model-install`: getting a named model's files onto disk and proving they are the published ones —
  resolving an artifact name to the files behind it, obtaining the manifest those files are verified
  against and what happens when it cannot be obtained, installing without extraction, keeping each
  model's files separable from every other model's, and reporting progress across the transfer.

  Distinct from both existing neighbours, and neither is widened. `initialization` scopes itself to
  "everything the application has to put on disk **before it can run a model**" — the runtime and the
  NVIDIA libraries, installed eagerly at startup from pinned release archives. `models` names and
  validates operations and resolves one to the artifacts it needs; it knows what a file is called and
  nothing about where it comes from. This capability is the seam between them.

### Modified Capabilities

None. `initialization` keeps its requirements exactly: the manifest is fetched lazily, so
`Opai::initialize` neither gains a network dependency nor a new way to fail.

## Impact

**Crates touched:** `opai` only. `gui` and `cli` are untouched, and no logic is placed in a front
end — everything here is library code both front ends reach through later slices.

**`rust-sak` - what was found before anything was proposed.** Almost every piece of infrastructure
this slice needs already exists there and is **not** rebuilt: `fetch` for the retrying, resumable,
progress-reporting transfer and its in-stream digest, already used by `deps/install.rs`; `crypto` for
the SHA-256 comparison on the path the transfer's own digest cannot cover; `fs::mk_user_config_dir`
for the models directory. The model transfer is the same shape as the archive transfer, and the JSON
parsing is `serde_json`, already a dependency.

**The one gap is reading a response header**, found while implementing rather than while proposing —
this section originally claimed there was none. The Hugging Face tree endpoint advertises its next
page as `Link: <url>; rel="next"` and puts no cursor in the body, while `rust_sak::fetch` returned
only bodies, so following the listing to exhaustion (design.md - D6) could not be expressed against
it. The need is general-purpose, so it is closed **in `rust-sak`** — a `Response<T>` carrying body,
status and headers, the four `*_response` methods, and `Response::link(rel)` for the RFC 8288 parse —
and consumed here through a version bump. That is explicit work with a task of its own (1.2a), and
the `rust-sak` repository is only pushed to with the go-ahead.

`fs::extract` is the one module deliberately *not* used, because a bare `.onnx` has nothing to
expand — which is the branch being added to the pipeline rather than a gap in the module.

**New third-party crates:** none.

**Parity divergences from the Go implementation, deliberate:**

- **No unverified install.** Go's `deps.ModelDependency` falls through to
  `Source{URL: ..., Sha256: ""}` when the manifest has no entry, logging "model is not in the remote
  manifest; downloading it unverified". The failure lands on a multi-gigabyte binary that is then
  loaded into the process, and it is reachable from an ordinary five-second timeout on a slow
  connection — Go's own comment says that turned "every expected hash into an empty string" for a
  whole session. The compiled-in manifest removes the case entirely for every model published at
  build time, so refusing the remainder costs nothing a user is likely to hit.
- **The manifest is fetched lazily**, not during `Initialize`. Go needs it at startup because its
  model registry budgets against `EstimateModelBytes` before a session is built; this project defers
  the memory budget (see design.md - Slices), so nothing needs the sizes before the first install.
- **A directory per model**, where Go installs every model flat into `models/`. Structural, and it
  also makes a model's execution-provider cache a sibling of its weights rather than a set of loose
  files the Go comment describes as impossible to attribute.

**Not in scope:** execution providers and session options (slice 2); session construction and its
cache (slice 3); tiling, tensor conversion and blending (slice 4); the Kyoto pipeline and the public
inference entry point (slice 5); the per-operation image cache (slice 6). Also out: the
execution-provider engine cache and its invalidation when weights are replaced, which belongs with
the slice that first compiles one; the model memory budget; and any `gui` or `cli` change.
