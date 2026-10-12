## 1. The provider report

- [x] 1.1 Derive `Serialize` on `opai::SupportedProviders` with a doc note that the four fields are what a front end
      builds its provider list from; verify `cargo test -p opai` still passes and the `public_api` test covers the
      serialized shape
- [x] 1.2 Give `Setup::ready` a projection so a caller reads a value off the stored handle without the handle
      leaving, keeping the memo behaviour; verify the existing `setup.rs` tests for one-at-a-time, once-per-session
      and retry-after-failure still pass unchanged
- [x] 1.3 Widen the `initialize` command to answer with `SupportedProviders`; verify a test asserts a second call
      answers with the same report and runs no second initialization

## 2. The log file's location and revealing it

- [x] 2.1 Add a `log_path` command in `crates/gui/src/logs.rs` over `opai::logging::log_path`, with its error
      crossing the boundary as this crate's other command errors do; verify a test asserts it answers a path and
      creates nothing on a machine where the application has never run
- [x] 2.2 Add `tauri-plugin-opener` to `crates/gui/Cargo.toml`, pinned as the workspace's other Tauri plugins are,
      and **no npm counterpart** — the frontend never invokes the plugin, because the reveal goes through a command
      of this crate's own; verify `cargo build -p gui` succeeds and nothing was added to `package.json`
- [x] 2.3 Add a `reveal_log` command in `crates/gui/src/logs.rs` calling `tauri-plugin-opener`'s **Rust API** on a
      thread its platform can serve — the main thread on macOS and Windows, a blocking thread on Linux — and leave
      the plugin unregistered so its own broken commands are off the invoke surface; verify nothing is granted for
      the opener in `capabilities/default.json` and that the capability JSON still validates at build time

## 3. The model catalogue

- [x] 3.1 Add a `catalogue` command in `crates/gui/src/catalogue.rs` returning `opai::catalogue()` unchanged and
      unfiltered; verify a test asserts every family is present, including detection and colorization, and that a
      family with no adjustable parameter carries an empty parameter list

## 4. Frontend bindings

- [x] 4.1 Widen `frontend/ipc/setup.ts` so `initialize` resolves with the provider report, adding the tolerant
      frontend type for it; verify `ipc/setup.test.ts` pins the command name and the resolved shape
- [x] 4.2 Record the provider report in `frontend/stores/setup.ts` the way the plan is already recorded; verify a
      store test asserts it survives a retry and is not cleared by a failure
- [x] 4.3 Add `frontend/ipc/logs.ts` binding the `log_path` and `reveal_log` commands, with the failure surfaced to
      the caller rather than swallowed and no direct call to the opener plugin; verify `ipc/logs.test.ts` pins both
      command names and asserts a rejected reveal propagates
- [x] 4.4 Add `frontend/ipc/catalogue.ts` binding the `catalogue` command with types matching the library's
      published shape, fetched once and kept rather than per call; verify `ipc/catalogue.test.ts` pins the command
      name and asserts a second read issues no second invoke

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`, `cargo clippy` and `cargo test` across the workspace; verify all three pass
- [x] 5.2 Run the frontend `lint`, `typecheck` and `test` scripts; verify all three pass
- [x] 5.3 Launch the application on macOS and confirm it still reaches the shell, that the provider report arrives
      with the resolved `initialize`, and that revealing the log path opens Finder with the file selected
