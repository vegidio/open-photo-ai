//! Turning a resolved plan and a model file into a live ONNX session.
//!
//! This is the first point in the project at which any of the provider option keys reaches the ONNX Runtime. Until
//! here every one of them was a string nobody had validated.
//!
//! # Every option is applied through `with_arbitrary_config`
//!
//! `ort` writes `with_arbitrary_config` into the **same** options map every typed setter writes into, and each
//! provider's `register` hands that map to the runtime verbatim — so a typed setter would mean parsing `"EXHAUSTIVE"`
//! or `"CpuAndNeuralEngine"` back out of the string into an `ort` enum and re-emitting it, a round trip that can only
//! lose. One loop over the map, for all three providers, and the option tables stay comparable line for line with the
//! provider's documentation.
//!
//! Every dispatch is built with `error_on_failure()`. `ort` defaults to registering silently — logging and carrying
//! on — which is precisely the failure mode the option tables warn about: ONNX Runtime rejects an options update
//! wholesale, so a single mistyped key costs the entire provider and surfaces only as a run that is ten times
//! slower. With `error_on_failure` it becomes a build failure, which the fallback then turns into a *reported*
//! downgrade.
//!
//! # What a runner with no runtime can check
//!
//! `ort::session::Session::builder()` needs a live environment, so nothing here that touches one can be exercised on
//! a CI runner. What is factored out to be checkable anyway is everything that decides *what* the runtime is asked
//! for: [`dispatch`] builds the three provider dispatches from the resolved maps, [`BuilderSettings`] is the typed
//! translation of the session settings into the three calls that carry them, and [`serialized`] is the TensorRT
//! ordering and the timing cache it drops. The one thing left is the sequence of calls itself, which the live tests
//! in the test-only `super::live` are what cover — by hand, on real hardware.
//!
//! # The build runs inside a span, and that is what makes the runtime's own records readable
//!
//! [`build`] enters a span carrying the artifact and the resolved provider. ONNX Runtime's C++ diagnostics arrive as
//! `tracing` events under the target `ort` with the runtime's own location and logger id and nothing that identifies
//! the model — the C++ side does not know it — and the log's formatter renders an enclosing span's fields onto every
//! record emitted inside it. So a *"Some nodes were not assigned"* now says which model was being built and what was
//! being attached to it. It covers the build thread only; see the comment on [`build`] for what that excludes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ort::ep::{ArbitrarilyConfigurableExecutionProvider, ExecutionProviderDispatch};
use ort::session::Session;
use ort::session::builder::SessionBuilder;

use crate::deps::manifest;
use crate::error::{InitError, SessionError};
use crate::models::ArtifactId;
use crate::providers::Accelerator;
use crate::providers::options::{CachePaths, ProviderOptions, SessionPlan, SessionSettings};
use crate::providers::profile::{DISABLED_OPTIMIZER_SEPARATOR, ExecutionMode};

/// Held across any session build whose attach list contains TensorRT.
///
/// The shared timing cache is one file that ONNX Runtime reads at the start of a build and rewrites at the end with
/// no locking of its own, and the session cache single-flights per artifact rather than globally — so two models can
/// legitimately build at once, and two builds that both attach TensorRT would interleave into it.
///
/// It costs nothing in the common case: the application builds one model at a time, and two concurrent engine builds
/// do not finish sooner for sharing one GPU. The alternative — a timing cache per model — was measured by the
/// reference implementation at up to 60% of a cold engine build.
static TENSORRT_BUILD: Mutex<()> = Mutex::new(());

/// Everything one session build is made from, owned rather than borrowed.
///
/// Owned because the build moves to a blocking thread: opening a graph is anything from a fifth of a second to
/// several minutes of engine compilation, and in a Tauri process the asynchronous runtime is the window's event loop.
#[derive(Debug, Clone)]
pub(crate) struct BuildRequest {
    /// The artifact being opened, for whatever a failure has to name.
    pub(crate) artifact: ArtifactId,
    /// What to attach, with what, and the settings the session itself is built with.
    pub(crate) plan: SessionPlan,
    /// The two directories the providers compile into. Only the timing one is read here, and only to drop it.
    pub(crate) paths: CachePaths,
    /// The graph file on disk.
    pub(crate) model: PathBuf,
}

