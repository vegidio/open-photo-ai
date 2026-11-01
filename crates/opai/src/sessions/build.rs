//! Turning one rung of a ladder and a model file into a live ONNX session.
//!
//! The build itself is `rust_sak::ai::build`: it attaches the rung's providers with their baseline and the model's
//! tuning, every dispatch with `error_on_failure()` so a refused option fails the rung instead of silently costing the
//! provider, serialises every TensorRT build against the shared timing cache and empties that cache when one fails,
//! and says whether a failure was the device running out of memory. What is left here is what this crate decides
//! around it: the span its records are read under, the model-file check that tells an install failure apart from a
//! build failure, and the error the fallback reads.
//!
//! # The build runs inside a span, and that is what makes the runtime's own records readable
//!
//! [`build`] enters a span carrying the artifact and the provider being attached. ONNX Runtime's C++ diagnostics arrive
//! as `tracing` events under the target `ort` with the runtime's own location and logger id and nothing that
//! identifies the model — the C++ side does not know it — and the log's formatter renders an enclosing span's fields
//! onto every record emitted inside it. `rust_sak::ai::build` runs on the calling thread as a documented contract, which
//! is what makes this work. See the comment on [`build`] for what it excludes.

use rust_sak::ai::ort;
use std::path::PathBuf;
use std::sync::Arc;

use ort::session::Session;
use rust_sak::ai::{CachePaths, RungError, SessionRequest, Tuning};

use crate::error::{InitError, OutOfDeviceMemory, SessionError};
use crate::models::ArtifactId;
use crate::providers::{ExecutionProvider, SupportedProviders};
use crate::runtime::diagnostics;

/// Everything one session build is made from, owned rather than borrowed.
///
/// Owned because the build moves to a blocking thread: opening a graph is anything from a fifth of a second to
/// several minutes of engine compilation, and in a Tauri process the asynchronous runtime is the window's event loop.
#[derive(Debug, Clone)]
pub(crate) struct BuildRequest {
    /// The artifact being opened, for whatever a failure has to name.
    pub(crate) artifact: ArtifactId,
    /// What the caller asked for, which the rung was taken from.
    pub(crate) requested: ExecutionProvider,
    /// What this machine offers.
    pub(crate) supported: SupportedProviders,
    /// The providers to attach, in order. Empty for the CPU.
    pub(crate) rung: Vec<ExecutionProvider>,
    /// The tuning measured for this artifact.
    pub(crate) tuning: Tuning,
    /// The folders the providers compile into: this model's own, and the timing cache every model shares.
    pub(crate) cache: CachePaths,
    /// The graph file on disk.
    pub(crate) model: PathBuf,
}

impl BuildRequest {
    /// The provider this build is on: the rung's first, or the CPU for an empty rung.
    pub(crate) fn provider(&self) -> ExecutionProvider {
        provider_of(&self.rung)
    }
}

/// The provider a rung is on: its first, or the CPU when it attaches nothing.
pub(crate) fn provider_of(rung: &[ExecutionProvider]) -> ExecutionProvider {
    rung.first().copied().unwrap_or(ExecutionProvider::Cpu)
}

/// Opens `request`'s model and returns the session.
///
/// # Errors
///
/// Returns [`SessionError::Install`] where the model file is not there or cannot be read — which is a model that is
/// not on disk, fails the same way on every provider, and is therefore never retried — and [`SessionError::Build`]
/// where the file opens and the runtime will not build a session from it, naming the provider being attached.
pub(crate) fn build(request: BuildRequest) -> Result<Session, SessionError> {
    let provider = request.provider();
    let BuildRequest { artifact, requested, supported, rung, tuning, cache, model } = request;

    // The one span in this crate, and it is spent here because of what it decorates: ONNX Runtime's own C++
    // diagnostics — *"Some nodes were not assigned…"*, a provider declining to attach, an engine being rebuilt —
    // arrive as `tracing` events under the target `ort` carrying the runtime's own location and logger id, and
    // nothing that says which model they are about, because the C++ side does not know. Entered here, this span's
    // fields are rendered onto every one of them by the formatter.
    //
    // **What it covers and what it does not.** A span's fields reach an event only on the thread that entered it.
    // The runtime emits the records that matter here — session construction, provider attachment, graph
    // partitioning — on the calling thread, so they are decorated. Anything it emits later from one of its own
    // worker threads, during inference rather than during the build, is not, and arrives with the runtime's fields
    // alone.
    let span = tracing::info_span!("session_build", artifact = %artifact, provider = %provider);
    let _entered = span.enter();

    // Before the builder is touched, and the whole of the distinction the specs ask for. Everything after this point
    // is "on disk and would not open", which is the only failure a CPU retry could answer.
    drop(std::fs::File::open(&model).map_err(InitError::io(&model))?);

    let request = SessionRequest {
        model: &model,
        providers: std::slice::from_ref(&requested),
        supported,
        tuning: &tuning,
        cache: &cache,
    };
    let session = rust_sak::ai::build(&request, &rung).map_err(|error| failed(&artifact, provider, error));

    // A build is where the runtime's bursts happen, so its end is where their tallies are written — inside the span
    // above, so they name the model too.
    diagnostics::flush();

    session
}

