## 1. The rename

- [x] 1.1 `git mv crates/opai/src/image/source.rs crates/opai/src/image/picture.rs` and rename the type to `Picture`, updating the module declaration and re-export in `image/mod.rs` and `lib.rs` (design.md - D1). Verify `cargo build -p opai` succeeds and `grep -r SourceImage crates/` returns nothing.
- [x] 1.2 Update the remaining call sites in `image/load.rs` and `image/save.rs`, changing the identifier and nothing else (design.md - D3). Verify `cargo test -p opai image::` passes with no test body altered beyond the type's name — if a test needs more than that, stop: the change has done something it said it would not.

## 2. The identity documentation

- [x] 2.1 Reword the type's identity documentation to separate the invariant that holds for every value — the identity describes *these* pixels and is never carried onto different ones — from where it comes from per producer: the XXH3-64 of the whole file when loaded, and a value derived from an input identity plus what was done to the pixels when computed (design.md - D2). Verify `cargo doc -p opai --no-deps` emits no warnings and that the wording no longer asserts file-byte equality as a property of the type.
- [x] 2.2 Check the module documentation in `image/mod.rs` and the crate documentation in `lib.rs` for the same over-generalisation and correct it where it appears; verify by reading the rendered `cargo doc` output for the module and the crate root.

## 3. Public surface and validation

- [x] 3.1 Update `crates/opai/tests/public_api.rs` to name `Picture`, which is the check that a front end can still reach the type through the public surface alone. Verify the test compiles and passes.
- [x] 3.2 Run `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test -p opai` on all three platforms; verify the suite is green with no test newly `#[ignore]`d and no test body changed beyond the identifier, which is the evidence that this was a rename and not a behaviour change.
