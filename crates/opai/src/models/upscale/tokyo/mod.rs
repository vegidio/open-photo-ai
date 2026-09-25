//! Tokyo: a SwinIR publishing native 4x weights only.

use super::conv::{ConvVariant, FOUR_ONLY, PassShape};
use super::variant::UpscaleVariant;
use imaging::TileGeometry;
use imaging::tensor::Normalisation;

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile, ExecutionMode};

/// Tokyo, native 4x weights only.
pub(crate) static TOKYO: ConvVariant = ConvVariant {
    codename: "tokyo",
    label: "Tokyo",
    buckets: FOUR_ONLY,
    profile,
    variant: UpscaleVariant::Tokyo,
    shape: PassShape { tiles: TileGeometry::DEFAULT, range: Normalisation::Unit },
};

/// The execution-provider tuning measured for this model, at the precision it carries.
fn profile(_precision: Precision) -> EpProfile {
    // Both precisions, and that is the difference from Kyoto and Saitama: what excludes the Neural Engine is the
    // graph's window attention, which is structural and survives the export precision. The CoreML numbers below say
    // the same thing the reference's do about *where* the margin is — FP16 earns it, FP32 is a wash — and the FP32 arm
    // is written anyway, because a setting that is free at one precision and worth 23% at the other is not worth
    // splitting to save nothing. Tokyo's FP16 export happens to carry the `Resize` FP32 islands `kyoto::profile`
    // describes — 334 `Cast` nodes, four of them wrapping the two upsamples, one in and one out of each — and for once
    // it does not matter, because the Neural Engine is excluded either way.
    EpProfile {
        // The CPU and the GPU: the one compute-unit setting here chosen for what it **excludes** rather than for what
        // it prefers. This is a SwinIR — 2682 nodes, of which 776 reshapes, 379 slices, 344 transposes and 110 layer
        // normalizations against just 36 convolutions — which is the op mix the Neural Engine handles worst.
        //
        // Asking for that unit produces `_ANECompiler : ANECCompile() FAILED`, one per transformer block, and the
        // failures are not free: the reference's `CPUAndNeuralEngine` session took **1067 seconds to build** and then
        // ran **22x slower than the GPU**, while `SpecializationStrategy=FastPrediction`, which routes through the same
        // compiler, wedged the process at 11.5 GB resident. `CPUAndGPU` is the one setting that cannot reach that
        // compiler.
        //
        // The margin against the default, re-measured here on an M2 Max against ONNX Runtime 1.26 with the
        // compiled-model cache cleared before each build:
        //
        //   tokyo 4x, CoreML, 640x640 in, fresh compile     first build     median run
        //     FP16   ALL (default)                             48.3s          18.43s
        //     FP16   CPUAndGPU                                  36.2s          14.19s     -23%
        //     FP32   ALL (default)                              41.2s          15.51s
        //     FP32   CPUAndGPU                                  32.0s          16.11s     +3.9%
        //
        // Same direction as the reference's 1.67x at FP16 and wash at FP32, at a smaller margin, and a quarter off the
        // first session build besides. Worth knowing for anyone re-running this: `ALL` did **not** reproduce the
        // pathology on this runtime — no `ANECompile` record reached either `ort`'s forwarded C++ diagnostics or the
        // system log, and the build stayed in the tens of seconds. That is consistent with the reference rather than
        // against it, since its figures come from *forcing* the unit; under `ALL` the planner appears simply not to
        // choose the Neural Engine for this graph. It does mean `ALL`'s safety here rests on a planner decision rather
        // than on anything declared, which is the reason to keep declaring `CPUAndGPU`.
        coreml_compute_units: CoreMlComputeUnits::CpuAndGpu,
        // `Sequential` rests on CUDA figures nobody here can reproduce — -26.9% at FP32 and -18.1% at FP16 — adopted
        // because the mechanism reads off the graph rather than off the provider: 2682 nodes arranged as 54
        // window-attention blocks in a chain give the inter-op pool no two independent nodes to hand out, so all it can
        // do is charge a thread handoff per node, and it charges it per `Run` rather than once. Output is bit-identical in both
        // modes, which makes this a setting rather than a trade. Under CoreML the question cannot arise at all, since
        // the provider takes all 2682 nodes as one partition.
        //
        // What *is* checkable here is that the graph's shape is what decides the answer, and it checks out. Run the
        // same comparison against Kyoto on the CPU provider — an RRDBNet whose dense blocks feed each concatenation
        // from several branches at once, which is the wide independent structure the inter-op pool exists for — and
        // the sign flips:
        //
        //   FP32, CPU provider, 640x640 in, median run     parallel   sequential
        //     tokyo   (chained window attention)            133.5s      101.2s     -24.2%
        //     kyoto   (dense RRDB blocks)                    62.5s       64.6s      +3.3%
        //
        // Two graphs, one setting, opposite answers — and Tokyo's -24.2% on a provider that was never the argument
        // lands within a couple of points of the -26.9% CUDA figure that was. That is what makes this an inherited
        // number with a locally verified mechanism behind it rather than an inherited number alone, which is the most
        // the hardware here can establish.
        execution_mode: ExecutionMode::Sequential,
        // Measured and rejected, recorded because the alternative is that the next reader with a CUDA box re-runs
        // every one of them:
        //
        // - `cuda_prefer_nhwc` **loses at both precisions**, FP16 included. NHWC earns its keep by reaching cuDNN's
        //   FP16 tensor-core kernels, and 36 convolutions are not enough of this graph to repay layout conversions
        //   threaded through 344 transposes. Athens sets it and is also transformer-bearing, which is why this is
        //   worth stating rather than leaving to be inferred.
        // - `fuse_conv_bias=1` is worse than a loss: garbage at FP32 (max|d| 2.9e+07) and a cuDNN
        //   `HEURISTIC_QUERY_FAILED` at FP16.
        // - `trt_fp16_enable` is worth 2.66x on the FP32 export and is rejected **on principle**. It does not produce a
        //   faster FP32 model, it produces the FP16 model under the FP32 model's name, and on TensorRT alone — so the
        //   same operation would carry a different precision depending on which provider the machine resolved to. The
        //   honest way to take the 2.66x is to select the FP16 model.
        // - `trt_bf16_enable` is that same trade taken badly: -55.2% on the FP32 export at five times FP16's
        //   deviation, and +23.6% on the FP16 export, where it is a plain loss.
        // - Nothing in the CUDA or TensorRT option sets moves this graph either. `EXHAUSTIVE` ties `HEURISTIC` on the
        //   convolution algorithm search, because there are only 36 `Conv` nodes among 2682 to search over, and TF32
        //   is worth 4.7% at FP32 across the 216 `Gemm`s and 108 `MatMul`s. TensorRT has nothing left to tune
        //   regardless: it swallows all 2682 nodes into a single engine, so every builder setting lands on the same
        //   fused engine.
        ..EpProfile::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::precision::FloatPrecision;
    use crate::models::upscale::UpscaleVariant;

    #[test]
    fn tokyo_declares_the_same_two_settings_at_both_precisions() {
        // The precision split the other two variants have would be wrong here: the Neural Engine is excluded because
        // the ANE compiler cannot compile window attention, which is the graph's shape rather than its export type.
        for precision in FloatPrecision::ALL {
            let tokyo = UpscaleVariant::Tokyo(precision).profile();

            assert_eq!(tokyo.coreml_compute_units, CoreMlComputeUnits::CpuAndGpu, "at {precision:?}");
            assert_eq!(tokyo.execution_mode, ExecutionMode::Sequential, "at {precision:?}");
            // And nothing else — `cuda_prefer_nhwc` and `fuse_conv_bias` were both measured and both rejected.
            assert_eq!(
                tokyo,
                EpProfile {
                    coreml_compute_units: CoreMlComputeUnits::CpuAndGpu,
                    execution_mode: ExecutionMode::Sequential,
                    ..EpProfile::default()
                },
                "at {precision:?}"
            );
        }
    }
}
