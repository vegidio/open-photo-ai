## 1. Startup order

- [x] 1.1 Call `prepare_library_path` as the first statement of `crates/gui/src/main.rs`, discard the
  directories, and pass the outcome to `gui_lib::run`, which reports a failure through `tracing` after the log
  sink is installed and goes on starting - verify by launching with an unwritable configuration directory and
  finding the failure in the log with the window still up.

## 2. The setup state and its commands

- [x] 2.1 Add `crates/gui/src/setup.rs` with `Setup<T>` over `tauri::async_runtime::Mutex<Option<T>>` and the
  run-once rule: one initialization at a time, a stored success answered without work, a failure left
  retryable - verify with unit tests driving a fake initializer that count how many times it ran across
  concurrent calls, a call after success and a call after a failure.
- [x] 2.2 Add the wire types - `SetupEvent` tagged `plan`/`progress`, `PlanRow`, `ProgressRow` and the row
  state - mapping `Dependency` through `as_str()` and `Phase` through a match whose `_` arm means "working" -
  verify with a unit test over every `Phase` variant and a `PlannedDependency`/`Progress` pair.
- [x] 2.3 Add the `initialize` command: takes `Channel<SetupEvent>` and `State<'_, Setup<Opai>>`, calls
  `Opai::initialize` under `opai::GUI` with `on_plan` and `on_progress` closing over the channel, stores the
  handle, and returns `Result<(), SetupError>` carrying `InitError`'s message - verify `cargo check` passes
  and the command is reachable through `generate_handler!`.
- [x] 2.4 Add the `quit` command calling `AppHandle::exit(0)`, register both commands and
  `.manage(Setup::default())` in `run()` **after** the single-instance plugin - verify by reading the builder
  order against the ordering comment already in `src/lib.rs`.

## 3. The frontend boundary

- [x] 3.1 Add `frontend/ipc/setup.ts`: the `SetupEvent` types, `initialize(onEvent)` creating and handing over
  the `Channel`, and `quit()` - verify with a vitest that asserts the command names that go on the wire, in
  the style of `ipc/app.test.ts`.
- [x] 3.2 Add `frontend/stores/setup.ts` - rows keyed by name, status, the plan and progress reducers, the
  size-weighted overall fraction and the installed/total count - not persisted - verify with tests over a
  synthetic four-row plan: weighting, monotonicity, one row per repeated report, and the count's denominator
  including already-installed rows.

## 4. The dialog

- [x] 4.1 Generate the shadcn `dialog` and `progress` primitives, then apply this project's four corrections
  (the `cn` import, arrow functions, named React imports, the bogus `cn` package in `package.json`) - verify
  `pnpm lint`, `pnpm typecheck` and `git diff package.json` are all clean.
- [x] 4.2 Add `frontend/features/setup/SetupDialog.tsx` to screen 02: overall bar and count, one row per
  planned dependency with name, size, state and its own track while working, the throttling note, and Quit;
  dismissal prevented on outside click, Escape and interaction, with no close button - verify with tests that
  render each row state and assert the dialog survives an Escape key press and an outside click.
- [x] 4.3 Wire `App.tsx`: initialize on mount, render the dialog while the status is `running` or `failed`,
  take it down on `ready` - verify with tests that a plan alone opens nothing, that the first working report
  opens the dialog, and that a launch whose every report is already-installed opens nothing.
- [x] 4.4 Add the new i18n keys under `setup.*` - title, the two body sentences, the component count, the four
  row states, the throttling note, the footer line and Quit - to all thirteen locale files - verify
  `i18n/parity.test.ts` passes.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy` and `cargo test` for the workspace, and `pnpm lint`,
  `pnpm typecheck` and `pnpm test` in `crates/gui` - all clean.
- [x] 5.2 Launch the application on macOS with the configuration directory moved aside and confirm the
  observable sequence: no dialog before the first transfer, the runtime's row filling, the dialog gone when
  the window is usable, and a second launch that shows no dialog at all.
- [x] 5.3 Confirm Quit ends the process from inside the dialog, and that a first launch interrupted by a
  disconnected network leaves the dialog standing with the failing row where it stopped and the failure in the
  log.
