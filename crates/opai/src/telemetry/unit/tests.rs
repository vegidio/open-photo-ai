use std::time::Duration;

use super::*;
use crate::logging::traced_of_blocking;

/// The error every failed ending below carries.
const REASON: &str = "the model failed on tile 3";

#[test]
fn a_failed_unit_records_its_outcome_and_the_reason() {
    let (_, observed, _) = traced_of_blocking("info", || {
        let span = unit_span!("enhancement");
        ended(
            Unit::Enhancement,
            &span,
            Duration::from_millis(5),
            Outcome::Failed { kind: "run", error: &REASON },
        );
    });

    let span = observed.only("enhancement");
    assert_eq!(span.field("outcome"), Some("failed"));
    assert_eq!(span.field("error"), Some(REASON), "the bridge marks a span failed by this field");
    assert!(span.closed);
}

#[test]
fn a_stopped_unit_is_not_marked_as_failed() {
    let (_, observed, _) = traced_of_blocking("info", || {
        let span = unit_span!("enhancement");
        ended(Unit::Enhancement, &span, Duration::from_millis(5), Outcome::Stopped);
    });

    let span = observed.only("enhancement");
    assert_eq!(span.field("outcome"), Some("stopped"));
    assert_eq!(span.field("error"), None, "a stop was marked as a failure");
}

#[test]
fn a_finished_unit_says_so() {
    let (_, observed, _) = traced_of_blocking("info", || {
        let span = unit_span!("analysis", id = "dt_newyork_fp32");
        ended(Unit::Analysis, &span, Duration::from_millis(5), Outcome::Finished);
    });

    let span = observed.only("analysis");
    assert_eq!(span.field("outcome"), Some("finished"));
    assert_eq!(span.field("id"), Some("dt_newyork_fp32"));
    assert_eq!(span.field("error"), None);
}

#[test]
fn every_unit_is_named_as_its_span_is_and_no_two_share_a_name() {
    let names: Vec<&str> = Unit::ALL.iter().map(|unit| unit.as_str()).collect();

    assert_eq!(
        names,
        [
            "initialize",
            "runtime_load",
            "install",
            "build",
            "enhancement",
            "analysis",
            "automatic_analysis",
            "image_load",
            "image_save",
            "image_encode",
            "image_probe",
            "image_probe_raw",
            "image_identity",
        ]
    );
}

#[test]
fn an_ending_is_measured_and_a_failure_counted_by_its_kind() {
    use crate::telemetry::metrics::{FAILURES, UNIT_DURATION};

    // A kind no real error has, so the counter's change is this test's alone. The duration's tag set is shared with
    // any initialization failing in a parallel test, hence `>=` there.
    let failures = || FAILURES.value(&[("unit", "initialize"), ("kind", "ended_test_kind")]);
    let durations = || {
        let tags = [("unit", "initialize"), ("outcome", "failed")];
        (UNIT_DURATION.count(&tags), UNIT_DURATION.sum(&tags))
    };

    let (failed_before, (count_before, sum_before)) = (failures(), durations());
    traced_of_blocking("info", || {
        let span = unit_span!("initialize");
        let failed = Outcome::Failed { kind: "ended_test_kind", error: &REASON };
        ended(Unit::Initialize, &span, Duration::from_secs(3), failed);
    });
    let (count_after, sum_after) = durations();

    assert_eq!(failures() - failed_before, 1);
    assert!(count_after > count_before, "the duration was not recorded");
    assert!(
        sum_after - sum_before >= 3.0,
        "the duration was recorded in the wrong unit: {}",
        sum_after - sum_before
    );

    // A stop is measured, and never counted as a failure.
    let stopped = || UNIT_DURATION.count(&[("unit", "initialize"), ("outcome", "stopped")]);
    let stopped_before = stopped();
    ended(Unit::Initialize, &Span::none(), Duration::from_millis(1), Outcome::Stopped);
    assert!(stopped() > stopped_before);
    assert_eq!(failures() - failed_before, 1);
}
