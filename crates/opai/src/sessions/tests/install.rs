//! Putting a model on disk before a session is built from it.

use super::*;

#[tokio::test]
async fn a_session_for_an_artifact_that_is_not_on_disk_installs_it_first() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, false, false));

    let handle = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cpu, &Interest::default())
        .await
        .unwrap();

    assert!(server.requests() > 0, "nothing was transferred for an artifact that was never installed");
    let model = app_dir.join("models").join(id.as_str()).join(format!("{id}.onnx"));
    assert!(model.is_file(), "the model was not installed before the session was built");
    assert_eq!(bench.builds(), 1);
    assert_eq!(lock(&bench.builds)[0].model, model, "the build was handed a path the install did not write");
    assert_eq!(handle.provider(), ExecutionProvider::Cpu);
}

#[tokio::test]
async fn a_request_cancelled_while_its_model_transfers_stops_the_transfer_and_reports_a_stop() {
    // The whole chain of it, against a real transfer: the run's token reaches the flight, the flight's audience
    // empties, the install is told to stop, and what comes back is a stop rather than a failed install. A server
    // that holds the connection open is what makes the cancellation land mid-transfer rather than after it.
    let server = TestServer::start(vec![Reply::Stalled(32 * 1024)]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, false, false));

    let cancel = CancellationToken::new();
    let interest = Interest { on_progress: None, cancel: cancel.clone() };

    let stopping = {
        // The partial the transfer is writing into. Waiting for bytes in it rather than for the request to be
        // answered, so the stop is timed off the transfer having started rather than off the server.
        let part = app_dir.join("models").join(id.as_str()).join(format!("{id}.onnx.part"));

        tokio::spawn(async move {
            for _ in 0..5_000 {
                if std::fs::metadata(&part).is_ok_and(|meta| meta.len() > 0) {
                    cancel.cancel();
                    return;
                }

                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            panic!("the transfer never wrote anything for the cancellation to interrupt");
        })
    };

    let outcome = sessions.session(&id, &EpProfile::default(), ExecutionProvider::Cpu, &interest).await;
    stopping.await.unwrap();

    let Err(SessionError::Install(error)) = &outcome else {
        panic!("a request cancelled mid-transfer returned {outcome:?}");
    };
    assert!(matches!(**error, InitError::Stopped), "a stopped transfer was reported as {error}");

    assert_eq!(bench.builds(), 0, "a model that never finished arriving was opened anyway");
    assert_eq!(sessions.cache().resident(), 0, "a stopped build left a session behind");
    assert!(
        !app_dir.join("models").join(id.as_str()).join(format!("{id}.onnx")).is_file(),
        "a stopped transfer was promoted as though it had finished"
    );
}

#[tokio::test]
async fn a_session_for_an_artifact_that_is_already_on_disk_transfers_nothing() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, false, false));

    sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cpu, &Interest::default())
        .await
        .unwrap();
    let after_install = server.requests();

    // Released, so the next request really does reach the install rather than being served from memory.
    sessions.cache().clear();
    sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cpu, &Interest::default())
        .await
        .unwrap();

    assert_eq!(server.requests(), after_install, "a model already on disk was transferred again");
    assert_eq!(bench.builds(), 2, "the released session was not rebuilt");
}

#[tokio::test]
async fn ten_concurrent_requests_for_an_uninstalled_model_install_it_once() {
    // The whole reason the install is inside the single flight rather than before it.
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    let sessions = Arc::new(sessions(
        &bench,
        &server,
        listing_for(&[&id]),
        &app_dir,
        machine_supporting(false, false, false),
    ));

    let racing: Vec<_> = (0..10)
        .map(|_| {
            let sessions = Arc::clone(&sessions);
            let id = id.clone();

            tokio::spawn(async move {
                sessions
                    .session(&id, &EpProfile::default(), ExecutionProvider::Cpu, &Interest::default())
                    .await
                    .map(|_| ())
            })
        })
        .collect();

    for racer in racing {
        racer.await.unwrap().unwrap();
    }

    assert_eq!(bench.builds(), 1, "ten requests opened the model more than once");
    // One request for the graph. A second would mean two installs raced, which is what the flight prevents.
    assert_eq!(server.requests(), 1, "the model was transferred more than once");
}

#[tokio::test]
async fn an_artifact_that_cannot_be_installed_fails_as_an_install_failure_with_no_session_built() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    // A listing that publishes a different model, so this one has no hash to verify against and is refused.
    let sessions =
        sessions(&bench, &server, listing_for(&[&tokyo()]), &app_dir, machine_supporting(false, false, false));

    let error = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Cpu, &Interest::default())
        .await
        .unwrap_err();

    match error {
        SessionError::Install(source) => {
            assert!(matches!(&*source, InitError::UnpublishedModel { .. }), "got {source:?}");
        }
        other => panic!("expected an install failure, got {other:?}"),
    }
    assert_eq!(bench.builds(), 0, "a session was built for a model that could not be installed");
}
