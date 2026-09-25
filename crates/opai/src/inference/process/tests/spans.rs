//! What a run sends to the collector: its span, a span per computed step, and the sessions each step needed.

use super::*;
use crate::deps::test_server::{TestServer, fixture_listing};
use crate::logging::observed::{Observed, SeenSpan};
use crate::providers::tests::machine_supporting;
use crate::sessions::Sessions;
use crate::sessions::build::BuildRequest;
use crate::telemetry::metrics::{CACHE_LOOKUPS, FAILURES, STEP_DURATION};
use crate::telemetry::unit::TARGET;

/// The process fake, acquiring through a real [`Sessions`]: the request, the flight, the install and the build.
///
/// What the plain fake answers by itself, which is why its runs carry no session spans.
struct Through {
    sessions: Sessions<FakeSession>,
}

impl Through {
    /// Installing from `server`, and opening every artifact as a fake session that runs the way [`Fake`]'s do.
    fn new(server: &TestServer, app_dir: &std::path::Path, artifacts: &[&str]) -> Self {
        let fake = Fake::new();
        let (log, freed) = (Arc::clone(&fake.log), Arc::clone(&fake.freed));
        let listing = fixture_listing(&artifacts.iter().map(|id| format!("{id}.onnx")).collect::<Vec<_>>());

        let build = Arc::new(move |request: BuildRequest| {
            Ok(FakeSession {
                artifact: request.artifact.as_str().to_string(),
                log: Arc::clone(&log),
                fails: false,
                cancels: None,
                reports: None,
                native: Arc::new(FakeNative { open: 1, freed: Arc::clone(&freed) }),
            })
        });
        let supported = machine_supporting(false, false, false);

        Self {
            sessions: Sessions::serving(app_dir.to_path_buf(), supported, server.base_url.clone(), listing, build),
        }
    }
}

impl Backend for Through {
    type Session = FakeSession;
    type Error = std::io::Error;

    async fn acquire(
        &self,
        artifact: &ArtifactId,
        profile: &EpProfile,
        requested: ExecutionProvider,
        interest: &Interest,
    ) -> Result<SessionHandle<Self::Session>, SessionError> {
        self.sessions.session(artifact, profile, requested, interest).await
    }

    fn run_tile(handle: &SessionHandle<Self::Session>, input: &[f32], output: &mut [f32]) -> Result<(), Self::Error> {
        <Fake as Backend>::run_tile(handle, input, output)
    }

    stub_backend_runs!("a Kyoto pass takes the tile seam"; run_graph, run_named_outputs, run_weighted);
}

/// The one child of `parent` named `name`.
fn child<'a>(observed: &'a Observed, parent: &SeenSpan, name: &str) -> &'a SeenSpan {
    let children: Vec<&SeenSpan> =
        observed.named(name).into_iter().filter(|span| span.parent == Some(parent.index)).collect();

    match children.as_slice() {
        [only] => only,
        _ => panic!("expected one {name} under {}, found {}: {:#?}", parent.name, children.len(), observed.spans),
    }
}

#[tokio::test]
async fn an_enhancement_that_installs_and_builds_is_one_trace_of_its_steps_and_their_sessions() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let backend = Through::new(&server, &root.path().join("opai-test"), &["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"]);

    let steps = || STEP_DURATION.count(&[("model", "up_kyoto_4x_fp16"), ("provider", "CPU")]);
    let before = steps();
    let (_, observed, outcome) = logging::traced_of("info", || async {
        process(&backend, None, &picture(300, 200), &[kyoto(2.0), kyoto(4.0)], None).await
    })
    .await;
    outcome.unwrap();

    let run = observed.only("enhancement");
    assert_eq!(run.parent, None);
    assert_eq!(run.field("outcome"), Some("finished"));
    assert_eq!(run.field("identity"), Some("cafebabecafebabe"));
    assert_eq!(
        run.field("ids"),
        Some(format!("{},{}", kyoto(2.0).cache_tag(), kyoto(4.0).cache_tag()).as_str())
    );

    let steps_seen = observed.named("step");
    assert_eq!(steps_seen.len(), 2, "{:#?}", observed.spans);

    for (index, (step, model)) in steps_seen.iter().zip(["up_kyoto_2x_fp16", "up_kyoto_4x_fp16"]).enumerate() {
        assert_eq!(step.parent, Some(run.index), "step {index} is not the run's child");
        assert_eq!(step.field("index"), Some(index.to_string().as_str()));
        assert_eq!(step.field("model"), Some(model));
        assert_eq!(step.field("outcome"), Some("finished"));

        // Beneath the step that needed it: the request, the build it started, and the install inside that build.
        let request = child(&observed, step, "session_request");
        assert_eq!(request.field("artifact"), Some(model));
        let build = child(&observed, request, "build");
        let install = child(&observed, build, "install");
        assert_eq!(install.field("outcome"), Some("finished"));
    }

    assert!(steps() > before, "the computed step's duration was not measured");
}

