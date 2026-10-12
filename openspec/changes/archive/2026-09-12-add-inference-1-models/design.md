## Slices

Running an image through a chain of models estimates far past 15 tasks. It is divided into six
changes, each leaving the project buildable and testable on its own. **This change is slice 1.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-inference-1-models` | A named model's files on disk, verified against the published manifest | the initialization series | **current** |
| 2 | `add-inference-2-providers` | The `ExecutionProvider` type including `Auto`, per-provider default options, and the per-model profile that overrides them | 1 | pending |
| 3 | `add-inference-3-sessions` | Session construction, the in-memory session cache with its 30-minute idle TTL, and CPU fallback | 2 | pending |
| 4 | `add-inference-4-tiling` | The shared inference primitives: the tile grid, CHW tensor conversion, seam blending | none (parallel with 1-3) | pending |
| 5 | `add-inference-5-kyoto` | The upscale pipeline and the public chain entry point, with progress, cancellation and `OutputDepth` — Kyoto runs end to end | 3, 4 | pending |
| 6 | `add-inference-6-cache` | The 24-hour per-step image cache, keyed on the input identity plus the operations applied | 5 | pending |

Slice 4 depends on nothing in 1-3 and can be implemented alongside them. Everything else is a
straight chain.

Deferred past the series, each to its own change when something needs it: the model memory budget
with LRU eviction (wanted once a multi-gigabyte model such as Osaka exists, not before); alpha
preservation through inference; and the remaining seven families' pipelines.

## Context

`models/` already resolves an operation to the artifacts it needs —
`Upscale::required_artifacts()` returns `up_kyoto_4x_fp32`, `up_kyoto_2x_fp32` for an 8x Kyoto — and
`ArtifactId::as_str()` is the published file's stem. Nothing turns that name into a file.

`deps/` installs the runtime and the NVIDIA libraries and is close to what a model needs, but it was
written for release archives and two of its assumptions do not hold here:

- **`install` always expands.** Every source is `acquire`d and then `expand`ed unconditionally. A
  model is a bare `.onnx` with nothing to expand.
- **A dependency owns its directory.** `install` calls `manifest::empty_dir(dir)` or
  `old.remove(dir)` before writing, and `record_tree(dir)` after. The doc comment states the
  assumption outright: *"It is assumed to be exclusive."*

Its hashes are also pinned in source as `&'static str` (`deps/artifact.rs`), which cannot work for
models: the set is large, changes when new models are published, and the hashes live on Hugging
Face.

See proposal.md - Why for motivation.

## Goals / Non-Goals

**Goals:**

- An `ArtifactId` installs to a verified file on disk, or fails naming what was missing.
- Every install is verified. No path reaches a session build with unverified weights.
- The pipeline's earned behaviour is reused unchanged: the fingerprint skip, the resumable transfer,
  the one-retry hash recovery, the manifest bookkeeping.
- A first run with no reachable network still verifies every model published at build time.

**Non-Goals:**

- Any public API. The install path is `pub(crate)` until slice 5 gives it a caller.
- Anything about sessions, providers or memory. This slice moves bytes; it never opens a graph.
- Pre-fetching, batching or parallel installs. One model at a time, as Go does, and for the reason
  its `LoadSessions` comment gives: two concurrent installs sharing one progress reporter interleave
  rather than combine.
- Removing installed models. Nothing here deletes a model except the pipeline replacing one version
  of it with another.

## Decisions

### D1. A directory per model, not the flat `models/` the Go app uses

Each model installs into `models/<artifact-id>/`, holding its graph and any external-data blob.

The alternative — Go's flat `models/` — cannot use the install pipeline as written. `install`
empties the dependency's directory before writing, so installing `up_kyoto_4x_fp32` into a shared
directory would delete the `up_kyoto_2x_fp32` sitting beside it, which is the *other half of the
same 8x operation*. Reusing the pipeline unchanged is worth far more than matching a directory
layout, and the exclusivity assumption is load-bearing rather than incidental: it is what makes
replacing a version a replacement instead of a merge.

It also pre-empts a problem Go documents and cannot fix. Its provider caches drop engine and
MLProgram files loose among the weights, named after the graph inside the file rather than the file
itself, so *"nothing said whose cache was whose"* — Go's fix was a second directory per model
elsewhere. Here a model's cache is simply a sibling of its weights.

`Dependency.name` and `Dependency.dir` are `&'static str` today because every dependency is a
compile-time constant. A model's are derived from an `ArtifactId`, so both become owned. That is the
whole of the change to the type; `sources` already anticipates this case — its doc reads *"One for a
release archive; a model splits into several."*

### D2. Four steps to a hash, and no fifth

A model installs only against a known SHA-256. The hash is resolved in order:

```
  1. the live Hugging Face tree listing
  2. the copy cached from the last successful fetch
  3. the manifest compiled into the binary
  4. refuse, naming the artifact
```

