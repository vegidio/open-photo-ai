//! What a run attaches, and what each attached provider is configured with.
//!
//! Nothing here calls the ONNX Runtime.

// The resolution reads four booleans and the configuration writes maps of strings, so every decision in this file is
// made before a runtime is loaded — which is what lets all of it be tested on a CI runner with no GPU, no Mac and no
// runtime installed.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::profile::{EpProfile, ExecutionMode};
use super::{ExecutionProvider, SupportedProviders};

// A property of the providers rather than of the operating system, which is why there is no per-platform table
// anywhere in this crate: CoreML is already unsupported off macOS and the two NVIDIA providers are unsupported until
// their libraries are on disk, so filtering this one order by the machine's report reproduces every platform's chain —
// and additionally stops offering a provider whose library failed to install, which a platform table cannot know about.
//
// The CPU is deliberately absent, as it is in the reference implementation's chains. It takes no configuration, and it
// is what the runtime falls back to on its own once every provider above it has declined a node.
/// The order [`ExecutionProvider::Auto`] prefers providers in, best first.
const AUTO_ORDER: [ExecutionProvider; 3] =
    [ExecutionProvider::TensorRt, ExecutionProvider::Cuda, ExecutionProvider::CoreMl];

/// What a request resolved to on this machine: which providers to attach, in order, and what was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChainResolution {
    // Named for the chain rather than plain `Resolution`, because `crate::Resolution` is already the public answer to a
    // different question — what one upscale operation resolves to — and two unrelated types under one name is an alias
    // or a qualified path at every call site that reaches for both.
    //
    // The requested provider is carried beside the resolved one because they can differ, and when they do the
    // difference is the whole explanation for a run that is ten times slower than the user expected. Carrying it as
    // data rather than as a log line is what lets the session builder state it once, and a front end show it, without
    // either re-deriving anything.
    pub(crate) requested: ExecutionProvider,
    /// What this machine could actually serve. [`ExecutionProvider::Cpu`] where nothing is attached — either because
    /// the CPU was asked for, or because the request could not be honoured.
    ///
    /// Not a promise that the provider builds: one that attaches can still decline the graph at session-build time,
    /// and the runtime falls through to the next in [`attach`](Self::attach) and ultimately to the CPU when it does.
    pub(crate) resolved: ExecutionProvider,
    /// The providers to attach, in attach order. Empty means a CPU run, and never contains
    /// [`ExecutionProvider::Cpu`] or [`ExecutionProvider::Auto`], neither of which is something a session attaches.
    pub(crate) attach: Vec<ExecutionProvider>,
}

/// What `requested` resolves to on a machine reporting `supported`.
///
/// Three answers, and none of them is an error:
///
/// - [`Auto`](ExecutionProvider::Auto) attaches every supported provider in [`AUTO_ORDER`].
/// - A supported provider attaches that provider **alone**. A deliberate choice is not quietly widened into the
///   `Auto` chain — asking for CUDA on a machine that also has TensorRT means CUDA.
/// - Anything else is a CPU run carrying what was asked for, rather than a failure.
pub(crate) fn resolve_chain(requested: ExecutionProvider, supported: SupportedProviders) -> ChainResolution {
    let attach: Vec<ExecutionProvider> = match requested {
        // Every one rather than only the best, so that TensorRT declining the graph at session-build time leaves CUDA
        // to run it instead of dropping the run to the CPU.
        ExecutionProvider::Auto => AUTO_ORDER.into_iter().filter(|&provider| supported.supports(provider)).collect(),
        // The CPU attaches nothing and is configured with nothing. It is not absent from the run — it is what runs
        // the graph — but there is no provider to append for it and no options to produce.
        ExecutionProvider::Cpu => Vec::new(),
        named if supported.supports(named) => vec![named],
        // The downgrade. Nothing is attached, and `resolved` below reports the CPU beside the request. A settings file
        // written on another machine, or before a driver was removed, costs speed rather than the run.
        //
        // This unifies two outcomes the reference implementation keeps apart, and neither of its answers is right: a
        // provider absent from the platform's chain (CoreML on Linux) fails the run outright, while one present but
        // uninstalled is attached anyway, declines natively, and is logged at Warn, which a user sees only as the app
        // getting slow. Both are the same fact, so both get the same answer here: this machine cannot serve it, the
        // run is a CPU run, and the resolution says so.
        _ => Vec::new(),
    };

    // The resolved provider is the first one that will be tried, or the CPU when there is none — so the two fields
    // cannot disagree with the list they describe.
    let resolved = attach.first().copied().unwrap_or(ExecutionProvider::Cpu);

    ChainResolution { requested, resolved, attach }
}

/// The directories the execution providers are pointed at.
///
/// Both are **inputs**: this module writes them into an option map and decides nothing about where they are, which
/// directory owns which, or when either is emptied — `sessions::caches` does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachePaths {
    // A struct rather than two path parameters because only one of the two is per-model, and a pair of bare paths at
    // every call site is how they end up swapped. Handing TensorRT the per-model directory as its timing cache silently
    // gives back the per-model behaviour the shared one exists to avoid, and nothing reports the loss — the builds are
    // simply slow.

    // Per-model because replacing a model's weights has to invalidate it.
    /// This model's own directory, holding what a provider compiled from it: a TensorRT engine, a CoreML MLProgram.
    pub(crate) engine: PathBuf,

    // Shared because kernel timings carry between graphs: a cold build against a cache other models seeded measures up
    // to 60% faster than against an empty one.
    /// The installation-wide TensorRT timing cache directory, shared by every model.
    pub(crate) timing: PathBuf,
}

