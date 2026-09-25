//! A provider that cannot open a model, and the CPU session built in its place.

use super::*;

#[tokio::test]
async fn a_provider_that_cannot_open_the_model_is_downgraded_to_the_cpu_and_reports_both() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { fails_on: vec![ExecutionProvider::Cuda], ..Bench::default() });
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, true, false));

    let handle = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
        .await
        .unwrap();

    assert_eq!(handle.provider(), ExecutionProvider::Cpu, "the downgrade did not build on the CPU");
    assert_eq!(handle.requested(), ExecutionProvider::Cuda, "the handle lost what was asked for");
    assert_eq!(bench.built_on(), vec![ExecutionProvider::Cuda, ExecutionProvider::Cpu]);
}

#[tokio::test]
async fn a_downgrade_served_a_cpu_session_an_explicit_cpu_request_filed_still_reports_the_downgrade() {
    // The entry is shared by both requests and cannot carry this: whichever arrived first would decide what the
    // other was told it had asked for, and here the first asked for the CPU and got it. Reporting `Cpu` beside
    // `Cpu` to the second is the exact shape of an honoured request, so both signals a broken driver produces —
    // this accessor and the `warn` beside it — would be lost precisely when a CPU session happened to be
    // resident.
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { fails_on: vec![ExecutionProvider::Cuda], ..Bench::default() });
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, true, false));

    let honoured = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cpu, &Interest::default())
        .await
        .unwrap();
    let downgraded = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
        .await
        .unwrap();

    assert_eq!(honoured.requested(), ExecutionProvider::Cpu, "the honoured request lost what it asked for");
    assert_eq!(honoured.provider(), ExecutionProvider::Cpu);
    assert_eq!(
        downgraded.requested(),
        ExecutionProvider::Cuda,
        "the downgrade was reported as an honoured CPU run"
    );
    assert_eq!(downgraded.provider(), ExecutionProvider::Cpu);

    // Both are the one session — the point of the fallback re-entering the cache — so the two handles differing
    // is the whole of what the fix buys. Copied out in two statements, not one: a tuple expression would hold the
    // first borrow while taking the second, and these two handles are on the one entry.
    let honoured_session = *honoured.session();
    let downgraded_session = *downgraded.session();
    assert_eq!(honoured_session, downgraded_session, "the downgrade rebuilt a session that was already filed");
    assert_eq!(
        bench.built_on(),
        vec![ExecutionProvider::Cpu, ExecutionProvider::Cuda],
        "the downgrade did not attempt the provider it asked for, or rebuilt the CPU session"
    );
}

#[tokio::test]
async fn a_second_request_after_a_downgrade_is_served_the_cpu_session_already_filed() {
    // What makes carrying no latch affordable: the second request pays one failed provider attach, not a rebuild.
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { fails_on: vec![ExecutionProvider::Cuda], ..Bench::default() });
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, true, false));

    let first = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
        .await
        .unwrap();
    let second = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
        .await
        .unwrap();

    // Copied out in two statements, not one: both handles are on the one entry, and a tuple expression would
    // hold the first borrow while taking the second — blocking on the very exclusion that makes a run of one
    // model safe.
    let first = *first.session();
    let second = *second.session();
    assert_eq!(first, second, "the CPU session was built a second time");
    assert_eq!(
        bench.built_on(),
        vec![ExecutionProvider::Cuda, ExecutionProvider::Cpu, ExecutionProvider::Cuda],
        "the downgrade was latched, or the CPU session was rebuilt"
    );
}

#[tokio::test]
async fn a_request_for_a_different_model_still_attempts_the_provider_it_asks_for() {
    // The reference implementation latches the first such failure for the rest of the process and clears it only
    // when the user changes processor, which this application has no way to do.
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { fails_on: vec![ExecutionProvider::Cuda], ..Bench::default() });
    let (first, second) = (kyoto(), tokyo());
    let sessions = sessions(
        &bench,
        &server,
        listing_for(&[&first, &second]),
        &app_dir,
        machine_supporting(false, true, false),
    );

    sessions
        .session(&first, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
        .await
        .unwrap();
    sessions
        .session(&second, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
        .await
        .unwrap();

    assert_eq!(
        bench.built_on(),
        vec![ExecutionProvider::Cuda, ExecutionProvider::Cpu, ExecutionProvider::Cuda, ExecutionProvider::Cpu],
        "the second model went straight to the CPU without attempting its provider"
    );
}

#[tokio::test]
async fn a_model_file_that_cannot_be_read_fails_without_a_cpu_attempt() {
    // It would fail there in the same way, and retrying it doubles the wait before reporting what was already
    // known.
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { unreadable: true, ..Bench::default() });
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, true, false));

    let error = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
        .await
        .unwrap_err();

    assert!(matches!(error, SessionError::Install(_)), "got {error:?}");
    assert_eq!(
        bench.built_on(),
        vec![ExecutionProvider::Cuda],
        "an unreadable model file was retried on the CPU"
    );
}
