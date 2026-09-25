//! Reporting a run's progress to the window, thinned to what an indicator can actually draw.
//!
//! A global event rather than a channel, because a run carries its own name — see [`PROGRESS_EVENT`] and
//! [`Thinning`].

use std::sync::Mutex;

use opai::Family;
use serde::Serialize;

use crate::sync::lock;

/// The event a run's reports arrive under.
///
/// A global event rather than a channel (unlike `setup.rs`'s), because a run carries its own name in
/// [`RunProgress::run`], letting a window subscribed once tell a report about its current run apart from one
/// about a run it abandoned — a per-call channel for a displaced run would keep delivering regardless.
///
/// **Two senders, one event.** A chain reports here — including the detection it makes for a face recovery, mapped
/// onto the head of its own bar by [`Handover`] — and so does the picker's detection [`crate::faces::detect_faces`]
/// runs: the window draws one indicator, every report already names its run, and a second event would be a second
/// subscription to unregister for a chip that can only ever show one thing. A detection reports under
/// [`Family::Detection`], which the frontend names Face Recovery — see design.md D8.
///
/// Written once on the TypeScript side too, in `frontend/ipc/enhance.ts`.
pub(crate) const PROGRESS_EVENT: &str = "enhance:progress";

/// One report about a run, as the window reads it.
///
/// `opai`'s own progress types don't derive `serde` and are `#[non_exhaustive]`, so this crate defines its own
/// thin wire shape.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// The fields are `pub(crate)` rather than private because the tests that read them are no longer all in this
/// module: `crate::faces` reports on this event too, and what its tests assert is what the window receives.
/// Nothing outside a test constructs or mutates one — [`describe`] is the only writer.
pub(crate) struct RunProgress {
    /// Which run this is about, so a report from an abandoned run is discardable.
    pub(crate) run: String,
    /// The operation being carried out, as a user would see it named: `Kyoto 4x (FP16)`. A diagnostic; the
    /// interface should use [`RunProgress::family`] instead.
    operation: String,
    /// Which enhancement the operation belongs to, spelled as [`Family`] spells itself — sent directly rather
    /// than parsed out of `operation` or reconstructed from the chain, which would break once a chain held two
    /// operations of one family. See design.md D7.
    pub(crate) family: Family,
    /// Whether the model is being fetched or being run.
    ///
    /// **Absent** for an operation already served from cache — it did neither, and labelling it `Running`
    /// would jump the bar with no explanation. See design.md D5.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stage: Option<Stage>,
    /// How far the whole chain has got, in `0..1`. Never decreases; reaches 1 exactly once.
    pub(crate) chain_fraction: f64,
    /// How far the fetch itself has got, in `0..1`, while one is happening.
    ///
    /// A fetch occupies only the head of one operation's share of the chain, so `chain_fraction` barely moves
    /// during a multi-gigabyte download — this is what makes that visible rather than looking like a stall.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) install_fraction: Option<f64>,
}

/// What a report says is happening to an operation. Two arms, not three: a cached operation is reported by the
/// **absence** of this — see [`RunProgress::stage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Stage {
    /// The operation's model is not on the machine and is being fetched.
    Installing,
    /// The model is on the machine and is being run over the image.
    Running,
}

/// Which report this is, for deciding whether the one before it was the last of its kind.
///
/// Includes the operation as well as the stage: two operations both running are two distinct stages, and a
/// handover with no boundary report would silently relabel the bar.
#[derive(Debug, Clone, PartialEq)]
struct Which {
    /// What the report is about, compared by value — `opai` shares one `Arc` per operation.
    subject: std::sync::Arc<opai::Subject>,
    /// Which of the stages was happening.
    stage: Option<Stage>,
}

/// How many reports a run is allowed to put on the wire.
///
/// A tiled run reports per tile — thousands of times — while a progress indicator has about a hundred
/// distinct positions. Thinning here (rather than in React, as the reference does) means the extra messages
/// are never serialized or sent at all.
///
/// The rule, taken from the reference:
///
/// - emit when the rounded whole-chain percentage changes, **or** the rounded install percentage does;
/// - always emit the **first** report of each stage;
/// - always emit the **last** report of each stage — held back and flushed on transition or run end, via
///   [`pending`](Self::pending) and [`finish`](Self::finish).
///
/// # What this deliberately does not do
///
/// A run that stalls stops reporting, and the indicator sits still rather than lying about progress — same
/// behaviour as the reference.
#[derive(Debug, Default)]
struct Thinning {
    /// The last report actually emitted, as the two percentages it carried and what it was about.
    seen: Option<(Which, u8, Option<u8>)>,
    /// The most recent report that was **not** emitted, held so the last of its stage is never lost.
    pending: Option<RunProgress>,
}

