## 1. The identity becomes a string

- [x] 1.1 Replace the `App` enum with a string identity: delete `App`, `UnknownApp`, `App::ALL`,
      `App::as_str` and the `FromStr` parse, export `gui`, `cli` and `perf` as `&'static str`
      constants, and make `Holder::app` an owned string with `Holder` deriving `Clone` rather than
      `Copy`. Verify `Holder`'s rendering test still produces `gui, pid 4821` and that `InitError`
      still compiles unchanged (D1, D4, D5).
- [x] 1.2 Add the identity's validation — non-empty, no whitespace, no control characters — and its
      rejection, carrying the rejected text and a reason in the shape `InitError::InvalidName`
      already uses. Verify by test that a space, an empty string, a tab and a line break are each
      refused with their reason, and that `gui`, `cli`, `perf` and a reverse-DNS application name are
      each accepted (D3).
- [x] 1.3 Carry the string through the claim: `instance::claim`, `acquire`, `OwnedLock::record` and
      `read_holder`. Verify the existing suite still passes — including the re-executed child
      process tests — and add a round-trip test that an accepted identity is the exact text a refusal
      names (D3).
- [x] 1.4 Verify by test that `read_holder`'s degradation is unchanged: a record that is absent,
      empty, truncated, or carries a third field still yields a refusal naming no holder, and never a
      grant. This is the property D3 moved the validation in order to preserve, so it is checked
      rather than assumed.

## 2. The options bundle

- [x] 2.1 Add `InitOptions` with `app` and `on_progress`, a `Default`, and a written-out `Debug` that
      reports whether a progress handler was registered rather than trying to print one — the shape
      `ProcessOptions` already uses. Verify `..Default::default()` sets one field without naming the
      others, in a doctest as `ProcessOptions` has (D2).
- [x] 2.2 Change `Opai::initialize` to `(name, Option<InitOptions>)` and resolve an undeclared
      identity to the application name the caller passed. Verify by test that a call with `None`
      records and logs under that name, and that a declared identity wins over it (D2).
- [x] 2.3 Change `logging::init` and the record formatter to take the identity as a string. Verify
      the existing formatter tests still render `app=gui`, `app=cli` and `app=perf` unchanged, and
      add one for an embedder's own identity.

## 3. Front ends and surface

- [x] 3.1 Update `gui` and `cli` to pass the exported constants at both of their call sites, leaving
      the ordering of `logging::init` against the single-instance plugin exactly as it is. Verify
      both crates build and that `cargo test` passes across the workspace.
- [x] 3.2 Update `crates/opai/tests/public_api.rs`: drop `App` and `UnknownApp`, name `InitOptions`
      and the three constants, and re-pin `Opai::initialize` and `logging::init` through function
      pointers as that file already does. Verify it compiles and passes from outside the crate.
- [x] 3.3 Rewrite the doc comments that argue for the closed enum — `app.rs`'s header and the
      paragraph beginning *"Closed rather than a `&str`"* — so they describe the identity that now
      exists rather than the one that did. Verify by grep that no comment still claims there are
      exactly three possible applications.
- [x] 3.4 Run `cargo fmt --check`, `cargo clippy` and `cargo test` and verify all three are clean.
