//! Where this application's log file is, and how a user is shown it.
//!
//! Showing a file in the platform's file manager — and the thread each platform's implementation has to be called
//! from — lives in [`crate::reveal`]. What is here is the subject: where the log file is, and the sentence a user
//! reads when it could not be shown.

// One command. The path is resolved in this process, immediately before the file manager is opened, so it never
// crosses the boundary in either direction. A second command answering *where* the log is would be the same
// resolution written twice, and the two could then disagree.
//
// Named for its subject rather than for the screen that asked first: the settings dialog is the first reader, not
// the only possible one.

use std::path::PathBuf;

use serde::Serialize;
use tauri::AppHandle;

use crate::command::{CommandError, Ended, Traceparent, command_span, traced};

/// Why one of this module's commands could not answer.
///
/// The tag names the step that failed rather than the command, which for this module's one command is the
/// distinction between "there is no such location" and "the file manager would not open" — two different things for
/// a caller to say.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum LogsError {
    // Neither `opai::logging::LogError` nor the plugin's error is `Serialize`, so both cross as their own sentence.
    /// The location could not be determined: the platform offers no configuration directory to resolve it from.
    #[error("{message}")]
    LogPath {
        /// What went wrong, in full, so it can be copied into a bug report.
        message: String,
    },

    // Reported rather than passed over, because the caller is a user who is already diagnosing something else: a
    // button that quietly does nothing leaves them with no file and no reason.
    /// The file manager could not be opened, or the log file was not there to be selected.
    #[error("{message}")]
    RevealLog {
        /// What went wrong, in full.
        message: String,
    },
}

impl CommandError for LogsError {
    fn ended(&self) -> Ended<'_> {
        match self {
            Self::LogPath { .. } | Self::RevealLog { .. } => Ended::Failed { error: self, recorded: false },
        }
    }
}

/// The log file for `name`, as a path.
///
/// **Creates nothing**: no directory, no file, no log sink and no record.
fn log_file(name: &str) -> Result<PathBuf, LogsError> {
    // Creating nothing is a property of `opai::logging::log_path` rather than of this wrapper — the library resolves
    // the path through `rust-sak`'s non-creating `fs::user_config_dir`.
    //
    // Split from the command so a test can ask for an application name that has never run on the machine running it,
    // which is the only way to assert the no-side-effects property against a directory that is genuinely absent.
    opai::logging::log_path(name).map_err(|error| LogsError::LogPath {
        message: format!("the log file's location could not be determined: {error}"),
    })
}

// The command's name is written once on the TypeScript side too, in `frontend/ipc/logs.ts`; nothing in either
// toolchain notices when one of the two is renamed alone.
/// Show this application's log file to the user, in the platform's own file manager, with the file selected.
///
/// Takes no path. The one it reveals is [`log_file`]'s, resolved in this process from the library's own application
/// name, so nothing the window says decides which file is opened and there is no path to validate on the way in.
///
/// # Errors
///
/// [`LogsError::LogPath`] where the location could not be determined, and [`LogsError::RevealLog`] where the file
/// manager could not be opened — including where the log file is not there to be selected.
#[tauri::command]
pub(crate) async fn reveal_log<R: tauri::Runtime>(
    app: AppHandle<R>,
    traceparent: Traceparent,
) -> Result<(), LogsError> {
    traced(command_span!("reveal_log", traceparent), async move {
        crate::reveal::reveal(&app, log_file(opai::APP_NAME)?).await.map_err(reveal_failed)
    })
    .await
}

/// The error a failed reveal crosses the boundary as, with this module's own subject in front of it.
fn reveal_failed(error: crate::reveal::RevealError) -> LogsError {
    LogsError::RevealLog { message: format!("the log file could not be shown in the file manager: {error}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An application name no machine has a configuration directory for.
    ///
    /// Composed per call rather than a constant, so two tests cannot observe each other's directory and so a run that
    /// somehow created one does not poison the next.
    fn never_run() -> String {
        format!("opai-logs-test-{}", std::time::SystemTime::UNIX_EPOCH.elapsed().unwrap().as_nanos())
    }

    #[test]
    fn it_answers_a_path_and_creates_nothing() {
        let name = never_run();
        let path = log_file(&name).expect("a path should be resolvable for an unused application name");

        let answered = path.display();
        assert!(path.is_absolute(), "a relative log path is not something a user can be sent to: {answered}");
        assert!(
            path.ends_with(std::path::Path::new("logs").join("opai.log")),
            "not the file this application logs to: {answered}"
        );

        // The whole of what `log_file` promises: asking is free.
        assert!(!path.exists(), "asking where the log is created the file");
        assert!(
            !path.parent().expect("the log file has a directory").exists(),
            "asking where the log is created its directory"
        );

        // And the application's own directory above it, which is what `user_config_dir` would have made had it been
        // the creating variant.
        let app_dir = path.parent().and_then(std::path::Path::parent).expect("the log directory has a parent");
        assert!(!app_dir.exists(), "asking where the log is created the application's configuration directory");
    }

    #[test]
    fn asking_twice_answers_the_same_place() {
        // It is derived from the application name alone, so it does not depend on whether this process is the one
        // logging there — which is what lets the reveal resolve it before anything has been written.
        let name = never_run();

        assert_eq!(log_file(&name).expect("first"), log_file(&name).expect("second"));
    }

    #[test]
    fn the_error_crossing_ipc_carries_its_own_sentence() {
        let error =
            LogsError::LogPath { message: "the log file's location could not be determined: no home".to_string() };

        assert_eq!(error.to_string(), "the log file's location could not be determined: no home");
        assert_eq!(
            serde_json::to_string(&error).expect("the error should serialize"),
            r#"{"kind":"logPath","message":"the log file's location could not be determined: no home"}"#
        );
    }

    /// The other failure, tagged with its own step.
    #[test]
    fn a_failed_reveal_is_tagged_with_its_own_command() {
        let error = reveal_failed(crate::reveal::failed("no such file or directory"));

        assert_eq!(
            serde_json::to_string(&error).expect("the error should serialize"),
            r#"{"kind":"revealLog","message":"the log file could not be shown in the file manager: no such file or directory"}"#
        );
    }

    #[test]
    fn each_failure_is_recorded_by_the_wrapper() {
        for failed in [LogsError::LogPath { message: String::new() }, LogsError::RevealLog { message: String::new() }] {
            assert_eq!(CommandError::ended(&failed).cell(), "recorded by the wrapper", "{failed:?}");
        }
    }
}
