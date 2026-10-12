## 1. Rust commands

- [x] 1.1 Add `gui/src/export/directory.rs` with the async `pick_directory(title)` command (design D7): the folder picker through `tauri-plugin-dialog`'s Rust API, answering `None` on dismissal and a lossy string otherwise. Register it in `generate_handler!`. Verify with `cargo build -p gui`.
- [x] 1.2 Add the managed `Written` record and `reveal_export(path)` (design D7). The `export` command in `export/mod.rs` records the path actually written after `export_with` answers `exported`. `reveal_export` refuses any path not in the record, and reveals the rest through `reveal::reveal`. Register it. Verify with unit tests that only exported paths are recorded (not stopped ones, not failed ones, and the numbered name rather than the requested one) and that an unrecorded path is refused before anything is opened.

## 2. Frontend foundations

- [x] 2.1 Add `pickDirectory(title)` and `revealExport(path)` to `ipc/export.ts`, and extend `export.test.ts` to pin both command names and argument shapes. Verify with `pnpm test`.
- [x] 2.2 Add `lib/export.ts` and `directoryOf` in `lib/paths.ts` (design D5): `formatFor`, `extensionFor`, `destinationFor`, `groupByChain`, `queueQualityFormat` and `describeExportError` (design D6). Verify with `lib/export.test.ts`: Preserve over JPEG, PNG, HEIF and a RAW source; each chosen format's extension; `/` and `\` sources; a browsed folder; grouping stability with an empty-stack group; the quality format for a uniform queue, a mixed one and a lossless one; each error kind's reason.
- [x] 2.3 Add the persisted `stores/exportSettings.ts` under `export-storage` (design D4), with the reference's defaults and a rehydration repair that maps `jpg` to `jpeg` and replaces an unknown format with `png`. Verify with tests for the defaults, a round trip through storage, and each repair.
- [x] 2.4 Move `withFaces` from `hooks/useEnhancementRun.ts` to `lib/faces.ts`, and make `analyse` in `hooks/useAutopilot.ts` answer an `AnalysisOutcome` and take `{ notify }` (design D3). Add `awaitAnalysis`, which waits on an analysis already in flight. Verify that the existing `useAutopilot` and `useEnhancementRun` tests pass unchanged, and add tests that `notify: false` neither toasts nor declines, that a failure comes back as `{ failed }`, and that `awaitAnalysis` asks for no second analysis while one is in flight.

## 3. The batch

- [x] 3.1 Add `stores/exportBatch.ts` (design D2): phase, queue snapshot, rows, current and aborted, with `open`, `reset`, `setStage` and `abort`, plus the summary and overall-fraction selectors. Verify with tests for `open`/`reset`, the summary in each phase (idle, running with one done, ended with one failed) and the overall fraction.
- [x] 3.2 Add `runBatch` in `features/export/batch.ts` and the progress routing (design D2). Verify with tests, with the IPC modules mocked:
  - queue order is kept, and the batch continues past a failed file
  - Abort during analysis, face detection and enhancing leaves the row Queued and starts nothing more; an `exported` answered after Abort reads Done
  - with Autopilot on, a stackless photograph is analysed, its suggestions land in the stack and they are the chain sent; with Autopilot off it is exported with an empty chain; an empty suggestion exports as-is; an analysis failure reads Failed without a toast
  - faces are detected only when unknown, and the chosen ones are sent
  - each file carries its committed quality, or its own format's quality under a mixed Preserve
  - progress for another run is ignored

## 4. The dialog

- [x] 4.1 Add `ExportRow` and `ExportQueue` (design D8): the thumbnail, name, dimensions, sizes, types, stage pill, the current row's own bar, the reveal button once Done (showing `errors.revealFailed` on rejection), and the Failed pill's controlled red tooltip with click-to-copy and the copy confirmation (design D6). Verify with component tests for each stage's pill, the reveal call with the written path, and the tooltip's copy (clipboard mocked).
- [x] 4.2 Add `ExportSettingsPanel` (design D4, D8): the prefix and suffix, Save to with Browse… through `pickDirectory` (a dismissal keeps the previous choice), the overwrite switch and its two footnotes, the format select, the quality draft shown only for a single lossy format, the whole column disabled while running, and the buttons for each phase, with Save committing the quality and starting `runBatch`. Verify with component tests for the slider's visibility, the footnote swap, a dismissed picker, a discarded and a committed draft, and each phase's labels.
- [x] 4.3 Add `ExportDialog` (design D8): the title with its summary, the close box withheld and Escape ignored while running, outside clicks always ignored, the overall bar, the queue snapshot taken on open, one progress subscription for its life, and `abort()` on unmount. Verify with component tests that Escape and the close box work while idle and ended, and are refused while running.
- [x] 4.4 Enable the sidebar's export button on a non-empty selection with the pluralised label, and mount `ExportDialog` from it (design D9). Replace the Sidebar test that pins it disabled with tests for none, one and several picked images and for opening the dialog. Verify with `pnpm test`.
- [x] 4.5 Add the i18n keys from design D9 and remove the unused ones, in all thirteen locale files, splitting `sidebar.exportImage` into its plural forms. Verify with the parity test and `pnpm test`.

## 5. Checks

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. Run the frontend lint (Biome scoped to the files this change touched), typecheck and test scripts. Verify that all of them pass. Then hand the user the by-hand macOS check from design.md's Validation to run: a mixed four-file batch, a read-only folder failure, Abort, Export again, and a numbered file revealed. Record the results here.
  - Automated, 2026-09-24: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass (the `gui` crate 286 passed, 2 ignored). Biome on the files touched is clean, `pnpm typecheck` passes, and `pnpm test` passes in full.
  - By hand on macOS, 2026-09-24: checked by the user and passed.
