//! The per-model tuning that overrides an execution provider's defaults.

// Provider vocabulary rather than model vocabulary — it names CoreML compute units and a CUDA layout — which is why it
// lives here and `models/` imports it, never the reverse. The option builders take a profile and two paths and nothing
// else, so nothing in `providers/` may depend on a model. It owns no resource and is plain `Copy`-adjacent data, which
// is the same relationship `models/` already has with `Precision`.

use std::collections::BTreeMap;

/// The per-model tuning applied on top of an execution provider's defaults.
///
/// A variant declares its own, per precision, in the file that names the graph it was measured against. The default
/// means *nobody has measured this graph*, and it changes nothing about any provider's options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EpProfile {
    // Per model because the right provider settings are a property of the **model**, not of the machine, and they do
    // not survive a precision change: the same graph exported at FP16 and at FP32 wants opposite answers from
    // `cuda_prefer_nhwc`. A default that changes nothing is what lets every variant answer `profile()` while only the
    // ones that were actually measured say anything.
    //
    // Seven fields rather than the reference implementation's fifteen. Those seven are what its twelve measured
    // profiles actually set; the eight it keeps permanently unset — `Fp16`, `TrtShapes`, `TrtWorkspaceBytes`,
    // `CudaOptions`, `Extra`, `GraphOptimization`, `DynamicShapes` and `ExcludeEPs` — are omitted along with the
    // chain-substitution logic `ExcludeEPs` alone justifies. Their default behaviour is reproduced exactly, so no model
    // runs differently for their absence, and a typed field is added the day a model needs one.

    // Per model because the right answer follows the graph's op mix rather than the machine. The Neural Engine is
    // FP16-only and is built for dense convolution and matmul; a graph heavy in normalization, reshape and transpose
    // spends more time crossing on and off it than it saves, and CoreML's planner does not work that out on its own —
    // it takes the Neural Engine whenever `ALL` permits it.
    /// Which of the Mac's engines CoreML may dispatch this model to.
    pub(crate) coreml_compute_units: CoreMlComputeUnits,

    // A trade rather than a win: the default strategy compiles once into a plan that is good across input sizes, while
    // `FastPrediction` spends longer specializing for the shapes it was given. Only worth paying for on a fixed-shape
    // graph that stays resident — which is what these tile models are — and even then it has to be measured: Lyon is
    // one fused CoreML node at a fixed 1024x1024, the best case for it, and lands within 0.3% of the default in both
    // precisions with bit-identical output.
    /// How CoreML compiles this model for the device.
    pub(crate) coreml_specialization: CoreMlSpecialization,

    // Per model because the win the inter-op pool exists for is a property of the graph's shape: it only pays for
    // itself on wide, genuinely independent branches, and it costs a thread handoff at every node either way. Most of
    // these graphs are close to linear backbones, so the handoff is all there is — and it is charged per `Run` rather
    // than once.
    //
    // It is worth trying on any new graph before reaching for a provider option, because it costs nothing and it
    // scales with the node count: Santorini measures 4-8% from switching to sequential, and Tokyo — 2682 nodes against
    // Santorini's few hundred — measures 18-27%, both with bit-identical output.
    //
    // Setting it on every provider was settled by measurement: TensorRT is a tie because it fuses the graph into one
    // node, and on CoreML every FP16 graph measured wins 3.5-6.5% while every FP32 graph is a tie, with nothing losing
    // by more than half a percent.
    /// Whether independent branches of the graph may run on separate threads.
    ///
    /// A session setting rather than a per-provider one, so a model that sets it sets it on every provider it runs
    /// on.
    pub(crate) execution_mode: ExecutionMode,

    /// Whether to turn off ONNX Runtime's static memory planner.
    ///
    /// The planner assumes shapes repeat between runs. When they vary it over-allocates and never returns what it
    /// reserved, so a model whose shapes genuinely vary switches it off.
    pub(crate) disable_mem_pattern: bool,

    // The scalpel to the optimization *level*'s hammer, which is why the level itself is not a field here: a model
    // that one transformer miscompiles keeps every other fusion instead of giving them all up.
    /// Individual graph transformers to switch off while leaving the rest of the optimization pipeline on.
    ///
    /// An unrecognised name is ignored by the runtime rather than reported, so a typo here is silent and the only sign
    /// the setting took effect is that the model loads.
    ///
    /// A model may name **several**. They are written into one config entry joined by
    /// [`DISABLED_OPTIMIZER_SEPARATOR`].
    pub(crate) disabled_optimizers: Vec<String>,

    // Per model *and* per precision, because it is not an optimization the runtime can pick on its own merits. The
    // intuition is that NHWC is the layout cuDNN's FP16 tensor-core kernels want, so an FP16 graph stops paying for the
    // transposes around every convolution while an FP32 one has no tensor-core path to reach and only pays the
    // conversion: Athens measures -4% end to end at FP16 and +8% at FP32 from this one flag, and sets it for FP16
    // alone.
    //
    // That is not a rule. New York wants NHWC in **both** precisions — -35% at FP16 and -17.5% at FP32 on the graph
    // alone — because what decides it is which layouts cuDNN has kernels for at the shapes the graph asks for, and a
    // RetinaFace backbone at 640x640 gets a different answer from a CodeFormer at 512x512. Measure it per graph, per
    // precision.
    //
    // The failure its doc describes is the strongest reason it is opt-in.
    /// Whether the CUDA provider should run convolutions in NHWC rather than ONNX Runtime's default NCHW.
    ///
    /// Not universally safe: the runtime's layout transform mishandles a convolution whose weights are computed rather
    /// than an initializer — a StyleGAN-style modulated convolution — and the graph then fails at `Run` with a channel
    /// mismatch rather than at session build.
    pub(crate) cuda_prefer_nhwc: bool,

    // The escape hatch that stops the next measured setting from needing a type change.
    //
    // Measuring what belongs here needs two precautions a CUDA sweep does not, and both produce a clean table of wrong
    // numbers rather than an obvious failure. TensorRT's engine cache file name carries only the graph hash and the
    // precision flags, so two configurations pointed at one cache directory hand each other the first one's engine and
    // every option reads as a no-op — give each configuration a directory of its own. And the builder is not
    // deterministic: six sessions built from an identical configuration, one cache directory each, spread from -2.5%
    // to +1.2% and did not all produce the same output, so nothing below roughly ±3% survives a second build.
    /// Raw TensorRT provider options for settings that have no typed field here, applied after the pinned defaults so
    /// they can also override one.
    ///
    /// An unrecognised key is not ignored: ONNX Runtime rejects the whole option update, so the session build fails
    /// and the fallback moves the graph to the next provider in the chain as a reported downgrade. A typo here costs
    /// the GPU rather than a setting.
    pub(crate) trt_options: BTreeMap<String, String>,
}