#[tokio::test]
async fn a_cancelled_run_and_the_step_it_stopped_in_say_stopped_and_are_not_failed() {
    let token = CancellationToken::new();
    let mut backend = Fake::new();
    backend.cancels = Some(("up_kyoto_2x_fp16".to_string(), token.clone()));

    let options = ProcessOptions { cancel: token, ..Default::default() };
    let (_, observed, outcome) = logging::traced_of("info", || async {
        process(&backend, None, &picture(300, 200), &[kyoto(2.0), kyoto(4.0)], Some(options)).await
    })
    .await;
    assert!(matches!(outcome, Err(InferenceError::Cancelled)), "{:?}", outcome.err());

    let run = observed.only("enhancement");
    assert_eq!(run.field("outcome"), Some("stopped"));
    assert_eq!(run.field("error"), None, "a cancelled run was marked failed");

    let step = observed.only("step");
    assert_eq!(step.field("outcome"), Some("stopped"));
    assert_eq!(step.field("error"), None, "the step a cancellation landed in was marked failed");
}

#[tokio::test]
async fn a_failed_step_marks_its_own_span_and_the_runs() {
    let mut backend = Fake::new();
    backend.fails_to_run = Some("up_kyoto_4x_fp16".to_string());

    // `>` rather than `+ 1`: other process tests fail runs outside the recording lock, into the same series.
    let failures = || FAILURES.value(&[("unit", "enhancement"), ("kind", "run")]);
    let before = failures();
    let (_, observed, outcome) = logging::traced_of("info", || async {
        process(&backend, None, &picture(300, 200), &[kyoto(2.0), kyoto(4.0)], None).await
    })
    .await;
    assert!(matches!(outcome, Err(InferenceError::Run { .. })), "{:?}", outcome.err());
    assert!(failures() > before, "the failed run was not counted by its kind");

    let run = observed.only("enhancement");
    assert_eq!(run.field("outcome"), Some("failed"));
    assert!(run.field("error").is_some_and(|error| error.contains("the model failed")), "{run:?}");

    let steps = observed.named("step");
    assert_eq!(steps.len(), 2, "{:#?}", observed.spans);
    assert_eq!(steps[0].field("outcome"), Some("finished"));
    assert_eq!(steps[0].field("error"), None);
    assert_eq!(steps[1].field("outcome"), Some("failed"));
    assert!(
        steps[1].field("error").is_some_and(|error| error.contains("the model failed")),
        "{:?}",
        steps[1]
    );
}

#[tokio::test]
async fn a_large_image_sends_as_many_spans_as_a_small_one() {
    let spans_for = |width, height| async move {
        let backend = Fake::new();
        let (_, observed, outcome) = logging::traced_of("info", || async {
            process(&backend, None, &picture(width, height), &[kyoto(2.0), kyoto(4.0)], None).await
        })
        .await;
        outcome.unwrap();

        (observed.count_under(TARGET), observed.spans.len())
    };

    let small = spans_for(64, 48).await;
    let large = spans_for(1200, 900).await;

    assert!(small.0 > 0, "no unit span was sent");
    assert_eq!(small, large, "the number of spans grew with the image");
}

#[tokio::test]
async fn a_repeated_run_counts_its_steps_as_hits_and_opens_no_step_span() {
    let backend = Fake::new();
    let memo = store();
    let chain = [kyoto(2.0), kyoto(4.0)];

    cached_run(&backend, Some(&memo), &picture(300, 200), &chain, OutputDepth::Eight).await.unwrap();

    // `>=`: the cache tests beside this one count step hits too, without the recording lock this holds.
    let hits = || CACHE_LOOKUPS.value(&[("kind", "step"), ("result", "hit")]);
    let before = hits();
    let (_, observed, outcome) = logging::traced_of("info", || async {
        cached_run(&backend, Some(&memo), &picture(300, 200), &chain, OutputDepth::Eight).await
    })
    .await;
    outcome.unwrap();

    assert!(hits() - before >= 2, "the two stored steps were not counted as hits");
    assert!(observed.named("step").is_empty(), "a step served from the store opened a step span");
    assert_eq!(observed.only("enhancement").field("outcome"), Some("finished"));
}
