//! The helpers models build their `rust_sak::ai::Tuning` from, and the limits a tuning is held to.

// Provider vocabulary rather than model vocabulary — it names CoreML compute units and a CUDA layout — which is why it
// lives here and `models/` imports it, never the reverse: nothing in `providers/` may depend on a model.

// A precision is not a model: it is the one piece of model vocabulary every profile is declared against, which is why
// importing it here does not break the rule above.
use crate::models::precision::Precision;
use rust_sak::ai::{CoreMlComputeUnits, Tuning};

// Each model declares a `rust_sak::ai::Tuning`, per precision, in the file that names the graph it was measured
// against. The default means *nobody has measured this graph*, and changes nothing about any provider's options.
//
// Why each field is per model, and what was measured across the catalogue for it, field by field (each note ends with
// the field it is about):
//
// Per model because the right provider settings are a property of the **model**, not of the machine, and they do
// not survive a precision change: the same graph exported at FP16 and at FP32 wants opposite answers from
// `cuda_prefer_nhwc`. A default that changes nothing is what lets every variant answer `profile()` while only the
// ones that were actually measured say anything.
//
// Seven fields rather than the reference implementation's fifteen, plus the two WebGPU ones it never had. Those
// seven are what its twelve measured profiles actually set; the eight it keeps permanently unset — `Fp16`, `TrtShapes`, `TrtWorkspaceBytes`,
// `CudaOptions`, `Extra`, `GraphOptimization`, `DynamicShapes` and `ExcludeEPs` — are omitted along with the
// chain-substitution logic `ExcludeEPs` alone justifies. Their default behaviour is reproduced exactly, so no model
// runs differently for their absence, and a typed field is added the day a model needs one.
//
// Per model because the right answer follows the graph's op mix rather than the machine. The Neural Engine is
// FP16-only and is built for dense convolution and matmul; a graph heavy in normalization, reshape and transpose
// spends more time crossing on and off it than it saves, and CoreML's planner does not work that out on its own —
// it takes the Neural Engine whenever `ALL` permits it.
// Which of the Mac's engines CoreML may dispatch this model to.
// ^ `coreml_compute_units`.
//
// A trade rather than a win: the default strategy compiles once into a plan that is good across input sizes, while
// `FastPrediction` spends longer specializing for the shapes it was given. Only worth paying for on a fixed-shape
// graph that stays resident — which is what these tile models are — and even then it has to be measured: Lyon is
// one fused CoreML node at a fixed 1024x1024, the best case for it, and lands within 0.3% of the default in both
// precisions with bit-identical output.
// How CoreML compiles this model for the device.
// ^ `coreml_specialization`.
//
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
// Whether independent branches of the graph may run on separate threads.
//
// A session setting rather than a per-provider one, so a model that sets it sets it on every provider it runs
// on.
// ^ `execution_mode`.
//
// Whether to turn off ONNX Runtime's static memory planner.
//
// The planner assumes shapes repeat between runs. When they vary it over-allocates and never returns what it
// reserved, so a model whose shapes genuinely vary switches it off.
// ^ `disable_mem_pattern`.
//
// The scalpel to the optimization *level*'s hammer, which is why the level itself is not a field here: a model
// that one transformer miscompiles keeps every other fusion instead of giving them all up.
// Individual graph transformers to switch off while leaving the rest of the optimization pipeline on.
//
// An unrecognised name is ignored by the runtime rather than reported, so a typo here is silent and the only sign
// the setting took effect is that the model loads.
//
// A model may name **several**. They are written into one config entry joined by
// [`DISABLED_OPTIMIZER_SEPARATOR`].
// ^ `disabled_optimizers`.
//
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
// Whether the CUDA provider should run convolutions in NHWC rather than ONNX Runtime's default NCHW.
//
// Not universally safe: the runtime's layout transform mishandles a convolution whose weights are computed rather
// than an initializer — a StyleGAN-style modulated convolution — and the graph then fails at `Run` with a channel
// mismatch rather than at session build.
// ^ `cuda_prefer_nhwc`.
//
// The escape hatch that stops the next measured setting from needing a type change.
//
// Measuring what belongs here needs two precautions a CUDA sweep does not, and both produce a clean table of wrong
// numbers rather than an obvious failure. TensorRT's engine cache file name carries only the graph hash and the
// precision flags, so two configurations pointed at one cache directory hand each other the first one's engine and
// every option reads as a no-op — give each configuration a directory of its own. And the builder is not
// deterministic: six sessions built from an identical configuration, one cache directory each, spread from -2.5%
// to +1.2% and did not all produce the same output, so nothing below roughly ±3% survives a second build.
// Raw TensorRT provider options for settings that have no typed field here, applied after the pinned defaults so
// they can also override one.
//
// An unrecognised key is not ignored: ONNX Runtime rejects the whole option update, so the session build fails
// and the fallback moves the graph to the next provider in the chain as a reported downgrade. A typo here costs
// the GPU rather than a setting.
// ^ `trt_options`.
//
// Per model and per precision, like `cuda_prefer_nhwc`, because what decides it is which layouts the plugin's WGSL
// kernels are fastest in at the shapes a graph asks for. NHWC is the plugin's own default; Paris measures -6.1% at
// FP32 and -7.3% at FP16 from NCHW on an M2 Max, end to end, with identical output.
// Whether the WebGPU plugin should prefer NCHW for its layout-sensitive kernels rather than its own default NHWC.
// ^ `webgpu_prefer_nchw`.
//
// A correctness setting rather than tuning: the escape hatch for a node whose WebGPU kernel is broken, which the
// plugin otherwise runs and fails on — sometimes by aborting the process, which no fallback can catch. The node then
// runs on the CPU provider, at the cost of a copy off the GPU and back.
//
// Only nodes **in the file** can be named. The runtime expands some ops (`Softmax`, `LeakyRelu`) into unnamed
// primitives before the plugin sees them, and those cannot be reached from here.
// Graph nodes, by name, the WebGPU plugin must leave to the CPU provider.
//
// An unrecognised name is ignored rather than reported.
// ^ `webgpu_force_cpu_nodes`.

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
/// What separates the names in [`Tuning::disabled_optimizers`] when `rust-sak` writes them into ONNX Runtime's
/// `optimization.disable_specified_optimizers` config entry.
///
/// **The entries are not trimmed**: a space after the separator makes an entry no transformer matches, and the graph
/// then fails as though nothing had been disabled.
#[cfg(test)]
pub(crate) const DISABLED_OPTIMIZER_SEPARATOR: &str = ";";

