//! `process` as a caller reaches it: options, progress, cancellation and what comes back.

use super::*;

#[tokio::test]
async fn stating_no_options_and_stating_every_default_produce_the_same_result() {
    let backend = Fake::new();
    let source = picture(300, 200);

    let implicit = process(&backend, None, &source, &[kyoto(2.0)], None).await.unwrap().picture;
    let explicit = process(&backend, None, &source, &[kyoto(2.0)], Some(ProcessOptions::default()))
        .await
        .unwrap()
        .picture;

    assert_eq!(implicit.identity(), explicit.identity());
    assert_eq!(implicit.dimensions(), explicit.dimensions());
    // Eight bits per channel, which is what every enhancement produced before a choice existed.
    assert!(matches!(implicit.pixels(), DynamicImage::ImageRgb8(_)), "{:?}", implicit.pixels());
    assert!(matches!(explicit.pixels(), DynamicImage::ImageRgb8(_)), "{:?}", explicit.pixels());
}

#[tokio::test]
async fn one_field_can_be_set_without_naming_the_rest() {
    // The property that made `cache` source-compatible for every caller when it arrived: a field added later does
    // not break a call site written this way.
    let backend = Fake::new();
    let options = ProcessOptions { depth: OutputDepth::Sixteen, ..Default::default() };

    assert_eq!(options.provider, ExecutionProvider::Auto);
    assert!(options.on_progress.is_none());

    let result = process(&backend, None, &picture(300, 200), &[kyoto(2.0)], Some(options)).await.unwrap().picture;

    assert!(matches!(result.pixels(), DynamicImage::ImageRgb16(_)), "{:?}", result.pixels());
}

#[tokio::test]
async fn progress_reaches_the_caller_while_the_run_is_still_going() {
    // Not merely "reports arrived in order once it finished": a caller blocked until the end could not draw a bar
    // at all, and the whole point of running the tile loop off the async runtime is that it can.
    let (reports, options) = recording();
    let backend = Fake { reports: Some(Arc::clone(&reports)), ..Fake::new() };

    // 600x400 is six tiles at the default geometry, so there are later tiles to observe earlier reports from.
    process(&backend, None, &picture(600, 400), &[kyoto(2.0)], Some(options)).await.unwrap();

    let seen = backend.log().reports_at_tile.clone();
    assert!(
        seen.last().copied().unwrap_or(0) > 0,
        "no report had reached the caller by the last tile: {seen:?}"
    );

    let reports = reports.lock().unwrap();
    assert_eq!(reports.last().map(|report| report.chain_fraction), Some(1.0));
    assert!(reports.iter().all(|report| matches!(report.stage, Stage::Running)));
    assert!(reports.iter().all(|report| *report.subject == as_subject(kyoto(2.0))));
}

/// Runs `chain` against a backend that has to put every model on disk first, and returns every report the caller
/// received — install and run together, in the order they arrived.
async fn install_and_run(chain: &[Operation]) -> Vec<InferenceProgress> {
    let (reports, options) = recording();
    let backend = Fake { installs: true, ..Fake::new() };

    process(&backend, None, &picture(300, 200), chain, Some(options))
        .await
        .expect("the chain must run");

    let collected = reports.lock().unwrap();
    collected.clone()
}

/// The artifacts named by the install reports among `reports`, in the order they arrived.
fn installing_artifacts(reports: &[InferenceProgress]) -> Vec<String> {
    reports
        .iter()
        .filter_map(|report| match &report.stage {
            Stage::Installing(progress) => Some(progress.dependency.as_str().to_string()),
            Stage::Running | Stage::Cached => None,
        })
        .collect()
}

#[tokio::test]
async fn an_install_reports_through_the_chain_naming_the_operation_and_the_model() {
    // The wiring none of the tests above can see, because their backend is handed the install callback and drops
    // it: `run` is what passes that callback down, and what divides the head of the operation's range between the
    // operation's *distinct* artifacts. An 8x Kyoto run installs the 4x weights and then the 2x, so both are
    // visible at once.
    let reports = install_and_run(&[kyoto(8.0)]).await;
    let installing: Vec<&InferenceProgress> =
        reports.iter().filter(|report| matches!(report.stage, Stage::Installing(_))).collect();

    assert!(!installing.is_empty(), "no install reached the caller, so the chain passed no callback down");
    assert!(
        installing.iter().all(|report| *report.subject == as_subject(kyoto(8.0))),
        "an install report did not name the operation it was for"
    );

    // Nested rather than flattened, so a front end drawing "Downloading Kyoto 4x" still has the install layer's
    // own report to read the dependency and the byte counts off.
    let named = installing_artifacts(&reports);
    assert_eq!(named.first().map(String::as_str), Some("up_kyoto_4x_fp16"));
    assert_eq!(named.last().map(String::as_str), Some("up_kyoto_2x_fp16"));

    // The two transfers fill the head of the range exactly once between them, which is what pins the *distinct*
    // count: dividing the head by the pass count instead would carry the second transfer past it, to 0.4.
    let filled = installing.last().expect("an install was reported").operation_fraction;
    assert!(
        (filled - INSTALL_SHARE).abs() < 1e-9,
        "the two installs filled {filled} of the operation rather than {INSTALL_SHARE}"
    );
}

