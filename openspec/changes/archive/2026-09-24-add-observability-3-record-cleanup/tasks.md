## 1. Stops and levels in `opai`

- [x] 1.1 Add a crate-private `stop_reason(&self) -> Option<&'static str>` to `InferenceError` and `InitError`
  (design.md D2):
  - `InferenceError::Cancelled` and `InitError::Stopped` return `"cancelled"`.
  - `InferenceError::Shutdown` and `InitError::Cancelled` return `"shutdown"`.
  - Every other variant returns `None`.

  Unit-test every variant beside `error.rs`. Verify with `cargo test -p opai error::`.
- [x] 1.2 Correct the two levels (D9):
  - `deps/model/mod.rs`'s unpublished-model refusal becomes `warn!`, and its comment is kept for the behavioural
    divergence it defends.
  - `setup.rs`'s `CacheMode::None` becomes `warn!`.

  Write the level rule (`error` only for a panic; `warn` for a failure or a costly degradation; `info` for the account
  of units of work, including their stops) into `logging/mod.rs`'s header. Point `inference/process/mod.rs`'s header
  at it instead of restating it. Update the tests asserting either level, and verify that
  `grep -rn "tracing::error!" crates/opai/src` lists only `logging/panic.rs` and test code.

## 2. Initialization and installs

- [x] 2.1 Split `Opai::initialize` into a timing wrapper around a private `initialize_inner` (D3). Record:
  - "initialized" with `duration`.
  - `AlreadyRunning` at `info`, with its holder.
  - A stop at `info`, with `reason` and `duration`.
  - Every other failure as "initialization failed" at `warn`, with `name`, `duration` and `error`.

  Add `duration` to `runtime::start`'s "ONNX Runtime started". Test each outcome through the records helpers in
  `tests/initialize.rs`, driving the failure and refusal arms through the paths the suite can reach without a runtime:
  an invalid name, and a claim held by another process. Verify with `cargo test -p opai initialize`.
- [x] 2.2 In `deps/install.rs` (D2):
  - Record `duration` on "the dependency is ready", timed from "installing a dependency".
  - Move "an install stopped" from `debug` to `info`, with `reason` and `duration`.
  - Route `InitError::Cancelled` to that stop record instead of "installing a dependency failed".

  Update the install log tests, and add one asserting that a shutdown writes no `warn`. Verify with
  `cargo test -p opai deps::install`.

## 3. Session builds

- [x] 3.1 Narrow the `impl` holding `SessionCache::get_or_build` to `SessionError` (D4). Fill the flight's `Err` arm:
  - "the build stopped" at `info` with `reason = "shutdown"` when `stop_reason()` says so.
  - Otherwise "session build failed" at `warn`, with `artifact`, `provider`, `requested`, `duration` and `error`.

  Drop `error` from the CPU-fallback `warn` in `sessions/mod.rs`. In `sessions/tests/log.rs`, test:
  - A failing build awaited by several concurrent requests writes exactly one "session build failed".
  - A GPU failure writes the build record, then the fallback record without `error`.

  Verify with `cargo test -p opai sessions::`.

## 4. Enhancement and analysis brackets

- [x] 4.1 In `inference/process` (D5):
  - Move `plan::<B>(operations)?` from `run` into `process`, ahead of "enhancement started".
  - Make `run` return a crate-private `RunFailure { error, step }`.
  - Remove "step failed", and have "enhancement failed" carry `operation` and `index` when the failure was in a step.
  - Split stops into "enhancement stopped" (`info`, with `reason`, `duration` and `identity`).
  - Add `provider` to "enhancement started", and `identity` to every closing record.

  Rewrite `a_failed_step_is_distinguishable_from_the_run_that_carried_it` to assert one `warn` carrying the step's
  fields. Add tests for a refused chain (only "chain refused") and a cancelled run (only an `info` stop). Verify with
  `cargo test -p opai inference::process`.

  Note: no shipped operation can be refused today. Upscale's `NoPasses` is unreachable
  (`every_scale_a_user_can_ask_for_is_covered_by_a_pass_sequence`), and so is Osaka's `IncompleteGraphSet`.
  `Operation` is a concrete enum, so a test cannot supply one that refuses. The refused-chain test therefore forces
  the refusal through a `#[cfg(test)]` hook in `plan.rs` (`plan::refused::refusing`), which is compiled out of every
  build but the suite's.
