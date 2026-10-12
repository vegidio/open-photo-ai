## 1. The command wrapper

- [x] 1.1 Add `crates/gui/src/command.rs` (design.md D3, D4):
  - `TARGET = "gui::command"`.
  - `command_span!`, which opens an `info` span named after the command, with `outcome` and `error` `Empty` and
    `run` only when given.
  - `Ended`, and the `Outcome`, `Answer` and `CommandError` traits. `Outcome` is implemented for `Result<T, E>` and
    for the infallible answers.
  - `traced` (async, `.instrument`) and `traced_sync`.

  Add a recording test layer on the `registry` dev-dependency. It captures spans (name, target, fields, parent, and
  recorded `outcome`/`error`) and events (level, target, own fields, parent span). Test each of the four endings:
  - Finished records `outcome=finished`.
  - Stopped records `outcome=stopped` and no `error`.
  - Recorded failed records `error` and writes no event.
  - Unrecorded failed writes exactly one `warn` inside the span, which carries `run` but no `outcome` or `error`
    among the span's formatted fields when it is emitted, and then records `error`.

  Verify with `cargo test -p gui command::`.
- [x] 1.2 Add `crates/gui/src/task.rs` with `spawn_blocking`, which carries `Span::current()` into `in_scope` on
  the blocking thread (D5). Route every non-test `tauri::async_runtime::spawn_blocking` in `gui` through it:
  `images/files.rs`, `images/serve.rs`, `images/crop.rs`, `export/run.rs` and `reveal.rs`. Test with the recording
  layer that an event and a span opened inside the closure parent under the caller's span. Test also that with no
  current span it runs as before. Verify with `cargo test -p gui task::` and
  `git grep -n "async_runtime::spawn_blocking" -- crates/gui/src`, which should show only the helper and tests.

## 2. Classify and wrap every command

- [x] 2.1 Implement `CommandError` for the nine error enums and `Answer` for `Enhancement` and `Exported`, each as one
  exhaustive `match` with no wildcard arm, following D4's table. Split `ExportError`'s claim and commit arms into a
  new `Unwritable { path, message }`, serialized with `#[serde(rename = "write")]`, and keep `Write` for the save.
  Test that:
  - every variant maps to its table cell
  - `Unwritable` and `Write` serialize to the same JSON shape as `Write` did before the split

  Verify with `cargo test -p gui` and `pnpm --dir crates/gui test`, which should pass with no frontend edit.
- [x] 2.2 Wrap the four commands that carry a run and do work: `enhance`, `suggest`, `detect_faces` and `export`,
  with `command_span!(…, run = …)`. Then remove from their bodies:
  - the explicit `run` fields on every record the span now encloses (`enhance/run.rs`, `enhance/slot.rs`,
    `autopilot.rs`, `faces.rs`, `export/run.rs`)
  - `export/run.rs`'s `unwritable` `error!`

  Test with the recording layer that:
  - A fake-backend enhancement's `opai` unit span parents under the `enhance` span.
  - A claim failure writes one `warn` from the wrapper and nothing at `error`.
  - An unreadable source writes no wrapper record.

  Verify with `cargo test -p gui enhance:: export:: autopilot:: faces::`.