#[tokio::test]
async fn a_transfer_handing_over_to_a_run_does_not_send_the_bar_back_to_the_start() {
    let reports = install_and_run(&[kyoto(8.0)]).await;

    // Every install is over before the first tile: the two phases are sequential, so a front end never sees them
    // interleave.
    let handover = reports
        .iter()
        .position(|report| matches!(report.stage, Stage::Running))
        .expect("the run itself reported nothing");
    assert!(
        reports[handover..].iter().all(|report| matches!(report.stage, Stage::Running)),
        "an install was reported after the run had already started"
    );

    // And the run picks up where the transfer left off rather than at zero, which is the whole reason the install
    // owns the head of the range instead of a range of its own.
    assert!(
        reports[handover].chain_fraction >= INSTALL_SHARE - 1e-9,
        "the run restarted the bar at {}",
        reports[handover].chain_fraction
    );

    assert_never_decreases(&reports, "across the handover");
    assert_eq!(reports.last().map(|report| report.chain_fraction), Some(1.0), "the run did not land on 1");
}

#[tokio::test]
async fn a_model_already_on_disk_gives_up_no_part_of_its_operations_range() {
    // The other half of the split, through the chain: the head is given up only where an install *actually
    // happened*, so a model already on disk starts its run at zero rather than a fifth of the way in.
    let (reports, options) = recording();
    let backend = Fake::new();

    process(&backend, None, &picture(300, 200), &[kyoto(8.0)], Some(options)).await.unwrap();

    let reports = reports.lock().unwrap();
    assert!(installing_artifacts(&reports).is_empty(), "a model already on disk reported an install");
    assert_eq!(reports.first().map(|report| report.chain_fraction), Some(0.0), "the run did not open on zero");
}

#[tokio::test]
async fn cancelling_partway_returns_cancelled_and_no_picture() {
    let cancel = CancellationToken::new();
    let backend = Fake { cancels: Some(("up_kyoto_2x_fp16".to_string(), cancel.clone())), ..Fake::new() };
    let options = ProcessOptions { cancel, ..Default::default() };

    let outcome = process(&backend, None, &picture(600, 400), &[kyoto(2.0)], Some(options)).await;

    assert!(matches!(outcome, Err(InferenceError::Cancelled)), "a cancelled run returned {outcome:?}");
}

#[tokio::test]
async fn the_result_carries_the_composed_identity_and_the_source_own_path() {
    let backend = Fake::new();
    let source = picture(300, 200);

    let result = process(&backend, None, &source, &[kyoto(2.0)], None).await.unwrap().picture;

    assert_eq!(result.identity(), identity_after(source.identity(), &[kyoto(2.0)], ChannelDepth::Eight));
    assert_ne!(result.identity(), source.identity(), "the result carried the input's identity");
    // A path says where pixels came from, never where they are going.
    assert_eq!(result.path(), source.path());
}

#[tokio::test]
async fn a_second_identical_run_acquires_no_session_and_runs_no_tile() {
    // The whole point of the slice, as a property of what the backend was asked for rather than of how long it
    // took: a hit skips the session acquisition entirely, because acquiring would install a model for an
    // operation that is not going to run.
    let backend = Fake::new();
    let cache = store();
    let source = picture(300, 200);

    let first = cached_run(&backend, Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight).await.unwrap();
    let acquired = backend.log().acquired.len();
    let ran = backend.log().ran.len();

    let second = cached_run(&backend, Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight).await.unwrap();

    assert_eq!(backend.log().acquired.len(), acquired, "the second run acquired a session");
    assert_eq!(backend.log().ran.len(), ran, "the second run ran a tile");

    // And it is the same picture, not merely the same shape: a caller cannot tell a served result from a computed
    // one, so anything less would be a degraded image presented as an enhancement.
    assert_eq!(second.identity(), first.identity());
    assert_eq!(second.dimensions(), first.dimensions());
    assert_eq!(second.pixels().as_bytes(), first.pixels().as_bytes());
}