impl Thinning {
    /// The reports to emit for `report`: none, one, or two (the held-back last of a stage that ended, then
    /// the first of the one that began — in that order).
    fn observe(&mut self, report: RunProgress, which: Which, install: Option<u8>) -> Vec<RunProgress> {
        // Rounded to whole percentages, the resolution an indicator actually has. Saturating cast: fractions
        // are documented as `0.0..=1.0`, so this only bounds a fault rather than wrapping it.
        let chain = percent(report.chain_fraction);

        match &self.seen {
            // The first report of the run, also the first of its stage.
            None => self.emit(report, which, chain, install),
            // A stage that just ended: flush what was held back from it, then emit this one.
            Some((last, _, _)) if *last != which => {
                let mut emitted = self.pending.take().into_iter().collect::<Vec<_>>();
                emitted.extend(self.emit(report, which, chain, install));

                emitted
            }
            // Same stage, and something a hundred-position indicator could actually draw differently.
            Some((_, last_chain, last_install)) if *last_chain != chain || *last_install != install => {
                self.emit(report, which, chain, install)
            }
            // Nothing drawable. Held rather than dropped, in case it turns out to be the stage's last report.
            _ => {
                self.pending = Some(report);

                Vec::new()
            }
        }
    }

    /// Records `report` as emitted and answers it as the one thing to send.
    fn emit(&mut self, report: RunProgress, which: Which, chain: u8, install: Option<u8>) -> Vec<RunProgress> {
        self.seen = Some((which, chain, install));
        self.pending = None;

        vec![report]
    }

    /// The report held back from the run's final stage, if any. Call when the run ends, whatever the outcome.
    fn finish(&mut self) -> Option<RunProgress> {
        self.pending.take()
    }
}

