## 1. Building blocks

- [x] 1.1 In `frontend/lib/enhancements.ts`, write `suggestedOperation(suggestion, entry, stored, file)` (D5). It
  delegates to `newOperation`, and for an upscale it replaces `scale` with the suggestion's. Verify with tests that:
  - each family's suggestion builds exactly what `newOperation` builds, with the user's stored model
  - an upscale carries the suggestion's scale and not `defaultScale(file)`
  - a family with no catalogue model answers `undefined`
- [x] 1.2 In `frontend/lib/faces.ts`, write `noFaceToRestore(faces)` (D6), reusing `faceArea` and
  `RESTORED_FACE_AREA`. Verify with tests that:
  - one face at or below 512x512 among larger ones answers false
  - only larger faces answers true
  - a face exactly at the area answers false
  - an empty array answers true
- [x] 1.3 In `frontend/stores/enhancements.ts`, add `addEnhancements(path, operations)` (D7): one `set` that skips
  families already present, puts the stack back in pipeline order, and writes the key even for an empty batch. Verify
  with tests that:
  - a batch lands in pipeline order
  - a family already in the stack keeps the existing entry
  - an empty batch makes `has(path)` true
  - a batch is a single store write (one subscriber notification)

## 2. The analyses in flight

- [x] 2.1 Write `frontend/stores/autopilot.ts` (D1): `analysing` keyed by path, `declined`, the actions `begin`,
  `end`, `stop`, `stopAll`, `decline` and `clearDeclined`, the `useAnalysing(path)` selector, and the file-owner
  registration. Verify with tests that:
  - `end` with a stale run name leaves a newer entry alone
  - `stop` and `stopAll` call `cancelSuggest` with each run's name and remove the entries
  - closing one file stops only its analysis
  - closing every file stops them all
- [x] 2.2 Write `analyse(file, crop)` in `frontend/hooks/useAutopilot.ts` (D3), with `ipc/autopilot`, `ipc/faces` and
  `ipc/catalogue` mocked. Verify with tests that:
  - the families sent are `ENHANCEMENTS` minus `autopilotExcluded`, and an empty set is still sent
  - suggestions land as one batch, and the entry is ended only after the stack is written
  - a `stopped` rejection is silent and writes nothing
  - any other rejection shows the `errors.autopilotFailed` toast, writes no stack, and declines the path only if it is
    current
  - face recovery is dropped when `noFaceToRestore`, and kept when a face is small
  - faces are recorded with `setFaces` at the captured crop in both cases
  - a failed follow-up detection keeps face recovery and records nothing
  - an answer arriving after the entry was stopped (close, switch-off or reframe) writes nothing, at either await
- [x] 2.3 Write the `useAutopilot()` trigger (D2 and D4). Verify with hook tests that:
  - the current photograph is analysed with Autopilot on and no stack
  - a photograph that is open but not current is never analysed
  - a photograph with a stack, including an emptied one, is not analysed
  - turning the switch on analyses the current photograph
  - moving off and back while in flight asks nothing new
  - switching off stops every analysis in flight
  - a crop change mid-flight stops the analysis and asks again with the new crop
  - a crop change mid-flight after a hand-added enhancement stops the analysis and asks nothing again
  - a failure is not retried until the current photograph changes or the switch is turned on again
  - a file with no identity is never analysed

## 3. The window

- [x] 3.1 In `features/enhancements/EnhancementList.tsx`, draw the analysing row (D8) with the design screen 04
  metrics, `--success-bright` and `--activity` on the ring, `sidebar.analysing` in `--primary`, and
  `role="status"`. Verify with component tests that:
  - the row replaces the list while the current photograph is being analysed
  - it shows for a photograph with no stack yet
  - moving to a photograph that is not being analysed shows that photograph's list
- [x] 3.2 Mount `useAutopilot()` in `features/sidebar/Sidebar.tsx`. Correct the stale comments:
  - `autopilotExcluded`'s "Nothing reads it yet" in `stores/settings.ts`
  - "the faces' only writer is the run path" in `stores/faces.ts`

  Verify with a `Sidebar` test that opening a photograph with Autopilot on calls `suggest`, and with it off does not.
- [x] 3.3 Add a `useEnhancementRun` test for the handover. After faces are recorded through `setFaces` at the crop in
  force and a stack carrying face recovery is written, the run calls `enhance` directly: no `detectFaces`, and the bar
  starts from the whole range. Verify that the test passes.

## 4. Specs and checks

- [x] 4.1 Widen the `## Purpose` of `openspec/specs/gui-autopilot/spec.md` so it covers the window's side: when an
  analysis is asked for, how it is shown, and how its answer is applied. Verify with
  `openspec validate add-gui-autopilot-2-run --strict`.
- [x] 4.2 Check by hand on macOS, with CPU and then CoreML selected:
  - Open `fixtures/test.dat` with Autopilot on: the analysing row shows, then the suggestions land, and the face
    recovery's run shows no detection phase (see the log for a cached detection).
  - Close a photograph mid-analysis: no toast, and nothing is added.
  - Switch Autopilot off mid-analysis: the row goes, and nothing is added.
  - Switch face recovery off in Settings and open a photograph with faces: no face recovery is added.

  Record what was observed in this task.

  Observed: checked by hand by the user on macOS, CPU and CoreML; every item behaved as listed.
- [x] 4.3 Run the frontend's lint (Biome scoped to the touched files), typecheck and test scripts. Verify that all of
  them pass, and that `cargo build -p gui` still succeeds with no Rust changes.
