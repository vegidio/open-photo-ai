//! The files this session's exports wrote, and showing one of them in the file manager.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};

use super::run::{ExportError, Exported};
use crate::command::{CommandError, Ended, Traceparent, command_span, traced};
use crate::sync::lock;

/// Every path an export of this session wrote — the only files [`reveal_export`] will show.
///
/// Nothing is ever removed, as nothing is from [`Opened`](crate::images::Opened): a path per exported file is noise.
///
/// Its own record rather than [`Opened`](crate::images::Opened)'s, because admitting an export's output there would
/// make it servable as an opened image, which it is not.
#[derive(Debug, Default)]
pub(crate) struct Written(Mutex<HashSet<PathBuf>>);

impl Written {
    /// Records the path `answer` says was written, where it says one was. A stop and a failure wrote nothing, and
    /// record nothing.
    ///
    /// The path recorded is the one the answer names — the numbered name where the destination asked for was taken.
    pub(crate) fn keep(&self, answer: &Result<Exported, ExportError>) {
        if let Ok(Exported::Exported { path, .. }) = answer {
            lock(&self.0).insert(PathBuf::from(path));
        }
    }

    /// Whether an export of this session wrote this path. Compared verbatim, not canonicalized: both sides come from
    /// the same string, the one [`Exported::Exported`] answered.
    fn holds(&self, path: &Path) -> bool {
        lock(&self.0).contains(path)
    }
}

// Shaped as `crate::images::files::ImagesError` is: a caller reading `kind` is reading what it asked for.
/// Why [`reveal_export`] showed nothing.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum RevealExportError {
    // One variant, since both look the same to the window — the log records which it was.
    /// The file is not one this session exported, or the file manager would not open it.
    #[error("{message}")]
    RevealExport {
        /// What went wrong, in full.
        message: String,
    },
}

impl CommandError for RevealExportError {
    fn ended(&self) -> Ended<'_> {
        match self {
            Self::RevealExport { .. } => Ended::Failed { error: self, recorded: false },
        }
    }
}

// The command's name is written once more, in `frontend/ipc/export.ts`.
/// Show a file an export wrote, in the platform's own file manager, with the file selected.
///
/// Takes the path an export answered, and **refuses one no export of this session wrote**, for the reason
/// `images/files.rs` records for refusing to reveal unvetted paths.
///
/// # Errors
///
/// [`RevealExportError::RevealExport`] for a path no export wrote, and for a file manager that would not open —
/// including a file since moved or deleted.
#[tauri::command]
pub(crate) async fn reveal_export<R: tauri::Runtime>(
    app: AppHandle<R>,
    path: String,
    written: State<'_, Written>,
    traceparent: Traceparent,
) -> Result<(), RevealExportError> {
    traced(command_span!("reveal_export", traceparent), async move {
        let path = PathBuf::from(path);

        // The window can only name a path an export answered, so this is a bug in it. The command wrapper records it,
        // which is why the refusal names the path.
        if !written.holds(&path) {
            return Err(RevealExportError::RevealExport {
                message: format!("`{}` is not a file this session has exported", path.display()),
            });
        }

        crate::reveal::reveal(&app, path).await.map_err(|error| RevealExportError::RevealExport {
            message: format!("the exported file could not be shown in the file manager: {error}"),
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use tauri::Manager;

    use super::*;
    use crate::enhance::EnhanceError;

    fn exported(path: &str) -> Result<Exported, ExportError> {
        Ok(Exported::Exported { path: path.to_string(), bytes: 42 })
    }

    #[test]
    fn an_exported_file_is_recorded_under_the_path_actually_written() {
        let written = Written::default();

        // The answer `export_with` gives when `beach-opai.png` was taken: the numbered name, not the one asked for.
        written.keep(&exported("/exports/beach-opai_1.png"));

        assert!(written.holds(Path::new("/exports/beach-opai_1.png")));
        assert!(!written.holds(Path::new("/exports/beach-opai.png")), "the name asked for was recorded");
    }

    #[test]
    fn a_stopped_or_failed_export_records_nothing() {
        let written = Written::default();

        written.keep(&Ok(Exported::Stopped));
        written.keep(&Err(ExportError::Write {
            path: "/exports/beach-opai.png".to_string(),
            message: "permission denied".to_string(),
        }));
        written.keep(&Err(EnhanceError::Enhance { message: "the chain failed".to_string() }.into()));

        assert!(lock(&written.0).is_empty(), "an export that wrote nothing was recorded");
    }

    #[test]
    fn a_reveal_naming_a_file_no_export_wrote_is_refused_before_anything_opens() {
        // As `reveal_image`'s own test: the refusal happens before `crate::reveal` is reached, so were the check
        // absent this would put Finder in front of whoever ran it.
        let app = tauri::test::mock_builder()
            .manage(Written::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build");
        app.state::<Written>().keep(&exported("/exports/beach-opai_1.png"));

        for path in ["/etc/passwd", "/exports/beach-opai.png"] {
            let refused = tauri::async_runtime::block_on(reveal_export(
                app.handle().clone(),
                path.to_string(),
                app.state::<Written>(),
                Traceparent::default(),
            ))
            .expect_err("a file no export wrote was revealed");

            assert_eq!(refused.to_string(), format!("`{path}` is not a file this session has exported"));
        }
    }

    #[test]
    fn a_failed_reveal_is_tagged_with_its_own_command() {
        let error = RevealExportError::RevealExport { message: "the file manager would not open".to_string() };

        assert_eq!(
            serde_json::to_string(&error).expect("the error should serialize"),
            r#"{"kind":"revealExport","message":"the file manager would not open"}"#
        );
    }

    #[test]
    fn a_failed_reveal_is_recorded_by_the_wrapper() {
        let failed = RevealExportError::RevealExport { message: String::new() };

        assert_eq!(CommandError::ended(&failed).cell(), "recorded by the wrapper");
    }
}