// Written once rather than in each model's file, because eight models reached this same answer and a copy each is
// eight places for one of them to drift. Each model still declares its own `profile` beside the measurement that
// justifies it, and calls this: the reasoning is per model even where the setting is shared.
/// CoreML kept off the Neural Engine at FP16, and the provider defaults at every other precision.
///
/// The profile of a graph whose op mix the Neural Engine handles badly: at FP16 CoreML routes it there by default and
/// pays for it in transitions, and at FP32 CoreML bars the program from the Neural Engine anyway, so a setting there
/// would restate a choice it has already made.
pub(crate) fn cpu_and_gpu_at_fp16(precision: Precision) -> Tuning {
    match precision {
        Precision::Fp16 => Tuning { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..Tuning::default() },
        _ => Tuning::default(),
    }
}

// The WebGPU plugin takes `forceCpuNodeNames` as one newline-joined value, and ONNX Runtime refuses any provider-option
// value past this length with "Config value is longer than maximum length" — the session then does not build at all.
// Held here so a list that outgrows it fails the suite rather than a user's session.
/// The longest value ONNX Runtime accepts for one provider option, in bytes.
#[cfg(test)]
pub(crate) const PROVIDER_OPTION_LIMIT: usize = 8192;

// Named rather than spelled at each site because a model's `trt_options` is merged over the pinned TensorRT map **by
// key**: a key that does not match does not fail, it silently leaves the pinned value in place and adds an unrecognised
// one alongside, and ONNX Runtime then rejects the whole update.
/// The key of TensorRT's builder optimization level, which a model may override through [`Tuning::trt_options`].
pub(crate) const TRT_BUILDER_OPTIMIZATION_LEVEL: &str = "trt_builder_optimization_level";

/// Node names as a tuning holds them, from the `const` list a model keeps them in.
pub(crate) fn nodes(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

// Written once for the same reason as `cpu_and_gpu_at_fp16`: seven models force nodes off WebGPU at FP16 alone, and
// a copy of the precision gate in each is seven places for one arm to pick up the list by mistake.
/// `tuning` with `names` forced off WebGPU at FP16, and unchanged at every other precision.
pub(crate) fn with_webgpu_cpu_nodes_at_fp16(tuning: Tuning, precision: Precision, names: &[&str]) -> Tuning {
    match precision {
        Precision::Fp16 => Tuning { webgpu_force_cpu_nodes: nodes(names), ..tuning },
        _ => tuning,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webgpu_cpu_nodes_are_added_at_fp16_only() {
        const NODES: &[&str] = &["node_a", "node_b"];
        let base = cpu_and_gpu_at_fp16(Precision::Fp16);

        assert_eq!(
            with_webgpu_cpu_nodes_at_fp16(base.clone(), Precision::Fp16, NODES),
            Tuning { webgpu_force_cpu_nodes: vec!["node_a".to_string(), "node_b".to_string()], ..base }
        );

        for precision in [Precision::Fp32, Precision::Int8] {
            assert_eq!(with_webgpu_cpu_nodes_at_fp16(Tuning::default(), precision, NODES), Tuning::default());
        }
    }

    #[test]
    fn the_provider_option_limit_is_the_one_onnx_runtime_enforces() {
        // Pinned as a literal because it is the runtime's number rather than this crate's: on 1.30 an 8,230-byte list
        // failed to build and a 7,984-byte one built.
        assert_eq!(PROVIDER_OPTION_LIMIT, 8192);
    }

    #[test]
    fn the_shared_fp16_profile_keeps_coreml_off_the_neural_engine_and_names_nothing_else() {
        // Compared as whole tunings, so a field added later is covered by existing: every model that declares this was
        // measured with every other option inside run-to-run spread.
        assert_eq!(
            cpu_and_gpu_at_fp16(Precision::Fp16),
            Tuning { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..Tuning::default() },
            "FP16 is not kept off the Neural Engine"
        );

        // Not load-bearing for speed — CoreML keeps an FP32 program off the Neural Engine anyway — but load-bearing
        // for not inviting the next editor to widen a setting that was never measured to help there.
        for precision in [Precision::Fp32, Precision::Int8] {
            assert_eq!(cpu_and_gpu_at_fp16(precision), Tuning::default(), "{precision:?} declared a setting");
        }
    }
}
