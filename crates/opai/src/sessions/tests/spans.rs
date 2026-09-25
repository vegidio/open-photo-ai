//! What a request, a build and the install inside it send to the collector.

use super::*;
use crate::telemetry::metrics::PROVIDER_FALLBACKS;
use rust_sak::o11y::metric;

#[tokio::test]
async fn two_requests_waiting_on_one_build_share_one_build_span_and_the_second_links_to_it() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, false, false));

    let (profile, interest) = (EpProfile::default(), Interest::default());

    let (_, observed, (first, second)) = logging::traced_of("info", || async {
        let request = || sessions.session(&id, &profile, ExecutionProvider::Cpu, &interest);
        tokio::join!(request(), request())
    })
    .await;
    first.unwrap();
    second.unwrap();
    assert_eq!(bench.builds(), 1, "the fixture did not share one build");

    let requests = observed.named("session_request");
    assert_eq!(requests.len(), 2, "{:#?}", observed.spans);
    let build = observed.only("build");

    // The build is in the trace of the request that started it, and the other request is linked to it.
    assert_eq!(build.parent, Some(requests[0].index));
    assert_eq!(build.field("artifact"), Some(id.as_str()));
    assert_eq!(build.field("provider"), Some("CPU"));
    assert_eq!(build.field("outcome"), Some("finished"));
    assert!(requests[0].links.is_empty(), "{:?}", requests[0]);
    assert_eq!(requests[1].links, [build.index], "the waiting request is not linked to the build");

    for request in &requests {
        assert_eq!(request.field("artifact"), Some(id.as_str()));
        assert_eq!(request.field("requested"), Some("CPU"));
        assert_eq!(request.field("resident"), Some("false"));
        assert_eq!(request.field("outcome"), Some("finished"));
    }

    // The install the build needed is its child, and the build's records are in its trace.
    let install = observed.only("install");
    assert_eq!(install.parent, Some(build.index));
    assert_eq!(observed.event("session ready").and_then(|event| event.span), Some(build.index));
}

#[tokio::test]
async fn a_request_served_from_memory_says_so_and_opens_no_build() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, false, false));
    let (profile, interest) = (EpProfile::default(), Interest::default());
    let request = || sessions.session(&id, &profile, ExecutionProvider::Cpu, &interest);
    drop(request().await.unwrap());

    let (_, observed, handle) = logging::traced_of("info", request).await;
    handle.unwrap();

    assert_eq!(observed.only("session_request").field("resident"), Some("true"));
    assert!(observed.named("build").is_empty(), "{:#?}", observed.spans);
}

#[tokio::test]
async fn a_failed_gpu_build_marks_its_span_failed_and_counts_a_fallback() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { fails_on: vec![ExecutionProvider::Cuda], ..Bench::default() });
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, machine_supporting(false, true, false));

    // `>`, not an exact change: the downgrade tests beside this one fall back from CUDA too, without the recording
    // lock this holds.
    let fallbacks = || PROVIDER_FALLBACKS.value(&[("requested", "CUDA")]);
    let before = fallbacks();
    let (_, observed, handle) = logging::traced_of("info", || async {
        sessions
            .session(&id, &EpProfile::default(), ExecutionProvider::Cuda, &Interest::default())
            .await
    })
    .await;
    assert_eq!(handle.unwrap().provider(), ExecutionProvider::Cpu);
    assert!(fallbacks() > before, "the fallback was not counted");

    // One request, two builds under it: the one that failed, then the CPU one that served it.
    let request = observed.only("session_request");
    assert_eq!(request.field("outcome"), Some("finished"), "a downgrade is not a failed request");
    let builds = observed.named("build");
    assert_eq!(builds.len(), 2, "{:#?}", observed.spans);
    assert!(builds.iter().all(|build| build.parent == Some(request.index)));

    assert_eq!(builds[0].field("provider"), Some("CUDA"));
    assert_eq!(builds[0].field("outcome"), Some("failed"));
    assert!(
        builds[0].field("error").is_some_and(|error| error.contains("would not build the model")),
        "{:?}",
        builds[0]
    );
    assert_eq!(builds[1].field("provider"), Some("CPU"));
    assert_eq!(builds[1].field("outcome"), Some("finished"));
    assert_eq!(builds[1].field("error"), None);
}

#[tokio::test]
async fn a_build_abandoned_part_of_the_way_is_carried_on_under_the_same_span() {
    // The request running a build can have its future dropped, and the cell then hands the build to one still
    // waiting. That one carries it on under the span it is already linked to, so the trace holds one build, ended.
    let cache: SessionCache<Fake> = SessionCache::default();
    let id = kyoto();
    let (_release, held) = tokio::sync::oneshot::channel::<()>();
    let (first, second) = (Interest::default(), Interest::default());

    let (_, observed, built) = logging::traced_of("info", || async {
        let mut abandoned = Box::pin(
            cache
                .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &first, |_| async move {
                    let _ = held.await;
                    Ok(Some(Fake(1)))
                })
                .instrument(tracing::info_span!("abandoned")),
        );
        let mut carrying_on = Box::pin(
            cache
                .get_or_build(&id, ExecutionProvider::Cpu, ExecutionProvider::Cpu, &second, |_| async {
                    Ok(Some(Fake(2)))
                })
                .instrument(tracing::info_span!("carrying_on")),
        );

        let pending = Duration::from_millis(20);
        assert!(tokio::time::timeout(pending, &mut abandoned).await.is_err(), "the held build finished");
        assert!(tokio::time::timeout(pending, &mut carrying_on).await.is_err(), "the waiter did not wait");
        drop(abandoned);

        carrying_on.await
    })
    .await;
    assert_eq!(built.unwrap().map(|handle| *handle.session()), Some(Fake(2)));

    let build = observed.only("build");
    assert_eq!(build.parent, Some(observed.only("abandoned").index));
    assert_eq!(build.field("outcome"), Some("finished"), "the carried-on build did not end its span");
    assert_eq!(observed.only("carrying_on").links, [build.index]);
    assert_eq!(observed.event("session ready").and_then(|event| event.span), Some(build.index));
}

#[tokio::test]
async fn the_resident_gauge_follows_what_the_cache_holds_zeros_included() {
    let gauge = metric::gauge("opai_resident_sessions_test");
    let cache: SessionCache<Fake> = SessionCache::publishing_to(gauge.clone());
    let builds = AtomicUsize::new(0);

    let handle = build_through(&cache, &builds, &kyoto(), ExecutionProvider::Cpu).await.unwrap();
    assert_eq!(gauge.value(&[("provider", "CPU")]), Some(1.0));
    assert_eq!(gauge.value(&[("provider", "CoreML")]), Some(0.0), "an empty provider was not published");
    assert_eq!(gauge.value(&[("provider", "Auto")]), None, "a request's provider was published as one built on");

    drop(handle);
    cache.sweep(Instant::now());
    assert_eq!(cache.resident(), 0, "the sweep did not evict the idle session");
    assert_eq!(gauge.value(&[("provider", "CPU")]), Some(0.0), "the gauge kept the evicted session");

    build_through(&cache, &builds, &tokyo(), ExecutionProvider::Cpu).await.unwrap();
    cache.clear();
    assert_eq!(gauge.value(&[("provider", "CPU")]), Some(0.0), "the gauge kept a released session");
}
