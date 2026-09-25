//! When the window becomes visible.
//!
//! The window ships **hidden** - `visible: false` in `tauri.conf.json` - and is shown from here, once the frontend
//! reports that it has a tree to show. [`window_ready`] is the normal way in: the frontend says it has committed.
//! [`reveal_when_late`] shows the window after a grace period regardless, empty if that is what it is.

// Without that handshake the webview is on screen from the moment the process starts and stays empty until the
// bundle has parsed, i18next has initialised and React has committed: an empty window for the better part of a
// second, which reads as a hang rather than as a launch.
//
// What the frontend reports is a commit, not a paint, and the distinction is the one thing to keep hold of here: a
// window that is ordered out does not render, so the webview produces no frames, no animation-frame callback fires
// and no font finishes loading. Every signal that means "this has been drawn" is unreachable from inside a window
// that has not been shown yet - waiting for one costs the full grace period. `frontend/main.tsx` therefore reports
// from a mount effect, which React flushes through its own scheduler, and the window is shown with a complete DOM
// behind it that paints the moment it becomes visible.
//
// Hiding it is the only thing that removes that window rather than recolouring it. `backgroundColor` in the same
// configuration block is the second half and not a substitute: it is what the *webview* paints before the
// stylesheet lands, so the frame between `reveal` and the page's own background is the application's black instead
// of the platform's white - on a reload, on a resize, and on Windows, where the empty window cannot be avoided at
// all (see below).
//
// The grace period is the guarantee that nothing else has to be true for the application to be usable - a bundle
// that throws at module scope, a dev server that is not up, a render that never finishes - all end with no call and
// a window that would otherwise never appear. A user cannot report a bug in a process that draws nothing.
//
// Windows does not honour the hidden state at all when the window is also `maximized: true`: `tao` applies the
// maximised flag by calling `ShowWindow(SW_MAXIMIZE)`, which makes the window visible, and the `set_visible(false)`
// that follows is a no-op because it diffs against a flag set that never had `VISIBLE` in it. So on Windows the
// empty window is still on screen, in the colour `backgroundColor` gives it, and `reveal` simply arrives at a window
// that is already up. macOS and Linux hide as asked.

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

use crate::command::{Traceparent, command_span, traced_sync};

// Long enough that it is never reached on a launch that works - the delay this module exists to hide is around a
// second, and a cold `pnpm dev` start, measured here at under three, is the slowest case - and short enough that a
// user meeting the failure it covers is looking at a window rather than at nothing.
/// How long the window may stay hidden waiting for a frontend that might never report.
const GRACE: Duration = Duration::from_secs(5);

// The command's name is written once on the TypeScript side too, in `frontend/ipc/window.ts`; nothing in either
// toolchain notices when one of the two is renamed alone.
/// The frontend has a tree in the DOM; show the window.
///
/// Called once per webview load, from the mount effect in `frontend/main.tsx` - a commit being reported rather than a
/// paint, for the reason the comment at the top of this module gives. StrictMode makes that call twice in development
/// and a reload makes it again against a window that is already visible, both of which [`reveal`] answers by doing
/// nothing.
///
/// The window is taken as an argument rather than looked up by label: Tauri injects the window the call came from,
/// so what is shown is what reported, and this needs no opinion about how many windows there are.
#[tauri::command]
pub(crate) fn window_ready(window: WebviewWindow, traceparent: Traceparent) {
    traced_sync(command_span!("window_ready", traceparent), || reveal(&window));
}

/// Show the window, unless it is already up.
///
/// A refusal is recorded and discarded.
fn reveal<R: Runtime>(window: &WebviewWindow<R>) {
    // Discarded because both callers return `()` to something that cannot act on an error - a frontend that has
    // already painted, and a timer thread with nobody waiting on it - and the cost of a window that will not show is
    // the same either way. The record is what makes it diagnosable, and the log sink is installed well before either
    // caller runs.
    //
    // Asked rather than tracked, because the two callers race by design: the grace period can expire in the moment
    // between the frontend's call and its effect. `show()` twice is harmless on every platform, so this is about the
    // log staying honest rather than about correctness - a second `show` should not read as a second launch.
    if window.is_visible().unwrap_or(false) {
        return;
    }

    if let Err(error) = window.show() {
        tracing::error!(%error, "could not show the main window; the application is running with nothing on screen");
    }

    // Reached even when the `show` above failed, rather than returned from: tao's `set_focus` is a no-op on a window
    // that is not visible, so the failed case costs a call and nothing else, and the branch it would save has no
    // reader. A warning rather than an error because what a refusal here costs is one launch that opens behind
    // another window, not a launch with nothing on screen.
    if let Err(error) = window.set_focus() {
        tracing::warn!(%error, "could not bring the main window to the front; it may have opened behind another application");
    }
}

/// Start the timer that shows the window whether or not the frontend ever reports.
///
/// **An absent window is a no-op**, for the same reason it is in [`crate::single_instance::raise_main_window`]: the
/// label is looked up rather than held, and a process that is shutting down during the grace period has none.
pub(crate) fn reveal_when_late<R: Runtime>(app: &AppHandle<R>) {
    // A detached thread rather than `tauri::async_runtime::spawn`: it sleeps and then touches one window, so a runtime
    // worker buys nothing, and a detached thread does not hold the process open past `main`. `show()` is dispatched to
    // the event loop by Tauri, so calling it from here is as safe as calling it from the command.
    let app = app.clone();

    std::thread::spawn(move || {
        std::thread::sleep(GRACE);

        // `"main"` is the label `tauri.conf.json` declares, written out here and in `single_instance.rs` for the
        // same reason both are `get_webview_window` calls rather than a stored handle: the window is Tauri's, and
        // the label is the only name either module has for it.
        let Some(window) = app.get_webview_window("main") else {
            return;
        };

        if window.is_visible().unwrap_or(false) {
            return;
        }

        // A warning rather than a debug record: reaching this means the frontend did not finish starting, so
        // whatever the user reports next begins here.
        tracing::warn!(
            grace = ?GRACE,
            "the frontend never reported that it had something to show; showing the window anyway"
        );

        reveal(&window);
    });
}

#[cfg(test)]
mod tests {
    use tauri::{
        WebviewWindowBuilder,
        test::{mock_builder, mock_context, noop_assets},
    };

    use super::*;

    /// The reveal runs against the real call sequence, and is idempotent.
    ///
    /// **What this does not prove**, for the reason the raise test states the same thing: `MockRuntime`'s window
    /// methods are no-ops that succeed, so nothing here shows a real window came up or took the front. What it
    /// covers is that `is_visible`, `show` and `set_focus` are called on the window that was handed in and that a
    /// second call is harmless - the race between the command and the grace timer is the whole reason the guard is
    /// there.
    #[test]
    fn the_reveal_runs_and_repeats_harmlessly() {
        let app = mock_builder().build(mock_context(noop_assets())).expect("the mock app should build");

        // Built here rather than read from tauri.conf.json, for the reason `single_instance`'s raise test gives.
        let window = WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
            .build()
            .expect("the mock window should build");

        reveal(&window);
        reveal(&window);
    }

    /// The timer tolerates an application with no window.
    ///
    /// This test does not wait out the grace period - what it pins is that arming the timer costs the caller nothing
    /// and cannot panic on the way out, which is the whole of what `run()` depends on.
    #[test]
    fn arming_the_timer_needs_no_window() {
        let app = mock_builder().build(mock_context(noop_assets())).expect("the mock app should build");
        assert!(app.get_webview_window("main").is_none(), "this app is deliberately window-less");

        reveal_when_late(app.handle());
    }
}
