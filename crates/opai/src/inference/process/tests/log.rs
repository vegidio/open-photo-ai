//! What a run leaves in the log.

use super::*;

#[tokio::test]
async fn a_run_is_bracketed_by_a_record_at_each_end() {
    let backend = Fake::new();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(4.0)];

    let (log, result) =
        logging::records_of("info", || async { process(&backend, None, &source, &chain, None).await }).await;
    result.unwrap();

    let opening = records(&log, "enhancement started");
    assert_eq!(opening.len(), 1, "the run's beginning was recorded {} times:\n{log}", opening.len());
    assert_eq!(field(opening[0], "operations"), Some("2"));
    // The identity rather than the path: it is what joins these records to the cache's, and a path is a place the
    // user chose.
    assert_eq!(field(opening[0], "identity"), Some(source.identity()));
    assert!(
        !opening[0].contains(&source.path().display().to_string()),
        "the source's path is on a record: {}",
        opening[0]
    );
    assert_eq!(field(opening[0], "depth"), Some(ChannelDepth::Eight.tag()));
    // What was asked for, which is what the provider report on the result is later compared against.
    assert_eq!(field(opening[0], "provider"), Some(ExecutionProvider::Auto.as_str()));

    let ids = field(opening[0], "ids").expect("the operations are named");
    assert_eq!(ids, chain.iter().map(Operation::cache_tag).collect::<Vec<_>>().join(","));

    let closing = records(&log, "enhancement finished");
    assert_eq!(closing.len(), 1, "the run's outcome was recorded {} times:\n{log}", closing.len());
    assert_eq!(field(closing[0], "operations"), Some("2"));
    // The photograph, so the closing record can be read on its own.
    assert_eq!(field(closing[0], "identity"), Some(source.identity()));

    // `1.523s`, `250ms`, `12.3µs` — a duration a reader can read, which is what `Duration`'s `Debug` gives and
    // what a field set (`Duration { secs: 0, nanos: 12300000 }`) would not.
    let duration = field(closing[0], "duration").expect("the elapsed time is recorded");
    assert!(
        duration.ends_with('s') && duration.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "the duration is not rendered as a duration: {duration:?} on {}",
        closing[0]
    );
    assert!(!duration.contains("Duration"), "the duration rendered as a field set: {duration:?}");
}

#[tokio::test]
async fn a_run_that_fails_records_its_own_outcome_rather_than_stopping_after_the_opening() {
    // The half of the bracket a reader needs most: a beginning with no end is indistinguishable from a process
    // that was killed, so the failure says so in its own record.
    let backend = Fake { fails_to_run: Some("up_kyoto_2x_fp16".to_string()), ..Fake::new() };
    let source = picture(120, 80);

    let (log, outcome) =
        logging::records_of("info", || async { process(&backend, None, &source, &[kyoto(2.0)], None).await }).await;
    assert!(outcome.is_err(), "the run was expected to fail");

    assert_eq!(records(&log, "enhancement started").len(), 1, "{log}");
    assert!(
        records(&log, "enhancement finished").is_empty(),
        "a failed run claimed to have finished:\n{log}"
    );

    let failed = records(&log, "enhancement failed");
    assert_eq!(failed.len(), 1, "the failure was recorded {} times:\n{log}", failed.len());
    assert!(failed[0].contains("level=WARN"), "a failed run is not a warning: {}", failed[0]);
    assert!(
        field(failed[0], "duration").is_some(),
        "the failure does not say how long it took: {}",
        failed[0]
    );
    assert!(field(failed[0], "error").is_some(), "the failure does not say what went wrong: {}", failed[0]);
    assert_eq!(field(failed[0], "identity"), Some(source.identity()));
}

#[tokio::test]
async fn every_step_says_whether_it_ran_or_was_served() {
    let cache = store();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(4.0)];

    // The first operation stored and the second not, so one run produces a hit and a miss.
    cached_run(&Fake::new(), Some(&cache), &source, &[kyoto(2.0)], OutputDepth::Eight)
        .await
        .unwrap();

    let backend = Fake::new();
    let (log, result) = logging::records_of("debug", || async {
        cached_run(&backend, Some(&cache), &source, &chain, OutputDepth::Eight).await
    })
    .await;
    result.unwrap();

    let served = records(&log, "step served from the store");
    assert_eq!(served.len(), 1, "the served step was recorded {} times:\n{log}", served.len());
    assert_eq!(field(served[0], "operation"), Some(chain[0].cache_tag().as_str()));
    assert_eq!(field(served[0], "index"), Some("0"));
    assert!(served[0].contains("level=DEBUG"), "a per-step record is above debug: {}", served[0]);

    let computed = records(&log, "step computed");
    assert_eq!(computed.len(), 1, "the computed step was recorded {} times:\n{log}", computed.len());
    assert_eq!(field(computed[0], "operation"), Some(chain[1].cache_tag().as_str()));
    assert_eq!(field(computed[0], "index"), Some("1"));
    assert!(computed[0].contains("level=DEBUG"), "a per-step record is above debug: {}", computed[0]);
}

