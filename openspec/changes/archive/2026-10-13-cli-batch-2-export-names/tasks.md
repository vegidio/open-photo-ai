## 1. `opai::export`

- [x] 1.1 Create `crates/opai/src/export/` (`mod.rs`, `format.rs`, `name.rs`, `claim.rs`), declare `pub mod export` in `lib.rs`, and write the module docs (scope, why it sits beside `image`, D1). Verify `cargo build -p opai` succeeds
- [x] 1.2 Move `crates/gui/src/export/format.rs` into `opai::export::format` with its tests, making `ExportFormat`, `FormatCapability`, `QualityRange`, `ExportFormats`, `capabilities()` and `encode_options()` public. Verify the moved tests pass under `cargo test -p opai export::format`
- [x] 1.3 Add `FormatChoice` (`Preserve` | `Format(ExportFormat)`) with `FromStr`/`Display` and string `Serialize`/`Deserialize` (`"preserve"`, `"jpeg"`, …) per D2. Verify with tests: every spelling round-trips, and `heif`, `jpg` and an unknown word are refused
- [x] 1.4 Implement `Naming`, `Named`, `name_for` and `destination_for` in `name.rs` per D3, and port every case from `lib/export.test.ts`'s `formatFor and extensionFor` and `destinationFor` blocks, plus a `#[cfg(windows)]` backslash-folder case and a non-UTF-8 stem case on Unix. Verify `cargo test -p opai export::name` passes and covers each `export-naming` scenario for format, name and folder
- [x] 1.5 Move `crates/gui/src/export/destination.rs` into `opai::export::claim` with every test (race, panic, permissions, `_999`, missing directory), making `claim`, `Claim::path` and `Claim::commit` public. Verify `cargo test -p opai export::claim` passes

## 2. `gui` (Rust)

- [x] 2.1 Delete `gui`'s `export/format.rs` and `export/destination.rs`, and point `export_formats`, `run.rs` and the wire types at `opai::export`. Verify `cargo build -p gui` succeeds and `the_capabilities_cross_the_boundary_as_the_window_reads_them` still passes unchanged
- [x] 2.2 Replace the `export` command's `destination`/`format` with `naming: ExportNaming { prefix, suffix, location, format: FormatChoice }` (D5). `export_with` computes the destination with `destination_for` from the resolved source path at the claim step. Update the `run.rs` tests to send a naming. Verify the tests for numbering, overwrite, unwritable destination (naming the path asked for) and written path pass
- [x] 2.3 Add the `export_names(paths, naming) -> Vec<ExportName { path, name, destination, format, extension }>` command (D6) and register it in `generate_handler!`. Verify with tests that the order is kept, Preserve over a RAW path gives `tiff`, and the wire shape is pinned with `serde_json`

## 3. Frontend

- [x] 3.1 In `ipc/export.ts`, add the `exportNames` binding and its `ExportName` type, and change `ExportRequest` to carry the naming instead of `destination`/`format`. Verify the command-name pin in the ipc test covers `export_names`, and `pnpm typecheck` passes
- [x] 3.2 Add the `useExportNames(queue, settings)` hook in `features/export`. It asks on every change to the queue, prefix, suffix, location or format, drops stale answers by sequence, and keeps the last names until the next answer arrives. Verify with vitest that an out-of-order answer is ignored and that rows have names after the first answer
- [x] 3.3 Make `ExportRow` (and the queue or dialog that owns the hook) show the name and `SRC → DST` type from `export_names`. Verify `ExportRow.test.tsx` passes with its existing expected strings, using a stubbed `exportNames`
- [x] 3.4 Update `batch.ts`: ask `export_names` once at Save, take each file's quality format from the answer, send the naming to `exportImage`, and have `describeExportError` name a write failure from the error's `path`. Change `qualityFor`/`queueQualityFor` to take a resolved format. Verify `batch.test.ts` and `lib/export.test.ts` pass, including the mixed-Preserve and write-failure cases
- [x] 3.5 Remove `formatFor`, `extensionFor`, `exportNameFor`, `destinationFor`, `stemOf`, `join` and `NamingChoices` (if it is no longer used) from `lib/export.ts`, along with their tests. Verify that `grep` finds no remaining caller and that `pnpm lint`, `pnpm typecheck` and `pnpm test` pass in `crates/gui`

## 4. Checks

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`. Verify all three are clean
- [x] 4.2 Manually check the export dialog on macOS (`gui-test-notify`). With prefix and suffix set, row names update as you type. Preserve over a `.nef` shows `→ TIFF`. Exporting twice to one folder writes `_1`. Overwrite replaces the file and leaves no hidden file. A browsed folder is honoured. Verify each matches the behaviour before this change
