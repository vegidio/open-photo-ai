//! The live checks: a real ONNX Runtime, a real published model, a real session.
//!
//! Everything else in this module is exercised against a fake session on every CI platform. What cannot be is the
//! sequence of calls [`build`](super::build) makes into the runtime, because `ort::session::Session::builder` needs a
//! live environment — so this is where a session is actually opened.
//!
//! # Why these are not in `tests/`
//!
//! The entry point is `pub(crate)` in this slice, so nothing in an integration-test binary can reach it. They are an
//! integration test in substance — real transfers, a real runtime, a real graph, end to end — and a unit test only in
//! where they live. The slice that gives inference a public surface is what can move them.
//!
//! # Why every one of them is `#[ignore]`d
//!
//! Each downloads the pinned ONNX Runtime, which is ~175 MB on the platforms that ship it with its execution
//! providers, and the whole of the rest of this repository's suite is hermetic — it serves its own archives from a
//! local listener and contacts nothing. The same arrangement, for the same reason, as
//! [`crate::runtime`]'s live test and as Go's `internal/deps/live_manual_test.go`. Run them by hand:
//!
//! ```text
//! cargo test -p opai -- --ignored --nocapture
//! ```
//!
//! **The consequence is recorded rather than papered over: no CI platform builds a real session.** What CI proves is
//! that the decision around one — the plan, the dispatches, the cache, the fallback, the install — is right on all
//! three.
//!
//! The three GPU tests additionally need hardware no CI runner has. Each states that as its reason, and each *fails*
//! on a machine that cannot serve its provider rather than quietly passing: a test that reported success without
//! opening a model would be worse than no test.

use crate::live_support::live_application;
use crate::models::{Bias, FloatPrecision, LightAdjustment, LightAdjustmentVariant, Operation};
use crate::providers::ExecutionProvider;

/// The application name these install under, which is also the configuration directory they create.
const NAME: &str = "opai-live-session-test";

/// The model every one of these opens: Paris at FP16, 122 KB, the smallest graph the project publishes.
///
/// Small deliberately. What is being checked is that a published graph becomes a session on the provider that was
/// asked for, and a larger model would only make the same check slower.
fn paris() -> Operation {
    Operation::LightAdjustment(LightAdjustment::new(
        LightAdjustmentVariant::Paris(FloatPrecision::Fp16),
        Bias::new(0.0).expect("0.0 is in range"),
    ))
}

/// Opens `paris()` on `provider` and asserts the session reports it.
///
/// Shared by all four, so the GPU tests differ from the CPU one only in the provider they ask for and in needing
/// hardware. A machine that cannot serve the provider fails here rather than being served a downgrade the assertion
/// would then have to be loosened for.
async fn opens_on(provider: ExecutionProvider) {
    let (_root, opai) = live_application(NAME).await;
    let operation = paris();
    let artifact = operation.required_artifacts().remove(0);

    assert!(
        opai.providers().supports(provider),
        "this machine reports no support for {provider}; this test needs hardware that has it"
    );

    let handle = opai
        .session(&artifact, &operation.profile(), provider, &crate::sessions::Interest::default())
        .await
        .unwrap_or_else(|err| panic!("{artifact} did not open on {provider}: {err}"));

    assert_eq!(handle.provider(), provider, "the session was downgraded rather than built on {provider}");

    println!("{artifact} opened on {} ({})", handle.provider(), ort::info());
}

#[tokio::test]
#[ignore = "downloads the pinned runtime and a published model; run by hand with --ignored"]
async fn a_published_model_is_installed_opened_on_the_cpu_and_kept_resident() {
    let (root, opai) = live_application(NAME).await;
    let operation = paris();
    let artifact = operation.required_artifacts().remove(0);

    let first = opai
        .session(&artifact, &operation.profile(), ExecutionProvider::Cpu, &crate::sessions::Interest::default())
        .await
        .unwrap_or_else(|err| panic!("{artifact} did not open on the CPU: {err}"));

    // Installed on the way, through the path slice 1 built and nothing had ever called.
    let model = root.path().join(NAME).join("models").join(artifact.as_str()).join(format!("{artifact}.onnx"));
    assert!(model.is_file(), "the graph is not on disk at {}", model.display());

    assert_eq!(first.provider(), ExecutionProvider::Cpu, "a CPU request was built on something else");

    // Kept resident: the second request is served the session the first built.
    let second = opai
        .session(&artifact, &operation.profile(), ExecutionProvider::Cpu, &crate::sessions::Interest::default())
        .await
        .expect("a resident session must be served again");
    assert_eq!(second.provider(), ExecutionProvider::Cpu);

    // And released on request, without waiting on the two handles still held.
    opai.release_sessions();

    println!("{artifact} opened on the CPU ({})", ort::info());
}

#[tokio::test]
#[ignore = "needs an Apple GPU, which no CI runner has; run by hand on a Mac with --ignored"]
async fn a_published_model_opens_on_coreml() {
    opens_on(ExecutionProvider::CoreMl).await;
}

#[tokio::test]
#[ignore = "needs an NVIDIA card, which no CI runner has; run by hand on one with --ignored"]
async fn a_published_model_opens_on_cuda() {
    opens_on(ExecutionProvider::Cuda).await;
}

#[tokio::test]
#[ignore = "needs an RTX-branded NVIDIA card, which no CI runner has; run by hand on one with --ignored"]
async fn a_published_model_opens_on_tensorrt() {
    opens_on(ExecutionProvider::TensorRt).await;
}
