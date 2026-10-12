## 1. opai: the shared step

- [x] 1.1 Add `crates/opai/src/inference/faces.rs` with `ChainFaces`, `FacedChain`, `default_face_selection` and the pure fill function (design D1, D2). Re-export them from `lib.rs`. Verify with `cargo check -p opai` and rustdoc on every public item.
- [x] 1.2 Unit-test the pure fill next to it, covering every `face-recovery` delta scenario that needs no detector: no recovery means NotAsked; found faces are filtered per recovery index in detector order; two recoveries are filled independently; non-recovery operations are untouched and input order is kept; nobody found gives Found(empty); a failure leaves recoveries empty and reports Failed with the error's text; Cancelled gives Err(Cancelled); the default selection keeps only `restorable` faces. Verify with `cargo test -p opai inference::faces`.
- [x] 1.3 Add `Opai::find_chain_faces`, built internally over a detect future so its "skip when not asked" and "exactly one detection" behaviour is tested without a model. The detection is `Detection::for_face_recovery()` through `Opai::execute`, with the caller's `ExecuteOptions` passed through. Verify with tests asserting zero detections for a chain with no recovery, one detection for two recoveries, and options (cancel token) passed through.
- [x] 1.4 Add the step to the "Running an enhancement" section of the crate docs in `lib.rs`, and verify with `cargo doc -p opai --no-deps` building without broken intra-doc links.

## 2. gui: switch to the shared step

- [x] 2.1 Replace `Enhancer::detect` with `Enhancer::find_chain_faces` in `enhance/run.rs`. The `Opai` impl delegates to the library, and the default impl answers via the pure fill with `Ok(Faces::empty())` (design D3). Verify the GUI crate compiles.
- [x] 2.2 Make `prepare_chain` call the seam with a selection built from each `Requested::faces` (`FaceChoice::keeps`, default when absent), and convert `opai::ChainFaces` where `enhance_with` matches on it. Verify the existing `enhance_with` Found/Failed tests pass unchanged.
- [x] 2.3 Remove `for_chain` and the GUI's `ChainFaces` from `faces.rs`, keep `FaceChoice`, `face_key`, `DetectedFace` and `detect_faces`, and update module docs that reference `for_chain`. Verify `rg 'for_chain' crates/` finds nothing.
- [x] 2.4 Port the test fakes that overrode `detect` (`FindingOne` in `enhance/run.rs`, `StoppedWhileDetecting` in `export/run.rs`) to override `find_chain_faces` via the pure fill. Drop the `for_chain` tests whose assertions now live in `opai` (1.2) and keep the `FaceChoice` key tests. Verify with `cargo test -p gui`.

## 3. Verification

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`. All clean and passing.
- [x] 3.2 GUI regression on macOS: open `fixtures/test.dat`, add face recovery plus upscale, and confirm that faces are found and counted in the list, the restored result renders, export writes a file, and progress shows the detection at the head of the bar, on both CoreML and CPU. Same behaviour as before the change.
