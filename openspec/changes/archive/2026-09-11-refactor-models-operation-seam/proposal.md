## Why

`models/mod.rs` opens by promising that "dispatch is an exhaustive match the compiler checks", and
the archived `add-models-2-enhancements` design chose eight operation types over one tagged shape on
exactly that ground: under a tagged shape "a family added later can fail to be handled".

The eight types exist. The match does not. Nothing in the crate is a value that is "an operation of
some family", so there is no place where forgetting a family is a compile error. All eight types
expose the same method set - `new`, `variant`, `precision`, `display_name`, `required_artifacts`,
`cache_tag`, and for six of them `params` - and no trait or enum names that set.

The consequence is measurable. Adding a ninth family today requires edits at **16 sites across five
files**, of which exactly one - `Family::prefix` - fails to compile if missed. The rest fail at test
time, and three of them are hardcoded integers (`checked == 40`, `checked == 5`) that a future author
will "fix" by bumping the literal. Every property that must hold *across* families - cache-tag
disjointness, artifact-name family correctness, catalogue completeness - is held by a test assertion
or a doc comment rather than by a type.

A second gap has the same root. `catalogue.rs` exists so that "a front end restates no model
vocabulary of its own"; its module doc cites the reference implementation's `min={0} max={300}` in a
React component as the failure it prevents. But the map from published catalogue data back to a
typed, runnable operation is an 80-line `#[cfg(test)]` helper. The round trip the catalogue exists to
guarantee is available to the test suite and to nobody else, so the first real consumer will write
its own 20-arm string map outside the crate - which is the very drift the module was built to stop.

## What Changes

- Add a public `Operation` enum to `models/`, with one arm per family holding that family's existing
  type, forwarding `family()`, `precision()`, `display_name()`, `required_artifacts()` and
  `cache_tag()` to the arm.
- Add `from_codename(&str, Precision) -> Option<Self>` to each of the eight variant enums, driven off
  the same `ALL` rows the catalogue is built from, plus an `Operation`-level constructor that turns a
  published catalogue row back into a runnable operation.
- Replace the `#[cfg(test)]` `build()` helper in `catalogue.rs` with calls to that public inverse,
  removing the third hand-maintained listing of all twenty variants and the two magic counters that
  stand in for completeness.
- Move the cross-family invariants from assertions over a hand-written list onto `Operation` and
  `Family::ALL`, so a ninth family is a compile error in the places that matter rather than a silently
  skipped test row.

**Not in scope, and deliberately so:** collapsing the seven near-identical family modules, or
`Scale`/`Strength`/`Bias`, into a macro or a generic. That was considered and rejected in
`refactor-models-family-layout` design.md, on the ground that the per-family doc comments carry most
of the reasoning in those modules and a macro would vaporise them. This change adds a seam *over* the
eight types and leaves all eight exactly as they are written.

## Impact

- **Crates:** `opai` only. No front-end code, so nothing to justify under the "logic belongs in
  `opai`" rule. `gui` and `cli` do not reference `models` today.
- **`rust-sak`:** nothing needed. This is domain vocabulary, not infrastructure - no hashing, HTTP,
  filesystem, image or cache work is involved, so no `rust-sak` module applies and none is duplicated.
- **Public surface:** additive. `Operation` and the `from_codename` constructors are new; no existing
  type, signature or re-export changes, so `tests/public_api.rs` gains assertions rather than edits.
- **Behavioural parity:** no divergence from the Go implementation. The Go app has no counterpart to
  this type - it dispatches on an operation-id string - and that difference is the point of the Rust
  design, already argued in the archived enhancement design.
- **Estimated tasks:** 11, within the 15-task limit. No slicing needed.
