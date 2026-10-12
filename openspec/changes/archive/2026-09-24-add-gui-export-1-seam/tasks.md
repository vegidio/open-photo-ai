## 1. Promote the cancellation table

- [x] 1.1 Move `Analyses` and `Registration` out of `gui/src/autopilot.rs` into `gui/src/stops.rs` as `Stops` (design D2). Keep their tests with them and generalise the prose from "analysis" to "job". Point `autopilot.rs` and its managed state at the new type. Verify with `cargo test -p gui`: the autopilot tests still pass unchanged.

## 2. Backend pieces

- [x] 2.1 Add `gui/src/export/destination.rs` with `claim(requested, overwrite)` and the `Claim` guard (design D4). Verify with unit tests in a temp directory: a free name, `_1` when the name is taken, the source's own path numbered, an overwrite written beside the destination and renamed over it only on `commit()` (the destination untouched when the claim is dropped, and its permissions kept), two claims racing for one name, a placeholder removed on drop but kept after `commit()`, a missing directory refused at the first try, and refusal after `_999`.
- [x] 2.2 Add the wire `ExportFormat` and `encode_options(format, quality)` (design D5): quality clamped to 1..=100 on the four lossy formats, ignored on the rest, AVIF at speed 6, `heic` mapped to `ImageFormat::Heif`. Verify with unit tests that each format gets its own `EncodeOptions` arm with the quality where expected, and that serde reads every wire spelling.
- [x] 2.3 Add the `export:progress` wire `ExportProgress` (`enhancing` wrapping the run's report, and `writing`) and a `reporting` constructor over `Reporting::new` (design D6). Verify with unit tests that a thinned run is re-sent as `enhancing` under the export's run name, and that `writing` serializes with no fraction.

## 3. The export seam and commands

- [x] 3.1 Add `Exported` and `ExportError` (design D7) and `export_with`, following the order in design D3. Reuse the `Enhancer` seam, run at `OutputDepth::Source`, and never touch `Runs`. Verify with unit tests using the recording fake enhancer: each refusal, and that it leaves no file; an empty chain writes the framed source; the answer names the numbered path and the bytes; `Writing` comes after the last enhancing report.
- [x] 3.2 Cover stopping in `export_with`. Verify with unit tests: a stop before registration answers `Stopped` without loading; a stop during the chain answers `Stopped` and leaves no file at the destination or any numbered name; a stop after `Writing` still writes; a canvas run held in `Runs` is neither displaced nor released by an export.
- [x] 3.3 Add the `export` and `cancel_export` commands in `gui/src/export/mod.rs`, manage a second `Stops` for exports, and register both in `generate_handler!` in `gui/src/lib.rs`. Verify with `cargo build -p gui` and a test that `cancel_export` for an unknown run is a silent no-op.
- [x] 3.4 Add an `#[ignore]`d integration test that exports `fixtures/test.dat` through the real `Opai` with a 2x Kyoto upscale, as PNG and as JPEG, checking the written dimensions and the reported bytes against the file on disk. Run it by hand on macOS on the CPU provider and on CoreML, and record both results in this task.
  - Run 2026-09-24, macOS (Apple silicon), release build, `fixtures/test.dat` (640x640) with a per-provider nonce so
    the run store had never seen it. Every export wrote 1280x1280, and the reported bytes matched the file on disk:
    - CPU: PNG 1,888,705 bytes in 13.7 s, JPEG (q90) 288,700 bytes in 40 ms
    - CoreML: PNG 1,888,748 bytes in 1.8 s, JPEG (q90) 288,621 bytes in 40 ms
  - Each JPEG export was served from the run cache that the PNG export of the same photograph had just filled, so the
    chain ran once per provider, and the JPEG leg exercised only the encoding and the write.

## 4. Frontend IPC

- [x] 4.1 Add `frontend/ipc/export.ts`: the wire types (`ExportFormat`, `ExportRequest`, `Exported`, `ExportError`, `ExportProgress`), `exportImage` minting its run through `mintRun` before the invoke, `cancelExport`, and `onExportProgress` (design D8). Verify with `pnpm typecheck`.
- [x] 4.2 Add `frontend/ipc/export.test.ts` with `invoke` and `listen` mocked. Verify that it pins the command and event names, that the run is minted before the invoke, that the argument shape is `{ run, source, operations, processor, crop, destination, format, quality, overwrite }`, and that progress payloads reach the handler unchanged. Run it with `pnpm test`.

## 5. Checks

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and verify that all three pass.
- [x] 5.2 Run the frontend lint (Biome scoped to the files this change touched), typecheck and test scripts, and verify that all three pass.
