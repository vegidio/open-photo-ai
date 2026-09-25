//! What a *second* launch of the graphical application does.
//!
//! Nothing here enforces the single-instance invariant - that is `opai`'s advisory lock, taken in
//! `Opai::initialize`, which the GUI calls from `setup::initialize`. This module is the gesture layered over it:
//! someone double-clicked the icon while the application was already running, which is not a mistake
//! and not a question. It is a request for their window, so the second process hands the first one a
//! message and exits successfully, and the first one raises what the user was asking for.

use std::path::Path;

use tauri::{Manager, Runtime};

/// Raise the running instance's window: show it, un-minimise it, focus it - in that order.
///
/// **An absent window is a no-op.** Each call's error is logged and discarded.
pub(crate) fn raise_main_window<R: Runtime>(app: &impl Manager<R>) {
    // The plugin claims the platform's name during `Builder::build()`, before the window is created, so
    // the callback can fire in that gap. `None` here means the first process is still starting: it is
    // unharmed, the second process has already exited, and doing nothing is the right answer.
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    // The order is load-bearing. A hidden window cannot take focus, and `unminimize` does nothing
    // unless the window really is minimised, so `show` must come first and `set_focus` last. Tauri
    // behaves the same way the Wails original does here, and the upstream report of it (tauri#12936,
    // *"Can't set window focus by the single instance plugin when the window was hidden"*) is closed as
    // not planned - show-before-focus is the caller's job, not something a later version fixes.
    //
    // `unmaximize` is deliberately never called, and this is the part most likely to be "simplified"
    // later by reaching for the obvious restore verb. The window ships `maximized: true`, so
    // un-maximising it would shrink the window of nearly every user who triggers this. Someone who
    // launched the app a second time asked to *see* it, not to resize it.
    //
    // Errors are discarded because the callback returns `()` and the process that asked for the raise
    // is gone, so there is nobody to propagate to; a window that refuses to show costs the user one
    // lost raise, which is a supported outcome.
    //
    // Records rather than `stderr`: this runs in the application already running, whose log sink is installed, and a
    // console nobody reads is no place for the reason a window did not come up.
    if let Err(error) = window.show() {
        tracing::warn!(%error, "could not show the main window on a second launch");
    }

    if let Err(error) = window.unminimize() {
        tracing::warn!(%error, "could not un-minimise the main window on a second launch");
    }

    if let Err(error) = window.set_focus() {
        tracing::warn!(%error, "could not focus the main window on a second launch");
    }
}

