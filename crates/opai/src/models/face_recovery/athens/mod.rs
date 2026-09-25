//! Athens: the CodeFormer-style restorer a front end offers first.
//!
//! The model's row, and nothing else: the execution-provider profile measured for this graph. The restoration it
//! runs is [`restore`](super::restore), the family's one contract.

// The geometry `restore` sequences sits beside it at family tier, so deleting the model is deleting this directory
// plus the arms in `FaceRecoveryVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile, ExecutionMode};

/// The execution-provider tuning measured for this model, at the precision it carries.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Transcribed from the reference's `athens.go`, whose comment carries both the numbers and the conditions they
    // were taken under. **No execution provider was exercised against this model in this project**: what this crate
    // validates is that the settings are *declared*, not that the percentages reproduce on other hardware.
    EpProfile {
        // CoreML off the Neural Engine, at both precisions. Left to choose, CoreML gives most of the FP16 graph to the
        // Neural Engine, and the transitions on and off it cost more than the engine saves — restricting it to the CPU
        // and the GPU is worth **1.57x on an M2 Max**. The op mix is why: this is a CodeFormer, 72 GroupNorms, 19
        // LayerNorms held at FP32 for precision, and several hundred reshapes and transposes through the transformer.
        // Asking for the CPU and the Neural Engine together is the tell — 10-14x slower is not a slower engine, it is
        // an engine rejecting most of the graph and thrashing on what is left.
        //
        // **Declared at FP32 as well, though only FP16 moves.** An FP32 MLProgram cannot reach the Neural Engine at
        // all, so the FP32 declaration states the intent rather than changing a measurement — and a future export
        // that shifted what the Neural Engine will accept cannot silently re-enable it.
        coreml_compute_units: CoreMlComputeUnits::CpuAndGpu,
        // Worth **-20.6% at FP32 and -13.2% at FP16 on the graph alone** under CUDA, diluted to -8.5% and -4.6% end to
        // end because most of a face-recovery run is the alignment and the composite around each face rather than the
        // graph. The output is bit-identical. On TensorRT it is a tie, as it is for New York.
        //
        // On CoreML it leans **0.1-0.3% the other way**, measured in both build orders with parallel ahead in all four
        // rows, so the sign is real: Athens is the one model in the catalogue that does not prefer sequential on
        // CoreML, where Tokyo, Santorini and New York each gain 3.5-6.5%. The execution mode is a session setting
        // rather than a per-provider one, so there is one answer to give, and 0.1-0.3% is not a reason to make this
        // the one model that runs a different mode from the rest. Do not "fix" it from the CoreML figures alone.
        execution_mode: ExecutionMode::Sequential,
        // CUDA's NHWC convolution layout, at FP16 only. On an RTX 5090 it is worth **-7.7% at FP16 and costs +5.9% at
        // FP32 on the graph**, which is **-4% and +8% end to end**: the same flag measured at two scopes, so these
        // agree with the end-to-end figures `providers::profile` and the `execution-providers` spec quote.
        //
        // `EpProfile::cuda_prefer_nhwc` says why the split falls at the precision. **That this is FP16 only is the
        // load-bearing part, and it is the opposite of New York's answer**, which declares the layout at both:
        // tidying this for symmetry with New York costs 8% end to end with nothing to report it. The output is
        // unaffected either way: through the pipeline the NHWC result lands 67.1 dB from the NCHW one, where both sit
        // 56.6 dB from the FP32 model's.
        cuda_prefer_nhwc: precision == Precision::Fp16,
        // Two CUDA defaults must not change for this graph. Disabling TF32 costs 37%. Convolution-bias fusion returns
        // a **different answer on every run** here: not an error and not a slower run, but a restoration that is not
        // reproducible and would make one image's cached result disagree with the same image recomputed.
        //
        // The convolution algorithm search, the default-stream copy, the arena extension strategy, the EP-level
        // unified stream, the tunable-op switch and the attention kernel selection were all measured on this graph
        // and are all within noise — but the search must not be set to its *default* setting, which costs 70%.
        ..EpProfile::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::precision::FloatPrecision;

    #[test]
    fn athens_is_kept_off_the_neural_engine_at_both_precisions() {
        // Mirrors `athens_test.go`'s own reason for existing: nothing else asserts the variant carries this, so a
        // refactor that dropped it would cost 1.57x on an M2 Max silently — the model would still load and still
        // restore the right faces, only far slower. Asked through the variant rather than of `profile` directly,
        // because the variant's match is the half a refactor can break.
        for precision in FloatPrecision::ALL {
            let carried = super::super::FaceRecoveryVariant::Athens(precision).profile();

            assert_eq!(
                carried.coreml_compute_units,
                CoreMlComputeUnits::CpuAndGpu,
                "Athens at {precision:?} did not restrict CoreML to the CPU and the GPU"
            );
        }
    }

    #[test]
    fn athens_runs_sequentially_at_both_precisions() {
        // Set for CUDA's sake alone — 13-21% of the graph — and CoreML measures marginally *against* it, 0.1-0.3%.
        // That makes this the one setting in the catalogue a reader could "fix" by reading only the CoreML figures
        // above it and concluding the mode was a mistake, which is why it is pinned.
        for precision in FloatPrecision::ALL {
            let carried = super::super::FaceRecoveryVariant::Athens(precision).profile();

            assert_eq!(
                carried.execution_mode,
                ExecutionMode::Sequential,
                "Athens at {precision:?} did not declare sequential execution"
            );
        }
    }

    #[test]
    fn only_athens_fp16_graph_runs_in_nhwc_on_cuda() {
        // The one setting here that differs by precision, and both halves are load-bearing: NHWC is what reaches
        // cuDNN's FP16 tensor-core kernels, and it is a measured cost on the FP32 export, which has no such kernels
        // to reach and pays only the layout conversion. Neither half announces itself if it is dropped or copied to
        // the other precision — the model loads and returns the right answer either way.
        assert!(
            super::super::FaceRecoveryVariant::Athens(FloatPrecision::Fp16).profile().cuda_prefer_nhwc,
            "the FP16 graph must run in NHWC on CUDA"
        );
        assert!(
            !super::super::FaceRecoveryVariant::Athens(FloatPrecision::Fp32).profile().cuda_prefer_nhwc,
            "the FP32 graph must stay in NCHW on CUDA: NHWC measured 8% slower end to end"
        );
    }

    #[test]
    fn athens_leaves_every_other_provider_setting_at_the_runtimes_own_default() {
        // The two CUDA defaults that are load-bearing for this graph are the reason: disabling TF32 costs 37%, and
        // convolution-bias fusion returns a different answer on every run here — which would make one image's cached
        // result disagree with the same image recomputed. Both are carried by the `..EpProfile::default()` rather
        // than named, so what this pins is that nothing else was named either.
        let default = EpProfile::default();

        for precision in FloatPrecision::ALL {
            let carried = super::super::FaceRecoveryVariant::Athens(precision).profile();

            assert_eq!(carried.coreml_specialization, default.coreml_specialization);
            assert_eq!(carried.disable_mem_pattern, default.disable_mem_pattern);
            assert_eq!(carried.disabled_optimizers, default.disabled_optimizers);
            assert_eq!(carried.trt_options, default.trt_options);
        }
    }
}
