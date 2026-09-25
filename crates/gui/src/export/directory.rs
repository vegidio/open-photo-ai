//! Asking the user for the folder an export writes into.

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::command::{Traceparent, command_span, traced};

// The command's name is written once more, in `frontend/ipc/export.ts`.
/// Ask the user for a folder, with the platform's own folder picker. `title` arrives pre-translated.
///
/// Answers the folder chosen, rendered lossily if it is not UTF-8 as
/// [`ImageRecord`](crate::images::files::ImageRecord)'s path is, or `None` where the picker was dismissed. **A
/// dismissal is not an error**: the window keeps its previous choice.
#[tauri::command]
pub(crate) async fn pick_directory(app: AppHandle, title: String, traceparent: Traceparent) -> Option<String> {
    traced(command_span!("pick_directory", traceparent), async move {
        // `async` for the reason `open_images` records: the plugin's blocking call must run off the main thread, while
        // the native picker itself is opened on it. A synchronous command would deadlock.
        let picked = app.dialog().file().set_title(&title).blocking_pick_folder();

        // `None` is a dismissal or a picker that failed to open, which nothing at the interface tells apart.
        // `into_path` refuses only a content:// URI, an Android-only shape, which is answered as a dismissal rather
        // than a folder.
        match picked.map(tauri_plugin_dialog::FilePath::into_path) {
            Some(Ok(path)) => Some(path.to_string_lossy().into_owned()),
            Some(Err(error)) => {
                tracing::info!(%error, "the folder picker answered something that is not a path");
                None
            }
            None => {
                tracing::debug!("the folder picker was dismissed or could not be opened");
                None
            }
        }
    })
    .await
}