/// One provider to attach, and the options to attach it with, in ONNX Runtime's own key spelling —
/// `trt_engine_cache_path`, `MLComputeUnits`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderOptions {
    // The runtime's own spelling is what makes a row here comparable line for line with the provider's documentation,
    // with the reference implementation's measured table, and with a runtime log line.
    /// Which provider these configure. Never [`ExecutionProvider::Auto`] or [`ExecutionProvider::Cpu`].
    pub(crate) provider: ExecutionProvider,
    // A `BTreeMap` rather than a `HashMap` so a test can compare a whole map by equality and a failure prints in a
    // stable order.
    /// The options, by ONNX Runtime's own key.
    pub(crate) options: BTreeMap<String, String>,
}

/// Builds a map from pairs of string slices, which is what every option map below is.
fn options_from<const N: usize>(entries: [(&str, &str); N]) -> BTreeMap<String, String> {
    entries.into_iter().map(|(key, value)| (key.to_string(), value.to_string())).collect()
}

// The reference implementation's default, and the only value any model there runs with.
/// TensorRT's workspace ceiling: 4 GiB.
const TRT_WORKSPACE_BYTES: u64 = 4 << 30;

// Measured and rejected rather than unexamined, and both providers fail it in opposite ways.
//
// On TensorRT the capture run is correct and every run after it silently returns an all-zero output. Re-measured on an
// RTX 5090 (driver 610.88, ONNX Runtime 1.26, both precisions): the first `Run` matches the graph-off result exactly,
// then 20 of 20 subsequent runs on that session return zeros, with no error and with plausible timings — the replay
// writes nothing, which is also where the "9% faster" that makes this tempting comes from. In the application that is
// no faces found from the second detection onwards, or a blank recovered face.
//
// On the CUDA provider it fails loudly instead: the capture run dies in `cudaStreamEndCapture`, because cuBLAS
// initializes lazily inside the captured stream. The runtime also requires every input and output to be bound to
// device memory for capture, which this project's host tensors are not.
//
// Anyone revisiting it must compare the **second** run on the **same** session against a CPU result. A single-run
// comparison cannot see either failure: it reports a speed-up and ships a broken model.
/// The value that keeps CUDA graph capture off on **both** NVIDIA providers: `trt_cuda_graph_enable` on TensorRT and
/// `enable_cuda_graph` on CUDA.
const GRAPH_CAPTURE_DISABLED: &str = "0";

// Named rather than spelled at each site because `tensorrt_options` merges a profile's overrides over the pinned map
// **by key**: a key that does not match does not fail, it silently leaves the pinned value in place and adds an
// unrecognised one alongside. ONNX Runtime rejects an options update wholesale (see `sessions::build::dispatch`), so
// that is the most expensive typo available in this area, and the two sides share one spelling.
/// The key of TensorRT's builder optimization level, which a model's profile may override through
/// [`EpProfile::trt_options`].
pub(crate) const TRT_BUILDER_OPTIMIZATION_LEVEL: &str = "trt_builder_optimization_level";

/// The TensorRT provider options for a model with this profile and these cache directories: the pinned defaults, with
/// the profile's raw overlay applied **last** so it can override any of them.
fn tensorrt_options(paths: &CachePaths, profile: &EpProfile) -> BTreeMap<String, String> {
    // Each default is measured rather than inherited.
    let mut options = options_from([
        ("device_id", "0"),
        ("trt_max_workspace_size", &TRT_WORKSPACE_BYTES.to_string()),
        // INT8 off and FP16 not forced. A graph already exported at FP16 gains nothing from the flag, and forcing it on
        // a model that was not validated for it silently changes the output — which is why the precision is chosen by
        // selecting the model rather than by a provider option.
        ("trt_fp16_enable", "0"),
        ("trt_int8_enable", "0"),
        // The single largest TensorRT setting here. A hardware-compatible engine runs on any Ampere-or-newer card, so
        // TensorRT may only pick kernels that exist on all of them — the newer the card, the more it gives up. Turning
        // it off is worth 4.7% to 48% across the catalogue on an RTX 5090, and it improves cold start too, since the
        // compatible engine is the slower one to build as well as to run. Nothing is lost: the engine cache is built on
        // the user's own machine and never shipped, and the cache file names carry the architecture they were built
        // for, so a machine that changes GPU asks for a name that is not there and rebuilds rather than loading
        // something wrong.
        ("trt_engine_hw_compatible", "0"),
        ("trt_cuda_graph_enable", GRAPH_CAPTURE_DISABLED),
        // 5 rather than TensorRT's own 3. A model that measures a loss sets it back through `EpProfile::trt_options` —
        // Osaka does, because on a 12,940-node graph the level costs 87 seconds of engine build for no runtime
        // difference.
        (TRT_BUILDER_OPTIMIZATION_LEVEL, "5"),
        ("trt_engine_cache_enable", "1"),
        ("trt_engine_cache_path", &paths.engine.display().to_string()),
        ("trt_timing_cache_enable", "1"),
        ("trt_timing_cache_path", &paths.timing.display().to_string()),
    ]);

    options.extend(profile.trt_options.iter().map(|(key, value)| (key.clone(), value.clone())));

    options
}

/// The CUDA provider options for a model with this profile: the pinned defaults, in the runtime's own NCHW layout
/// unless the profile asks for NHWC.
///
/// There is no raw overlay here, unlike TensorRT.
fn cuda_options(profile: &EpProfile) -> BTreeMap<String, String> {
    // No overlay because the reference implementation carries one with no setter: the sweep it was added alongside
    // found every CUDA option already at its best value in these defaults.
    options_from([
        // Exhaustive search and the maximum workspace, because these graphs are run many times over one resident
        // session, so the one-off search cost is amortised over every tile.
        ("cudnn_conv_algo_search", "EXHAUSTIVE"),
        ("cudnn_conv_use_max_workspace", "1"),
        ("device_id", "0"),
        ("do_copy_in_default_stream", "1"),
        ("enable_cuda_graph", GRAPH_CAPTURE_DISABLED),
        ("gpu_mem_limit", "0"),
        ("prefer_nhwc", if profile.cuda_prefer_nhwc { "1" } else { "0" }),
    ])
}

