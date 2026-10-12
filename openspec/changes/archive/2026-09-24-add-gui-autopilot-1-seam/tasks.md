## 1. The wire

- [x] 1.1 In a new `gui/src/autopilot.rs`, write `SuggestionWire` and `SuggestError` (D4):
  - `SuggestionWire` is serialize-only, tagged by `family` in snake_case. The upscale arm carries `scale`.
  - A `From<Suggestion>` conversion with one arm per `Suggestion` arm.
  - `SuggestError` is tagged by `kind` in camelCase, with the arms `notReady`, `unknownSource`, `unreadableSource`,
    `stopped` and `analyse`.
  - Verify with tests that each `Suggestion` arm serializes to its family's spelling, and that an upscale carries its
    scale as a number.
  - Verify with tests that each error arm serializes to the `kind` the frontend will match on.

## 2. The cancellation table

- [x] 2.1 Write `Analyses`: a `std::sync::Mutex` over the running map and a bounded early-stop set (D3). Give it:
  - a `register(run)` that answers either a token plus a drop guard that deregisters, or "already stopped"
  - a `stop(run)`
  - Verify with tests:
    - A stop cancels the named token and leaves another run's token alone.
    - A stop before its register makes the register answer "already stopped", and consumes the entry.
    - A stop for a finished run neither panics nor affects a later run with a different name.
    - Dropping the guard removes the entry, including when the analysis panics.
    - The early-stop set never exceeds its bound.

## 3. The command

- [x] 3.1 Write the `Analyser` seam trait over `Opai::suggest`, the `Request` struct, and `suggest_with`, in the order
  D6 gives. Map `Cancelled` and `Shutdown` to `stopped`, and anything else to `analyse`. Log a `warn!` naming the run
  when `incomplete` is `Some`, and answer only `suggested`. Verify with tests behind a recording seam that:
  - an unopened identity is refused before anything is decoded or analysed
  - an unreadable file is refused as `unreadableSource`
  - a stop registered before the request answers `stopped` with the seam never called
  - the picture handed to the seam is the framed one (compare against `cropped`)
  - the families and the processor reach the seam unchanged, including an empty set
  - an incomplete result answers its suggestions and is not an error
  - a seam answering `Cancelled` or `Shutdown` is `stopped`
- [x] 3.2 Add the `suggest` and `cancel_suggest` Tauri commands and manage `Analyses` in `gui/src/lib.rs`, then
  register both in `invoke_handler`. Verify that the existing capability-grant test still passes unchanged, and that
  `cargo build -p gui` succeeds.
- [x] 3.3 Test that an analysis does not disturb an enhancement run. Verify that `suggest_with` and `cancel_suggest`
  never touch `Runs`: a test holds a run in the slot, analyses and stops another photograph, and confirms that the
  run's token is not cancelled and its result is still held.

## 4. The frontend seam

- [x] 4.1 Write `frontend/ipc/autopilot.ts`:
  - the `Suggestion` union (`{ family: "upscale"; scale: number }` plus a member per unit family, using `Family`
    from `ipc/catalogue`)
  - the `SuggestError` union
  - `suggest(source, processor, families, crop?)`, which mints the run with `mintRun()` before the invoke and answers
    `{ run, done }` as `detectFaces` does
  - `cancelSuggest(run)`
  - Verify with tests that the command names and argument names match the Rust signatures, that `crop` is sent as
    `undefined` when absent, that a refusal rejects rather than resolving empty, and that `cancelSuggest` invokes
    `cancel_suggest` with the run name.

## 5. Checks

- [x] 5.1 Check the analysis against real models from a Rust test marked `#[ignore]` that runs with the models
  installed:
  1. Analyse `fixtures/test.dat` with face recovery in the families, at the identity crop.
  2. Run `detect_with` on the same framing with a recording `on_progress`.
  3. Confirm that the detection reports `Stage::Cached` and runs no model.
  - Verify that it passes on macOS with CPU and CoreML.
- [x] 5.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`, plus the
  frontend's lint, typecheck and test scripts scoped to `frontend/ipc/autopilot*.ts`. Verify that all of them pass.
