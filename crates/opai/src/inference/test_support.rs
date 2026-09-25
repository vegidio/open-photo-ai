//! The fixtures the drivers' tests share: a sink for a chain run's reports, and the one property every report
//! sequence must have.
//!
//! The pictures those tests run are `imaging::test_support`'s, and the backends they run them through are
//! `pipeline::test_support`'s; what is left here is about orchestration alone.

use std::sync::{Arc, Mutex};

use crate::inference::process::ProcessOptions;
use crate::inference::progress::InferenceProgress;
use crate::task::lock;

/// A sink collecting every report a chain run makes, and the options that route the run into it.
///
/// The same decision [`crate::progress::recording`] makes one layer down, and for the same reason: the
/// `Arc<Mutex<Vec<_>>>` plus the closure that pushes into it plus the `ProcessOptions` around it was written out four
/// times across the chain's tests, so a change to `OnInference`'s shape was four edits. The collector is handed back
/// alongside the options because a caller that also drives a fake backend needs to share it.
pub(crate) fn recording() -> (Arc<Mutex<Vec<InferenceProgress>>>, ProcessOptions) {
    let reports: Arc<Mutex<Vec<InferenceProgress>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&reports);

    let options = ProcessOptions {
        on_progress: Some(Arc::new(move |report: &InferenceProgress| lock(&sink).push(report.clone()))),
        ..Default::default()
    };

    (reports, options)
}

/// Fails unless the chain fraction never goes backwards across `reports`.
///
/// The one property every progress test shares, and the one a front end actually depends on: a bar that retreats is
/// worse than one that jumps. `context` names which handover is being checked, so a failure says which of them moved.
pub(crate) fn assert_never_decreases(reports: &[InferenceProgress], context: &str) {
    for pair in reports.windows(2) {
        assert!(
            pair[1].chain_fraction >= pair[0].chain_fraction,
            "the bar went backwards {context}: {} then {}",
            pair[0].chain_fraction,
            pair[1].chain_fraction
        );
    }
}
