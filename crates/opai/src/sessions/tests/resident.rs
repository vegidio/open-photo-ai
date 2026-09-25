//! What stays resident, what a release frees, and when the idle sweep lets a session go.

use super::*;
use crate::sessions::resident::{IDLE_LIFE, SWEEP_INTERVAL};

#[tokio::test]
async fn the_same_model_requested_twice_is_opened_once_and_both_requests_are_served_it() {
    let (cache, builds) = counting_cache();
    let id = kyoto();

    let first = build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    let second = build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();

    assert_eq!(builds.load(Ordering::SeqCst), 1, "the model was opened twice");
    assert_eq!(*first.session(), Fake(1));
    assert_eq!(*second.session(), Fake(1), "the second request was served a different session");
    assert_eq!(cache.resident(), 1);
}

#[tokio::test]
async fn one_model_on_two_providers_is_two_sessions_and_each_request_gets_the_one_it_asked_for() {
    // Two sessions holding two separate sets of native resources, and a request naming a provider is only ever
    // served one built on that provider — a CPU session handed to a CUDA request would be a silent downgrade
    // nothing reported.
    let (cache, builds) = counting_cache();
    let id = kyoto();

    let cpu = build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    let cuda = build_through(&cache, &builds, &id, ExecutionProvider::Cuda).await.unwrap();

    assert_eq!(builds.load(Ordering::SeqCst), 2);
    assert_eq!(cache.resident(), 2);
    assert_eq!(cpu.provider(), ExecutionProvider::Cpu);
    assert_eq!(cuda.provider(), ExecutionProvider::Cuda);
    let on_cpu = *cpu.session();
    let on_cuda = *cuda.session();
    assert_ne!(on_cpu, on_cuda, "two providers were served one session");
}

#[tokio::test]
async fn a_session_removed_while_in_use_stays_usable_and_the_next_request_gets_a_fresh_one() {
    // The half of "released exactly once, only when nothing is using it" that is observable: the compiler owns
    // the other half. Eviction drops the cache's reference and nothing else, so a holder keeps a fully usable
    // session and a request arriving afterwards builds its own rather than resurrecting a dying one.
    let (cache, builds) = counting_cache();
    let id = kyoto();

    let held = build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    cache.clear();

    assert_eq!(cache.resident(), 0, "clearing left an entry behind");
    assert_eq!(*held.session(), Fake(1), "a session removed from the cache stopped being usable");

    let fresh = build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();

    assert_eq!(builds.load(Ordering::SeqCst), 2);
    assert_eq!(*fresh.session(), Fake(2), "the new request was served the session that had been removed");
    assert_eq!(*held.session(), Fake(1), "the two are not separate");
}

#[tokio::test]
async fn clearing_returns_without_waiting_on_a_session_that_is_still_held() {
    // A run in progress can legitimately last far longer than anything a shutdown should wait for, and the
    // session it is using is released by its own last holder rather than by this call.
    let (cache, builds) = counting_cache();
    let id = kyoto();
    let running = build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();

    cache.clear();

    assert_eq!(cache.resident(), 0);
    assert_eq!(*running.session(), Fake(1), "the run in progress lost its session");
}

#[tokio::test]
async fn clearing_with_nothing_in_use_frees_the_sessions_before_it_returns() {
    // The half of the requirement `resident() == 0` cannot show. Releasing first is how a caller makes a
    // measurement start from nothing, and a release that had not finished by the time the call returned would
    // hand back a still-resident session and make the rebuild after it look free. It falls out of ownership
    // rather than being enforced: with nothing holding a handle, the cache's own `Arc` is the last one, so the
    // native session is dropped inside `clear`.
    let cache: SessionCache<Dropping> = SessionCache::default();
    let released = Arc::new(Mutex::new(Vec::new()));

    for (serial, id) in [kyoto(), tokyo()].into_iter().enumerate() {
        build_and_release(&cache, &released, &id, serial).await;
    }

    assert_eq!(cache.resident(), 2);
    assert!(lock(&released).is_empty(), "a session was released before anything asked for it");

    cache.clear();

    // Read on the line after the call and with nothing awaited in between, so this is the state `clear` left
    // rather than one a deferred sweep was given time to reach.
    let mut freed = lock(&released).clone();
    freed.sort_unstable();
    assert_eq!(freed, vec![0, 1], "clearing returned with a session it had released still alive");
}