/// Opens `request`'s model and returns the session.
///
/// # Errors
///
/// Returns [`SessionError::Install`] where the model file is not there or cannot be read — which is a model that is
/// not on disk, fails the same way on every provider, and is therefore never retried — and [`SessionError::Build`]
/// where the file opens and the runtime will not build a session from it, naming the provider being attached.
pub(crate) fn build(request: BuildRequest) -> Result<Session, SessionError> {
    let BuildRequest { artifact, plan, paths, model } = request;

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
    // alone exactly as it does today. That is a narrowing of what the reference reconstructed by hand, and what it
    // buys back is the entire stderr-capture stack slice 1 deleted.
    let span = tracing::info_span!("session_build", artifact = %artifact, provider = %plan.resolved);
    let _entered = span.enter();

    // Before the builder is touched, and the whole of the distinction the specs ask for. Everything after this point
    // is "on disk and would not open", which is the only failure a CPU retry could answer.
    drop(std::fs::File::open(&model).map_err(InitError::io(&model))?);

    serialized(attaches_tensorrt(&plan), &paths.timing, || open(&artifact, &plan, &model))
}

/// Builds the session itself: providers attached in the plan's order, the session settings applied, the graph
/// committed from the file.
///
/// # Errors
///
/// Returns [`SessionError::Build`] for every step, each naming the artifact and the provider the plan resolved to.
fn open(artifact: &ArtifactId, plan: &SessionPlan, model: &Path) -> Result<Session, SessionError> {
    // `Copy`, because every captured value is a shared reference — so one constructor serves all four steps rather
    // than four closures that could each name the failure differently.
    let failed = |source: ort::Error| SessionError::Build {
        artifact: artifact.as_str().to_string(),
        provider: plan.resolved,
        source: Arc::new(source),
    };

    // In the plan's own order, so that one provider declining a node at session-build time leaves the next to run
    // the graph. Empty for a CPU run, which attaches nothing.
    let dispatches: Vec<ExecutionProviderDispatch> = plan.providers.iter().map(dispatch).collect();

    let mut builder = Session::builder()
        .map_err(failed)?
        .with_execution_providers(&dispatches)
        .map_err(|err| failed(err.into()))?;
    builder = BuilderSettings::of(&plan.settings).apply(builder).map_err(failed)?;

    builder.commit_from_file(model).map_err(failed)
}

/// The dispatch `options` describes.
///
/// One arm per [`Accelerator`], and none for the request or the CPU: the attach list cannot hold either — the first is
/// a request rather than a provider, and the second takes no configuration and is what the runtime falls back to on
/// its own once everything above it has declined.
fn dispatch(options: &ProviderOptions) -> ExecutionProviderDispatch {
    let dispatch = match options.provider {
        Accelerator::TensorRt => configure(ort::ep::TensorRT::default(), &options.options).build(),
        Accelerator::Cuda => configure(ort::ep::CUDA::default(), &options.options).build(),
        Accelerator::CoreMl => configure(ort::ep::CoreML::default(), &options.options).build(),
    };

    dispatch.error_on_failure()
}

/// Applies every entry of `options` to `provider`.
///
/// One function for all three providers, which is the whole of what the provider-specific code amounts to: three
/// constructors and this loop.
fn configure<E: ArbitrarilyConfigurableExecutionProvider>(provider: E, options: &BTreeMap<String, String>) -> E {
    options
        .iter()
        .fold(provider, |provider, (key, value)| provider.with_arbitrary_config(key, value))
}

/// Whether TensorRT is among the providers this plan attaches.
fn attaches_tensorrt(plan: &SessionPlan) -> bool {
    plan.providers.iter().any(|options| options.provider == Accelerator::TensorRt)
}