/// A fraction in `0..1` as a whole percentage. Saturating and clamped, so a fault produces a rounding error
/// rather than a percentage jumping from 100 to 3.
fn percent(fraction: f64) -> u8 {
    (fraction.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// The callback handed to [`opai::ProcessOptions::on_progress`], emitting thinned reports under
/// [`PROGRESS_EVENT`].
pub(crate) fn reporting<R: tauri::Runtime>(app: &tauri::AppHandle<R>, run: &str) -> Reporting {
    let emitter = app.clone();

    Reporting::new(run, move |report| emit(&emitter, report))
}

impl Reporting {
    /// Reports for `run`, sent through `send`. `send` is a parameter (rather than a window emit written
    /// inline) so tests can collect what was sent.
    pub(crate) fn new(run: &str, send: impl Fn(RunProgress) + Send + Sync + 'static) -> Self {
        let thinning = std::sync::Arc::new(Mutex::new(Thinning::default()));

        let state = std::sync::Arc::clone(&thinning);
        let run = run.to_string();

        // Shared via `Arc` so both the callback and the flush send through the same sender.
        let sender: std::sync::Arc<dyn Fn(RunProgress) + Send + Sync> = std::sync::Arc::new(send);
        let callback = std::sync::Arc::clone(&sender);

        let on_progress: opai::OnInference = std::sync::Arc::new(move |report: &opai::InferenceProgress| {
            let (wire, which, install) = describe(&run, report);

            // Lock held across the thinning only, released before the send.
            let emitted = {
                let mut thinning = lock(&state);

                thinning.observe(wire, which, install)
            };

            for report in emitted {
                callback(report);
            }
        });

        Self { on_progress, thinning, flush: sender }
    }

    /// Sends the report the run's final stage held back, if it held one.
    pub(crate) fn release(&self) {
        let last = {
            let mut thinning = lock(&self.thinning);

            thinning.finish()
        };

        if let Some(report) = last {
            (self.flush)(report);
        }
    }
}

impl std::fmt::Debug for Reporting {
    /// Written out rather than derived, as `ProcessOptions`'s own is: a callback has nothing to print.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporting").field("thinning", &self.thinning).finish_non_exhaustive()
    }
}

/// One of `opai`'s reports as the window reads it, beside what the thinning needs to judge it by.
fn describe(run: &str, report: &opai::InferenceProgress) -> (RunProgress, Which, Option<u8>) {
    let (stage, install_fraction) = match &report.stage {
        opai::Stage::Installing(progress) => (Some(Stage::Installing), Some(progress.fraction)),
        opai::Stage::Running => (Some(Stage::Running), None),
        opai::Stage::Cached => (None, None),
        // A stage this crate doesn't know (the library's enum is `#[non_exhaustive]`): advance the chain and
        // claim no label, same as `Cached`, rather than guess at `Running`.
        _ => (None, None),
    };

    let wire = RunProgress {
        run: run.to_string(),
        operation: report.subject.display_name(),
        family: report.subject.family(),
        stage,
        chain_fraction: report.chain_fraction,
        install_fraction,
    };
    let which = Which { subject: std::sync::Arc::clone(&report.subject), stage };

    (wire, which, install_fraction.map(percent))
}

/// Sends one report to the window, logging and discarding a failure — there's no window to draw it (reload,
/// close) and nobody to propagate to from inside this callback.
fn emit<R: tauri::Runtime>(app: &tauri::AppHandle<R>, report: RunProgress) {
    use tauri::Emitter;

    if let Err(error) = app.emit(PROGRESS_EVENT, report) {
        tracing::warn!(%error, "could not report an enhancement's progress to the window");
    }
}

// ── One bar over a detection and the chain it was run for ───────────────────────────────────────────────────────────

/// How much of the bar a detection owns, before the chain that asked for it starts.
///
/// A chain carrying a face recovery is two runs underneath — the detection `crate::faces::for_chain` makes and the
/// chain itself — and each reports its own `0..1` and lands on exactly 1. Drawn as they arrive that is two sweeps: the
/// bar fills, empties and fills again, which reads as the enhancement having been applied twice. So the detection is
/// mapped onto the head of the range and the chain onto what it leaves.
///
/// The figure is the reference's own `progressAfterDetect`, which reserved exactly this fifth inside its face recovery
/// because its package acquired the detector itself.
const DETECTION_SHARE: f64 = 0.2;

/// One run's reports split over a detection and the chain after it, so the window draws one bar that moves forward
/// once.
///
/// **A detection the store served owns nothing.** Nearly every chain after the first at a framing finds its faces in
/// the run store — the analysis, the previous run or the picker already asked — and reporting that as a fifth of the
/// bar would jump every re-run a fifth of the way in for nothing. So the detection's own reports are forwarded only
/// once one says it is fetching or running, and the chain takes the whole bar unless one was.
pub(crate) struct Handover {
    /// Where every report goes in the end: the run's own [`Reporting`] callback.
    inner: opai::OnInference,
    /// Whether the detection reported any work, which decides the share the chain is mapped onto.
    detected: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Handover {
    /// Splits the reports `inner` receives.
    pub(crate) fn new(inner: &opai::OnInference) -> Self {
        Self { inner: std::sync::Arc::clone(inner), detected: std::sync::Arc::default() }
    }

    /// The callback for the detection: its reports mapped onto the head of the bar, and a detection served from the
    /// store not reported at all.
    pub(crate) fn detection(&self) -> opai::OnInference {
        let inner = std::sync::Arc::clone(&self.inner);
        let detected = std::sync::Arc::clone(&self.detected);

        std::sync::Arc::new(move |report: &opai::InferenceProgress| {
            if matches!(report.stage, opai::Stage::Cached) {
                return;
            }

            detected.store(true, std::sync::atomic::Ordering::Relaxed);
            inner(&opai::InferenceProgress {
                chain_fraction: report.chain_fraction * DETECTION_SHARE,
                ..report.clone()
            });
        })
    }

    /// The callback for the chain: what a detection that reported left of the bar, or the whole of it.
    ///
    /// Asked for once the detection has ended, which is when whether it reported anything is known.
    pub(crate) fn chain(&self) -> opai::OnInference {
        if !self.detected.load(std::sync::atomic::Ordering::Relaxed) {
            return std::sync::Arc::clone(&self.inner);
        }

        let inner = std::sync::Arc::clone(&self.inner);

        std::sync::Arc::new(move |report: &opai::InferenceProgress| {
            let chain_fraction = DETECTION_SHARE + report.chain_fraction * (1.0 - DETECTION_SHARE);

            inner(&opai::InferenceProgress { chain_fraction, ..report.clone() });
        })
    }
}

// ── What a run's reports go through ───────────────────────────────────────────────────────────────────────────────

/// Where a run's reports go, and what is left to send when it ends.
///
/// A pair rather than a callback alone: the thinning holds a report back (see [`Thinning`]), and something has
/// to release the last one on completion. Taken as an `Option` so tests can exercise the rules without a
/// window.
pub(crate) struct Reporting {
    /// What [`opai::ProcessOptions::on_progress`] is set to, or [`opai::ExecuteOptions::on_progress`] for the
    /// detection [`crate::faces`] runs.
    pub(crate) on_progress: opai::OnInference,
    /// The thinning behind it, so the run's final held-back report can be flushed.
    thinning: std::sync::Arc<Mutex<Thinning>>,
    /// What to do with that last report — the same emit the callback uses.
    flush: std::sync::Arc<dyn Fn(RunProgress) + Send + Sync>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::enhance::run::{Enhancement, Enhancer, enhance_with};
    use crate::enhance::test_support::{enhanced, fixture, kyoto, request};

    use opai::{Enhanced, FloatPrecision, InferenceError, Picture, ProcessOptions, Scale, Upscale};
    use std::future::Future;

    /// A [`Reporting`] that collects what it sent, and the collection.
    fn collecting(run: &str) -> (Reporting, Arc<Mutex<Vec<RunProgress>>>) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let collector = Arc::clone(&sent);

        (Reporting::new(run, move |report| collector.lock().expect("unpoisoned").push(report)), sent)
    }

    /// One of `opai`'s reports, built as the library builds one.
    fn reported(subject: &Arc<opai::Subject>, stage: opai::Stage, chain: f64) -> opai::InferenceProgress {
        opai::InferenceProgress {
            subject: Arc::clone(subject),
            stage,
            operation_fraction: chain,
            chain_fraction: chain,
        }
    }

    /// The subject of a report: one upscale, as `opai` carries it.
    fn subject() -> Arc<opai::Subject> {
        Arc::new(opai::Subject::Enhancement(Upscale::kyoto(FloatPrecision::Fp32, Scale::clamped(2.0))))
    }

    /// One install report at `fraction` of its own transfer.
    fn installing(fraction: f64) -> opai::Stage {
        opai::Stage::Installing(opai::Progress {
            dependency: opai::Dependency::Model(
                subject().required_artifacts().first().expect("an upscale needs a model").clone(),
            ),
            phase: opai::Phase::Downloading,
            bytes: (fraction * 1000.0) as u64,
            total: Some(1000),
            fraction,
        })
    }

    #[test]
    fn an_install_report_carries_both_fractions() {
        let (reporting, sent) = collecting("run-1");
        let subject = subject();

        // A fetch occupies only the head of one operation's share of the chain, so the two figures differ.
        (reporting.on_progress)(&reported(&subject, installing(0.5), 0.1));

        let sent = sent.lock().expect("unpoisoned");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].run, "run-1", "a report did not name the run it was about");
        assert_eq!(sent[0].stage, Some(Stage::Installing));
        assert_eq!(sent[0].chain_fraction, 0.1);
        assert_eq!(sent[0].install_fraction, Some(0.5), "a fetch was reported without its own figure");
        assert_eq!(sent[0].operation, Upscale::kyoto(FloatPrecision::Fp32, Scale::clamped(2.0)).display_name());

        // Named twice over: the model, and the enhancement family.
        assert_eq!(sent[0].family, Family::Upscale);
    }

    #[test]
    fn a_running_report_carries_the_chain_and_no_fetch() {
        let (reporting, sent) = collecting("run-1");

        (reporting.on_progress)(&reported(&subject(), opai::Stage::Running, 0.4));

        let sent = sent.lock().expect("unpoisoned");
        assert_eq!(sent[0].stage, Some(Stage::Running));
        assert_eq!(sent[0].install_fraction, None, "a run that fetched nothing claimed a fetch figure");
        assert_eq!(sent[0].family, Family::Upscale);
    }

    #[test]
    fn a_cached_operation_advances_the_chain_and_names_neither_stage() {
        let (reporting, sent) = collecting("run-1");

        // Neither fetched nor run — labelling it `Running` would jump the bar with no explanation. D5.
        (reporting.on_progress)(&reported(&subject(), opai::Stage::Cached, 1.0));

        let sent = sent.lock().expect("unpoisoned");
        assert_eq!(sent[0].chain_fraction, 1.0, "a cached operation did not advance the chain");
        assert_eq!(sent[0].stage, None);
        assert_eq!(sent[0].install_fraction, None);
        assert_eq!(sent[0].family, Family::Upscale);

        // On the wire the two absences are absences, so the window draws no indicator at all.
        let wire = serde_json::to_value(&sent[0]).expect("a report serializes");
        assert!(wire.get("stage").is_none(), "a cached operation was labelled on the wire");
        assert!(wire.get("installFraction").is_none());
        assert_eq!(wire["chainFraction"], serde_json::json!(1.0));
        assert_eq!(wire["run"], serde_json::json!("run-1"));

        assert_eq!(wire["family"], serde_json::json!("upscale"));
    }

    #[test]
    fn a_tile_rate_stream_is_thinned_to_something_an_indicator_can_draw() {
        let (reporting, sent) = collecting("run-1");
        let subject = subject();

        // One report per tile — the indicator has about a hundred positions.
        const REPORTS: usize = 5_000;

        for tile in 0..REPORTS {
            let chain = tile as f64 / REPORTS as f64;

            (reporting.on_progress)(&reported(&subject, opai::Stage::Running, chain));
        }
        reporting.release();

        let sent = sent.lock().expect("unpoisoned").len();

        // Bounded rather than proportional to the five thousand inputs.
        assert!(sent <= 102, "a tile-rate stream put {sent} reports on the wire");
        assert!(sent >= 100, "the indicator would move in fewer steps than it has positions: {sent}");
    }

    #[test]
    fn a_report_that_moves_only_the_fetch_is_still_emitted() {
        let (reporting, sent) = collecting("run-1");
        let subject = subject();

        // Both figures must be compared: a fetch occupies the head of one operation's share, so the chain
        // barely moves while it runs — thinned on the chain alone, the download percentage would freeze.
        for tenth in 0..=10 {
            (reporting.on_progress)(&reported(&subject, installing(f64::from(tenth) / 10.0), 0.02));
        }
        reporting.release();

        let sent = sent.lock().expect("unpoisoned");
        assert_eq!(sent.len(), 11, "a fetch advancing under a motionless chain stopped being reported");
        assert_eq!(sent.last().expect("reports were sent").install_fraction, Some(1.0));
    }

    #[test]
    fn the_first_and_last_report_of_each_stage_survive_the_thinning() {
        let (reporting, sent) = collecting("run-1");
        let subject = subject();

        // A fetch finishing and handing over to a run, with the chain motionless across the handover.
        (reporting.on_progress)(&reported(&subject, installing(1.0), 0.2));
        (reporting.on_progress)(&reported(&subject, installing(1.0), 0.2));
        (reporting.on_progress)(&reported(&subject, opai::Stage::Running, 0.2));
        (reporting.on_progress)(&reported(&subject, opai::Stage::Running, 0.2));
        reporting.release();

        let sent = sent.lock().expect("unpoisoned");
        let stages: Vec<_> = sent.iter().map(|report| report.stage).collect();

        assert_eq!(
            stages,
            [Some(Stage::Installing), Some(Stage::Installing), Some(Stage::Running), Some(Stage::Running)],
            "a stage transition or the report that ended a stage was thinned away"
        );
    }

    #[test]
    fn two_operations_at_the_same_position_are_two_stages() {
        let (reporting, sent) = collecting("run-1");

        let kyoto = subject();
        let tokyo = Arc::new(opai::Subject::Enhancement(Upscale::tokyo(FloatPrecision::Fp32, Scale::clamped(2.0))));

        // A chain handing over from one operation to the next at the same chain fraction.
        (reporting.on_progress)(&reported(&kyoto, opai::Stage::Running, 0.5));
        (reporting.on_progress)(&reported(&tokyo, opai::Stage::Running, 0.5));
        reporting.release();

        let sent = sent.lock().expect("unpoisoned");
        let operations: Vec<_> = sent.iter().map(|report| report.operation.clone()).collect();

        assert_eq!(operations.len(), 2, "the second operation's first report was thinned away as a repeat");
        assert_ne!(operations[0], operations[1]);
    }

    #[test]
    fn the_chain_never_goes_backwards_and_reaches_completion_exactly_once() {
        let (reporting, sent) = collecting("run-1");
        let kyoto = subject();
        let tokyo = Arc::new(opai::Subject::Enhancement(Upscale::tokyo(FloatPrecision::Fp32, Scale::clamped(2.0))));

        // A chain of two whose first operation fetches its model — the two handovers where a bar could go
        // backwards.
        for step in 0..=50 {
            let fetched = f64::from(step) / 50.0;

            (reporting.on_progress)(&reported(&kyoto, installing(fetched), fetched / 10.0));
        }
        for step in 0..=50 {
            (reporting.on_progress)(&reported(&kyoto, opai::Stage::Running, 0.1 + f64::from(step) / 100.0));
        }
        for step in 0..=50 {
            (reporting.on_progress)(&reported(&tokyo, opai::Stage::Running, 0.6 + f64::from(step) / 125.0));
        }
        reporting.release();

        let sent = sent.lock().expect("unpoisoned");

        assert!(
            sent.windows(2).all(|pair| pair[1].chain_fraction >= pair[0].chain_fraction),
            "the figure for the whole chain went backwards: {:?}",
            sent.iter().map(|report| report.chain_fraction).collect::<Vec<_>>()
        );

        // Exactly once, however many operations or fetches happened.
        let completed = sent.iter().filter(|report| report.chain_fraction >= 1.0).count();
        assert_eq!(completed, 1, "the chain reported completion {completed} times");
        assert_eq!(
            sent.last().expect("reports were sent").chain_fraction,
            1.0,
            "the run ended on a report other than the one that completed it"
        );
    }

    #[test]
    fn a_run_that_is_entirely_already_known_completes_without_reporting_any_work() {
        let (_dir, opened, runs, identity) = fixture();
        let (reporting, sent) = collecting("run-1");

        // Every operation served from the run cache — no label for any of it, so the window draws no
        // indicator at all. D5, end to end.
        let outcome = tauri::async_runtime::block_on(enhance_with(
            &Known,
            &opened,
            &runs,
            Some(reporting),
            request("run-1", &identity, vec![kyoto(), kyoto()]),
        ))
        .expect("a chain the application already knows the answer to should finish");

        assert!(matches!(outcome, Enhancement::Enhanced { .. }));

        let sent = sent.lock().expect("unpoisoned");
        assert!(!sent.is_empty(), "a cached run reported nothing at all, not even that it had finished");
        assert!(
            sent.iter().all(|report| report.stage.is_none() && report.install_fraction.is_none()),
            "a run that fetched nothing and ran nothing claimed to have done one of them"
        );
        assert_eq!(
            sent.last().expect("reports were sent").chain_fraction,
            1.0,
            "a cached run ended without the window being told it had"
        );
    }

    /// An enhancer whose every operation was already known: reports [`opai::Stage::Cached`] and does no work.
    struct Known;

    impl Enhancer for Known {
        fn process(
            &self,
            source: &Picture,
            operations: &[opai::Operation],
            options: ProcessOptions,
        ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
            if let Some(on_progress) = &options.on_progress {
                for (index, operation) in operations.iter().enumerate() {
                    let subject = Arc::new(opai::Subject::Enhancement(operation.clone()));
                    let chain = (index + 1) as f64 / operations.len() as f64;

                    on_progress(&reported(&subject, opai::Stage::Cached, chain));
                }
            }

            let result = enhanced(source, 4, 2, "cafebabecafebabe");

            async move { Ok(result) }
        }
    }

    #[test]
    fn a_run_that_ends_mid_stage_still_reports_where_it_got_to() {
        let (reporting, sent) = collecting("run-1");
        let subject = subject();

        (reporting.on_progress)(&reported(&subject, opai::Stage::Running, 0.5));
        // Two reports the thinning has nothing to say about, the last of which is where the run stopped.
        (reporting.on_progress)(&reported(&subject, opai::Stage::Running, 0.5004));
        (reporting.on_progress)(&reported(&subject, opai::Stage::Running, 0.5009));

        assert_eq!(
            sent.lock().expect("unpoisoned").len(),
            1,
            "a report a hundred-position bar cannot draw was sent"
        );

        reporting.release();

        let sent = sent.lock().expect("unpoisoned");
        assert_eq!(sent.len(), 2, "the run ended on a report the window was never sent");
        assert_eq!(sent[1].chain_fraction, 0.5009, "the flushed report was not the last one the run produced");
    }

    #[test]
    fn a_run_reports_through_the_seam_and_flushes_when_it_ends() {
        let (_dir, opened, runs, identity) = fixture();
        let (reporting, sent) = collecting("run-1");

        tauri::async_runtime::block_on(enhance_with(
            &Chatty,
            &opened,
            &runs,
            Some(reporting),
            request("run-1", &identity, vec![kyoto()]),
        ))
        .expect("the run should finish");

        let sent = sent.lock().expect("unpoisoned");

        // What the callback emitted plus the one the run's end released.
        assert!(sent.len() >= 2, "a run reported nothing through the options it was given");
        assert!(sent.iter().all(|report| report.run == "run-1"), "a report named a run other than its own");
        assert_eq!(
            sent.last().expect("reports were sent").chain_fraction,
            1.0,
            "the run ended without the window being told it had"
        );
    }

    /// An enhancer that reports the way a real run does before it answers.
    struct Chatty;

    impl Enhancer for Chatty {
        fn process(
            &self,
            source: &Picture,
            _operations: &[opai::Operation],
            options: ProcessOptions,
        ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
            let subject = subject();

            if let Some(on_progress) = &options.on_progress {
                on_progress(&reported(&subject, installing(0.5), 0.1));
                on_progress(&reported(&subject, opai::Stage::Running, 0.5));
                // A pair the thinning has nothing to say about; the flush must release the last of them.
                on_progress(&reported(&subject, opai::Stage::Running, 1.0));
                on_progress(&reported(&subject, opai::Stage::Running, 1.0));
            }

            let result = enhanced(source, 4, 2, "cafebabecafebabe");

            async move { Ok(result) }
        }
    }

    // ── One bar over a detection and its chain ──────────────────────────────────────────────────────────────────

    /// What a [`Handover`] forwarded, as the chain fractions it forwarded them at.
    fn handed_over(detection: &[opai::Stage], chain: usize) -> Vec<f64> {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let collector = Arc::clone(&sent);
        let inner: opai::OnInference = Arc::new(move |report: &opai::InferenceProgress| {
            collector.lock().expect("unpoisoned").push(report.chain_fraction);
        });
        let handover = Handover::new(&inner);

        let detecting = handover.detection();
        let subject = Arc::new(opai::Subject::Analysis(opai::Detection::for_face_recovery()));
        for stage in detection {
            detecting(&reported(&subject, stage.clone(), 1.0));
        }

        let chaining = handover.chain();
        let upscale = Arc::new(opai::Subject::Enhancement(Upscale::kyoto(FloatPrecision::Fp32, Scale::clamped(2.0))));
        for step in 0..=chain {
            chaining(&reported(&upscale, opai::Stage::Running, step as f64 / chain as f64));
        }

        let sent = sent.lock().expect("unpoisoned");
        sent.clone()
    }

    #[test]
    fn a_detection_that_worked_owns_the_head_of_the_bar_and_the_chain_the_rest() {
        let fractions = handed_over(&[opai::Stage::Running], 2);
        let percent = fractions.iter().map(|fraction| percent(*fraction)).collect::<Vec<_>>();

        assert_eq!(percent, [20, 20, 60, 100]);
        // And lands on exactly 1, which is what the window reads as done.
        assert_eq!(fractions.last(), Some(&1.0));
    }

    #[test]
    fn a_detection_the_store_served_reports_nothing_and_the_chain_owns_the_whole_bar() {
        let fractions = handed_over(&[opai::Stage::Cached], 2);

        assert_eq!(fractions, [0.0, 0.5, 1.0]);
    }
}