#[tokio::test]
async fn a_session_a_run_is_holding_is_freed_when_that_run_ends_and_not_before() {
    // The converse, and the reason the release is not a wait: a run in progress can legitimately last far
    // longer than anything a shutdown should block on, so `clear` gives up the cache's reference and the run's
    // own handle keeps the session alive until it is done with it.
    let cache: SessionCache<Dropping> = SessionCache::default();
    let released = Arc::new(Mutex::new(Vec::new()));

    build_and_release(&cache, &released, &kyoto(), 0).await;
    let running = cache
        .get_or_build(&tokyo(), ExecutionProvider::Cpu, ExecutionProvider::Cpu, &Interest::default(), |_| async {
            Ok::<Option<Dropping>, SessionError>(Some(Dropping { serial: 1, released: Arc::clone(&released) }))
        })
        .await
        .unwrap()
        .expect("the build was not stopped");

    cache.clear();

    assert_eq!(cache.resident(), 0, "clearing left an entry behind");
    assert_eq!(lock(&released).clone(), vec![0], "the session a run was holding was freed underneath it");

    drop(running);

    assert_eq!(lock(&released).clone(), vec![0, 1], "the session in use was not freed when its run ended");
}

#[tokio::test]
async fn several_concurrent_requests_for_one_model_open_it_exactly_once() {
    let cache: Arc<SessionCache<Fake>> = Arc::new(SessionCache::default());
    let builds = Arc::new(AtomicUsize::new(0));
    let id = kyoto();

    let racing: Vec<_> = (0..10)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let builds = Arc::clone(&builds);
            let id = id.clone();

            tokio::spawn(async move {
                cache
                    .get_or_build(
                        &id,
                        ExecutionProvider::Cpu,
                        ExecutionProvider::Cpu,
                        &Interest::default(),
                        |_| async {
                            // An await inside the build, so the requests really do overlap rather than each finding
                            // the work of the one before it already finished.
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            Ok(Some(Fake(builds.fetch_add(1, Ordering::SeqCst) + 1)))
                        },
                    )
                    .await
                    .map(|handle| *handle.expect("the build was not stopped").session())
            })
        })
        .collect();

    for racer in racing {
        assert_eq!(racer.await.unwrap().unwrap(), Fake(1), "a request was served a session of its own");
    }

    assert_eq!(builds.load(Ordering::SeqCst), 1, "the model was opened more than once");
    assert_eq!(cache.resident(), 1);
}

#[tokio::test]
async fn an_entry_unused_past_the_cutoff_is_released_and_one_used_since_survives() {
    let (cache, builds) = counting_cache();
    let stale = kyoto();
    let fresh = tokyo();

    drop(build_through(&cache, &builds, &stale, ExecutionProvider::Cpu).await.unwrap());
    // A gap the clock can resolve on every platform, so "before" and "after" are not the same instant.
    std::thread::sleep(Duration::from_millis(5));
    let cutoff = Instant::now();
    std::thread::sleep(Duration::from_millis(5));
    drop(build_through(&cache, &builds, &fresh, ExecutionProvider::Cpu).await.unwrap());

    cache.sweep(cutoff);

    assert_eq!(cache.resident(), 1, "the sweep kept the wrong number of sessions");
    assert!(
        cache.get(&fresh, ExecutionProvider::Cpu).is_some(),
        "a session used since the cutoff was released"
    );
    assert!(cache.get(&stale, ExecutionProvider::Cpu).is_none(), "a session unused past the cutoff survived");
}

#[tokio::test]
async fn a_session_something_is_using_survives_the_sweep_whatever_its_timestamp() {
    // One use can legitimately last far longer than the interval, and the sweep must not be what decides whether
    // that is allowed.
    let (cache, builds) = counting_cache();
    let id = kyoto();
    let running = build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();

    // Everything, by any timestamp: the present is later than any stamp an entry can carry.
    cache.sweep(Instant::now());

    assert_eq!(cache.resident(), 1, "a session in use was released");
    assert_eq!(*running.session(), Fake(1));
}

#[tokio::test]
async fn letting_go_of_a_handle_restamps_the_entry_so_the_interval_runs_from_the_end_of_the_use() {
    // Stamping only when a handle is handed out would evict a session fifteen minutes after a two-hour run
    // *started*, moments after that run let go of it.
    let (cache, builds) = counting_cache();
    let id = kyoto();

    let held = build_through(&cache, &builds, &id, ExecutionProvider::Cpu).await.unwrap();
    std::thread::sleep(Duration::from_millis(5));
    let during = Instant::now();
    std::thread::sleep(Duration::from_millis(5));
    drop(held);

    cache.sweep(during);

    assert_eq!(
        cache.resident(),
        1,
        "the entry was stamped when it was handed out rather than when it was let go"
    );
}

