//! What building, serving and releasing sessions leaves in the log.

use super::*;

#[tokio::test]
async fn one_build_and_one_hit_are_recorded_for_two_requests_for_one_model() {
    let (cache, builds) = counting_cache();
    let id = kyoto();

    let (log, ()) = logging::records_of("debug", || async {
        build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
        build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    })
    .await;

    let building = records(&log, "building session");
    assert_eq!(building.len(), 1, "one build was recorded {} times:\n{log}", building.len());
    assert_eq!(field(building[0], "artifact"), Some(id.as_str()));
    assert_eq!(field(building[0], "provider"), Some(ExecutionProvider::Cpu.as_str()));
    assert!(building[0].contains("level=INFO"), "{}", building[0]);

    let ready = records(&log, "session ready");
    assert_eq!(ready.len(), 1, "the session was reported ready {} times:\n{log}", ready.len());
    assert_eq!(field(ready[0], "artifact"), Some(id.as_str()));
    assert_eq!(field(ready[0], "provider"), Some(ExecutionProvider::Cpu.as_str()));
    // `requested` beside `provider` on every build, so a downgrade's CPU build reads as one event with its
    // warning and an honoured request shows the two agreeing.
    assert_eq!(field(ready[0], "requested"), Some(ExecutionProvider::Cpu.as_str()));
    assert!(field(ready[0], "duration").is_some(), "the build does not say what it cost: {}", ready[0]);

    // The second request built nothing and said so at debug, which is the record that answers "why was the
    // second image instant".
    let resident = records(&log, "session already resident");
    assert_eq!(resident.len(), 1, "a resident hit was recorded {} times:\n{log}", resident.len());
    assert!(resident[0].contains("level=DEBUG"), "a per-request record is above debug: {}", resident[0]);
    assert_eq!(field(resident[0], "artifact"), Some(id.as_str()));

    // And an ordinary session carries the build without the hit.
    let (ordinary, ()) = logging::records_of("info", || async {
        let (cache, builds) = counting_cache();
        build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
        build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    })
    .await;
    assert_eq!(records(&ordinary, "building session").len(), 1, "{ordinary}");
    assert!(records(&ordinary, "session already resident").is_empty(), "{ordinary}");
}

#[tokio::test]
async fn releasing_sessions_says_how_many_went_and_why() {
    let (cache, builds) = counting_cache();

    for id in [kyoto(), tokyo()] {
        build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    }
    assert_eq!(cache.resident(), 2);

    // Sorted, because the map has no order of its own and a reader comparing two sessions of the same
    // application needs one.
    let both = {
        let mut names = [kyoto().to_string(), tokyo().to_string()];
        names.sort_unstable();
        names.join(",")
    };

    // On request, which is what `Opai::release_sessions` reaches.
    let (requested, ()) = logging::records_of_blocking("info", || cache.clear());
    let released = records(&requested, "released resident sessions");
    assert_eq!(released.len(), 1, "{requested}");
    assert_eq!(field(released[0], "released"), Some("2"));
    assert_eq!(field(released[0], "reason"), Some("requested"));
    // What went, not only how many: a front end's "free memory" gesture that let go of the model the next run
    // needs is a different thing to read than one that let go of a model nothing was using.
    assert_eq!(field(released[0], "artifacts"), Some(both.as_str()), "{}", released[0]);

    // And by the sweeper, which reports under its own reason so the two are distinguishable in one file.
    for id in [kyoto(), tokyo()] {
        build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    }
    let (idle, ()) = logging::records_of_blocking("info", || cache.sweep(Instant::now()));
    let swept = records(&idle, "released resident sessions");
    assert_eq!(swept.len(), 1, "{idle}");
    assert_eq!(field(swept[0], "released"), Some("2"));
    assert_eq!(field(swept[0], "reason"), Some("idle"));
    assert_eq!(field(swept[0], "artifacts"), Some(both.as_str()), "{}", swept[0]);

    // A sweep that found nothing writes nothing: this runs every few minutes for the life of the process.
    let (quiet, ()) = logging::records_of_blocking("info", || cache.sweep(Instant::now()));
    assert!(
        records(&quiet, "released resident sessions").is_empty(),
        "an empty sweep wrote a record:\n{quiet}"
    );

    // **What the list adds over the count**, at the only case that can tell them apart: a sweep that let go of
    // one of two says which one. A count alone leaves a reader chasing a model that is still resident.
    for id in [kyoto(), tokyo()] {
        build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    }
    let held = build_through(&cache, &builds, &kyoto(), ExecutionProvider::Cpu).await.unwrap();

    let (partial, ()) = logging::records_of_blocking("info", || cache.sweep(Instant::now()));
    drop(held);

    let one = records(&partial, "released resident sessions");
    assert_eq!(one.len(), 1, "{partial}");
    assert_eq!(field(one[0], "released"), Some("1"));
    assert_eq!(field(one[0], "artifacts"), Some(tokyo().as_str()), "the wrong model was named: {}", one[0]);
}