- [x] 4.2 In `inference/execute.rs` (D6):
  - Record "analysis refused" at `warn`, with `id` and `error`, where `operation.pipeline::<B>()` refuses. It stays
    before "analysis started".
  - Add `provider` to "analysis started", and `identity` to the closing records.
  - Split the error arm into "analysis stopped" (`info`) and "analysis failed" (`warn`).

  Test the refusal, a cancellation and a failure against the fake backend. Verify with
  `cargo test -p opai inference::execute`.
- [x] 4.3 In `autopilot/mod.rs` (D7):
  - Remove `record_unread`.
  - Wrap `suggest` around a `suggest_inner`, and move the conclusion record into the wrapper. Its early return for an
    empty image or an empty scope must still be recorded.
  - Add `duration` to the conclusion, and `incomplete = "face"` with `error` when the face signal was unread.
  - Record a withdrawal as "automatic analysis stopped" (`info`, with `identity`, `reason` and `duration`), and any
    other error as "automatic analysis failed" (`warn`).

  Update the unread-signal test to assert one `warn` ("analysis failed") and the conclusion's `incomplete` and
  `error`. Add a test that a cancelled analysis writes the stop and no `warn`. Verify with `cargo test -p opai autopilot`.

## 5. Image IO

- [x] 5.1 Record failures at the six public blocking boundaries in `image/`, per D8's table: `load_blocking`,
  `save_blocking`, `encode_blocking`, `probe_blocking`, `probe_raw_blocking` and `identity_blocking`. Each writes one
  `warn` with `op`, `path` or `format`, and `error`, and success writes nothing. Test each operation once:
  - One failure yields exactly one record through the blocking function.
  - One failure yields exactly one record through its async twin.
  - A success yields none.

  Confirm by `grep` that `cache.rs` still reaches no public image function. Verify with `cargo test -p opai image::`.

## 6. `gui`: repeats removed

- [x] 6.1 Remove the `tracing` calls for the run-path repeats in D10's table, leaving each `map_err` or match arm
  returning the same error to the window:
  - `setup.rs`
  - `enhance/run.rs` (both records)
  - `faces.rs` (both)
  - `autopilot.rs` (all three)
  - `export/run.rs`: the read and chain records. The `opai::image::save` failure is split out of `unwritable` and
    builds `ExportError::Write` without recording. The claim and commit arms keep their `error` record.

  Verify that `cargo test -p gui` passes, that no `gui` test asserts on a removed record, and that no `use` or
  binding is left unused.
- [x] 6.2 Remove the image-path repeats in `images/serve.rs` (decode and encode) and `images/files.rs` (dimensions and
  identity), keeping the same return values. Keep `files.rs`'s other records, which are `gui`'s own. Verify with
  `cargo test -p gui`.

## 7. Verification

- [x] 7.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace --locked`, and verify all three are green. Then run
  `openspec validate add-observability-3-record-cleanup --strict` and verify it passes.
- [x] 7.2 Hand the owner a manual check on macOS, with a `gui` debug build at the default `RUST_LOG`:
  - Drag a slider mid-enhancement.
  - Open a file that is not an image.
  - Export to a read-only folder.
  - Enhance once with CoreML and once on the CPU.

  In `opai.log`, verify that each failure appears exactly once at `warn`, each cancellation once as a stop at `info`,
  and no `opai` record is at `ERROR`. Record the lines seen in this task's checkbox note.

  Note: checked by the owner on 2026-09-24, who confirmed the records behave as described. The log lines themselves
  were not captured here.