#[tokio::test]
async fn an_ordinary_session_carries_the_run_and_none_of_its_steps() {
    // The volume rule, at the level a user's file is actually written at: the bracket is there, the per-operation
    // detail is not, and asking for it is one environment variable rather than a rebuild.
    let backend = Fake::new();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(4.0)];

    let (log, result) = logging::records_of("info", || async {
        cached_run(&backend, Some(&store()), &source, &chain, OutputDepth::Eight).await
    })
    .await;
    result.unwrap();

    assert_eq!(records(&log, "enhancement started").len(), 1, "{log}");
    assert_eq!(records(&log, "enhancement finished").len(), 1, "{log}");
    assert!(
        records(&log, "step computed").is_empty(),
        "a per-step record reached an ordinary session:\n{log}"
    );
    assert!(records(&log, "step served from the store").is_empty(), "{log}");
}

#[tokio::test]
async fn a_failed_step_is_distinguishable_from_the_run_that_carried_it() {
    // One record rather than two: the run's own failure names the step it failed in, so a reader learns which
    // operation stopped and that the run produced nothing without being told of two failures where there was one.
    let backend = Fake { fails_to_run: Some("up_kyoto_4x_fp16".to_string()), ..Fake::new() };
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(4.0)];

    let (log, outcome) = logging::records_of("debug", || async {
        cached_run(&backend, None, &source, &chain, OutputDepth::Eight).await
    })
    .await;
    assert!(outcome.is_err(), "the run was expected to fail");

    let warnings: Vec<&str> = log.lines().filter(|line| line.contains("level=WARN")).collect();
    assert_eq!(warnings.len(), 1, "one failure was recorded {} times:\n{log}", warnings.len());
    assert!(records(&log, "step failed").is_empty(), "the step's failure was recorded a second time:\n{log}");

    let run = records(&log, "enhancement failed");
    assert_eq!(run.len(), 1, "the run's own failure was recorded {} times:\n{log}", run.len());
    assert_eq!(field(run[0], "operation"), Some(chain[1].cache_tag().as_str()));
    assert_eq!(field(run[0], "index"), Some("1"));
    assert!(field(run[0], "error").is_some(), "the failure does not say why: {}", run[0]);
    assert!(
        field(run[0], "duration").is_some(),
        "the run's failure does not say how long it took: {}",
        run[0]
    );

    // The step that succeeded before it is still accounted for, so the log says how far the run got.
    assert_eq!(records(&log, "step computed").len(), 1, "{log}");
}

#[tokio::test]
async fn a_refused_chain_is_one_record_and_never_a_run_that_began() {
    // The second operation is refused, so this also shows the whole chain is judged before anything is installed on
    // behalf of the first.
    let backend = Fake::new();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(4.0)];
    let _refusing = super::super::plan::refused::refusing(chain[1].clone());

    let (log, outcome) =
        logging::records_of("info", || async { process(&backend, None, &source, &chain, None).await }).await;
    assert!(matches!(outcome, Err(InferenceError::Unsupported { .. })), "{:?}", outcome.err());

    let [refused] = records(&log, "chain refused")[..] else {
        panic!("the refusal was not recorded exactly once:\n{log}");
    };
    assert!(refused.contains("level=WARN"), "{refused}");
    assert_eq!(
        field(refused, "ids"),
        Some(chain.iter().map(Operation::cache_tag).collect::<Vec<_>>().join(",").as_str()),
        "the refusal does not name the chain: {refused}"
    );
    assert!(field(refused, "error").is_some(), "the refusal does not say why: {refused}");

    assert_eq!(
        log.lines().filter(|line| line.contains("msg=")).count(),
        1,
        "a refusal wrote more than itself:\n{log}"
    );
    assert!(records(&log, "enhancement started").is_empty(), "a refused chain was recorded as begun:\n{log}");
    assert!(records(&log, "enhancement failed").is_empty(), "a refusal was recorded twice:\n{log}");
    assert!(backend.log().acquired.is_empty(), "a refused chain installed a model");
}

#[tokio::test]
async fn a_cancelled_run_is_recorded_as_stopped_and_not_as_a_failure() {
    let cancel = CancellationToken::new();
    let backend = Fake { cancels: Some(("up_kyoto_2x_fp16".to_string(), cancel.clone())), ..Fake::new() };
    let source = picture(600, 400);
    let options = ProcessOptions { cancel, ..Default::default() };

    let (log, outcome) =
        logging::records_of("info", || async { process(&backend, None, &source, &[kyoto(2.0)], Some(options)).await })
            .await;
    assert!(matches!(outcome, Err(InferenceError::Cancelled)), "{:?}", outcome.err());

    let stopped = records(&log, "enhancement stopped");
    assert_eq!(stopped.len(), 1, "{log}");
    assert!(stopped[0].contains("level=INFO"), "{}", stopped[0]);
    assert_eq!(field(stopped[0], "reason"), Some("cancelled"));
    assert_eq!(field(stopped[0], "identity"), Some(source.identity()));
    assert!(field(stopped[0], "duration").is_some(), "{}", stopped[0]);

    assert!(
        records(&log, "enhancement failed").is_empty(),
        "a cancellation was recorded as a failure:\n{log}"
    );
    assert!(!log.contains("level=WARN"), "a cancellation wrote a warning:\n{log}");
}

