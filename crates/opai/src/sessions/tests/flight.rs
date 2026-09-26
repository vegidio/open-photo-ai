//! Requests joining a build already in flight, and the install inside it that answers to them.

use super::*;

/// One transfer report about `id`, as an install of a model's single file would produce.
fn downloading(id: &ArtifactId, fraction: f64) -> crate::progress::Progress {
    crate::progress::Progress {
        dependency: crate::progress::Dependency::Model(id.clone()),
        phase: crate::progress::Phase::Downloading,
        bytes: (fraction * 100.0) as u64,
        total: Some(100),
        fraction,
    }
}

/// A request that wants to hear about the install and can be stopped.
fn interested() -> (Interest, CancellationToken, Arc<Mutex<Vec<crate::progress::Progress>>>) {
    let (on_progress, heard) = crate::progress::recording();
    let cancel = CancellationToken::new();

    (Interest { on_progress: Some(on_progress), cancel: cancel.clone() }, cancel, heard)
}

/// The flight for `id` on the CPU, as the cache is holding it.
fn flight_for<S>(cache: &SessionCache<S>, id: &ArtifactId) -> Option<Arc<Flight<S, SessionError>>> {
    lock(&cache.flights).get(&(id.clone(), ExecutionProvider::Cpu, false)).map(Arc::clone)
}

/// Waits until exactly `count` requests are waiting on the CPU flight for `id`.
///
/// Read off the flight itself rather than slept for, so what the build waits for is the moment the other
/// requests have actually joined or gone — a sleep would make these tests either slow or a race, and the whole
/// question in each of them is what happens *at* that moment.
async fn waiting_on<S>(cache: &SessionCache<S>, id: &ArtifactId, count: usize) {
    for _ in 0..2_000 {
        let waiting = flight_for(cache, id).map(|flight| lock(&flight.audience).waiting.len());

        if waiting == Some(count) {
            return;
        }

        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    panic!("the flight for {id} never had exactly {count} requests waiting on it");
}

/// Whether the flight for `id` has told its install to stop.
fn stopping<S>(cache: &SessionCache<S>, id: &ArtifactId) -> bool {
    flight_for(cache, id).is_some_and(|flight| lock(&flight.audience).cancel.is_cancelled())
}

/// A build that stands in for one that installs: it reports, runs until nothing is waiting for it, and then
/// resumes if something has joined since — [`Sessions::install`]'s own loop, in the one form a suite with no
/// network can drive.
async fn transferring(install: Install, id: ArtifactId) -> Result<Option<Fake>, SessionError> {
    loop {
        let installing = install.attempt();
        installing.on_progress.expect("a flight always reports")(&downloading(&id, 0.5));

        // The bytes, standing in for minutes of them: this transfer runs until it is told to stop.
        installing.cancel.cancelled().await;

        // Stopped. Exactly the question `Sessions::install` asks before giving up: has anything joined since?
        if !install.wanted() {
            return Ok(None);
        }
    }
}

#[tokio::test]
async fn a_request_that_joins_a_build_already_in_flight_is_told_about_its_install() {
    // A model is installed *inside* the single flight, and an install is a transfer of gigabytes. Reporting it
    // only to the request that started the flight is what left a user who interrupted a download and asked for
    // the same enhancement again watching an indicator that said nothing: their run was handed the transfer that
    // was still running, and heard none of it.
    let cache: Arc<SessionCache<Fake>> = Arc::new(SessionCache::default());
    let builds = Arc::new(AtomicUsize::new(0));
    let id = kyoto();

    let (release, held) = tokio::sync::oneshot::channel::<()>();
    let (first, _, heard_first) = interested();
    let (second, _, heard_second) = interested();

    let starting = {
        let (cache, builds, id) = (Arc::clone(&cache), Arc::clone(&builds), id.clone());

        tokio::spawn(async move {
            let reported = id.clone();
            cache
                .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &first, |install| async move {
                    // Mid-transfer: the report goes out only once a second request has joined the flight.
                    held.await.unwrap();
                    let installing = install.attempt();
                    installing.on_progress.expect("a flight always reports")(&downloading(&reported, 0.5));

                    Ok(Some(Fake(builds.fetch_add(1, Ordering::SeqCst) + 1)))
                })
                .await
                .map(|handle| handle.map(|handle| *handle.session()))
        })
    };

    let joining = {
        let (cache, builds, id) = (Arc::clone(&cache), Arc::clone(&builds), id.clone());

        tokio::spawn(async move {
            cache
                .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &second, |_| async move {
                    Ok(Some(Fake(builds.fetch_add(1, Ordering::SeqCst) + 1)))
                })
                .await
                .map(|handle| handle.map(|handle| *handle.session()))
        })
    };

    waiting_on(&cache, &id, 2).await;
    release.send(()).unwrap();

    assert_eq!(starting.await.unwrap().unwrap(), Some(Fake(1)));
    assert_eq!(joining.await.unwrap().unwrap(), Some(Fake(1)), "a request was served a session of its own");
    assert_eq!(builds.load(Ordering::SeqCst), 1, "the model was installed and opened more than once");

    let report = downloading(&id, 0.5);
    assert_eq!(heard_first.lock().unwrap().clone(), vec![report.clone()]);
    assert_eq!(
        heard_second.lock().unwrap().clone(),
        vec![report],
        "a request that joined a transfer already in flight heard nothing of it"
    );
}