Go has a fifth step — install unverified — and reaches it from an ordinary timeout. Its own comment
describes the consequence: a slow connection *"turned every expected hash into an empty string: the
whole session then downloaded models with nothing checking them, and said nothing about it."* Step 3
removes the case for every model that existed when the binary was built, which leaves refusal
covering only a model published afterwards on a machine that cannot reach the manifest.

**Refusing is also the more accurate failure.** A model with no manifest entry is one that is not
published, so the URL Go composes for it 404s anyway. Step 4 reports "no published hash for
`up_kyoto_4x_fp32`" instead of a transfer error against a URL that was never going to resolve.

**An entry with no LFS object id counts as no hash.** Hugging Face stores small files as ordinary
git blobs, where `lfs` is absent and Go's `file.LFS.OID` is `""`. Go admits that as a manifest entry
and builds a `Source` with an empty hash — a second silent unverified path, distinct from the one
above and with no warning at all. Here such an entry is dropped during parsing, so it falls through
to step 3 like any other missing entry.

### D3. One manifest type, one format, three sources

Steps 1-3 all deserialize into the same type through the same `serde` derive, and the cached file at
step 2 is a verbatim copy of what step 1 produced. So the compiled-in manifest at step 3 is the same
JSON, `include_str!`'d and parsed once on first use.

The alternative — generating a `.rs` file with a `const` array — saves one parse of a few hundred
entries, once per process, and costs a second representation of the same data that only the
generator can write. Not worth it.

**The generator is a script run by hand, not a `build.rs`.** A build script that fetched from Hugging
Face would put the network on the build path, break offline and sandboxed builds, and make the
binary's contents depend on when it was compiled rather than on what is checked in. The script is run
when models are published and its output is committed.

**Staleness is caught by a test, not by discipline.** Every artifact the compiled-in manifest names
must be a name the catalogue can produce, so an entry for a model this build cannot ask for fails the
suite. The converse does not hold and must not be asserted: `models/` can name artifacts that were
never published — a precision a variant does not ship — and those are exactly the names step 4 is
there to refuse.

**The generator filters to what the catalogue can name, and says what it dropped.** The published
directory carries `dt_losangeles_encoder_int8` and `dt_losangeles_decoder_int8`, which neither this
project nor the Go application names; that model is not used by this project and will not be. A
compiled-in fallback exists to let a machine with no network install the models this build can ask
for, so an artifact no build can ask for is dead weight in it. The generator therefore keeps only the
nameable entries and reports the rest, which puts the "a model was published that the catalogue does
not name" signal at the moment a human runs the generator rather than in a suite that would fail on a
model deliberately left unnamed. The assertion above keeps its teeth for the drift that *is* a bug: a
variant renamed in `models/` orphans a checked-in entry and fails the suite.

### D4. The manifest is fetched lazily and memoized for the process

`Opai::initialize` is untouched; the first install fetches. This follows from the memory budget being
deferred past the series (see Slices): Go fetches at startup because its registry budgets against
`EstimateModelBytes` before building a session, and nothing here reads a size before an install.

What it buys: `initialize` gains no network dependency, no timeout, and no new failure mode, and an
application that never runs inference never contacts Hugging Face. What it costs: the first install
pays one round trip, inside an operation already transferring tens of megabytes.

Held for the process behind a `OnceCell`-shaped async initializer so that two concurrent first
installs fetch once rather than twice, and write the step-2 cache once.

### D5. The install pipeline grows a branch, not a second pipeline

`install` gains one decision — whether the acquired source is expanded — driven by the descriptor
rather than by sniffing the file. Everything above and below it is shared: the fingerprint skip that
makes a launch with nothing to do cost only `symlink_metadata` per recorded file, the resume, the
single hash-mismatch retry from zero, the manifest written last so an interruption is never mistaken
for a finished install.

The alternative, a separate install path for models, duplicates all of that and gives the two copies
room to disagree about what a half-finished install looks like.

### D6. The tree listing is paginated properly, not bounded by a large `limit`

Go passes `?limit=1000` and warns that the endpoint pages at 50 by default, where *"silently
returning the first page would drop models off the manifest."* A bound that is merely large is a
bound that is eventually wrong, and under D2 its failure mode changes from "installs unverified" to
"refuses to install" — safer, still a bug, and one that would appear only for whichever models sorted
last.

The listing is followed to exhaustion via the endpoint's pagination, so the number of published
models is not a constant anywhere in this project.

**That pagination lives in a response header, and reading one is a `rust-sak` gap.** The Hugging
Face tree endpoint returns a bare JSON array and advertises the next page as
`Link: <url>; rel="next"`; there is no cursor in the body. `rust_sak::fetch` exposed only response
*bodies* — `text`, `json` and their `_with_options` forms — so following the pagination was not
expressible against it at all. The gap is general-purpose rather than specific to this project (RFC
8288 `Link` pagination is what GitHub and most paginated APIs use, and an `ETag` or a rate-limit
header is the same need), so it was closed **in `rust-sak`** rather than worked around here: a
`Response<T>` carrying the body with its status and headers, the four `*_response` methods that
return one, and `Response::link(rel)` for the RFC 8288 parse. `opai` then follows `link("next")`
until there is none.