/// The CoreML provider options for a model with this profile and these cache directories. Compute units and the
/// specialization strategy are the profile's; the rest is pinned.
fn coreml_options(paths: &CachePaths, profile: &EpProfile) -> BTreeMap<String, String> {
    // `ModelFormat` and `RequireStaticInputShapes` are **pinned rather than offered**, and both are correctness rather
    // than tuning.
    options_from([
        ("EnableOnSubgraphs", "0"),
        ("MLComputeUnits", profile.coreml_compute_units.as_str()),
        // The per-model engine directory: what CoreML compiles is as per-model as a TensorRT engine.
        ("ModelCacheDirectory", &paths.engine.display().to_string()),
        // The older `NeuralNetwork` format has no typed execution, so CoreML may put an FP32 graph on the Neural Engine
        // and run it in half precision — and it does: Saitama's FP32 graph measures -38.9% under it, landing to four
        // significant figures on the time *and* the accuracy of that model's FP16 export. Anyone sweeping provider
        // options would read that as the largest win available and ship a precision downgrade the user never asked
        // for. The honest way to take it is to select the FP16 model.
        ("ModelFormat", "MLProgram"),
        // Correct for these fixed-shape tile models, and the setting that would turn it off is not carried here at all
        // — not for lack of a field, but because turning it off can manufacture the failure it appears to document: on
        // a graph that is in fact fixed-shape, CoreML then tries to convert nodes it would otherwise have declined,
        // which for Osaka's VAE failed session creation outright with *"axis 4 is not in valid range [-4,3]"* — an
        // error that reads as a CoreML bug.
        ("RequireStaticInputShapes", "1"),
        ("SpecializationStrategy", profile.coreml_specialization.as_str()),
    ])
}

/// The session-level settings a model runs with, as opposed to the per-provider ones.
///
/// These apply on **every** provider the model runs on, which is what makes them session settings rather than
/// provider ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionSettings {
    // Typed rather than key/value, because ONNX Runtime takes these through dedicated calls — `SetSessionExecutionMode`,
    // `SetMemPattern` — rather than through the config-entry map the provider options are written in. The one
    // exception is `disabled_optimizers`, which *is* a config entry; the plan carries the names and the session builder
    // joins and writes them.
    //
    // Whether applying the execution mode on every provider is safe was settled by measurement across every provider
    // this project ships — see `EpProfile::execution_mode`.
    /// Whether independent branches of the graph may run on separate threads.
    pub(crate) execution_mode: ExecutionMode,
    /// Whether the static memory planner is used. `true` unless a model measured otherwise.
    pub(crate) mem_pattern: bool,
    /// The graph transformers to switch off, by the runtime's own names for them. Empty for every model that has
    /// measured none; see [`EpProfile::disabled_optimizers`].
    pub(crate) disabled_optimizers: Vec<String>,
}

impl SessionSettings {
    /// The settings `profile` declares.
    ///
    /// A model that declares nothing runs parallel, with the memory planner on and no optimizer disabled.
    fn from_profile(profile: &EpProfile) -> Self {
        // Those defaults are what every model in the reference implementation runs with before it is measured. The
        // graph optimization *level* is deliberately not here; see `EpProfile::disabled_optimizers`.
        Self {
            execution_mode: profile.execution_mode,
            mem_pattern: !profile.disable_mem_pattern,
            disabled_optimizers: profile.disabled_optimizers.clone(),
        }
    }
}

/// Everything a session is built from, decided before any runtime is loaded.
///
/// Owned data with no handle in it — nothing here has to be destroyed, which is what lets a plan be computed, compared
/// and logged without holding a native resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionPlan {
    pub(crate) requested: ExecutionProvider,
    /// What this machine could serve. [`ExecutionProvider::Cpu`] where the request could not be honoured; see
    /// [`ChainResolution`].
    pub(crate) resolved: ExecutionProvider,
    /// The providers to attach, in attach order, each with the options it was measured with. Empty means a CPU run.
    pub(crate) providers: Vec<ProviderOptions>,
    /// The settings the session itself is built with, whichever provider was resolved.
    pub(crate) settings: SessionSettings,
}

/// The plan for running a model with `profile` on a machine reporting `supported`, having been asked for `requested`.
///
/// The whole of the decision, and it touches nothing: no runtime is loaded, no model file is opened, and neither
/// cache directory is created or read. `paths` is written into the option maps as text and otherwise left alone.
pub(crate) fn resolve(
    requested: ExecutionProvider,
    supported: SupportedProviders,
    profile: &EpProfile,
    paths: &CachePaths,
) -> SessionPlan {
    // Infallible, deliberately. Every outcome other than the one asked for is a downgrade rather than an error; the one
    // thing that can fail is parsing a provider *name*, which happens at the boundary long before this.
    let ChainResolution { requested, resolved, attach } = resolve_chain(requested, supported);

    let providers = attach
        .into_iter()
        .map(|provider| {
            let options = match provider {
                ExecutionProvider::TensorRt => tensorrt_options(paths, profile),
                ExecutionProvider::Cuda => cuda_options(profile),
                ExecutionProvider::CoreMl => coreml_options(paths, profile),
                // Unreachable by construction: `resolve_chain` never puts either in the attach list, because neither
                // is something a session attaches. Answered with an empty map rather than a panic, so a sixth
                // provider added to the chain and left out of this match costs its options rather than the process.
                ExecutionProvider::Auto | ExecutionProvider::Cpu => BTreeMap::new(),
            };

            ProviderOptions { provider, options }
        })
        .collect();

    SessionPlan { requested, resolved, providers, settings: SessionSettings::from_profile(profile) }
}