#[tokio::test]
async fn a_stored_result_that_will_not_decode_says_what_it_cost() {
    // The rewind: the prefix the run walked past is worthless, so the operations it stood for are run again. That
    // is work a user did not ask for twice, which is what makes it a warning rather than a debug line.
    let cache = store();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(2.0)];

    cached_run(&Fake::new(), Some(&cache), &source, &chain, OutputDepth::Eight).await.unwrap();

    // Only the first prefix is corrupted, so the walk takes it as a hit and discovers it at the decode.
    let first = identity_after(source.identity(), &chain[..1], ChannelDepth::Eight);
    let last = identity_after(source.identity(), &chain, ChannelDepth::Eight);
    cache.set_bytes(&first, b"\x89PNG\r\n\x1a\n and then nothing", ENTRY_TTL).unwrap();
    cache.set_bytes(&last, b"\x89PNG\r\n\x1a\n and then nothing", ENTRY_TTL).unwrap();

    let backend = Fake::new();
    let (log, result) = logging::records_of("debug", || async {
        cached_run(&backend, Some(&cache), &source, &chain, OutputDepth::Eight).await
    })
    .await;
    result.unwrap();

    let rewound = records(&log, "a stored result would not decode; rerunning the operations it stood for");
    assert!(!rewound.is_empty(), "a rewind was not recorded:\n{log}");
    assert!(rewound[0].contains("level=WARN"), "a rewind is not a warning: {}", rewound[0]);
    assert!(field(rewound[0], "from").is_some() && field(rewound[0], "to").is_some(), "{}", rewound[0]);

    // And the run still succeeded, which is the whole reason this is a warning and not an error.
    assert!(!records(&log, "step computed").is_empty(), "the rewind ran nothing:\n{log}");
}

#[tokio::test]
async fn a_run_with_no_store_says_so_once_rather_than_per_operation() {
    // The two ways a run has none — a machine whose store would not open, and a caller that turned it off — and
    // both produce one record however long the chain, which is the volume rule applied to the cache's own.
    let backend = Fake::new();
    let source = picture(120, 80);
    let chain = [kyoto(2.0), kyoto(2.0), kyoto(2.0)];

    let (none, result) = logging::records_of("debug", || async {
        cached_run(&backend, None, &source, &chain, OutputDepth::Eight).await
    })
    .await;
    result.unwrap();

    let storeless = records(&none, "this run has no store; every operation is computed and nothing is kept");
    assert_eq!(storeless.len(), 1, "a storeless run recorded {} times:\n{none}", storeless.len());
    assert!(storeless[0].contains("level=DEBUG"), "{}", storeless[0]);
    assert_eq!(field(storeless[0], "requested"), Some("true"), "{}", storeless[0]);

    // Turned off by the caller, with a store present: same record, and `requested` is what tells the two apart.
    let off = ProcessOptions { cache: false, ..Default::default() };
    let (bypassed, result) = logging::records_of("debug", || async {
        process(&Fake::new(), Some(&store()), &source, &chain, Some(off)).await
    })
    .await;
    result.unwrap();

    let storeless = records(&bypassed, "this run has no store; every operation is computed and nothing is kept");
    assert_eq!(storeless.len(), 1, "{bypassed}");
    assert_eq!(field(storeless[0], "requested"), Some("false"), "{}", storeless[0]);

    // And a run that does have one says nothing of the sort.
    let (served, result) = logging::records_of("debug", || async {
        cached_run(&Fake::new(), Some(&store()), &source, &chain, OutputDepth::Eight).await
    })
    .await;
    result.unwrap();
    assert!(
        records(&served, "this run has no store; every operation is computed and nothing is kept").is_empty(),
        "{served}"
    );
}

#[tokio::test]
async fn a_chain_is_bounded_by_its_operations_rather_than_by_the_work_each_one_did() {
    // The second half of the requirement: more operations may write more, more pixels may not. Counted at the
    // default level, which is the file a user actually attaches.
    let source = picture(120, 80);

    async fn count(source: &Picture, chain: &[Operation]) -> usize {
        let (log, result) = logging::records_of("info", || async {
            cached_run(&Fake::new(), None, source, chain, OutputDepth::Eight).await
        })
        .await;
        result.unwrap();
        log.lines().filter(|line| line.contains("msg=")).count()
    }

    // An 8x Kyoto is two passes and a 2x is one, so the second chain does strictly more work per operation as
    // well as having more operations — and the default-level count still moves only with the operation count.
    let one = count(&source, &[kyoto(2.0)]).await;
    let three = count(&source, &[kyoto(2.0), kyoto(8.0), kyoto(2.0)]).await;

    // The bracket and nothing else: the per-operation and per-pass records are below the default level.
    assert_eq!(one, 2, "a one-operation run wrote {one} records at the default level");
    assert_eq!(three, 2, "a three-operation run wrote {three} records at the default level");
}
