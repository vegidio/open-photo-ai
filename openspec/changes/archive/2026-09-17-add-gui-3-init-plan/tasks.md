## 1. The plan report

- [x] 1.1 Add `PlannedDependency` (the `Dependency` it names and the `u64` sum of its sources' published sizes) and the `OnPlan` callback alias to `src/progress.rs`, re-exported from `lib.rs` beside `OnProgress`; verify `cargo build -p opai` succeeds.
- [x] 1.2 Add `InitOptions::on_plan` and its line in the hand-written `Debug` impl, documenting it as fired once before anything is transferred; verify the `InitOptions` doc-test still compiles with `cargo test -p opai --doc`.
- [x] 1.3 Build the rows from `Plan` and fire `on_plan` in `Opai::initialize`, between `Self::select` and `Self::install`; verify with tests that the report names the runtime alone for an adapter list with no NVIDIA card, and the runtime, CUDA, cuDNN and TensorRT in install order for a synthetic RTX adapter list, each carrying its published size.
- [x] 1.4 Verify with tests that exactly one plan report is made per initialization, that it arrives before the first progress report, and that an initialization with no plan recipient behaves identically and reports nothing.
- [x] 1.5 Verify with a test that an initialization refused with `InitError::AlreadyRunning` reports no plan, because nothing was probed.

## 2. The already-installed outcome

- [x] 2.1 Change `deps::install::install` to return an outcome distinguishing an install that ran from one that was already current, decided at the existing early return, and update the model call site in `deps/model/mod.rs` to discard it; verify `cargo build -p opai` succeeds and the existing install tests still pass.
- [x] 2.2 Add `Phase::AlreadyInstalled` and the `Reporter` method that emits one report carrying `bytes` of 0, `total` of the published size and a fraction of exactly `1.0`, and emit it from `install_one` on the already-current outcome; verify with a test that a second launch reports exactly one already-in-place report per dependency and no download or expansion phase.
- [x] 2.3 Verify with tests that the runtime's already-in-place report precedes the first download report for a GPU library that does need work, and that the model install path still reports nothing at all when its install is already current.

## 3. Verification

- [x] 3.1 Update `crates/opai/tests/public_api.rs` to pin the new public exports - `PlannedDependency`, `OnPlan`, the `InitOptions` field and the `Phase` variant; verify `cargo test -p opai --test public_api` passes.
- [x] 3.2 Run `cargo fmt --check`, `cargo clippy` and the full test suite, and verify all three are clean.