#[tokio::test]
async fn a_downgrade_is_recorded_as_a_pair_naming_what_was_asked_for_and_what_ran() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { fails_on: vec![ExecutionProvider::Cuda], ..Bench::default() });
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, true, false));

    let (log, handle) = logging::records_of("info", || async {
        sessions
            .session(&id, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
            .await
            .unwrap()
    })
    .await;

    // The handle still reports what ran, unchanged: the log is a second reader, not a replacement.
    assert_eq!(handle.provider(), ExecutionProvider::Cpu);

    let downgraded =
        records(&log, "the execution provider could not open this model; falling back to the next provider");
    assert_eq!(downgraded.len(), 1, "the downgrade was recorded {} times:\n{log}", downgraded.len());
    assert!(downgraded[0].contains("level=WARN"), "a downgrade is not a warning: {}", downgraded[0]);
    assert_eq!(field(downgraded[0], "artifact"), Some(id.as_str()));
    assert_eq!(field(downgraded[0], "provider"), Some(ExecutionProvider::Cuda.as_str()));
    assert_eq!(field(downgraded[0], "next"), Some(ExecutionProvider::Cpu.as_str()));
    // The reason is on the failed build's record, written just before, rather than repeated here.
    assert!(
        field(downgraded[0], "error").is_none(),
        "the downgrade repeats the build's error: {}",
        downgraded[0]
    );

    let failed = records(&log, "session build failed");
    assert_eq!(failed.len(), 1, "the failed build was recorded {} times:\n{log}", failed.len());
    assert!(failed[0].contains("level=WARN"), "{}", failed[0]);
    assert_eq!(field(failed[0], "artifact"), Some(id.as_str()));
    assert_eq!(field(failed[0], "provider"), Some(ExecutionProvider::Cuda.as_str()));
    assert_eq!(field(failed[0], "requested"), Some(ExecutionProvider::Cuda.as_str()));
    assert!(field(failed[0], "duration").is_some(), "{}", failed[0]);
    assert!(field(failed[0], "error").is_some(), "the failed build does not say why: {}", failed[0]);
    let (build_at, fallback_at) = (
        log.find("session build failed").unwrap(),
        log.find("falling back to the next provider").unwrap(),
    );
    assert!(build_at < fallback_at, "the fallback was recorded before the build that caused it:\n{log}");

    // And the CPU build that follows carries `requested` beside `provider`, so the pair reads as one event.
    let cpu = records(&log, "session ready");
    assert_eq!(cpu.len(), 1, "the CPU session was reported ready {} times:\n{log}", cpu.len());
    assert_eq!(field(cpu[0], "provider"), Some(ExecutionProvider::Cpu.as_str()));
    assert_eq!(field(cpu[0], "requested"), Some(ExecutionProvider::Cuda.as_str()));

    // At the default level, which is the file a user attaches without being asked to reproduce anything.
    assert!(downgraded[0].contains("level=WARN") && cpu[0].contains("level=INFO"));
}