/// The error a rung's failure is reported as: marked as the device running out of memory where `rust-sak` says it
/// was, which is the one failure releasing other sessions can cure.
fn failed(artifact: &ArtifactId, provider: ExecutionProvider, error: RungError) -> SessionError {
    let out_of_memory = error.out_of_memory;
    let source: Arc<dyn std::error::Error + Send + Sync> = Arc::new(error);
    let source = if out_of_memory { Arc::new(OutOfDeviceMemory { source }) } else { source };

    SessionError::Build { artifact: artifact.as_str().to_string(), provider, source }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging;

    #[test]
    fn the_runtime_s_own_diagnostics_are_told_which_model_they_are_about() {
        // What the span buys, checked at the seam it actually works through: the formatter renders an enclosing span's
        // fields onto every record emitted inside it, including ones this crate did not emit. An `ort`-target event
        // stands in for a C++ diagnostic, which cannot be provoked without a live runtime and a real model.
        //
        // What this does *not* cover, and cannot: a record the runtime emits later from one of its own worker
        // threads. A span's fields reach an event only on the thread that entered the span, and the build thread is
        // the one ORT constructs sessions and partitions graphs on.
        let artifact = crate::models::test_support::kyoto();

        let (log, ()) = logging::records_of_blocking("info,ort=warn", || {
            let span = tracing::info_span!(
                "session_build",
                artifact = %artifact,
                provider = %ExecutionProvider::TensorRt
            );
            let _entered = span.enter();

            tracing::warn!(target: "ort", location = "session_state.cc:1166", "Some nodes were not assigned...");
        });

        let diagnostic = log
            .lines()
            .find(|line| line.contains("target=ort"))
            .unwrap_or_else(|| panic!("the runtime's record did not reach the file:\n{log}"));

        assert!(
            diagnostic.contains(&format!("artifact={artifact}")),
            "the runtime's diagnostic does not say which model: {diagnostic}"
        );
        assert!(
            diagnostic.contains(&format!("provider={}", ExecutionProvider::TensorRt)),
            "the runtime's diagnostic does not say what was being attached: {diagnostic}"
        );
        // And the runtime's own fields survive beside them rather than being replaced.
        assert!(diagnostic.contains("location=session_state.cc:1166"), "{diagnostic}");
    }

    /// A rung's failure on `providers`, as `rust-sak` reports it.
    fn rung_error(providers: Vec<ExecutionProvider>, reason: &str, out_of_memory: bool) -> RungError {
        RungError { providers, reason: reason.to_string(), out_of_memory }
    }

    /// The runtime's message for the TensorRT failure from the field, which does not mention memory.
    const NO_ENGINE: &str = "TensorRT EP failed to create engine from network for fused node: TRTKernel_graph_0";

    #[test]
    fn a_rung_rust_sak_marks_out_of_memory_is_an_out_of_memory_build_failure() {
        let artifact = crate::models::test_support::kyoto();
        let rung = vec![ExecutionProvider::TensorRt, ExecutionProvider::Cuda];

        let error = failed(&artifact, provider_of(&rung), rung_error(rung.clone(), NO_ENGINE, true));
        assert!(error.is_out_of_memory());
        assert!(error.to_string().contains("the device ran out of memory"), "{error}");
        assert!(matches!(error, SessionError::Build { provider: ExecutionProvider::TensorRt, .. }));

        let error = failed(&artifact, provider_of(&rung), rung_error(rung, NO_ENGINE, false));
        assert!(!error.is_out_of_memory());
    }

    #[test]
    fn the_runtimes_reason_is_what_the_failure_says() {
        let artifact = crate::models::test_support::kyoto();
        let error =
            failed(&artifact, ExecutionProvider::Cuda, rung_error(vec![ExecutionProvider::Cuda], NO_ENGINE, false));

        let SessionError::Build { source, .. } = &error else { panic!("{error:?}") };
        assert_eq!(source.to_string(), NO_ENGINE);
    }

    #[test]
    fn an_empty_rung_is_the_cpu() {
        assert_eq!(provider_of(&[]), ExecutionProvider::Cpu);
        assert_eq!(provider_of(&[ExecutionProvider::CoreMl, ExecutionProvider::WebGpu]), ExecutionProvider::CoreMl);
    }

    #[test]
    fn an_install_failure_is_never_marked_out_of_memory() {
        assert!(!SessionError::from(InitError::Stopped).is_out_of_memory());
    }
}