#[cfg(test)]
mod tests {
    use crate::models::{FloatPrecision, OsakaPrecision, Scale, Upscale, UpscaleVariant};

    use super::super::profile::CoreMlSpecialization;
    use super::super::tests::{machine_supporting, no_accelerator};
    use super::*;

    /// A machine with both NVIDIA providers installed.
    fn nvidia() -> SupportedProviders {
        machine_supporting(false, true, true)
    }

    /// A Mac.
    fn mac() -> SupportedProviders {
        machine_supporting(true, false, false)
    }

    #[test]
    fn auto_on_a_machine_with_the_nvidia_libraries_attaches_tensorrt_before_cuda() {
        let resolution = resolve_chain(ExecutionProvider::Auto, nvidia());

        assert_eq!(resolution.attach, vec![ExecutionProvider::TensorRt, ExecutionProvider::Cuda]);
        // Both are attached rather than only the best one, so that TensorRT declining the graph at session-build
        // time leaves CUDA to run it instead of dropping the run to the CPU.
        assert_eq!(resolution.resolved, ExecutionProvider::TensorRt);
    }

    #[test]
    fn auto_on_a_mac_attaches_coreml_alone() {
        let resolution = resolve_chain(ExecutionProvider::Auto, mac());

        assert_eq!(resolution.attach, vec![ExecutionProvider::CoreMl]);
        assert_eq!(resolution.resolved, ExecutionProvider::CoreMl);
    }

    #[test]
    fn auto_on_a_machine_with_no_accelerator_attaches_nothing_and_runs_on_the_cpu() {
        let resolution = resolve_chain(ExecutionProvider::Auto, no_accelerator());

        assert!(resolution.attach.is_empty(), "a machine with no accelerator attached {:?}", resolution.attach);
        assert_eq!(resolution.resolved, ExecutionProvider::Cpu);
        assert_eq!(resolution.requested, ExecutionProvider::Auto);
    }

    #[test]
    fn auto_never_offers_the_cpu_as_something_to_attach() {
        // The CPU takes no configuration and is what the runtime falls back to on its own; putting it in the chain
        // would mean producing options for a provider that has none. `Auto` is likewise not something to attach.
        for machine in [nvidia(), mac(), machine_supporting(true, true, true), no_accelerator()] {
            let resolution = resolve_chain(ExecutionProvider::Auto, machine);

            assert!(!resolution.attach.contains(&ExecutionProvider::Cpu), "{machine:?}");
            assert!(!resolution.attach.contains(&ExecutionProvider::Auto), "{machine:?}");
        }
    }

    #[test]
    fn auto_follows_one_preference_order_rather_than_the_platform() {
        // What makes the per-platform table unnecessary: the order is fixed, and every machine's chain is that order
        // with the unsupported providers removed. A machine claiming everything shows the order whole.
        let resolution = resolve_chain(ExecutionProvider::Auto, machine_supporting(true, true, true));

        assert_eq!(
            resolution.attach,
            vec![ExecutionProvider::TensorRt, ExecutionProvider::Cuda, ExecutionProvider::CoreMl]
        );
    }

    #[test]
    fn a_supported_provider_is_attached_alone_rather_than_widened_into_the_auto_chain() {
        // The point of asking for one: a deliberate choice on a machine that supports more than one must not quietly
        // become the `Auto` chain, or the choice would have no effect on the machines where it matters most.
        let resolution = resolve_chain(ExecutionProvider::Cuda, nvidia());

        assert_eq!(resolution.attach, vec![ExecutionProvider::Cuda]);
        assert!(
            !resolution.attach.contains(&ExecutionProvider::TensorRt),
            "an explicit CUDA request attached TensorRT"
        );
        assert_eq!(resolution.resolved, ExecutionProvider::Cuda);
        assert_eq!(resolution.requested, ExecutionProvider::Cuda);
    }

    #[test]
    fn every_provider_a_machine_supports_is_attached_alone_when_it_is_the_one_asked_for() {
        for (provider, machine) in [
            (ExecutionProvider::TensorRt, nvidia()),
            (ExecutionProvider::Cuda, nvidia()),
            (ExecutionProvider::CoreMl, mac()),
        ] {
            let resolution = resolve_chain(provider, machine);

            assert_eq!(resolution.attach, vec![provider]);
            assert_eq!(resolution.resolved, provider);
        }
    }

    #[test]
    fn a_cpu_request_attaches_no_provider_at_all() {
        // Asked for on the machine best able to do otherwise, which is where a CPU request could most easily be
        // widened into something else.
        let resolution = resolve_chain(ExecutionProvider::Cpu, machine_supporting(true, true, true));

        assert!(resolution.attach.is_empty(), "a CPU request attached {:?}", resolution.attach);
        assert_eq!(resolution.resolved, ExecutionProvider::Cpu);
        assert_eq!(resolution.requested, ExecutionProvider::Cpu);
    }

    #[test]
    fn coreml_requested_off_a_mac_becomes_a_cpu_run_that_reports_what_was_asked_for() {
        // The case the reference implementation fails the run over: CoreML is absent from Linux's and Windows's
        // chains there, so a settings file carrying it stops the application rather than slowing it down.
        let resolution = resolve_chain(ExecutionProvider::CoreMl, nvidia());

        assert!(resolution.attach.is_empty());
        assert_eq!(resolution.resolved, ExecutionProvider::Cpu);
        assert_eq!(resolution.requested, ExecutionProvider::CoreMl, "the downgrade lost what was asked for");
    }

