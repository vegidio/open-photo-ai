## 1. The fixed-canvas presentation, promoted

- [x] 1.1 Create `crates/opai/src/inference/present.rs` and declare it in `inference/mod.rs`. Move `Plan`, `plan`,
  `fit_rounded` and `scaled` out of `models/light_adjustment/process.rs` with their documentation transplanted
  rather than retyped — including the argument that `fit_rounded` is deliberately not
  `detection/newyork/input.rs`'s truncating `fit`. `Plan` and `plan` become `pub(crate)`; `fit_rounded` and
  `scaled` stay private (design D4). Verify `cargo build -p opai` succeeds with `process.rs` importing them.
- [x] 1.2 Move `presented` into `present.rs` as `pub(crate)`, taking `norm: Normalisation` in place of the `RANGE`
  it reads today (design D2), and keeping the resampled photograph as its return value (design D3). Update the one
  call site in `process.rs` to pass its own `RANGE`. Verify `cargo build -p opai` succeeds and `RANGE` is still
  declared in `process.rs`.

## 2. The per-run control blend, promoted

- [x] 2.1 Create `crates/opai/src/inference/mix.rs`, declare it in `inference/mod.rs`, and move `blended` into it
  as `pub(crate)` with its documentation. Leave `inference/blend.rs` and `inference/canvas.rs` untouched — the
  reason is design D1, and a diff touching either means the wrong file was chosen. Verify `cargo build -p opai`
  succeeds and `git status` shows neither file modified.

## 3. The tests, relocated by what they assert

- [x] 3.1 Move the 7 plan tests and the 4 presentation tests, with the `truncating_fit`, `photograph` and `red`
  helpers, into `present.rs`'s own `mod tests` (design D5). Leave
  `the_graph_is_fed_the_unit_range_rather_than_the_signed_one_face_recovery_uses` in `process.rs`, where it asserts
  light adjustment's own choice. Verify `cargo test -p opai inference::present` runs all eleven and they pass.
- [x] 3.2 Add a test to `present.rs` that `presented` honours whichever `Normalisation` it is handed, writing
  different tensors for two ranges over one photograph — the property a shared function owes its callers, which no
  existing test covers now that the constant no longer lives inside it. Verify it passes and fails if the parameter
  is ignored.
- [x] 3.3 Move the 5 blend tests and the `flat` helper into `mix.rs`'s own `mod tests`. Verify
  `cargo test -p opai inference::mix` runs all five and they pass.

## 4. What stays, and the proof that this was a move

- [x] 4.1 Update `models/light_adjustment/process.rs`: its imports, and its module doc, which presents the plan,
  the presentation and the blend as this family's contract and must now name only `corrected`, `gain`, `RANGE`,
  `STEPS` and `Adjust`. Verify the 5 gain-map tests and the whole 9-test fake-backend pipeline suite still pass
  **with no change but imports** — an assertion that needed editing means something was rewritten rather than
  moved, and the task stops there (design D5, risk 1).
- [x] 4.2 Update the two prose sites that describe the old tiers: `inference/mod.rs`'s opening enumeration of what
  the module holds, which gains the presentation and the control blend, and `models/mod.rs`'s tier description
  where it attributes these to light adjustment. Verify `cargo doc -p opai --no-deps` builds with no broken intra-doc
  links.

## 5. Toolchain

- [x] 5.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, and confirm
  `crates/opai/tests/public_api.rs` needed no change — the mechanical check that this slice is invisible from
  outside the crate (design D4).
- [x] 5.2 Run the light adjustment live check by hand on macOS/CoreML —
  `cargo test -p opai models::light_adjustment::live -- --ignored --nocapture --test-threads=1` — against the real
  Paris and Lyon weights. It is the only thing that proves a real graph still renders a plausible photograph
  through the new module path, because every failure mode here produces an image and no diagnostic (design D6).
