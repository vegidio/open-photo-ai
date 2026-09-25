//! Showing a file to the user in the platform's own file manager, on a thread that survives it.
//!
//! The reveal itself is [`tauri_plugin_opener`]'s — three platform implementations this crate has no reason to own.
//! What this crate owns is the thread they run on, and that is the whole of what lives here.

// Its own module rather than a function inside the first command that needed one: the log file is one subject, and
// an image the user asks to see on disk is a second. A second caller promotes a file rather than importing across
// into the first caller's module or copying what it found there — the rule `opai::models` states for its own files.
//
// This crate calls the plugin's Rust API, not its `#[tauri::command]`, which is `async` — so Tauri runs it on a tokio
// worker, and every one of the three implementations is doing something that must not happen there:
//
// - macOS aborts the process. `NSWorkspace::activateFileViewerSelectingURLs` raises an Objective-C exception off the
//   main thread, which Rust cannot catch: `fatal runtime error: Rust cannot catch foreign exceptions`. The method's
//   own documentation says it is safe from any thread; from macOS 26.4 it is not, which Apple has acknowledged as a
//   bug (FB22340592, developer.apple.com/forums/thread/820622). Reproduced here on 26.6.2.
// - Linux panics and the promise never settles, because the plugin's blocking zbus call cannot drive a runtime from
//   inside one (tauri-apps/plugins-workspace#3552, open). The frontend is left awaiting forever, which is worse than
//   a rejection.
// - Windows is COM — `CoInitialize` and `SHOpenFolderAndSelectItems` — and wants a main-thread apartment.
//
// Only the macOS arm has been run. It is the development machine, and it is where the abort above was reproduced -
// three launches out of three on 26.6.2, the crash report putting the faulting thread at `tokio-rt-worker`. The Linux
// arm is written against the filed upstream issue rather than against a launch here, and the Windows arm against
// what COM requires; both stay unexercised until this application is first built on those platforms, and both are
// the first thing to check when it is.

use std::path::PathBuf;

use tauri::AppHandle;

// Not `Serialize`: it never crosses the boundary as itself, because it names no subject. The plugin's error is not
// `Serialize` either, which is why this carries a rendered sentence rather than the error it came from.
/// Why a reveal did not happen.
///
/// **It does not name what failed to be revealed.** Each caller knows its own subject — a log file, an opened
/// image — and says so itself, because this module is asked to show a path and is told nothing about what the path
/// means. A caller turns this into its own error with its own sentence in front of it.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct RevealError(String);

/// Reveals `path` on a thread its platform's implementation can actually run on.
///
/// # Errors
///
/// [`RevealError`] where the file manager could not be opened — including where the file is not there to be
/// selected, which the plugin reports because it canonicalizes the path before it reveals it.
#[cfg(not(target_os = "linux"))]
pub(crate) async fn reveal<R: tauri::Runtime>(app: &AppHandle<R>, path: PathBuf) -> Result<(), RevealError> {
    // The one `cfg` in this module, and every arm of it is a defect in something else — see the comment at the top of
    // this module for the three, with their references. Neither arm is a second implementation of the reveal: both
    // call the same plugin function, and what differs is only where it is called from.
    //
    // macOS and Windows: the main thread. Apple's regression needs it and COM wants it. The reply comes back over a
    // channel this awaits rather than blocks on, so a busy main thread delays the button rather than parking a runtime
    // worker.
    //
    // Generic over the runtime, as `crate::images::serve` is, so that a caller's own refusal — a path it will not
    // reveal — can be asserted against a mock app without a window.
    let (sender, mut receiver) = tauri::async_runtime::channel(1);

    app.run_on_main_thread(move || {
        // The send fails only if this task went away, which leaves the reveal done and nobody to tell. There is
        // nothing to propagate to from inside a closure the main thread runs for effect.
        let _ = sender.blocking_send(tauri_plugin_opener::reveal_item_in_dir(&path).map_err(failed));
    })
    .map_err(|error| failed(&error))?;

    receiver
        .recv()
        .await
        .unwrap_or_else(|| Err(RevealError("the file manager was asked and never answered".to_string())))
}

/// Reveals `path` as the other arm does.
///
/// # Errors
///
/// [`RevealError`], as the other arm.
#[cfg(target_os = "linux")]
pub(crate) async fn reveal<R: tauri::Runtime>(_app: &AppHandle<R>, path: PathBuf) -> Result<(), RevealError> {
    // Linux: a blocking thread, and deliberately not the main one. The plugin's zbus call must be off the runtime,
    // which is what `plugins-workspace#3552` is about. The main thread would satisfy that too and is the wrong choice:
    // `org.freedesktop.FileManager1.ShowItems` activates the file manager, so a cold one can take seconds to answer,
    // and those seconds would be the window not drawing.
    crate::task::spawn_blocking(move || tauri_plugin_opener::reveal_item_in_dir(&path))
        .await
        .map_err(|error| failed(&error))?
        .map_err(failed)
}

/// The reason a reveal failed, rendered, with no subject in front of it.
pub(crate) fn failed(error: impl std::fmt::Display) -> RevealError {
    // `pub(crate)` for the callers' sake rather than this module's: each one wraps a `RevealError` in its own
    // sentence, and the test pinning that sentence needs one to wrap. There is nothing else to build one from, because
    // every real one comes out of the plugin.
    RevealError(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The error says what went wrong and deliberately not what it was trying to show: `logs.rs` and `images/files.rs`
    /// each put their own subject in front of this sentence, so a subject here would be a second one in every message.
    #[test]
    fn the_failure_names_the_reason_and_not_the_subject() {
        let error = failed("no such file or directory");

        assert_eq!(error.to_string(), "no such file or directory");
    }
}
