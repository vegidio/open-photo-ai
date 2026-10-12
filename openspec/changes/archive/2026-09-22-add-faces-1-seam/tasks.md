## 1. The wire arm

- [x] 1.1 Add the `FaceRecovery { codename, precision, faces }` arm to `Requested` in
  `gui/src/enhance/operation.rs`, resolving through `VariantEntry::published(Family::FaceRecovery, ..)`
  with `ParameterValues::new().with_faces(..).with_fidelity(Fidelity::MAXIMUM)` (D7) — verify with
  tests that every published face-recovery model resolves at every precision it is published at, that
  Athens resolves to exactly what `FaceRecovery::athens(precision, faces, Fidelity::MAXIMUM)` builds
  and Santorini to `FaceRecovery::santorini(precision, faces)` with `fidelity: None`, and that an
  unknown codename is still refused by index

## 2. The detection command

- [x] 2.1 Write `gui/src/faces.rs` with the `detect_faces` command — resolve the identity through
  `Opened`, decode, apply `cropped` on a blocking thread, run `Detection::newyork(Fp32)` through
  `Opai::execute`, and answer the faces (D1, D2, D3) — verify with tests behind the same
  `Enhancer`-style seam `enhance/run.rs` uses that an unopened identity and an unreadable file are
  each refused before anything is run, and that the picture handed to the seam is the framed one
- [x] 2.2 Report the detection on the existing `enhance:progress` event under `Family::Detection`,
  reusing `enhance/progress.rs`'s `Reporting` and its thinning (D8) — verify with a test that a
  transfer is reported as `installing` with an `installFraction`, that the run name on every report
  is the one the command was given, and that the fraction reaches 1 exactly once
- [x] 2.3 Register `detect_faces` in `gui/src/lib.rs`'s `invoke_handler` and confirm the main
  window's permission grant is unchanged — verify the existing grant assertion test still passes

## 3. The frontend seam

- [x] 3.1 Write `frontend/ipc/faces.ts`: the `Face`, `Point` and `Rect` types in `opai`'s own
  snake_case shape with the five landmarks as a fixed-length tuple, and `detectFaces` minting its run
  name before the invoke and answering `{ run, done }` as `enhance` does (D6) — verify with tests that
  the command name and argument names match the Rust signature and that a refusal rejects rather than
  resolving empty
- [x] 3.2 Write `frontend/stores/faces.ts`: `Map<identity, { crop?, faces }>`, its setter, a reader
  that answers only for a matching framing, and `registerFileOwner` so a closed photograph releases
  its faces (D9) — verify with tests that a read at a different framing is a miss, that every write
  replaces the map, and that closing one photograph leaves the others' faces held
- [x] 3.3 Add `face_recovery` to `RUNNABLE` in `frontend/lib/enhancements.ts`, give `newOperation` its
  arm (the default model, no faces — D5), and add the family-to-chip-label mapping that names a
  `detection` report Face Recovery (D8) — verify with tests that the add menu now offers two
  enhancements in pipeline order and that the mapping answers a label for every family a report can
  carry

## 4. The run path

- [x] 4.1 Extend `frontend/hooks/useEnhancementRun.ts` to detect before enhancing when the stack
  carries a face recovery whose faces are not known for the framing in force, writing the store and
  injecting the faces into the operation it sends (D4, D5) — verify with tests that no run starts
  before the faces land, that a known framing skips the detection, that a framing change detects
  again, that a change while a detection is in flight asks nothing further and goes on drawing the
  one it is waiting for, and that an answer landing for a photograph that has since been closed is
  discarded while one for a photograph merely moved off is kept
- [x] 4.2 Make a failed detection raise `errors.faceDetectFailed`, record the photograph as having no
  faces, and let the chain run anyway (D10) — verify with tests that the other operations in the chain
  still run and that a detection finding no faces reports nothing to the user

## 5. The sidebar

- [x] 5.1 Write `frontend/features/enhancements/FaceRecoveryOptions.tsx` — the model tray inside the
  shared `OptionsPanel`, with no other control — and give `EnhancementRow`'s `options()` its
  `face_recovery` arm — verify with a test that the panel renders both models at both tiers and that
  choosing one replaces the operation on the stack
- [x] 5.2 Give `EnhancementRow`'s `info()` its `face_recovery` arm, composing
  `enhancements.infoFaces` with `enhancements.faces` — verify with tests that a photograph with two
  faces reads `Athens, 2 Faces, HD`, that a framing removing one reads `1 Face`, and that a photograph
  whose faces are not known yet is distinguishable from one where none were found

## 6. Checks

- [x] 6.1 Verify by hand on macOS, with `pnpm dev` and `target/debug/gui`: add Face Recovery to a
  photograph with two people and confirm the chip reads Face Recovery through the transfer and the
  run; confirm the row reads `2 Faces`; crop one person out and confirm the row reads `1 Face` and the
  result restores only the remaining face; rotate to 90°, then 50°, then back to 90°, and confirm from
  the log that the third framing runs no detection model; run both models over one photograph and
  confirm neither is served the other's result
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy` and `cargo test` for the workspace, and the
  frontend's lint, typecheck and test scripts scoped to the files this change touched — verify all
  pass