/// Whether the plugin is safe to register given this Linux session's D-Bus configuration. It errs
/// toward **skipping**:
///
/// 1. A non-empty address that is not `autolaunch:` and that carries a `:` transport separator →
///    register.
/// 2. Address unset → register only if the runtime directory holds `bus` or `dbus-session`.
/// 3. Anything else → skip.
///
/// `runtime_dir` is `$XDG_RUNTIME_DIR`.
// Only the Linux build has a caller; everywhere else this is compiled and exercised by its test alone,
// which is the entire point of not `cfg`-gating it. A crash-at-launch rule checked on one CI leg is a
// rule checked nowhere most of the time.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn session_bus_is_usable(address: Option<&str>, runtime_dir: Option<&Path>) -> bool {
    // The plugin's Linux backend opens its connection with
    // `zbus::blocking::connection::Builder::session().unwrap()`. `Builder::session()` resolves an address
    // and does no I/O, so:
    //
    // - No bus running, `DBUS_SESSION_BUS_ADDRESS` unset - the fallback address
    //   (`unix:path=$XDG_RUNTIME_DIR/bus`) always parses, `.build()` then fails to connect, and the
    //   plugin's own `_ => {}` arm starts the application unguarded. Already graceful; needs no gate.
    // - The variable set but unparseable - that `.unwrap()` is a panic *inside `Builder::build()`*,
    //   at launch, before any window exists. The empty string is the shape that actually occurs:
    //   containers and stripped-down sessions export `DBUS_SESSION_BUS_ADDRESS=` rather than unsetting it.
    //
    // So this gate exists to prevent a panic, not to make a headless box work - that already works. It
    // errs toward skipping because skipping costs the raise and registering wrongly costs the launch.
    //
    // The rules mirror the first two steps of the Go app's `hasSessionBus`. `autolaunch:` is skipped for
    // the reason godbus skips it - it is a request to spawn a *private* bus rather than the address of one
    // that already exists, and a bus no other instance shares would leave the name always free and the
    // plugin guarding nothing.
    //
    // `runtime_dir` is `$XDG_RUNTIME_DIR` rather than `/run/user/<euid>` as Go derives it. That is what
    // the directory is called on every logind session and what `zbus` itself falls back to, and it avoids
    // adding `libc` or `rustix` (plus an `unsafe` block, in a workspace that warns on undocumented ones)
    // for one integer. The case it misses - `XDG_RUNTIME_DIR` unset, the address unset, and a live bus at
    // `/run/user/<uid>/bus` - loses the raise where it could have worked, which is the harmless direction.
    //
    // The residual: an address that is non-empty and carries a `:` but names a transport `zbus` does
    // not know still reaches that `unwrap()`. Closing it would mean keeping an allowlist of `zbus`'s
    // transports here, which would age against `zbus` silently. The shapes that occur are both closed.
    match address {
        Some(address) if !address.is_empty() => !address.starts_with("autolaunch:") && address.contains(':'),
        Some(_) => false,
        None => runtime_dir.is_some_and(|dir| dir.join("bus").exists() || dir.join("dbus-session").exists()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tauri::{
        Manager, WebviewWindowBuilder,
        test::{mock_builder, mock_context, noop_assets},
    };

    use super::*;

    /// The raise runs, and runs without a window.
    ///
    /// **What this does not prove**, stated because a test that looked like it verified focus would be
    /// worse than no test at all: `MockRuntime`'s window methods are no-ops that succeed, so nothing
    /// here shows that a real window was shown, un-minimised or focused. What it covers is that the
    /// path executes against the real `get_webview_window` lookup and the real call sequence, and that
    /// the absent-window branch returns rather than panicking. The real behaviour is a manual check on
    /// macOS and unverified on Windows and Linux.
    #[test]
    fn the_raise_runs_with_a_window_and_without_one() {
        let app = mock_builder().build(mock_context(noop_assets())).expect("the mock app should build");

        // The window is built here rather than read from tauri.conf.json: `generate_context!` embeds
        // the Info.plist and can only be expanded once per crate, and `run()` already does. Only the
        // label matters to the lookup, and it is the same `main` the configuration declares.
        WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
            .build()
            .expect("the mock window should build");

        assert!(app.get_webview_window("main").is_some(), "the mock app should carry the `main` window");
        raise_main_window(&app);

        // The gap the plugin can fire in: the plugin claims the platform's name during
        // `Builder::build()`, so the callback can reach this before any window has been created. A
        // separate app with no `main` is how that is reached - closing the window built above would
        // only queue a close event that no event loop is running to process.
        let starting = mock_builder().build(mock_context(noop_assets())).expect("the mock app should build");
        assert!(starting.get_webview_window("main").is_none(), "this app is deliberately window-less");
        let (recorded, ()) = crate::test_support::recorded(|| raise_main_window(&starting));

        // Nothing failed, because nothing was tried: the absent window is the ordinary gap, not a failure to record.
        assert!(recorded.events.is_empty(), "a raise with no window recorded something: {:#?}", recorded.events);
    }

    /// The gate's decision table, running on every platform because the function is not `cfg`-gated.
    #[test]
    fn the_session_bus_decision_covers_every_shape() {
        let dir = tempdir();

        // 1. A well-formed address registers.
        assert!(session_bus_is_usable(Some("unix:path=/run/user/1000/bus"), None));

        // 2. The empty variable skips - the container shape the gate exists for.
        assert!(!session_bus_is_usable(Some(""), Some(&dir)));

        // 3. `autolaunch:` skips: a private bus no other instance shares guards nothing.
        assert!(!session_bus_is_usable(Some("autolaunch:"), None));
        assert!(!session_bus_is_usable(Some("autolaunch:scope=user"), None));

        // 4. A value with no transport separator is not an address `zbus` can parse.
        assert!(!session_bus_is_usable(Some("nonsense"), None));

        // 5. Unset, with a socket in the runtime directory, registers - either name.
        assert!(!session_bus_is_usable(None, Some(&dir)), "no socket yet");
        fs::write(dir.join("bus"), b"").expect("the runtime directory should be writable");
        assert!(session_bus_is_usable(None, Some(&dir)));
        fs::rename(dir.join("bus"), dir.join("dbus-session")).expect("the socket should be renamable");
        assert!(session_bus_is_usable(None, Some(&dir)));

        // 6. Unset with no runtime directory at all skips.
        assert!(!session_bus_is_usable(None, None));

        fs::remove_dir_all(&dir).ok();
    }

    /// The gate errs toward skipping rather than crashing.
    ///
    /// Its own test rather than one row of the table above, because the empty variable is the shape
    /// [`session_bus_is_usable`] exists for.
    #[test]
    fn an_empty_dbus_address_skips_rather_than_panicking() {
        let dir = tempdir();
        fs::write(dir.join("bus"), b"").expect("the runtime directory should be writable");

        // False even with a perfectly good socket sitting next to it: an unparseable address is not
        // something the runtime directory can rescue, because the plugin never reaches the fallback.
        assert!(!session_bus_is_usable(Some(""), Some(&dir)));

        fs::remove_dir_all(&dir).ok();
    }

    /// A scratch directory: the gate stats paths, so the test only needs a directory that exists.
    fn tempdir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("opai-si-{}-{:?}", std::process::id(), std::thread::current().id()));
        fs::create_dir_all(&dir).expect("the temp directory should be creatable");
        dir
    }
}