#[test]
fn the_sweep_runs_oftener_than_the_idle_life_it_enforces() {
    // The two being one number is the mistake this guards against. A task that looks every `IDLE_LIFE` does not
    // see a session that fell idle a moment after a tick until the next one, so a session would be released
    // anywhere between fifteen and thirty minutes after its last use — and fifteen is what is specified.
    assert!(SWEEP_INTERVAL < IDLE_LIFE, "the sweep period must be shorter than the idle life it enforces");

    // What a session can outlive its last use by is `IDLE_LIFE + SWEEP_INTERVAL`, so this is what bounds the
    // overshoot at a quarter of the interval rather than the whole of it.
    assert!(
        SWEEP_INTERVAL <= IDLE_LIFE / 4,
        "a session can outlive its idle life by more than a quarter of it"
    );
}

/// Every run the catalogue can name, built through the round trip a front end takes.
///
/// Every published row, at every precision it publishes, with the parameters that row published — which is the
/// whole of what a front end can name, and therefore the whole of what the invariant below has to hold over.
///
/// Reported as [`Subject`](crate::models::Subject)s, because the eight families are carried in two
/// values and the catalogue crosses both.
fn every_catalogue_run() -> Vec<crate::models::Subject> {
    use crate::models::test_support::published_values;

    let mut runs = Vec::new();

    for entry in catalogue() {
        for variant in &entry.variants {
            for &precision in variant.precisions {
                let built = variant.build(precision, &published_values(variant));

                runs.push(built.unwrap_or_else(|error| {
                    panic!(
                        "{:?}'s {} at {precision} is published but names no run: {error}",
                        entry.family, variant.codename
                    )
                }));
            }
        }
    }

    runs
}

#[test]
fn no_two_operations_resolving_to_one_artifact_declare_different_profiles() {
    // The invariant keying the cache on the artifact rests on. The key does not carry the profile, and the
    // profile decides the options a session is built with — so two operations that shared an artifact and
    // declared different profiles would be served each other's session.
    //
    // A test rather than a comment because the diffusion upscaler is the case that tests it: a per-graph
    // override, where one graph of a set wants a setting the other two do not. That override is per *graph*, and
    // a graph is an artifact, so it is expressible within this key, which is what this says.
    let catalogued = every_catalogue_run();
    assert!(!catalogued.is_empty(), "the traversal reached no published row at all");

    let mut declared: BTreeMap<ArtifactId, (EpProfile, String)> = BTreeMap::new();
    let mut families = BTreeSet::new();

    // Both spreads: the catalogue's, which is what a front end can name, and the typed constructors', which is
    // what the library itself can build. And both carriers, which is what makes it all eight families rather
    // than the seven whose result is an image — a profile is declared per graph, and detection declares one.
    //
    // A profile is not on `Subject`, because it is not something a report names: it is asked of the carrier,
    // which is what the two arms below do.
    let analyses = crate::models::test_support::every_analysis();
    let profiles = crate::models::test_support::every_operation()
        .into_iter()
        .map(|operation| {
            (
                operation.family(),
                operation.display_name(),
                operation.profile(),
                operation.required_artifacts(),
            )
        })
        .chain(analyses.iter().map(|analysis| {
            (analysis.family(), analysis.display_name(), analysis.profile(), analysis.required_artifacts())
        }))
        .chain(catalogued.iter().map(|run| {
            use crate::models::Subject;

            let profile = match run {
                Subject::Enhancement(operation) => operation.profile(),
                Subject::Analysis(analysis) => analysis.profile(),
            };

            (run.family(), run.display_name(), profile, run.required_artifacts())
        }));

    for (family, display_name, profile, required) in profiles {
        families.insert(family.prefix());

        for artifact in required {
            if let Some((previous, by)) = declared.get(&artifact) {
                assert_eq!(
                    previous, &profile,
                    "{artifact} is required by {by} and by {display_name}, which declare different profiles"
                );
            } else {
                declared.insert(artifact, (profile.clone(), display_name.clone()));
            }
        }
    }

    assert_eq!(families.len(), 8, "the traversal did not reach all eight families: {families:?}");
    assert!(!declared.is_empty(), "the traversal named no artifacts at all");
}