- [x] 2.3 Wrap the remaining commands:
  - `initialize`, `quit`, `version`, `catalogue`
  - `cancel_enhance`, `release_enhanced`, `release_all_enhanced`, `cancel_suggest`, `cancel_export` (the `cancel_*`
    commands with `run`, dropping their records' own `run` field)
  - `pick_directory`, `reveal_export`, `open_images`, `describe_images`, `reveal_image`, `input_extensions`
  - `open_link`, `reveal_log`, `is_outdated`, `window_ready`

  Remove the reveal records the wrapper now writes (`export/written.rs`, `images/files.rs`). Add the drift test in
  `lib.rs` (Risks): the names in `generate_handler!` must equal those given to `command_span!` across `src/`, plus
  `set_analytics`. Test that:
  - a failing `open_link` writes one `warn` naming the URL
  - describing files parents each image IO unit span under `describe_images`

  Verify with `cargo test -p gui`.

## 3. The Analytics switch in Rust

- [x] 3.1 Add `crates/gui/src/analytics.rs` (D1):
  - Resolve `<config>/opai/analytics.json` with `rust_sak::fs::user_config_dir(opai::APP_NAME, …)`.
  - `read` answers on for an absent file, the stored value for a valid one, and off with a `warn` for an unreadable
    or malformed one.
  - `write` goes through `mk_temp_file_in` then `persist`, and skips a write that changes nothing.

  Add `fs` to `gui`'s `rust-sak` feature list, with a manifest comment in the file's style. Keep the path-taking
  halves `pub(crate)` so tests run under a `tempfile` directory. Test absent, `true`, `false`, garbage, a repeat
  write that leaves the modification time alone, and a write over an existing file. Verify with
  `cargo test -p gui analytics::`, and that `cargo tree -p gui -e normal --prefix none`, diffed before and after,
  adds nothing.
- [x] 3.2 Add the `set_analytics(enabled)` command (D2) and register it:
  - **Off:** `opai::telemetry::stop()` first, then write, then the `info` record.
  - **On:** write, then the "next launch" `info` record when telemetry is not running because of an earlier opt-out.
  - **A failed write:** `warn`, and the error returned as a tagged `AnalyticsError`.
  - **No command span.**

  Keep the ordering behind a function that takes the stop and the write as parameters, so a test can assert:
  - off calls the stop before the write and before any record
  - on never calls the stop
  - the recording layer sees no span for the call

  Verify with `cargo test -p gui analytics::`.
- [x] 3.3 In `lib.rs`'s `setup`, read the choice before `telemetry::start()`. When it is off, skip `start` and write
  a `debug` "telemetry is not sending: the user turned analytics off". Keep the existing `NoCollector` and error
  arms. Keep the "is telemetry running" answer `set_analytics` needs in one place beside it. Test the start
  decision as a function of the read's answer. Verify with `cargo test -p gui`, and that `cargo build -p gui`
  succeeds.

## 4. The Analytics switch in the frontend

- [x] 4.1 Add `crates/gui/frontend/ipc/analytics.ts`, with `setAnalytics(enabled)` invoking `set_analytics`, a
  `AnalyticsError` type for the tagged rejection, and the command-name comment the other bindings carry. Add
  `ipc/analytics.test.ts`, pinning the command name and the argument's shape as `ipc/logs.test.ts` does. Verify
  with `pnpm --dir crates/gui test ipc/analytics`.
- [x] 4.2 Add the mirror (D2), started from `main.tsx` before the app mounts:
  - One `setAnalytics` call with the rehydrated store's `analytics`.
  - A `useSettingsStore.subscribe` that calls it again whenever `analytics` changes.
  - A rejection dropped with `console.error`.

  Update the comments that say the choice is not read: the `analytics` field in `stores/settings.ts`, and
  `AnalyticsRow` in `GeneralPage.tsx`. Leave the i18n copy unchanged (design.md Risks). Test with a mocked `invoke`
  that:
  - boot sends the stored value once
  - a Save that turns it off sends `false` once
  - a Save that changes something else sends nothing

  Verify with `pnpm --dir crates/gui test`, `pnpm --dir crates/gui exec tsc --noEmit`, and Biome scoped to the
  touched files.

## 5. The session's end and the raise records

- [x] 5.1 Record the session's end (D6): store an `Instant` at the top of `run`, and on `RunEvent::Exit` write
  `info!(target: "gui", uptime = ?…, "Open Photo AI is closing")` before `opai::telemetry::stop()`. Replace the
  three `eprintln!`s in `single_instance.rs` with `warn!(%error, …)` carrying the same sentences. Extend the existing
  raise test under `MockRuntime` to assert that a window that is absent still writes nothing. Test the exit ordering
  through a function that takes the stop as a parameter: the record comes first. Verify with
  `cargo test -p gui single_instance::`, and that `git grep -n eprintln -- crates/gui/src` shows only `lib.rs`'s
  log-sink line and the test in `export/run.rs`.

## 6. Specs, docs and verification

- [x] 6.1 Update the prose that describes the old shape:
  - `gui/Cargo.toml`'s `tracing` comment, which now says the crate also opens spans
  - `lib.rs`'s `setup` comment on telemetry
  - the header of each module whose records lost `run`

  Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace --locked`, and verify all three are green. Verify that
  `openspec validate add-observability-5-gui-switch --strict` passes.
- [x] 6.2 Hand the owner a manual check on macOS (design.md Migration Plan), using a `gui` debug build with
  `OPAI_OTEL_ENDPOINT` and `OPAI_OTEL_AUTH` set. Record in this task's checkbox note what was seen:
  - Enhance one photograph. In Tempo, the trace is rooted at `enhance`, with `enhancement` and its units beneath it.
    In `opai.log`, `opai`'s records inside carry `run=` exactly once.
  - Turn Analytics off and save, then enhance again. Loki receives nothing after the save.
  - Quit and relaunch. Loki and Tempo receive nothing from that session, and `opai.log` has the `debug` reason when
    run with `RUST_LOG=debug`.
  - Turn Analytics on, save, quit and relaunch. The new session's records arrive, and on quitting, "Open Photo AI is
    closing" arrives with its `uptime`.

  **Seen on 2026-09-25**, macOS, three sessions between 06:18 and 06:25 (+02:00), checked in `opai.log`, Loki and
  Tempo:
  - Trace `e95e8409…` is rooted at `enhance` (`run=adn2jmc3-3`, `outcome=finished`). `enhancement` is its child,
    with its steps, session requests, builds and cache reads beneath it, and `image_load` is a direct child of
    `enhance`. `suggest`, `detect_faces`, `open_images`, `initialize`, `window_ready`, `version`, `catalogue` and
    `is_outdated` each root their own trace. In `opai.log`, every `opai` record inside a request carries `run=`
    once.
  - After the save at 06:23:39, Loki and Tempo hold nothing more from that session: not the opt-out record, not
    the second enhancement, and not the session's closing record.
  - The relaunch at 06:24:25 sent nothing to Loki or Tempo. `opai.log` has "telemetry is not sending: the user
    turned analytics off" at `DEBUG`, and "analytics was turned on; it takes effect at the next launch" after the
    save.
  - The relaunch at 06:25:06 sent to both. Loki received "Open Photo AI is closing" with `uptime=19.062524833s`.
  - Traces rooted at `image_load` and `image_encode` come from the `opai://` scheme handler, which is not a
    command. This is out of scope by design.md Non-Goals.
