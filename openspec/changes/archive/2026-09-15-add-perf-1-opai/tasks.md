## 1. Releasing every resident session

- [x] 1.1 Make `Opai::release_sessions` public and drop the `#[expect(dead_code)]` from it and from
      `SessionCache::clear`, rewriting both doc comments and the `sessions/mod.rs` note that says no
      caller reaches it — they describe a state this slice ends. Verify `cargo test` still passes
      `releasing_sessions_says_how_many_went_and_why` and the `lib.rs` record test unchanged, and
      that the build is warning-free without the allowances (D7).

## 2. The provider report

- [x] 2.1 Add the run's result bundle and its provider report type — the requested provider beside
      the ordered set of providers sessions were built on — and export both from the crate root.
      Verify they are reachable from `tests/public_api.rs` and that the report's `Debug` renders both
      halves (D2).
- [x] 2.2 Fold the report out of `held` at the end of `chain::run` and return it beside the pixels.
      Verify with fake-backend tests for a run served on the provider asked for, a run downgraded to
      the CPU, and a two-operation chain downgraded for its second model only — the last reporting
      both providers, and a chain taking four handles on one provider reporting it once (D1).
- [x] 2.3 Have `Opai::process` return the bundle composed from that report, and update the existing
      `chain.rs` tests to the new return type. Verify a run whose every operation is served from the
      store, and an empty chain, both report the requested provider with an empty account of what ran
      — not the requested provider as though it had been used (D1).
- [x] 2.4 Verify by test that the result's identity is unchanged by this slice: the same image and
      chain run on two different providers carry the same identity, so nothing keyed on it moves
      (image-enhancement spec, final scenario).

## 3. Trusting the model files already on disk

- [x] 3.1 Add the two-valued model-trust type, take it as the `models` field of `InitOptions`, store it
      on `Inner`, and carry it to the model installer through `Sessions`. Verify a process that
      declares ordinary handling behaves exactly as today across the existing `deps` and `sessions`
      suites (D3).
- [x] 3.2 Carry the trust on `deps::release::Dependency`, set only where a model's descriptor is
      built in `deps/model/descriptor.rs`. Verify by test that a descriptor built from a pinned
      release row — the runtime and each GPU library — is never trusted, whatever the process
      declared (D8).
- [x] 3.3 Add the trusted branch to `install_inner`: the identity stamped from each source's name,
      size and mtime instead of the published fingerprint; the `intact` check skipped; the record
      written with an empty file list; the derived directories emptied when the stamp moves; and a
      `warn` naming the model each time the path is taken. Verify the record's level and fields with
      the `logging::records_of` helper the instrumentation slice left (D5, D6).
- [x] 3.4 Verify by test against the local test server that a model whose every file is present is
      used as it is under a declaration of trust — the files untouched, no request made, even where
      the published hash differs — and that a model with one file missing is installed from the
      published sources and verified instead.
- [x] 3.5 Verify by test that re-acquiring a trusted model whose files have not changed transfers
      nothing and leaves the derived engine directory in place, while replacing one of its files
      moves the stamp and empties that directory (D5).
- [x] 3.6 Verify by test that trust does not escape its process: a model used on trust is reinstalled
      from the published sources and verified when acquired by a handle that declared ordinary
      handling, and a downloaded file whose hash does not match still fails the install under a
      declaration of trust, exactly as it does without one (D4, D6).

## 4. Surface, records and checks

- [x] 4.1 Update `tests/public_api.rs` for the three changed or added surfaces — `initialize`'s new
      argument, `process`'s return type, and `release_sessions` — pinning each through a function
      pointer as that file already does, and naming the new types in its import list. Verify it
      compiles and passes from outside the crate.
- [x] 4.2 Rewrite the comments elsewhere in `opai` that describe the state this slice ends —
      `lib.rs`'s note that `process` discards what it ran on, and any remaining text predicting the
      first caller of the session release. Verify by grep that no comment still says a facility added
      here is unreachable.
- [x] 4.3 Run `cargo fmt --check`, `cargo clippy` and `cargo test` and verify all three are clean.
