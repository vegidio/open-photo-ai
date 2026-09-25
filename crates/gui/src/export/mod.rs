//! Writing an open photograph, enhanced, to a file, and stopping an export.
//!
//! [`mod@run`] is [`export_with`], the seam the command below is a thin wrapper over. [`mod@destination`] is the
//! name a file is written under. [`mod@format`] is what the window's format and quality become. [`mod@progress`] is
//! the event an export reports on. [`mod@written`] is the record of what was written, and the reveal it gates;
//! [`mod@directory`] is the folder picker the window browses with.
//!
//! # Why this is its own module and not part of `enhance`
//!
//! It shares `enhance`'s seam and its chain, but not its slot: an export holds no result and must not disturb the one
//! the window is drawing. See design.md D3.

// The commands stay here, because `tauri::generate_handler!` in `src/lib.rs` names them through this module.

use opai::Opai;
use tauri::State;

use crate::command::{Traceparent, command_span, traced, traced_sync};
use crate::enhance::{Processor, Requested};
use crate::images::{Crop, Opened};
use crate::setup::Setup;
use crate::stops::Stops;

mod destination;
pub(crate) mod directory;
mod format;
mod progress;
mod run;
pub(crate) mod written;

use format::ExportFormat;
use run::{ExportError, Exported, Request, export_with};
pub(crate) use written::Written;

/// The kind of job [`Exports`] holds. Never constructed; it only keeps this table a type of its own.
pub(crate) enum Export {}

/// The exports in flight, by the window's name for each, and the stops that overtook their own export.
///
/// A [`Stops`] of its own rather than Autopilot's, so `cancel_suggest` can never stop an export, nor
/// [`cancel_export`] an analysis.
pub(crate) type Exports = Stops<Export>;

// The command's name is written once more, in `frontend/ipc/export.ts`.
/// Run a chain of enhancements over an open image, at its own depth, and write the result to a file.
///
/// `run` is the window's own name for this export, minted before the call — a stop and its export cross the boundary
/// independently and either may arrive first. Reports arrive under it on `export:progress`.
///
/// `operations` may be empty, which writes the framed photograph as it is. `crop` is how the user has framed it, or
/// absent to export the whole photograph.
///
/// `format` decides the bytes, whatever `destination` ends in. `quality` applies to AVIF, HEIC, JPEG and WebP and is
/// brought inside 1..=100. Without `overwrite`, a taken destination is written as `name_1.ext` and so on.
///
/// Answers the path actually written and the bytes written, or that the export was stopped. A path written is
/// recorded in [`Written`], which is what lets [`reveal_export`](written::reveal_export) show it.
///
/// # Errors
///
/// [`ExportError`]. A stop is [`Exported::Stopped`] rather than an error.
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its wire shape plus its managed state"
)]
#[tauri::command]
pub(crate) async fn export(
    app: tauri::AppHandle,
    run: String,
    source: String,
    operations: Vec<Requested>,
    processor: Processor,
    crop: Option<Crop>,
    destination: std::path::PathBuf,
    format: ExportFormat,
    quality: f64,
    overwrite: bool,
    setup: State<'_, Setup<Opai>>,
    opened: State<'_, Opened>,
    exports: State<'_, Exports>,
    written: State<'_, Written>,
    traceparent: Traceparent,
) -> Result<Exported, ExportError> {
    traced(command_span!("export", traceparent, run = run), async move {
        // A clone of the handle rather than the handle, and the lock released before the export starts — see
        // `Setup::peek`.
        let opai = setup.peek(Opai::clone).await.ok_or(ExportError::NotReady)?;
        let reporting = progress::reporting(&app, &run);

        let request = Request { run, source, operations, processor, crop, destination, format, quality, overwrite };

        // Recorded here rather than in `export_with`, so the seam's tests stay free of this record.
        let answer = export_with(&opai, &opened, &exports, Some(reporting), request).await;
        written.keep(&answer);

        answer
    })
    .await
}

// The command's name is written once more, in `frontend/ipc/export.ts`.
/// Stop an export the window asked for.
///
/// **Names the export**, and touches no other one, no analysis and no enhancement run. A stop that arrives **before**
/// its own export still lands on it. One that arrives once the file is being written lets the write finish.
///
/// Answers nothing and fails at nothing; whether anything was stopped is only in the log.
#[tauri::command]
pub(crate) fn cancel_export(run: String, exports: State<'_, Exports>, traceparent: Traceparent) {
    traced_sync(command_span!("cancel_export", traceparent, run = run), || {
        if exports.stop(&run) {
            tracing::debug!("an export in flight was stopped from the window");
        } else {
            tracing::debug!("a stop named an export that is not in flight");
        }
    });
}

#[cfg(test)]
mod tests {
    use tauri::Manager;

    use super::*;

    #[test]
    fn a_stop_for_an_export_nobody_heard_of_does_nothing_and_fails_at_nothing() {
        let app = tauri::test::mock_builder()
            .manage(Exports::default())
            .manage(crate::autopilot::Analyses::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build");

        let analyses = app.state::<crate::autopilot::Analyses>();
        let analysis = analyses.register("run-1").expect("nothing stopped it");

        cancel_export("run-1".to_string(), app.state::<Exports>(), Traceparent::default());
        cancel_export("never-asked-for".to_string(), app.state::<Exports>(), Traceparent::default());

        assert!(app.state::<Exports>().is_idle(), "a stop for no export registered one");
        assert!(!analysis.cancel().is_cancelled(), "a stop for an export reached an analysis of the same name");
    }
}