// Established by measurement rather than taken from a binding's documentation, because getting it wrong is invisible:
// the runtime ignores an entry that names no transformer rather than reporting it, so a wrongly joined list is read as
// one name that matches nothing, disables nothing, and leaves the model failing to load exactly as though the setting
// had never been written. There is no error and no log line — the only symptom is a model that does not open.
//
// A named constant rather than a literal inside a `.join()` call, so a sweep of this setting can reach it: one that
// varies the *order* of the names and the spacing around the separator but not the separator itself reads a wrong
// separator's table of failures as the runtime taking a single name.
//
// Measured against ONNX Runtime 1.26 on Osaka's diffusion transformer, which opens on the CPU provider only when
// `SimplifiedLayerNormFusion` is genuinely switched off — the one instrument this setting has, since it cannot be read
// back. The semicolon loads in either order; a comma does not; and an unrecognised name is dropped on its own rather
// than costing the whole value. See `models::upscale::osaka::live` for the table.
/// What separates the names in [`EpProfile::disabled_optimizers`] when they are written into ONNX Runtime's
/// `optimization.disable_specified_optimizers` config entry.
///
/// **The entries are not trimmed**: a space after the separator makes an entry no transformer matches, and the graph
/// then fails as though nothing had been disabled.
pub(crate) const DISABLED_OPTIMIZER_SEPARATOR: &str = ";";