    #[test]
    fn a_provider_whose_libraries_are_absent_becomes_a_cpu_run_rather_than_a_native_decline() {
        // The other half of the same fact, which the reference implementation answers the opposite way: TensorRT is
        // in the platform's chain, so it is attached, declines inside the runtime, and leaves one Warn line behind.
        // Here it never reaches the runtime, and the downgrade is a value rather than a log.
        let resolution = resolve_chain(ExecutionProvider::TensorRt, machine_supporting(false, true, false));

        assert!(resolution.attach.is_empty(), "TensorRT was attached on a machine that never installed it");
        assert_eq!(resolution.resolved, ExecutionProvider::Cpu);
        assert_eq!(resolution.requested, ExecutionProvider::TensorRt);
    }

    #[test]
    fn a_request_a_machine_cannot_honour_never_fails_and_never_attaches_anything() {
        // Across every provider and the barest machine there is: a stale settings file costs speed, never the run.
        for provider in ExecutionProvider::ALL {
            let resolution = resolve_chain(provider, no_accelerator());

            assert!(resolution.attach.is_empty(), "{provider} attached something on a machine with no accelerator");
            assert_eq!(resolution.resolved, ExecutionProvider::Cpu);
            assert_eq!(resolution.requested, provider);
        }
    }

    #[test]
    fn what_was_resolved_is_always_the_first_provider_that_will_be_tried() {
        // The invariant the two fields are built on, checked across every request on every shape of machine: the
        // pair cannot disagree with the list it describes, which is what makes reporting the downgrade from it safe.
        let machines = [
            no_accelerator(),
            mac(),
            nvidia(),
            machine_supporting(true, true, true),
            machine_supporting(false, true, false),
        ];

        for machine in machines {
            for provider in ExecutionProvider::ALL {
                let resolution = resolve_chain(provider, machine);

                assert_eq!(
                    resolution.resolved,
                    resolution.attach.first().copied().unwrap_or(ExecutionProvider::Cpu),
                    "{provider} on {machine:?}"
                );
            }
        }
    }

    /// The two cache directories, as the slice that owns them would supply them: one per-model, one shared.
    fn paths() -> CachePaths {
        CachePaths { engine: PathBuf::from("/config/models/up_kyoto_fp16"), timing: PathBuf::from("/config/timing") }
    }