This corrects proposal.md's original claim that this slice needed no `rust-sak` change; the
alternatives — using `reqwest` directly here, which bypasses `fetch`'s retries, backoff, proxy
handling and timeouts, or keeping a large `limit` and failing loudly on a full page, which is the
bound this decision rejects — were both worse. It is task 1.2a, and it is why 1.2 depends on a
`rust-sak` version bump.

### D7. A model install reports as its own `Dependency`

`progress::Dependency` is `#[non_exhaustive]` and names the four archive dependencies. It gains a
variant for a model, so a model install reports through the existing `Reporter` and `Progress`
unchanged.

The download/expansion split does not apply — there is no expansion — so a model's transfer owns the
whole 0-1 range rather than `DOWNLOAD_SHARE` of it. That falls out of the reporter already: an
install that never calls `extracting` and finishes with `finish()` lands on `1.0` from the transfer
alone.

This is distinct from the progress the *inference chain* reports in slice 5, which is a different
type for a different thing (an operation and its phase, not a dependency and its phase). Slice 5
carries the model download inside its own report; this slice reports only the install.

### D8. The blob beside the graph comes from the manifest, not from probing names

A model too large for the 2 GB protobuf limit keeps its weights in a separate file. Go finds it by
prefix over the manifest (`strings.HasPrefix(model.Name, id+".onnx")`), and separately probes three
naming conventions on disk when measuring a session.

The manifest is the right source and the only one used here: it already lists every published file
with its size and hash, so a prefix scan over it finds the blob *and* the hash needed to verify it.
Probing disk names would find a file it has no hash for, which under D2 is not installable anyway.

The prefix is `<id>.onnx` rather than `<id>`, which is what keeps a future `up_osaka_fp16_v2` out of
`up_osaka_fp16`'s file set.

### D9. Ownership and lifetime

Nothing in this slice owns a native resource. An install is a transfer plus filesystem bookkeeping;
the only shared state is the memoized manifest from D4, which is immutable once resolved and held
behind an `Arc`. The session ownership model the project conventions call for — `Arc<Mutex<Session>>`
with no lease refcounting, so a session cannot be freed under a live run — is slice 3's to describe,
because slice 3 is where the first session exists.

### D10. Module layout and errors

`deps/model/` holds what is specific to models — the artifact-to-sources resolution, the manifest
type and its three sources — while `deps/install.rs`, `deps/manifest.rs` and `deps/release.rs` are
extended in place rather than copied. Errors extend the existing `thiserror` enum in `error.rs`,
matching the project convention and `rust-sak`.

## Risks / Trade-offs

**The compiled-in manifest is stale by construction for models published after a release** →
Step 1 covers them whenever the network is reachable, which is the normal case. The uncovered case is
a *new* model on a machine that cannot reach Hugging Face, which refuses with an accurate message
rather than installing something unchecked.

**Refusing where Go installs unverified is a behaviour regression for anyone on a hostile network**
→ Accepted deliberately, and the trade is stated in proposal.md. The failure is loud, names the
artifact, and is recoverable by restoring network access; the alternative failure is silent and lands
on a binary loaded into the process.

**A directory per model diverges from the Go on-disk layout, so an existing Go installation's models
are not adopted** → No mitigation, and none wanted: this is a ground-up rewrite with its own config
directory, and the models re-download. Noted so it is not discovered as a bug.

**`Dependency.name`/`.dir` becoming owned touches the four existing archive descriptors** → They are
built from `const` rows through `Dependency::from_release_at`, so the change is confined to that
constructor and the struct; the pinned table stays `&'static`. The existing suite covers every pinned
platform's URL and destination, so a regression here fails loudly.

**Hugging Face changes its tree API's shape or pagination** → The parse is one small `serde` struct
over three fields, and the suite exercises it against a recorded response through the existing
`deps/test_server.rs` rather than against the live endpoint, so the failure is a test failure at
upgrade time rather than a runtime surprise.

## Migration Plan

No migration. Nothing is installed under `models/` today, so the first run creates the layout D1
describes. There is no rollback concern: reverting the change leaves a `models/` directory that
nothing reads.

## Validation

**Platforms:** the design is platform-independent — no execution provider, no native library, no
platform-specific path beyond `fs::mk_user_config_dir`, which the initialization series already
exercises on all three. The suite runs the install against the local test server on whichever runner
CI provides.

**Execution providers:** none are touched. Provider validation begins in slice 2.

**What is untested until slice 5:** that an installed model actually loads. This slice proves the
bytes match their published hash and nothing more; the first proof that `ort` can open one of these
files is the Kyoto pipeline.

## Open Questions

- **Whether the cached manifest at step 2 should carry an expiry.** It is a fallback for a failed
  fetch, so a stale copy is strictly better than none, and Go gives it no expiry. If model
  publication ever becomes frequent enough that a stale cache masks a new model for a user who *is*
  online, step 1 already covers them — so this only matters in a case that does not currently exist.
  Answerable later without changing the specs, the approach or the tasks.