/// The set of engines CoreML may dispatch a model to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CoreMlComputeUnits {
    // The right default: most of these graphs are convolutional, which is what the Neural Engine is good at.
    /// CoreML chooses freely between the CPU, the GPU and the Neural Engine.
    #[default]
    All,
    /// Keeps a model off the Neural Engine, for the graphs it handles badly — where `All` costs real time in
    /// transitions.
    CpuAndGpu,
    /// Keeps a model off the GPU, leaving it for other work.
    CpuAndNeuralEngine,
    /// Runs the CoreML partition on the CPU. A diagnostic: it isolates whether a wrong result came from the GPU's or
    /// the Neural Engine's reduced precision.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the first model to measure it arrives with its family's pipeline")
    )]
    CpuOnly,
}

impl CoreMlComputeUnits {
    /// The value ONNX Runtime's `MLComputeUnits` option takes.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::CpuAndGpu => "CPUAndGPU",
            Self::CpuAndNeuralEngine => "CPUAndNeuralEngine",
            Self::CpuOnly => "CPUOnly",
        }
    }
}

/// The strategy CoreML uses when it compiles a model for a device.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CoreMlSpecialization {
    /// CoreML's own default strategy.
    #[default]
    Default,
    // For what it is worth and what it costs, see `models::face_recovery::santorini::profile`.
    /// Trades compile time for prediction latency. It needs macOS 15 or newer; older systems ignore it rather than
    /// failing, so setting it is safe on any Mac.
    FastPrediction,
}

impl CoreMlSpecialization {
    /// The value ONNX Runtime's `SpecializationStrategy` option takes.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::FastPrediction => "FastPrediction",
        }
    }
}

/// Whether ONNX Runtime may run independent branches of a graph on separate threads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ExecutionMode {
    /// Independent branches may run on separate threads.
    #[default]
    Parallel,
    /// One node at a time on the calling thread. For the graphs that have no branch wide enough to pay for the
    /// inter-op pool, which is most of them.
    Sequential,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_profile_means_nobody_has_measured_this_graph() {
        // The option builders' own tests are what prove the default changes nothing about the maps they produce; this
        // pins the values they read.
        let profile = EpProfile::default();

        assert_eq!(profile.coreml_compute_units, CoreMlComputeUnits::All);
        assert_eq!(profile.coreml_specialization, CoreMlSpecialization::Default);
        assert_eq!(profile.execution_mode, ExecutionMode::Parallel);
        assert!(!profile.disable_mem_pattern, "the memory planner was off before anything was measured");
        assert!(profile.disabled_optimizers.is_empty(), "an optimizer was disabled before anything was measured");
        assert!(!profile.cuda_prefer_nhwc, "NCHW is the runtime's own default and stays the default here");
        assert!(profile.trt_options.is_empty(), "a TensorRT override was applied before anything was measured");
    }

    #[test]
    fn each_coreml_compute_unit_renders_the_value_the_runtime_takes() {
        // These strings are ONNX Runtime's own, not ours: a fourth spelling of any of them makes the runtime reject
        // the whole option update, which costs the provider rather than the setting.
        assert_eq!(CoreMlComputeUnits::All.as_str(), "ALL");
        assert_eq!(CoreMlComputeUnits::CpuAndGpu.as_str(), "CPUAndGPU");
        assert_eq!(CoreMlComputeUnits::CpuAndNeuralEngine.as_str(), "CPUAndNeuralEngine");
        assert_eq!(CoreMlComputeUnits::CpuOnly.as_str(), "CPUOnly");
    }

    #[test]
    fn each_specialization_strategy_renders_the_value_the_runtime_takes() {
        assert_eq!(CoreMlSpecialization::Default.as_str(), "Default");
        assert_eq!(CoreMlSpecialization::FastPrediction.as_str(), "FastPrediction");
    }

    #[test]
    fn the_defaults_are_the_first_arm_of_each_enum_rather_than_a_second_declaration() {
        // `#[default]` on the arm rather than a hand-written `Default`, so the zero value and the documented default
        // cannot drift apart.
        assert_eq!(CoreMlComputeUnits::default(), CoreMlComputeUnits::All);
        assert_eq!(CoreMlSpecialization::default(), CoreMlSpecialization::Default);
        assert_eq!(ExecutionMode::default(), ExecutionMode::Parallel);
    }
}
