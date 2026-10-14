//! Reporting an export's progress to the window, on an event of its own.
//!
//! An export's chain reports exactly as a canvas run's does — the same thinning, the same rule that a cached operation
//! names no stage, the same held-back final report — so those reports are [`Reporting`]'s, re-sent here wrapped as
//! [`ExportProgress::Enhancing`]. What an export adds is [`ExportProgress::Writing`]. See design.md D6.

use std::sync::Arc;

use serde::Serialize;

use crate::enhance::{Reporting, RunProgress};

/// The event an export's reports arrive under.
///
/// **Not `enhance:progress`.** That event drives the canvas chip, and an export must never move it. A separate event
/// makes that true by construction rather than by every listener filtering on the run.
///
/// Written once on the TypeScript side too, in `frontend/ipc/export.ts`.
pub(crate) const EXPORT_PROGRESS_EVENT: &str = "export:progress";

/// One report about an export, as the window reads it.
///
/// ```text
/// { run, phase: "enhancing", operation, family, stage?, chainFraction, installFraction? }
/// { run, phase: "writing" }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub(crate) enum ExportProgress {
    /// The chain is being run: one of an enhancement run's own reports, under the export's name.
    Enhancing(RunProgress),
    /// The chain is done and the file is being written. Sent once, with no fraction: the encoder has none to give.
    Writing {
        /// Which export this is about.
        run: String,
    },
}

/// Where an export's reports go.
pub(crate) struct ExportReporting {
    /// The chain's reporting, handed to [`opai::ProcessOptions::on_progress`] and released when the chain ends.
    pub(crate) enhancing: Reporting,
    /// The export's name, for the report that it is writing.
    run: String,
    /// What to do with each report.
    send: Arc<dyn Fn(ExportProgress) + Send + Sync>,
}

impl ExportReporting {
    /// Reports for the export called `run`, sent through `send`. A parameter so tests can collect what was sent.
    pub(crate) fn new(run: &str, send: impl Fn(ExportProgress) + Send + Sync + 'static) -> Self {
        let send: Arc<dyn Fn(ExportProgress) + Send + Sync> = Arc::new(send);
        let wrapped = Arc::clone(&send);

        Self {
            enhancing: Reporting::new(run, move |report| wrapped(ExportProgress::Enhancing(report))),
            run: run.to_string(),
            send,
        }
    }

    /// Reports that the export is writing its file.
    pub(crate) fn writing(&self) {
        (self.send)(ExportProgress::Writing { run: self.run.clone() });
    }
}

impl std::fmt::Debug for ExportReporting {
    /// Written out rather than derived: a callback has nothing to print.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportReporting").field("run", &self.run).finish_non_exhaustive()
    }
}

/// Reports for the export called `run`, emitted to the window under [`EXPORT_PROGRESS_EVENT`].
pub(crate) fn reporting<R: tauri::Runtime>(app: &tauri::AppHandle<R>, run: &str) -> ExportReporting {
    let emitter = app.clone();

    ExportReporting::new(run, move |report| emit(&emitter, report))
}

/// Sends one report to the window, logging and discarding a failure — there's no window to draw it, and nobody to
/// propagate to from inside this callback.
fn emit<R: tauri::Runtime>(app: &tauri::AppHandle<R>, report: ExportProgress) {
    use tauri::Emitter;

    if let Err(error) = app.emit(EXPORT_PROGRESS_EVENT, report) {
        tracing::warn!(%error, "could not report an export's progress to the window");
    }
}

/// An [`ExportReporting`] that collects what it sent, and the collection. Shared with the export seam's tests.
#[cfg(test)]
pub(super) fn collecting(run: &str) -> (ExportReporting, Arc<std::sync::Mutex<Vec<ExportProgress>>>) {
    let sent = Arc::new(std::sync::Mutex::new(Vec::new()));
    let collector = Arc::clone(&sent);

    (
        ExportReporting::new(run, move |report| collector.lock().expect("unpoisoned").push(report)),
        sent,
    )
}

#[cfg(test)]
mod tests {
    use opai::{FloatPrecision, Scale, Upscale};

    use super::*;

    /// One of `opai`'s reports about a Kyoto upscale running, at `chain` of the chain.
    fn running(chain: f64) -> opai::InferenceProgress {
        opai::InferenceProgress {
            subject: Arc::new(opai::Subject::Enhancement(Upscale::kyoto(FloatPrecision::Fp32, Scale::clamped(2.0)))),
            stage: opai::Stage::Running,
            operation_fraction: chain,
            chain_fraction: chain,
        }
    }

    #[test]
    fn an_export_reports_on_an_event_the_canvas_does_not_listen_to() {
        assert_ne!(
            EXPORT_PROGRESS_EVENT,
            crate::enhance::PROGRESS_EVENT,
            "an export's reports would move the canvas chip"
        );
    }

    #[test]
    fn a_chains_reports_are_re_sent_as_enhancing_under_the_exports_name() {
        let (reporting, sent) = collecting("export-1");

        (reporting.enhancing.on_progress)(&running(0.25));
        // Nothing a hundred-position bar could draw differently: held back, then released as the chain's last.
        (reporting.enhancing.on_progress)(&running(0.2501));
        reporting.enhancing.release();

        let sent = sent.lock().expect("unpoisoned");
        assert_eq!(sent.len(), 2, "the chain's reports were not thinned and released as a canvas run's are");

        let ExportProgress::Enhancing(first) = &sent[0] else {
            panic!("a chain's report was not sent as enhancing: {:?}", sent[0]);
        };
        assert_eq!(first.run, "export-1", "a report named a run other than the export's");

        let wire = serde_json::to_value(&sent[1]).expect("a report serializes");
        assert_eq!(wire["phase"], "enhancing");
        assert_eq!(wire["run"], "export-1");
        assert_eq!(wire["family"], "upscale");
        assert_eq!(wire["stage"], "running");
        assert_eq!(wire["chainFraction"], serde_json::json!(0.2501));
    }

    #[test]
    fn writing_is_sent_under_the_exports_name_with_no_fraction() {
        let (reporting, sent) = collecting("export-1");

        reporting.writing();

        let sent = sent.lock().expect("unpoisoned");
        assert_eq!(
            serde_json::to_value(&sent[..]).expect("a report serializes"),
            serde_json::json!([{ "phase": "writing", "run": "export-1" }])
        );
    }
}
