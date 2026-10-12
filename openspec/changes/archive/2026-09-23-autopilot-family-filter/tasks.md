## 1. The scope

- [x] 1.1 Add the `Copy` scope of design D2 to `autopilot`. It is built from `Option<&[Family]>` and has
      `includes(Family)`, `needs_pass()`, `needs_blocks()` and `needs_face()`. Also add a way to tell "not narrowed"
      apart from "narrowed", for the log. Verify with unit tests showing:
      - `None` includes every family;
      - `Some(&[])` and `Some(&[Detection])` include no suggestible family and need no read;
      - duplicates are harmless;
      - each `needs_*` is true exactly when one of its families (the table in design Context) is included.

## 2. Into the analysis

- [x] 2.1 Thread the scope through `autopilot::suggest` (design D3 and D4). Return early and complete when nothing
      suggestible was asked for. Run `geometry` only for `Upscale`. Have `PixelSignals::read` take the scope and read
      the pass and the blocks only when needed, spawning no blocking task when neither is. Run `face` only when
      `needs_face()`. Filter the final list by `includes` once, just before the sort. Verify: the existing autopilot
      tests pass with `None`, unchanged.
- [x] 2.2 Add tests for each scenario in the delta spec, using the existing fixtures and the `Fake` backend:
      - `[Upscale, Colorization]` on a picture that calls for more returns only those two, in order, with no
        detection acquired;
      - a face model that fails to open leaves `incomplete` as `None` when face recovery was not asked for, and sets it
        when it was;
      - `[Sharpen]` on a blurred, grainy fixture returns sharpen and not denoise;
      - `Some(&[])` and `Some(&[Detection])` read nothing and are complete;
      - a narrowed analysis cancelled during the face signal returns `Cancelled`.
      Also add the design's Risks guard: for each suggestible family in `Family::ALL`, an analysis narrowed to it
      alone suggests it on a fixture that calls for it. Verify with `cargo test -p opai autopilot`.
- [x] 2.3 Add the `asked` field to `record` (design D5). Verify with a `logging::records_of` test: a narrowed analysis
      logs the requested families at `info`, and one that is not narrowed logs `asked = "all"`.

## 3. Public API

- [x] 3.1 Add the `families` parameter to `Opai::suggest` in `lib.rs` (design D1). Update the doc example to pass
      `None`, and document what `None`, a set, an empty set and `Detection` each mean, and that an unrequested signal
      never makes the result incomplete. Update the function-pointer pin in `tests/public_api.rs` to the new
      signature, and add the new parameter to the module diagram in `autopilot/mod.rs`. Verify:
      `cargo test -p opai --doc` and `cargo test -p opai --test public_api` pass.

## 4. Finish

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and fix
      anything they report. Verify: all three pass clean.