#[tokio::test]
async fn an_install_is_stopped_only_when_the_last_request_waiting_on_it_has_gone() {
    // The whole of "cancel means cancel", and the half of it that is easy to get wrong: a user who stops one run
    // has not stopped a second run that wants the same model, so the first cancellation must leave the transfer
    // running and the last one must end it.
    let cache: Arc<SessionCache<Fake>> = Arc::new(SessionCache::default());
    let id = kyoto();

    let (first, stop_first, _) = interested();
    let (second, stop_second, _) = interested();

    let requests: Vec<_> = [first, second]
        .into_iter()
        .map(|interest| {
            let (cache, id) = (Arc::clone(&cache), id.clone());

            tokio::spawn(async move {
                let transferring = |install| transferring(install, id.clone());

                cache
                    .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &interest, transferring)
                    .await
                    .map(|handle| handle.map(|handle| *handle.session()))
            })
        })
        .collect();

    waiting_on(&cache, &id, 2).await;
    assert!(!stopping(&cache, &id), "a transfer was stopped before anything had stopped waiting for it");

    stop_first.cancel();
    waiting_on(&cache, &id, 1).await;
    assert!(
        !stopping(&cache, &id),
        "one run's cancellation stopped a transfer another run was still waiting on"
    );

    stop_second.cancel();

    for request in requests {
        assert_eq!(
            request.await.unwrap().unwrap(),
            None,
            "a request that stopped waiting was handed a session rather than a stop"
        );
    }
}

#[tokio::test]
async fn a_request_that_joins_while_a_transfer_is_stopping_has_it_resumed_rather_than_given_up_on() {
    // The race the window makes on every keystroke: its cleanup stops the run in flight and its next request
    // starts a new one, and the two cross the boundary independently. The stop reaches the transfer first, so
    // what keeps that from becoming "the next request fails" is that joining re-arms the question — and the
    // install asks it again before giving up.
    let flight: Flight<Fake, SessionError> = Flight::default();
    let install = flight.installing();

    let mut waiting = flight.join(&Interest::default());
    let stopping = install.attempt().cancel;
    assert!(install.wanted());

    waiting.withdraw();

    assert!(stopping.is_cancelled(), "the transfer was not stopped when the last request waiting on it went");
    assert!(!install.wanted(), "an install with nobody waiting reported that it was still wanted");

    // The next request, arriving while that stop is still unwinding.
    let _joining = flight.join(&Interest::default());

    assert!(install.wanted(), "a request that joined a stopping transfer was not counted");
    assert!(
        !install.attempt().cancel.is_cancelled(),
        "a transfer resumed for the request that joined would have been stopped again at once"
    );
}

#[tokio::test]
async fn a_request_that_stops_waiting_stops_being_told_about_the_build_it_was_waiting_on() {
    // The other half of the audience: a run whose future is dropped - a command whose task went away, a window
    // that closed - must not leave a callback behind to be called for the rest of a transfer.
    let cache: Arc<SessionCache<Fake>> = Arc::new(SessionCache::default());
    let id = kyoto();

    let (release, held) = tokio::sync::oneshot::channel::<()>();
    let (first, _, heard_first) = interested();
    let (second, _, heard_second) = interested();

    let starting = {
        let (cache, id) = (Arc::clone(&cache), id.clone());

        tokio::spawn(async move {
            let reported = id.clone();
            cache
                .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &first, |install| async move {
                    held.await.unwrap();
                    let installing = install.attempt();
                    installing.on_progress.expect("a flight always reports")(&downloading(&reported, 0.5));

                    Ok(Some(Fake(1)))
                })
                .await
                .map(|handle| handle.map(|handle| *handle.session()))
        })
    };

    let abandoning = {
        let (cache, id) = (Arc::clone(&cache), id.clone());

        tokio::spawn(async move {
            cache
                .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &second, |_| async move {
                    Ok(Some(Fake(2)))
                })
                .await
                .map(|handle| handle.map(|handle| *handle.session()))
        })
    };

    waiting_on(&cache, &id, 2).await;

    // Dropped rather than cancelled through anything of this layer's: what the guard has to survive is the
    // future simply going away.
    abandoning.abort();
    let _ = abandoning.await;
    waiting_on(&cache, &id, 1).await;

    release.send(()).unwrap();
    assert_eq!(starting.await.unwrap().unwrap(), Some(Fake(1)));

    assert_eq!(
        heard_first.lock().unwrap().len(),
        1,
        "the request still waiting was not told about the transfer"
    );
    assert!(
        heard_second.lock().unwrap().is_empty(),
        "a request that had gone away was still being reported to"
    );
}

#[tokio::test]
async fn a_build_that_fails_fails_every_request_waiting_on_it_with_no_second_attempt() {
    // Each of them starting its own attempt at something that has just proved impossible would multiply one
    // failed provider initialization by however many tiles were in flight.
    let cache: Arc<SessionCache<Fake>> = Arc::new(SessionCache::default());
    let attempts = Arc::new(AtomicUsize::new(0));
    let id = kyoto();

    let racing: Vec<_> = (0..6)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let attempts = Arc::clone(&attempts);
            let id = id.clone();

            tokio::spawn(async move {
                cache
                    .get_or_build(
                        &id,
                        ExecutionProvider::Cuda,
                        ExecutionProvider::Cuda,
                        &Interest::default(),
                        |_| async {
                            attempts.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            Err(build_failure(&id, ExecutionProvider::Cuda))
                        },
                    )
                    .await
                    .map(|_| ())
            })
        })
        .collect();

    for racer in racing {
        let outcome = racer.await.unwrap();
        assert!(matches!(outcome, Err(SessionError::Build { .. })), "a waiter did not fail: {outcome:?}");
    }

    assert_eq!(attempts.load(Ordering::SeqCst), 1, "a waiter started a second build of its own");
    assert_eq!(cache.resident(), 0, "a failed build left something resident");
}