    /// What one option map says for `key`, or a panic naming the key when it carries none.
    fn option<'a>(options: &'a BTreeMap<String, String>, key: &str) -> &'a str {
        options.get(key).unwrap_or_else(|| panic!("the option map carries no {key:?}")).as_str()
    }

    #[test]
    fn tensorrt_carries_every_pinned_default_it_was_measured_with() {
        let options = tensorrt_options(&paths(), &EpProfile::default());

        assert_eq!(option(&options, "trt_builder_optimization_level"), "5");
        assert_eq!(option(&options, "trt_engine_cache_enable"), "1");
        assert_eq!(option(&options, "trt_timing_cache_enable"), "1");
        assert_eq!(option(&options, "trt_engine_hw_compatible"), "0");
        assert_eq!(option(&options, "trt_int8_enable"), "0");
        assert_eq!(option(&options, "trt_fp16_enable"), "0");
        assert_eq!(option(&options, "trt_max_workspace_size"), (4u64 << 30).to_string());
        assert_eq!(option(&options, "device_id"), "0");
    }

    #[test]
    fn graph_capture_is_disabled_on_both_nvidia_providers() {
        // Measured and rejected rather than unexamined: on TensorRT every run after the capture returns zeros with no
        // error, and on CUDA the capture run itself dies. See `GRAPH_CAPTURE_DISABLED`.
        assert_eq!(option(&tensorrt_options(&paths(), &EpProfile::default()), "trt_cuda_graph_enable"), "0");
        assert_eq!(option(&cuda_options(&EpProfile::default()), "enable_cuda_graph"), "0");

        // And it is not reachable through the one overlay a profile has, unless a model asks for it by that key
        // deliberately — there is no typed field that could turn it on by accident.
        let profile = EpProfile { cuda_prefer_nhwc: true, ..EpProfile::default() };
        assert_eq!(option(&cuda_options(&profile), "enable_cuda_graph"), "0");
    }

    #[test]
    fn two_models_carry_their_own_engine_path_and_one_shared_timing_path() {
        // The distinction `CachePaths` exists to keep: the compiled engine is per-model because replacing a model's
        // weights has to invalidate it, while the timing cache is shared because kernel timings carry between graphs.
        let shared_timing = PathBuf::from("/config/timing");
        let kyoto = CachePaths { engine: PathBuf::from("/config/models/kyoto"), timing: shared_timing.clone() };
        let tokyo = CachePaths { engine: PathBuf::from("/config/models/tokyo"), timing: shared_timing.clone() };

        let kyoto_options = tensorrt_options(&kyoto, &EpProfile::default());
        let tokyo_options = tensorrt_options(&tokyo, &EpProfile::default());

        assert_eq!(option(&kyoto_options, "trt_engine_cache_path"), kyoto.engine.display().to_string());
        assert_eq!(option(&tokyo_options, "trt_engine_cache_path"), tokyo.engine.display().to_string());
        assert_ne!(
            option(&kyoto_options, "trt_engine_cache_path"),
            option(&tokyo_options, "trt_engine_cache_path"),
            "two models shared one engine cache directory"
        );

        assert_eq!(option(&kyoto_options, "trt_timing_cache_path"), shared_timing.display().to_string());
        assert_eq!(
            option(&kyoto_options, "trt_timing_cache_path"),
            option(&tokyo_options, "trt_timing_cache_path"),
            "each model was given a timing cache of its own, which is the 60% this setting exists to keep"
        );
    }

    #[test]
    fn a_raw_tensorrt_overlay_is_applied_last_and_overrides_a_pinned_default() {
        // The escape hatch, and the case it was measured for: Osaka drops the builder optimization level from the 5
        // every other model gets to TensorRT's own 3, which on a 12,940-node graph is 87 seconds of engine build for
        // no runtime difference.
        let profile = EpProfile {
            trt_options: [
                ("trt_builder_optimization_level".to_string(), "3".to_string()),
                ("trt_dla_enable".to_string(), "1".to_string()),
            ]
            .into_iter()
            .collect(),
            ..EpProfile::default()
        };

        let options = tensorrt_options(&paths(), &profile);

        assert_eq!(
            option(&options, "trt_builder_optimization_level"),
            "3",
            "the overlay did not override the default"
        );
        assert_eq!(option(&options, "trt_dla_enable"), "1", "the overlay did not add a key with no typed field");
        // And it overrides only what it names.
        assert_eq!(option(&options, "trt_engine_cache_enable"), "1");
    }

    #[test]
    fn cuda_carries_the_pinned_defaults_it_was_measured_with() {
        let options = cuda_options(&EpProfile::default());

        assert_eq!(option(&options, "cudnn_conv_algo_search"), "EXHAUSTIVE");
        assert_eq!(option(&options, "cudnn_conv_use_max_workspace"), "1");
        assert_eq!(option(&options, "gpu_mem_limit"), "0", "a GPU memory ceiling was imposed");
        assert_eq!(option(&options, "do_copy_in_default_stream"), "1");
        assert_eq!(option(&options, "device_id"), "0");
    }

    #[test]
    fn the_cuda_convolution_layout_follows_the_profile_in_both_directions() {
        // NCHW is the runtime's own default and stays the default here, because the models that want NHWC do not
        // agree on when: Athens is a win at FP16 and a loss at FP32, New York is a win in both.
        assert_eq!(option(&cuda_options(&EpProfile::default()), "prefer_nhwc"), "0");

        let nhwc = EpProfile { cuda_prefer_nhwc: true, ..EpProfile::default() };
        assert_eq!(option(&cuda_options(&nhwc), "prefer_nhwc"), "1");
    }

    #[test]
    fn coreml_pins_the_model_format_and_static_input_shapes() {
        // Neither is offered as a per-model setting, and both are correctness rather than tuning: the older format
        // has no typed execution and would run an FP32 graph at half precision, and turning static shapes off on a
        // graph that is in fact fixed-shape has failed session creation outright.
        let options = coreml_options(&paths(), &EpProfile::default());

        assert_eq!(option(&options, "ModelFormat"), "MLProgram");
        assert_eq!(option(&options, "RequireStaticInputShapes"), "1");
        assert_eq!(option(&options, "EnableOnSubgraphs"), "0");
        assert_eq!(option(&options, "ModelCacheDirectory"), paths().engine.display().to_string());
    }

    #[test]
    fn kyoto_at_fp16_reaches_the_cpu_and_the_neural_engine_and_at_fp32_takes_the_default() {
        // A measured declaration read through the option map it ends up in — so the profile and the key it
        // configures are checked together rather than each against itself. Kyoto's is the precision-split case, and
        // the two arms below it cover the other two shapes a declaration takes.
        let scale = Scale::new(4.0).expect("4.0 is in range");
        let fp16 = Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp16), scale);
        let fp32 = Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp32), scale);

        assert_eq!(
            option(&coreml_options(&paths(), &fp16.profile()), "MLComputeUnits"),
            "CPUAndNeuralEngine",
            "Kyoto at FP16 did not reach the Neural Engine"
        );
        assert_eq!(
            option(&coreml_options(&paths(), &fp32.profile()), "MLComputeUnits"),
            "ALL",
            "Kyoto at FP32 was given the FP16 answer, which drops the graph onto the CPU"
        );
    }

    #[test]
    fn tokyo_reaches_the_cpu_and_the_gpu_at_both_precisions() {
        // The exclusion rather than the preference, read through the same option map: what keeps this graph off the
        // Neural Engine is its window attention, so both precisions have to arrive at `CPUAndGPU` rather than only
        // the one where the restriction also happens to be faster.
        let scale = Scale::new(4.0).expect("4.0 is in range");

        for precision in FloatPrecision::ALL {
            let tokyo = Upscale::new(UpscaleVariant::Tokyo(precision), scale);

            assert_eq!(
                option(&coreml_options(&paths(), &tokyo.profile()), "MLComputeUnits"),
                "CPUAndGPU",
                "Tokyo at {precision:?} was left able to reach the Neural Engine"
            );
        }
    }

    #[test]
    fn saitama_at_fp16_reaches_the_cpu_and_the_neural_engine_and_at_fp32_takes_the_default() {
        // Kyoto's assertion against the other RRDBNet, because the two agreeing is a fact about their graphs: same
        // op mix, same answer, and the same FP32 arm that would be a measured regression if it were generalised.
        let scale = Scale::new(4.0).expect("4.0 is in range");
        let fp16 = Upscale::new(UpscaleVariant::Saitama(FloatPrecision::Fp16), scale);
        let fp32 = Upscale::new(UpscaleVariant::Saitama(FloatPrecision::Fp32), scale);

        assert_eq!(
            option(&coreml_options(&paths(), &fp16.profile()), "MLComputeUnits"),
            "CPUAndNeuralEngine",
            "Saitama at FP16 did not reach the Neural Engine"
        );
        assert_eq!(
            option(&coreml_options(&paths(), &fp32.profile()), "MLComputeUnits"),
            "ALL",
            "Saitama at FP32 was given the FP16 answer, which drops the graph onto the CPU"
        );
    }

    #[test]
    fn osakas_builder_level_reaches_tensorrt_as_three_while_another_models_reaches_it_as_five() {
        // The override read through the option map it ends up in, beside a model that declares none — so what is
        // checked is that Osaka's declaration *arrives*, not merely that the overlay mechanism works.
        // `a_raw_tensorrt_overlay_is_applied_last_and_overrides_a_pinned_default` proves the mechanism from a
        // hand-built profile; this proves the model that needs it reaches the mechanism.
        let scale = Scale::new(4.0).expect("4.0 is in range");
        let kyoto = Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp16), scale);

        for precision in OsakaPrecision::ALL {
            let osaka = Upscale::new(UpscaleVariant::Osaka(precision), scale);
            let options = tensorrt_options(&paths(), &osaka.profile());

            assert_eq!(
                option(&options, "trt_builder_optimization_level"),
                "3",
                "Osaka at {precision:?} was given the level every other model carries, which is 87 seconds of engine build for no runtime difference"
            );
            // The two absences that are decisions rather than omissions: both modes stay at the pinned `0` rather
            // than being set by this profile. See `models::upscale::osaka::profile`'s own section for why each is refused.
            assert_eq!(option(&options, "trt_fp16_enable"), "0", "at {precision:?}");
            assert_eq!(option(&options, "trt_int8_enable"), "0", "at {precision:?}");
            // And the override names one key: everything else is still the pinned default.
            assert_eq!(option(&options, "trt_engine_hw_compatible"), "0", "at {precision:?}");
            assert_eq!(option(&options, "trt_max_workspace_size"), (4u64 << 30).to_string(), "at {precision:?}");
        }

        assert_eq!(
            option(&tensorrt_options(&paths(), &kyoto.profile()), "trt_builder_optimization_level"),
            "5",
            "Osaka's declaration reached a model that never made it"
        );
    }

    #[test]
    fn osakas_session_settings_survive_the_resolution_on_every_provider_it_reaches() {
        // The session half, and the one that is correctness rather than tuning: without the second of these disabled
        // the transformer does not load at all, so a provider path that dropped them would fail every Osaka run.
        let scale = Scale::new(4.0).expect("4.0 is in range");
        let osaka = Upscale::new(UpscaleVariant::Osaka(OsakaPrecision::Fp16), scale);

        let machines = [
            (ExecutionProvider::Auto, mac()),
            (ExecutionProvider::CoreMl, mac()),
            (ExecutionProvider::Cpu, machine_supporting(true, true, true)),
            (ExecutionProvider::Cuda, machine_supporting(false, true, true)),
            (ExecutionProvider::TensorRt, machine_supporting(false, true, true)),
        ];

        for (requested, machine) in machines {
            let plan = resolve(requested, machine, &osaka.profile(), &paths());

            assert_eq!(plan.settings.execution_mode, ExecutionMode::Sequential, "{requested} on {machine:?}");
            assert!(!plan.settings.mem_pattern, "{requested} on {machine:?}: the memory planner survived");
            assert_eq!(
                plan.settings.disabled_optimizers,
                vec!["ReshapeFusion".to_string(), "SimplifiedLayerNormFusion".to_string()],
                "{requested} on {machine:?}: the graph would not load on the CPU without these off"
            );
        }
    }

    #[test]
    fn osaka_reaches_the_cpu_and_the_gpu_at_both_precisions() {
        // The default `ALL` lets CoreML dispatch to the Neural Engine, which costs 4.2x on the VAE encoder and 3.9x
        // on the decoder — so like Tokyo's, this declaration is chosen for what it excludes.
        let scale = Scale::new(4.0).expect("4.0 is in range");

        for precision in OsakaPrecision::ALL {
            let osaka = Upscale::new(UpscaleVariant::Osaka(precision), scale);

            assert_eq!(
                option(&coreml_options(&paths(), &osaka.profile()), "MLComputeUnits"),
                "CPUAndGPU",
                "Osaka at {precision:?} was left able to reach the Neural Engine"
            );
        }
    }

    #[test]
    fn tokyos_sequential_mode_survives_the_resolution_on_every_provider_it_reaches() {
        // The session half of Tokyo's declaration, driven from the variant's own profile rather than from a
        // hand-built one: `a_model_measured_as_sequential_carries_it_whichever_provider_resolved` proves the
        // mechanism, and this proves the model that declares it actually arrives at the mechanism.
        let scale = Scale::new(4.0).expect("4.0 is in range");
        let tokyo = Upscale::new(UpscaleVariant::Tokyo(FloatPrecision::Fp16), scale);

        let machines = [
            (ExecutionProvider::Auto, mac()),
            (ExecutionProvider::CoreMl, mac()),
            (ExecutionProvider::Cpu, machine_supporting(true, true, true)),
            (ExecutionProvider::Cuda, machine_supporting(false, true, true)),
        ];

        for (requested, machine) in machines {
            let plan = resolve(requested, machine, &tokyo.profile(), &paths());

            assert_eq!(plan.settings.execution_mode, ExecutionMode::Sequential, "{requested} on {machine:?}");
        }
    }

    #[test]
    fn the_default_profile_changes_nothing_about_any_providers_options() {
        // What "nobody has measured this graph" means, checked against every provider that takes options: a model
        // declaring nothing gets the provider defaults with nothing added and nothing removed.
        let nothing_measured = EpProfile::default();

        assert_eq!(tensorrt_options(&paths(), &nothing_measured), tensorrt_options(&paths(), &EpProfile::default()));
        assert_eq!(option(&cuda_options(&nothing_measured), "prefer_nhwc"), "0");
        assert_eq!(option(&coreml_options(&paths(), &nothing_measured), "MLComputeUnits"), "ALL");
        assert_eq!(option(&coreml_options(&paths(), &nothing_measured), "SpecializationStrategy"), "Default");

        // And the maps carry exactly the pinned keys — nothing a profile would have had to remove.
        assert_eq!(tensorrt_options(&paths(), &nothing_measured).len(), 11);
        assert_eq!(cuda_options(&nothing_measured).len(), 7);
        assert_eq!(coreml_options(&paths(), &nothing_measured).len(), 6);
    }

    #[test]
    fn the_coreml_specialization_strategy_follows_the_profile() {
        let fast = EpProfile { coreml_specialization: CoreMlSpecialization::FastPrediction, ..EpProfile::default() };

        assert_eq!(option(&coreml_options(&paths(), &fast), "SpecializationStrategy"), "FastPrediction");
    }

    #[test]
    fn configuring_a_provider_reads_and_writes_nothing_on_disk() {
        // The directories are written into a map of strings and never touched, which is what lets a plan be produced
        // on a machine with no runtime installed — and, here, against paths that do not exist at all.
        let absent = CachePaths { engine: PathBuf::from("/nowhere/engine"), timing: PathBuf::from("/nowhere/timing") };

        let _ = tensorrt_options(&absent, &EpProfile::default());
        let _ = coreml_options(&absent, &EpProfile::default());

        assert!(!absent.engine.exists(), "configuring TensorRT created its engine directory");
        assert!(!absent.timing.exists(), "configuring TensorRT created its timing directory");
    }

    #[test]
    fn an_unmeasured_model_runs_parallel_with_the_memory_planner_on_and_no_optimizer_disabled() {
        // What every model runs with before it is measured, which is what "nobody has measured this graph" has to
        // mean at the session level as well as at the provider level.
        let plan = resolve(ExecutionProvider::Auto, no_accelerator(), &EpProfile::default(), &paths());

        assert_eq!(plan.settings.execution_mode, ExecutionMode::Parallel);
        assert!(plan.settings.mem_pattern, "the memory planner was off before anything was measured");
        assert!(plan.settings.disabled_optimizers.is_empty());
    }

    #[test]
    fn a_model_measured_as_sequential_carries_it_whichever_provider_resolved() {
        // A session setting rather than a per-provider one, so a model that sets it sets it everywhere — including
        // on a CPU run, where there is no provider at all to carry it.
        let profile = EpProfile {
            execution_mode: ExecutionMode::Sequential,
            disable_mem_pattern: true,
            disabled_optimizers: vec!["ConstantFolding".to_string(), "GeluFusion".to_string()],
            ..EpProfile::default()
        };

        let machines = [
            (ExecutionProvider::Auto, machine_supporting(false, true, true)),
            (ExecutionProvider::CoreMl, mac()),
            (ExecutionProvider::Cpu, machine_supporting(true, true, true)),
            (ExecutionProvider::Cuda, no_accelerator()),
        ];

        for (requested, machine) in machines {
            let plan = resolve(requested, machine, &profile, &paths());

            assert_eq!(plan.settings.execution_mode, ExecutionMode::Sequential, "{requested} on {machine:?}");
            assert!(!plan.settings.mem_pattern, "{requested} on {machine:?}");
            assert_eq!(plan.settings.disabled_optimizers, ["ConstantFolding", "GeluFusion"]);
        }
    }

    #[test]
    fn a_plan_carries_the_providers_in_attach_order_each_with_its_own_options() {
        let plan =
            resolve(ExecutionProvider::Auto, machine_supporting(false, true, true), &EpProfile::default(), &paths());

        let attached: Vec<ExecutionProvider> = plan.providers.iter().map(|entry| entry.provider).collect();
        assert_eq!(attached, vec![ExecutionProvider::TensorRt, ExecutionProvider::Cuda]);

        // Each carries its own provider's keys rather than a shared map: TensorRT's cache paths are not CUDA's
        // business, and one option update rejected wholesale is what a misplaced key costs.
        assert_eq!(plan.providers[0].options, tensorrt_options(&paths(), &EpProfile::default()));
        assert_eq!(plan.providers[1].options, cuda_options(&EpProfile::default()));
    }

    #[test]
    fn a_cpu_run_produces_no_provider_options_at_all() {
        for (requested, machine) in [
            (ExecutionProvider::Cpu, machine_supporting(true, true, true)),
            (ExecutionProvider::CoreMl, no_accelerator()),
        ] {
            let plan = resolve(requested, machine, &EpProfile::default(), &paths());

            assert!(plan.providers.is_empty(), "{requested} produced options for a CPU run");
            assert_eq!(plan.resolved, ExecutionProvider::Cpu);
            assert_eq!(plan.requested, requested, "the plan lost what was asked for");
        }
    }

    #[test]
    fn a_plan_is_produced_on_a_machine_with_no_runtime_installed_and_touches_no_file() {
        // The property the whole slice is arranged around: the decision is made from four booleans, a profile and two
        // paths, so it needs no ONNX Runtime, opens no model file, and creates neither cache directory. The paths
        // here name a directory that does not exist and is not created.
        let absent = CachePaths { engine: PathBuf::from("/nowhere/engine"), timing: PathBuf::from("/nowhere/timing") };

        for requested in ExecutionProvider::ALL {
            let plan = resolve(requested, machine_supporting(true, true, true), &EpProfile::default(), &absent);

            assert_eq!(plan.requested, requested);
        }

        assert!(!absent.engine.exists(), "resolving a plan created the engine cache directory");
        assert!(!absent.timing.exists(), "resolving a plan created the timing cache directory");
        assert!(
            !PathBuf::from("/nowhere").exists(),
            "resolving a plan created a directory it was only told about"
        );
    }

    #[test]
    fn a_models_measured_profile_reaches_the_provider_it_was_measured_for() {
        // End to end through the plan rather than through one option builder: Kyoto's FP16 declaration has to survive
        // the resolution, the attach list and the per-provider dispatch to reach `MLComputeUnits`.
        let scale = Scale::new(4.0).expect("4.0 is in range");
        let kyoto = Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp16), scale);

        let plan = resolve(ExecutionProvider::Auto, mac(), &kyoto.profile(), &paths());

        assert_eq!(plan.resolved, ExecutionProvider::CoreMl);
        assert_eq!(option(&plan.providers[0].options, "MLComputeUnits"), "CPUAndNeuralEngine");
    }
}
