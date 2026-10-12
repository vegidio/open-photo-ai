## Why

`opai::initialize()` shipped across four slices and works. This change does not add behaviour — it
removes three places where the implementation holds an invariant by hand that the types could hold
for it, and one where the crate breaks a promise its own documentation makes.

A `/simplify` review over the whole codebase found these. Everything it found that was mechanical
has already been applied directly; what is left is the part that changes shapes rather than lines,
which is why it is a change of its own rather than a cleanup commit.

The three hand-held invariants all trace to the same gap: **the release table records what a
dependency *is* named, where it installs, and what it reports as — but not what it is *for*.**
Without that, the code compensates three times over:

- `Opai::initialize` takes the runtime as `plan[0]`, with a comment explaining that `select_at`
  builds it first. A positional invariant spanning two functions, enforced by prose.
- `select_at` sets `providers.cuda = true` and `providers.tensorrt = true` by hand next to each
  `plan.push(..)`. The install plan and the provider report are two representations of one decision,
  kept in agreement manually — which is why `lib.rs` has to *assert* they agree in a test instead of
  the disagreement being unrepresentable.
- `Artifact::lib` uses `""` as "this dependency has no loadable library", and `runtime::library_path`
  turns that into `InitError::UnsupportedPlatform` — an error that is factually wrong. The platform
  *is* supported; CUDA simply is not a single library. `runtime.rs` documents that this branch is
  unreachable for the only dependency that ever reaches it.

Separately, `initialize`'s own documentation promises "Every one of them names the URL of the
artifact that failed". For `InitError::Io` that is not true: it is a catch-all carrying a bare
`std::io::Error` for manifest writes, archive deletion, directory creation and the fallback hash —
and, through `io::Error::other(JoinError)`, for the Tokio runtime shutting down, which is not an
I/O error at all. What a user sees today is `permission denied (os error 13)` with no path.

## What Changes

Task estimate: **13**. This is a refactor of existing behaviour, not a slice of a larger feature —
it stands alone and is not part of the `add-init-*` series. Nothing in `../rust-sak` changes.

**What `rust-sak` covers here: nothing, and nothing should.** This change touches only `opai`'s own
domain types — the pinned release table, the install descriptor, and this crate's error enum. None
of `crypto`, `fetch`, `fs`, `image`, `memo`, `o11y` or `sysinfo` has an opinion about what an ONNX
execution provider is. No new third-party crate is introduced.

**In `opai`:**

- **A `role` on the pinned row.** `Release`/`Dependency` gain what they are for: minimally
  `lib: Option<&'static str>` (the loadable library, `None` for a directory of several hundred files)
  and `provides: Option<Provider>` (the execution provider installing it unlocks, `None` for the
  runtime itself, which is not a provider).
- **`select_at` returns a `Plan`, not a `Vec`.** With the runtime as a named field rather than
  element zero, and the GPU libraries as the rest. `plan[0]` stops being an invariant anybody has to
  know.
- **`SupportedProviders` derived from the plan rather than set beside it.** The report becomes a
  function of what was actually selected, so the two cannot drift and the test asserting they agree
  becomes a statement about the derivation rather than a tripwire.
- **`library_path` takes something that has a library.** With `lib` an `Option`, the empty-string
  branch and the bogus `UnsupportedPlatform` both go.
- **`Platform` as a typed key.** `Artifact::platform` is a free-form string that must match what
  `asset_platform` spells, and nothing connects the two: a typo (`win_amd64`) is not a compile error
  and silently makes that platform unpublished. A guard test for this landed with the cleanup pass;
  this change replaces it with a small `Platform { os, arch }` pair whose `const fn` spells the
  segment, making an unspellable platform a compile error instead.
- **`InitError::Io` carrying its path**, filled in at each construction site, plus a distinct variant
  for the `JoinError` non-panic case — the one failure here a caller can genuinely act on
  differently, since it means shutdown is in progress rather than the install being broken.

**No behaviour changes** for any input that succeeds today. What changes for failures is that the
message names the file. `InitError` is `#[non_exhaustive]`, so adding variants is not a breaking
change for front ends; dropping `#[from] std::io::Error` means roughly ten `?` sites need an
explicit mapping, which is the point — each one knows the path it was working on.

**Deliberate divergence from the Go implementation:** none. The Go app had neither a provider report
derived from its install plan nor typed error context; this is Rust doing what Rust can do, which the
project context explicitly asks for over mirroring the original's structure.

## Not in this change

`libpath::apply`'s Linux re-exec is left exactly as it is. There is an argument that it could be
replaced outright — see design D3, which records it as an open question for D6 rather than a
proposal. It is unverified, and it is not a refactor: it would change how the application starts.