/// Runs `build`, serialized against every other TensorRT build in this process and dropping the shared timing cache
/// if it fails.
///
/// A build with TensorRT absent runs straight through: it neither reads nor writes the shared cache, so there is
/// nothing to order it against and nothing of its to discard.
///
/// Generic in what it builds so the ordering and the discard are exercisable without a runtime — which is the only
/// way either can be checked at all, since a real TensorRT build needs an NVIDIA card.
fn serialized<T, E>(tensorrt: bool, timing: &Path, build: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    if !tensorrt {
        return build();
    }

    // A poisoned lock means a previous build panicked. What this guards is an ordering rather than state, so the
    // guard is taken rather than the panic propagated — the next build still has to be the only one.
    let _serialized = crate::task::lock(&TENSORRT_BUILD);

    build().inspect_err(|_| {
        // A half-written timing cache makes TensorRT's `createTimingCache` return null, which ONNX Runtime turns into
        // a failed session build — so every later model quietly runs somewhere slower, with a symptom that points
        // nowhere near a cache file. Dropping it costs one slow rebuild if the failure was about something else,
        // which is the cheaper way to be wrong.
        //
        // The removal failure is swallowed: this runs on a path that is already returning an error, and a cache file
        // that could not be removed is not worth replacing that error with.
        let _ = manifest::empty_dir(timing);

        // `info`, and it is one of the two records in this slice that exist purely to account for minutes a user is
        // about to spend with nothing on screen explaining them: the next TensorRT build has no measured timings to
        // start from and recompiles from scratch.
        tracing::info!(
            path = %timing.display(),
            "the shared TensorRT timing cache was discarded after a failed build; the next engine build will be slower"
        );
    })
}

/// The session settings as the three calls that carry them.
///
/// A translation step of its own rather than three expressions inlined into [`apply`](Self::apply), because the
/// builder it applies to cannot exist without a live runtime: this is the half that says what the runtime will be
/// asked for, and it is checkable on any runner.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BuilderSettings {
    /// What `with_parallel_execution` is given.
    parallel: bool,
    /// What `with_memory_pattern` is given.
    mem_pattern: bool,
    /// The disabled optimizers as the runtime's config entry takes them — the names joined by
    /// [`DISABLED_OPTIMIZER_SEPARATOR`] — or `None` where the model disables none, in which case the entry is not
    /// written at all.
    disabled_optimizers: Option<String>,
}

impl BuilderSettings {
    /// What `settings` asks the builder for.
    fn of(settings: &SessionSettings) -> Self {
        Self {
            parallel: matches!(settings.execution_mode, ExecutionMode::Parallel),
            mem_pattern: settings.mem_pattern,
            // Joined here rather than by the model, so that a wrongly separated value is not something a
            // declaration is able to write — see `DISABLED_OPTIMIZER_SEPARATOR` for why that matters and for how the
            // separator was established. An empty name is dropped rather than written: the runtime would take `""`
            // as a transformer name, match nothing with it, and say nothing — and a list of nothing but those is no
            // entry at all rather than an empty one, since the entry itself is a question nobody asked.
            disabled_optimizers: Some(
                settings
                    .disabled_optimizers
                    .iter()
                    .filter(|name| !name.is_empty())
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(DISABLED_OPTIMIZER_SEPARATOR),
            )
            .filter(|joined| !joined.is_empty()),
        }
    }