#[tokio::test]
async fn a_build_that_fails_under_several_waiting_requests_is_recorded_once() {
    let cache: SessionCache<Fake> = SessionCache::default();
    let id = kyoto();
    let interest = Interest::default();

    // On one task rather than spawned, so every request's records reach the recording bound to this thread. The
    // build awaits, so the others join its flight rather than each finding it already over.
    let request = || {
        cache.get_or_build(&id, ExecutionProvider::CoreMl, ExecutionProvider::CoreMl, &interest, |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Err::<Option<Fake>, _>(build_failure(&id, ExecutionProvider::CoreMl))
        })
    };

    let (log, outcomes) = logging::records_of("info", || async {
        let (a, b, c, d) = tokio::join!(request(), request(), request(), request());
        [a, b, c, d]
    })
    .await;

    for outcome in outcomes {
        assert!(matches!(outcome, Err(SessionError::Build { .. })), "a waiter did not fail: {outcome:?}");
    }

    assert_eq!(records(&log, "building session").len(), 1, "{log}");
    let failed = records(&log, "session build failed");
    assert_eq!(failed.len(), 1, "one failed build was recorded {} times:\n{log}", failed.len());
    assert!(failed[0].contains("level=WARN"), "{}", failed[0]);
    assert_eq!(field(failed[0], "artifact"), Some(id.as_str()));
    assert_eq!(field(failed[0], "provider"), Some(ExecutionProvider::CoreMl.as_str()));
    assert!(field(failed[0], "duration").is_some(), "{}", failed[0]);
    assert!(field(failed[0], "error").is_some(), "{}", failed[0]);
}

#[tokio::test]
async fn a_build_ended_by_a_shutdown_is_recorded_as_stopped() {
    let cache: SessionCache<Fake> = SessionCache::default();
    let id = kyoto();

    let (log, outcome) = logging::records_of("info", || async {
        cache
            .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &Interest::default(), |_| async {
                Err::<Option<Fake>, _>(SessionError::from(crate::task::Cancelled))
            })
            .await
    })
    .await;

    assert!(outcome.is_err());
    let stopped = records(&log, "the build stopped");
    assert_eq!(stopped.len(), 1, "{log}");
    assert!(stopped[0].contains("level=INFO"), "{}", stopped[0]);
    assert_eq!(field(stopped[0], "reason"), Some("shutdown"));
    assert!(!log.contains("level=WARN"), "a shutdown was recorded as a failed build:\n{log}");
}

#[tokio::test]
async fn a_build_nothing_was_left_waiting_for_is_recorded_as_stopped_by_cancellation() {
    let cache: SessionCache<Fake> = SessionCache::default();
    let id = kyoto();

    // `Ok(None)` directly, which is what a build answers once its install stopped with nobody waiting: the same
    // record whatever made it stop, so the transfer itself need not be driven here.
    let (log, outcome) = logging::records_of("info", || async {
        cache
            .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &Interest::default(), |_| async {
                Ok::<Option<Fake>, SessionError>(None)
            })
            .await
    })
    .await;

    assert!(matches!(outcome, Ok(None)), "{outcome:?}");
    let [stopped] = records(&log, "the build stopped")[..] else {
        panic!("the stop was not recorded exactly once:\n{log}");
    };
    assert!(stopped.contains("level=INFO"), "{stopped}");
    assert_eq!(field(stopped, "reason"), Some("cancelled"));
    assert_eq!(field(stopped, "artifact"), Some(id.as_str()));
    assert!(field(stopped, "duration").is_some(), "{stopped}");
    assert!(!log.contains("level=WARN"), "a stopped build was recorded as a failure:\n{log}");
}
