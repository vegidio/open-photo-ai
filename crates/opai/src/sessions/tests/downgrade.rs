//! A provider that cannot open a model, and the CPU session built in its place.

use super::*;
use crate::providers::Accelerator;

#[tokio::test]
async fn a_provider_that_cannot_open_the_model_is_downgraded_to_the_cpu() {
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
    assert_eq!(bench.built_on(), vec![ExecutionProvider::Cuda, ExecutionProvider::Cpu]);
}

#[tokio::test]
async fn a_downgrade_is_served_the_cpu_session_an_explicit_cpu_request_filed() {
    // The fallback re-enters the cache rather than building beside it, so a CPU session that happened to be resident
    // is the one a downgrade is handed.
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

    assert_eq!(honoured.provider(), ExecutionProvider::Cpu);
    assert_eq!(downgraded.provider(), ExecutionProvider::Cpu);

    // Both are the one session — the point of the fallback re-entering the cache. Copied out in two statements, not
    // one: a tuple expression would hold the first borrow while taking the second, and these two handles are on the
    // one entry.
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

/// A machine offering every NVIDIA provider and WebGPU, so `Auto` has a four-rung ladder.
fn nvidia_with_webgpu() -> SupportedProviders {
    machine_supporting(false, true, true).with_webgpu(true)
}

/// The accelerators each build so far attached, in order.
fn attached(bench: &Bench) -> Vec<Vec<Accelerator>> {
    lock(&bench.builds)
        .iter()
        .map(|request| request.plan.providers.iter().map(|options| options.provider).collect())
        .collect()
}

#[tokio::test]
async fn auto_walks_its_ladder_one_provider_at_a_time_keeping_the_rest_attached_behind() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench =
        Arc::new(Bench { fails_on: vec![ExecutionProvider::TensorRt, ExecutionProvider::Cuda], ..Bench::default() });
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, nvidia_with_webgpu());

    let handle = sessions
        .session(&id, &EpProfile::default(), ExecutionProvider::Auto, &Interest::default())
        .await
        .unwrap();

    assert_eq!(handle.provider(), ExecutionProvider::WebGpu, "the ladder skipped a rung or fell to the CPU");
    use Accelerator::{Cuda, TensorRt, WebGpu};
    assert_eq!(attached(&bench), vec![vec![TensorRt, Cuda, WebGpu], vec![Cuda, WebGpu], vec![WebGpu]]);
}

#[tokio::test]
async fn auto_remembers_what_failed_for_a_model_until_the_sessions_are_released() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { fails_on: vec![ExecutionProvider::TensorRt], ..Bench::default() });
    let (first, second) = (kyoto(), tokyo());
    let sessions = sessions(&bench, &server, listing_for(&[&first, &second]), &app_dir, nvidia_with_webgpu());
    let request = |id: ArtifactId| {
        let sessions = &sessions;
        async move {
            sessions
                .session(&id, &EpProfile::default(), ExecutionProvider::Auto, &Interest::default())
                .await
                .unwrap()
                .provider()
        }
    };

    assert_eq!(request(first.clone()).await, ExecutionProvider::Cuda);
    // Served the CUDA session already filed, without attempting TensorRT again.
    assert_eq!(request(first.clone()).await, ExecutionProvider::Cuda);
    assert_eq!(bench.built_on(), vec![ExecutionProvider::TensorRt, ExecutionProvider::Cuda]);

    // Per model: another one still attempts TensorRT.
    assert_eq!(request(second.clone()).await, ExecutionProvider::Cuda);
    assert_eq!(bench.builds(), 4, "a different model did not attempt TensorRT");

    // A release forgets what was declined, so the whole ladder is tried again.
    sessions.cache().clear();
    assert_eq!(request(first.clone()).await, ExecutionProvider::Cuda);
    assert_eq!(bench.built_on()[4..], [ExecutionProvider::TensorRt, ExecutionProvider::Cuda]);
}

#[tokio::test]
async fn a_provider_declined_at_run_time_is_let_go_of_and_skipped_by_the_next_request() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, nvidia_with_webgpu());
    let request = || async {
        sessions
            .session(&id, &EpProfile::default(), ExecutionProvider::Auto, &Interest::default())
            .await
            .unwrap()
            .provider()
    };

    assert_eq!(request().await, ExecutionProvider::TensorRt);
    assert!(sessions.decline(&id, ExecutionProvider::TensorRt), "TensorRT was not newly declined");
    assert!(!sessions.decline(&id, ExecutionProvider::TensorRt), "declining twice reported a change");
    assert_eq!(sessions.cache().resident(), 0, "the session that failed was kept");

    assert_eq!(request().await, ExecutionProvider::Cuda);
    assert!(sessions.decline(&id, ExecutionProvider::Cuda));
    assert!(sessions.decline(&id, ExecutionProvider::WebGpu));
    assert_eq!(request().await, ExecutionProvider::Cpu);

    // The CPU is the last rung and is never declined.
    assert!(!sessions.decline(&id, ExecutionProvider::Cpu));
    assert_eq!(request().await, ExecutionProvider::Cpu);
}

#[tokio::test]
async fn an_explicit_request_is_not_widened_into_the_ladder_nor_remembered() {
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench { fails_on: vec![ExecutionProvider::TensorRt], ..Bench::default() });
    let id = kyoto();
    let sessions = sessions(&bench, &server, listing_for(&[&id]), &app_dir, nvidia_with_webgpu());

    for _ in 0..2 {
        let handle = sessions
            .session(&id, &EpProfile::default(), ExecutionProvider::TensorRt, &Interest::default())
            .await
            .unwrap();
        assert_eq!(handle.provider(), ExecutionProvider::Cpu, "an explicit TensorRT fell to CUDA");
    }

    assert_eq!(
        bench.built_on(),
        vec![ExecutionProvider::TensorRt, ExecutionProvider::Cpu, ExecutionProvider::TensorRt],
        "an explicit request's failure was remembered"
    );
}

#[tokio::test]
async fn an_auto_rung_and_an_explicit_request_for_its_provider_are_two_sessions() {
    // `Auto`'s CUDA rung keeps WebGPU attached behind it, and an explicit CUDA request does not.
    let server = TestServer::start(vec![]).await;
    let (_root, app_dir) = app();
    let bench = Arc::new(Bench::default());
    let id = kyoto();
    let sessions = sessions(
        &bench,
        &server,
        listing_for(&[&id]),
        &app_dir,
        machine_supporting(false, true, false).with_webgpu(true),
    );

    for provider in [ExecutionProvider::Auto, ExecutionProvider::Cuda] {
        let handle = sessions.session(&id, &EpProfile::default(), provider, &Interest::default()).await.unwrap();
        assert_eq!(handle.provider(), ExecutionProvider::Cuda);
    }

    assert_eq!(attached(&bench), vec![vec![Accelerator::Cuda, Accelerator::WebGpu], vec![Accelerator::Cuda]]);
}
