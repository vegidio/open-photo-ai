## 1. The plugin

- [x] 1.1 Add `tauri-plugin-single-instance` to `crates/gui/Cargo.toml`, pinned as `"2"` alongside the
  Tauri core, with **no features** — and a comment recording D4: the `semver` feature is off on purpose,
  because it version-qualifies the identifier and two versions sharing one configuration directory is
  the situation the claim exists to prevent. Verify `cargo build -p gui` succeeds and that
  `cargo tree -p gui -i tauri-plugin-single-instance` shows no `semver` feature enabled.
- [x] 1.2 Add `tauri` with the `test` feature as a dev-dependency of `gui`, for the mock-runtime test in
  3.2. Verify `cargo test -p gui` still passes with the existing drift guard as its only test.

## 2. The raise

- [x] 2.1 Write the raise as a free function generic over `Runtime` (D3): look up the window labelled
  `main`, and on `Some` call `show()`, then `unminimize()`, then `set_focus()` in that order, logging and
  discarding each error. **Do not call `unmaximize`** — the window ships `maximized: true` and Go's
  comment is explicit that un-maximising it is the bug. Document the ordering and that refusal in the
  function's own doc comment, since it is the part most likely to be "simplified" later. Verify
  `cargo build -p gui`.
- [x] 2.2 Make the absent-window case an explicit no-op (D3): `None` from the lookup returns without
  panicking and without error, because the plugin can fire the callback before the window is created.
  Verify by the mock-runtime test in 3.2.
- [x] 2.3 Register the plugin as the **first** `.plugin(..)` call on the builder in `run()`, with a
  callback that binds the second launch's arguments and working directory as `_args` and `_cwd` and
  calls the function from 2.1. Record D2 at the call site: the plugin exits the second process from
  inside its own setup hook, so anything registered ahead of it runs in a process about to vanish, and
  a future `Opai::initialize` must stay after it. Verify `cargo build -p gui` and that launching the app
  twice on macOS raises the first window rather than opening a second.

## 3. Proving what can be proven

- [x] 3.1 Write the session-bus decision as a pure function taking the address as `Option<&str>` and the
  runtime directory as `Option<&Path>`, **not** `cfg`-gated, implementing D5's three rules: a non-empty
  address that is not `autolaunch:` and contains a `:` separator registers; an unset address registers
  only if `bus` or `dbus-session` exists in the runtime directory; anything else skips. Verify a table
  test covers unset-with-socket, unset-without-socket, unset-with-no-runtime-dir, empty, `autolaunch:`,
  a separator-less value and a well-formed `unix:path=...`, and that it runs on every CI platform
  because the function is not gated.
- [x] 3.2 Verify the raise against Tauri's mock runtime: build a `mock_builder` app with a `main` window
  and assert the function from 2.1 runs and returns, then build one without `main` and assert it returns
  without panicking. State in the test's own comment what it does not prove — `MockRuntime`'s window
  methods are succeeding no-ops, so this covers the code path and the `None` guard, never that a real
  window was shown or focused (D8).
- [x] 3.3 Extend the comment on the existing `tauri_conf_json_agrees_with_the_crate` test in
  `crates/gui/src/lib.rs` to record that `identifier` now also keys the single-instance activation on all
  three platforms — the D-Bus name, the Win32 mutex and class names, and the macOS socket path — so the
  assertion is not read as bundle-identity-only and weakened later. Verify the test still passes
  unchanged.

## 4. Wiring the gate

- [x] 4.1 Call the decision function from 3.1 behind `#[cfg(target_os = "linux")]`, reading
  `DBUS_SESSION_BUS_ADDRESS` and `XDG_RUNTIME_DIR`, and skip registering the plugin when it returns
  false, logging that the application is starting without single-instance activation. Provide the
  `#[cfg(not(target_os = "linux"))]` counterpart that always registers. Verify `cargo build -p gui` on
  macOS and that `cargo clippy -p gui --all-targets` is clean on both branches.
- [x] 4.2 Verify the gate errs toward skipping rather than crashing (D5), by a test asserting the
  decision is false for `DBUS_SESSION_BUS_ADDRESS=""` — the container shape that would otherwise reach
  the plugin's `unwrap()` and panic inside `Builder::build()` — and that no `libc` or `rustix`
  dependency was added to reach it.

## 5. Closing out

- [x] 5.1 Extend `README.md`'s "One process at a time" section with the GUI's exception: a second launch
  of the graphical application raises the window that is already open and exits successfully instead of
  reporting an error, and on Linux that activation is skipped when no session bus is reachable. Note
  that the GUI does not yet take the claim itself (D9), so a GUI started while the CLI holds it is not
  yet refused.
- [x] 5.2 Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings` and `cargo test`, and verify
  all three are clean. The frontend is untouched by this change (D6), so its lint, typecheck and test
  scripts need no new run beyond whatever CI already does.