    /// Applies these to `builder`.
    ///
    /// # Errors
    ///
    /// Returns whatever `ort` reports for any of the three, recovered into this crate's error by the caller.
    fn apply(&self, builder: SessionBuilder) -> Result<SessionBuilder, ort::Error> {
        let mut builder = builder.with_parallel_execution(self.parallel)?.with_memory_pattern(self.mem_pattern)?;

        if let Some(disabled) = &self.disabled_optimizers {
            builder = builder.with_disabled_optimizers(disabled)?;
        }

        Ok(builder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging;
    use crate::providers::ExecutionProvider;
    use crate::providers::profile::EpProfile;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// A stand-in for a provider, recording every key and value it is configured with.
    ///
    /// `ort`'s own providers keep their options in a private map with no way to read it back, so what
    /// [`configure`] did to one cannot be asserted through `ort` at all. The trait it is applied through is public,
    /// which is what lets this type be configured by exactly the same code the three real providers are.
    #[derive(Debug, Default)]
    struct Recording {
        applied: RefCell<Vec<(String, String)>>,
    }

    impl ArbitrarilyConfigurableExecutionProvider for Recording {
        fn with_arbitrary_config(self, key: impl ToString, value: impl ToString) -> Self {
            self.applied.borrow_mut().push((key.to_string(), value.to_string()));
            self
        }
    }

    /// The options resolved for `provider` on a machine that supports it, for the Kyoto profile.
    fn resolved(provider: ExecutionProvider) -> ProviderOptions {
        let paths = CachePaths {
            engine: PathBuf::from("/somewhere/opai/engines/up_kyoto_4x_fp16"),
            timing: PathBuf::from("/somewhere/opai/engines/.timing"),
        };
        let supported = crate::providers::tests::machine_supporting(true, true, true);
        let plan = crate::providers::options::resolve(provider, supported, &EpProfile::default(), &paths);

        plan.providers.into_iter().next().expect("a supported provider resolves to one set of options")
    }

    /// A plan attaching exactly `providers`, with default settings.
    fn plan_attaching(providers: Vec<Accelerator>) -> SessionPlan {
        let resolved = providers.first().map_or(ExecutionProvider::Cpu, |&accelerator| accelerator.into());

        SessionPlan {
            requested: resolved,
            resolved,
            providers: providers.into_iter().map(resolved_for).collect(),
            settings: SessionSettings {
                execution_mode: ExecutionMode::Parallel,
                mem_pattern: true,
                disabled_optimizers: Vec::new(),
            },
        }
    }

    /// The options for one accelerator, without going through the whole resolution.
    fn resolved_for(provider: Accelerator) -> ProviderOptions {
        ProviderOptions { provider, options: BTreeMap::new() }
    }

    #[test]
    fn every_resolved_option_is_applied_to_the_provider() {
        // The one thing a mistyped or dropped key costs is the whole provider — ONNX Runtime rejects an options
        // update wholesale — so what has to be checked is that the map arrives entire, not that it arrives at all.
        for provider in [ExecutionProvider::TensorRt, ExecutionProvider::Cuda, ExecutionProvider::CoreMl] {
            let options = resolved(provider);
            assert!(!options.options.is_empty(), "{provider} resolved to no options at all");

            let recording = configure(Recording::default(), &options.options);

            let applied: BTreeMap<String, String> = recording.applied.into_inner().into_iter().collect();
            assert_eq!(applied, options.options, "{provider} was not configured with its resolved map");
        }
    }

    #[test]
    fn each_of_the_three_providers_produces_a_dispatch_that_fails_rather_than_declining_silently() {
        // `ort` registers silently by default, which is the failure mode this whole slice has to avoid: a single bad
        // key costs the provider and shows up only as a run that is ten times slower.
        let cases = [
            (ExecutionProvider::TensorRt, "TensorrtExecutionProvider"),
            (ExecutionProvider::Cuda, "CUDAExecutionProvider"),
            (ExecutionProvider::CoreMl, "CoreMLExecutionProvider"),
        ];

        for (provider, name) in cases {
            let dispatch = dispatch(&resolved(provider));

            // `ort` exposes neither the provider's name nor the flag as accessors; its `Debug` prints both, and they
            // are the two facts that decide which provider is attached and what happens when it will not.
            let rendered = format!("{dispatch:?}");
            assert!(rendered.starts_with(name), "{provider} produced {rendered}");
            assert!(rendered.contains("error_on_failure: true"), "{provider} would decline silently: {rendered}");
        }
    }

    #[test]
    fn neither_auto_nor_the_cpu_is_anything_the_builder_can_be_handed() {
        // The resolution narrows a request to the accelerators it attaches, and the builder takes nothing wider: the
        // two that are not one have no `Accelerator` to be, so there is no dispatch to refuse them with.
        for provider in [ExecutionProvider::Auto, ExecutionProvider::Cpu] {
            assert_eq!(provider.accelerator(), None, "{provider} named an accelerator");
        }
    }

    #[test]
    fn the_session_settings_become_the_three_calls_that_carry_them() {
        let settings = |mode, mem_pattern, disabled: &[&str]| SessionSettings {
            execution_mode: mode,
            mem_pattern,
            disabled_optimizers: disabled.iter().map(|name| (*name).to_string()).collect(),
        };
        let disabled =
            |names: &[&str]| BuilderSettings::of(&settings(ExecutionMode::Parallel, true, names)).disabled_optimizers;

        // What every model that has measured nothing runs with, which is what shipped before profiles existed.
        assert_eq!(
            BuilderSettings::of(&settings(ExecutionMode::Parallel, true, &[])),
            BuilderSettings { parallel: true, mem_pattern: true, disabled_optimizers: None }
        );

        // A model that measured the inter-op pool as a loss, and one whose shapes vary.
        assert_eq!(
            BuilderSettings::of(&settings(ExecutionMode::Sequential, false, &[])),
            BuilderSettings { parallel: false, mem_pattern: false, disabled_optimizers: None }
        );

        // One name reaches the config entry verbatim: nothing is separated, so nothing can be separated wrongly.
        assert_eq!(disabled(&["ConstantFolding"]).as_deref(), Some("ConstantFolding"));

        // And two are joined by the separator the runtime parses — the assertion this whole change exists for, and
        // the one a comma would fail. Spelled out rather than composed from `DISABLED_OPTIMIZER_SEPARATOR`, so that
        // changing that constant fails here instead of agreeing with itself.
        assert_eq!(
            disabled(&["ReshapeFusion", "SimplifiedLayerNormFusion"]).as_deref(),
            Some("ReshapeFusion;SimplifiedLayerNormFusion")
        );

        // No space around it, which is not cosmetic: the runtime does not trim an entry, so a joined `"; "` would
        // make the second name match no transformer and disable nothing — silently, and looking correct.
        assert!(!disabled(&["ReshapeFusion", "SimplifiedLayerNormFusion"]).unwrap_or_default().contains(' '));

        // An empty name is dropped rather than written, since the runtime would take `""` as a transformer name and
        // match nothing with it.
        assert_eq!(disabled(&["", "ConstantFolding"]).as_deref(), Some("ConstantFolding"));

        // And a list with nothing left in it is no entry rather than an empty one, the same as naming none.
        assert_eq!(disabled(&[""]), None);
    }

    #[test]
    fn a_plan_attaching_tensorrt_is_recognised_wherever_tensorrt_sits_in_it() {
        // The `Auto` chain attaches TensorRT first and CUDA behind it; an explicit CUDA request attaches neither
        // ahead of nor behind TensorRT. Reading only the resolved provider would miss a chain that attaches it
        // second, which no chain does today and which is exactly the kind of thing a later provider changes.
        assert!(attaches_tensorrt(&plan_attaching(vec![Accelerator::TensorRt, Accelerator::Cuda])));
        assert!(attaches_tensorrt(&plan_attaching(vec![Accelerator::Cuda, Accelerator::TensorRt])));
        assert!(!attaches_tensorrt(&plan_attaching(vec![Accelerator::Cuda])));
        assert!(!attaches_tensorrt(&plan_attaching(vec![Accelerator::CoreMl])));
        assert!(!attaches_tensorrt(&plan_attaching(Vec::new())));
    }

    #[test]
    fn two_builds_that_attach_tensorrt_run_one_after_the_other() {
        // Not a throughput loss worth avoiding: the application builds one model at a time, and two concurrent
        // engine builds do not finish sooner for sharing one GPU. What it buys is that the one timing-cache file the
        // runtime rewrites at the end of every build is never being rewritten twice at once.
        let timing = tempfile::tempdir().unwrap();
        let inside = Arc::new(AtomicUsize::new(0));
        let overlapped = Arc::new(AtomicUsize::new(0));

        std::thread::scope(|scope| {
            for _ in 0..2 {
                let inside = Arc::clone(&inside);
                let overlapped = Arc::clone(&overlapped);
                let path = timing.path().to_path_buf();

                scope.spawn(move || {
                    let outcome: Result<(), ()> = serialized(true, &path, || {
                        if inside.fetch_add(1, Ordering::SeqCst) != 0 {
                            overlapped.fetch_add(1, Ordering::SeqCst);
                        }
                        // Long enough that a second build entering concurrently would be seen, and short enough to
                        // cost the suite nothing.
                        std::thread::sleep(Duration::from_millis(50));
                        inside.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    });
                    outcome.unwrap();
                });
            }
        });

        assert_eq!(overlapped.load(Ordering::SeqCst), 0, "two TensorRT builds ran at the same time");
    }

    #[test]
    fn a_failed_build_with_tensorrt_attached_drops_the_shared_timing_cache_and_still_reports_its_error() {
        // A half-written timing cache fails every later TensorRT build with a symptom — every model running
        // somewhere slower — that points nowhere near a cache file.
        let timing = tempfile::tempdir().unwrap();
        let cache = timing.path().join("timings.bin");
        std::fs::write(&cache, b"half of a timing cache").unwrap();

        let outcome: Result<(), &str> = serialized(true, timing.path(), || Err("the engine build failed"));

        assert_eq!(outcome, Err("the engine build failed"), "the discard replaced the failure it was reporting");
        assert!(!cache.exists(), "a failed TensorRT build left the shared timing cache in place");
        assert!(timing.path().is_dir(), "the timing directory itself was removed rather than emptied");
    }

    #[test]
    fn a_failed_build_with_tensorrt_absent_leaves_the_timing_cache_alone() {
        // A build that neither read nor wrote the shared cache has nothing of its own in it, and the measurements
        // every other model seeded are worth minutes.
        let timing = tempfile::tempdir().unwrap();
        let cache = timing.path().join("timings.bin");
        std::fs::write(&cache, b"measured across every model").unwrap();

        let outcome: Result<(), &str> = serialized(false, timing.path(), || Err("the model would not open"));

        assert_eq!(outcome, Err("the model would not open"));
        assert!(cache.is_file(), "a failure that never touched the timing cache discarded it");
    }

    #[test]
    fn the_discarded_timing_cache_is_recorded_beside_the_files_it_removed() {
        // The filesystem effect is what the test above checks. This is the half a user needs: the next TensorRT build
        // recompiles from nothing, which is minutes with no visible cause.
        let timing = tempfile::tempdir().unwrap();
        let cache = timing.path().join("timings.bin");
        std::fs::write(&cache, b"half of a timing cache").unwrap();

        let (log, outcome) = logging::records_of_blocking("info", || {
            let outcome: Result<(), &str> = serialized(true, timing.path(), || Err("the engine build failed"));
            outcome
        });

        assert_eq!(outcome, Err("the engine build failed"));
        assert!(!cache.exists());

        let discarded = log
            .lines()
            .find(|line| line.contains("the shared TensorRT timing cache was discarded after a failed build"))
            .unwrap_or_else(|| panic!("the discard was not recorded:\n{log}"));
        assert!(discarded.contains("level=INFO"), "{discarded}");
        assert!(discarded.contains("path="), "the record does not say which cache: {discarded}");

        // A build that never touched the cache discards nothing and says nothing.
        let (quiet, ()) = logging::records_of_blocking("info", || {
            let _: Result<(), &str> = serialized(false, timing.path(), || Err("the model would not open"));
        });
        assert!(!quiet.contains("timing cache was discarded"), "{quiet}");
    }

    #[test]
    fn the_runtime_s_own_diagnostics_are_told_which_model_they_are_about() {
        // What D5 buys, checked at the seam it actually works through: the formatter renders an enclosing span's
        // fields onto every record emitted inside it, including ones this crate did not emit. An `ort`-target event
        // stands in for a C++ diagnostic, which cannot be provoked without a live runtime and a real model.
        //
        // What this does *not* cover, and cannot: a record the runtime emits later from one of its own worker
        // threads. A span's fields reach an event only on the thread that entered the span, and the build thread is
        // the one ORT constructs sessions and partitions graphs on. Inference-time diagnostics arrive with the
        // runtime's fields alone, as they do today.
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
}
