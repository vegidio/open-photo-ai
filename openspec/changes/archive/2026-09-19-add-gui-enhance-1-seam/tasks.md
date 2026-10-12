## 1. The operation on the wire

- [x] 1.1 Add `gui/src/enhance.rs` with the deserializable operation shape - a union tagged by
      family, carrying `codename`, `precision` and the family's own values, with `upscale` its one
      variant (D1) - and verify unit tests deserialize the four upscale codenames at each precision
      they are published at.
- [x] 1.2 Convert the wire shape into `opai::Operation`, resolving the codename and precision through
      the same catalogue vocabulary `catalogue.rs` publishes and building the scale with
      `Scale::clamped` (D7), and verify tests cover a valid pairing, a codename the family does not
      publish, a precision the model is not published at, and scales above, below and inside the
      permitted range.

## 2. The run slot

- [x] 2.1 Add the managed single-run slot holding the run identifier, its `CancellationToken` and the
      most recent `Option<Picture>` (D2), and verify tests show writing it cancels the token it
      displaces and drops the result it displaces.
- [x] 2.2 Register the slot with `.manage(...)` in `gui/src/lib.rs` and verify the application still
      builds and starts.

## 3. Running and stopping

- [x] 3.1 Add the `enhance` command - resolve the source identity through `Opened`, refuse an
      unadmitted one, write the slot, call `Opai::process` at `OutputDepth::Eight` (D7) with the
      window's provider, and answer the result's identity and dimensions without its pixels - and
      verify tests cover a chain over an admitted source, an unadmitted source, and an empty chain.
- [x] 3.2 Add the `cancel_enhance` command taking the run identifier, no-op unless the slot still
      holds that run (D3), and verify tests cover stopping the run in flight, naming a run already
      displaced, and a stop that arrives before its own run starts.
- [x] 3.3 Map `InferenceError::Cancelled` to an outcome the window can tell apart from a failure, and
      verify a test asserts a stopped run and a failed run are reported distinguishably.
- [x] 3.4 Register both commands in `generate_handler![]` in `gui/src/lib.rs` and verify the
      application builds, which is what pins the Rust half of the command names.

## 4. Reporting progress

- [x] 4.1 Emit the progress event from `ProcessOptions::on_progress`, carrying the run identifier,
      the operation's name, the stage, the whole-chain fraction and the install's own fraction, and
      verify tests assert an install report carries both fractions and a `Cached` operation advances
      the chain fraction while naming neither stage (D5).
- [x] 4.2 Thin the emissions at the emitter - on a change in either rounded percentage, plus the
      first and last report of each stage (D5) - and verify a test drives a tile-rate report stream
      through it and asserts the emitted count is bounded rather than proportional to the input.

## 5. Serving the enhanced result

- [x] 5.1 Resolve an `opai://` request against the run slot as well as `Opened`, serving the resident
      result's pixels through the existing `Renditions` path (D4), and verify tests cover a bounded
      rendition of a result, an unbounded one, a displaced result refused as no longer held, and an
      identity no run produced refused as unknown.
- [x] 5.2 Verify the `gui-images` delta holds end to end: a test asserts an enhanced identity is
      served while resident and refused after a later run displaces it, and that no path outside the
      admitted set became reachable.

## 6. The frontend binding

- [x] 6.1 Add `gui/frontend/ipc/enhance.ts` - the two command names, the operation and result types,
      the run-identifier mint, and the progress event subscription - and verify `enhance.test.ts`
      pins both command names, the event name and the identifier being minted before the call.

## 7. Checks

- [x] 7.1 Run `cargo fmt --check`, `cargo clippy` and `cargo test`, and verify all three pass.
- [x] 7.2 Run the frontend lint, typecheck and test scripts scoped to the files this slice touched,
      and verify all three pass.
