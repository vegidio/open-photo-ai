//! Moscow: a Restormer sharpener a front end offers first — the execution-provider profile measured for it. The
//! sharpening it runs is the shared [`filter`](crate::models::filter), with no guard.

// Deleting the model is deleting this directory plus the arms in `SharpenVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::{EpProfile, cpu_and_gpu_at_fp16};

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML off the Neural Engine
/// at FP16, and the provider defaults at FP32.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Ported from the reference's `moscow.go` (not re-measured here) — acceptable since a wrong CoreML-only,
    // FP16-only setting only costs speed, never a wrong image.
    //
    // Kept off the Neural Engine at FP16, same as Novgorod, Gothenburg and Malmö: all four share this Restormer
    // backbone, and the op mix that decides it is a property of the architecture rather than of any one checkpoint.
    // Its 44 blocks are mostly layer normalization, reshape and transpose around a channel-attention matmul, which is
    // not what the Neural Engine is built for, and CoreML takes the graph there anyway whenever its default compute
    // units permit.
    //
    // Measured by the reference end to end over its 640x640 sample on an Apple Silicon Mac it does not name, FP16:
    //
    //   MLComputeUnits          FP16, whole pipeline
    //   ALL (default)           5.067s
    //   CPUAndGPU               2.803s
    //
    // Taking the GPU away instead of the Neural Engine (CPUAndNeuralEngine) is the direct confirmation of which half
    // of the default costs: +59% per tile.
    //
    // Not declared at FP32: the reference declares it at FP16 only and records no FP32 figure, and CoreML bars an FP32
    // program from the Neural Engine anyway, so a setting there would restate a choice CoreML has already made.
    // Fast-prediction and sequential execution mode measured within run-to-run spread and are left at defaults.
    //
    // **The size of the win depends on the export.** The reference records the same switch as -22% per tile on a
    // single-partition build of this graph, against -45% on the export behind the figures above, without saying which
    // export is the one published. The sign is the same either way, so the setting stands. Re-measuring this model
    // requires first counting how many CoreML partitions the published graph compiles to — otherwise the figures here
    // and the new ones are not measuring the same thing.
    //
    // The WebGPU nodes at FP16 are correctness, not tuning, and nothing is wrong with the export: every tensor it
    // declares FP16 fits. What goes wrong is WebGPU's own — it accumulates the channel-attention `q·kᵀ` product and the
    // attention's normalizing means, each a sum over every pixel of the tile, in FP16, and the tile comes out at 23 dB
    // against FP32. Left to the CPU provider, the 68 of those that matter bring it to 67 dB. The largest list here, and
    // the costliest: measured on an M2 Max against the WebGPU plugin 0.4.0 and ONNX Runtime 1.30, per graph, +13.0% on
    // WebGPU, 532.0 ms to 601.2 ms. FP32 needs none.
    cpu_and_gpu_at_fp16(precision).with_webgpu_cpu_nodes_at_fp16(precision, WEBGPU_CPU_NODES_FP16)
}

/// The nodes WebGPU is kept off at FP16, as the graph names them — see `profile`.
const WEBGPU_CPU_NODES_FP16: &[&str] = &[
    "/encoder_level1/encoder_level1.0/attn/MatMul",
    "/encoder_level1/encoder_level1.1/attn/MatMul",
    "/encoder_level1/encoder_level1.2/attn/MatMul",
    "/encoder_level1/encoder_level1.3/attn/MatMul",
    "/encoder_level2/encoder_level2.0/attn/MatMul",
    "/encoder_level2/encoder_level2.1/attn/MatMul",
    "/encoder_level2/encoder_level2.2/attn/MatMul",
    "/encoder_level2/encoder_level2.3/attn/MatMul",
    "/encoder_level2/encoder_level2.4/attn/MatMul",
    "/encoder_level2/encoder_level2.5/attn/MatMul",
    "/decoder_level3/decoder_level3.2/attn/MatMul",
    "/decoder_level3/decoder_level3.4/attn/MatMul",
    "/decoder_level3/decoder_level3.5/attn/MatMul",
    "/decoder_level2/decoder_level2.0/attn/MatMul",
    "/decoder_level2/decoder_level2.1/attn/MatMul",
    "/decoder_level2/decoder_level2.2/attn/MatMul",
    "/decoder_level2/decoder_level2.3/attn/MatMul",
    "/decoder_level2/decoder_level2.4/attn/MatMul",
    "/decoder_level2/decoder_level2.5/attn/MatMul",
    "/decoder_level1/decoder_level1.0/attn/MatMul",
    "/decoder_level1/decoder_level1.1/attn/MatMul",
    "/decoder_level1/decoder_level1.2/attn/MatMul",
    "/decoder_level1/decoder_level1.3/attn/MatMul",
    "/refinement/refinement.0/attn/MatMul",
    "/refinement/refinement.1/attn/MatMul",
    "/refinement/refinement.2/attn/MatMul",
    "/refinement/refinement.3/attn/MatMul",
    "/encoder_level1/encoder_level1.0/attn/ReduceMean",
    "/encoder_level1/encoder_level1.0/attn/ReduceMean_1",
    "/encoder_level1/encoder_level1.1/attn/ReduceMean",
    "/encoder_level1/encoder_level1.1/attn/ReduceMean_1",
    "/encoder_level1/encoder_level1.2/attn/ReduceMean",
    "/encoder_level1/encoder_level1.2/attn/ReduceMean_1",
    "/encoder_level1/encoder_level1.3/attn/ReduceMean",
    "/encoder_level1/encoder_level1.3/attn/ReduceMean_1",
    "/encoder_level2/encoder_level2.0/attn/ReduceMean",
    "/encoder_level2/encoder_level2.0/attn/ReduceMean_1",
    "/encoder_level2/encoder_level2.1/attn/ReduceMean",
    "/encoder_level2/encoder_level2.1/attn/ReduceMean_1",
    "/encoder_level2/encoder_level2.5/attn/ReduceMean",
    "/encoder_level2/encoder_level2.5/attn/ReduceMean_1",
    "/encoder_level3/encoder_level3.0/attn/ReduceMean",
    "/encoder_level3/encoder_level3.0/attn/ReduceMean_1",
    "/decoder_level3/decoder_level3.0/attn/ReduceMean",
    "/decoder_level3/decoder_level3.0/attn/ReduceMean_1",
    "/decoder_level3/decoder_level3.1/attn/ReduceMean",
    "/decoder_level3/decoder_level3.1/attn/ReduceMean_1",
    "/decoder_level3/decoder_level3.2/attn/ReduceMean",
    "/decoder_level3/decoder_level3.2/attn/ReduceMean_1",
    "/decoder_level3/decoder_level3.3/attn/ReduceMean",
    "/decoder_level2/decoder_level2.0/attn/ReduceMean",
    "/decoder_level2/decoder_level2.0/attn/ReduceMean_1",
    "/decoder_level1/decoder_level1.0/attn/ReduceMean",
    "/decoder_level1/decoder_level1.0/attn/ReduceMean_1",
    "/decoder_level1/decoder_level1.1/attn/ReduceMean",
    "/decoder_level1/decoder_level1.1/attn/ReduceMean_1",
    "/decoder_level1/decoder_level1.2/attn/ReduceMean",
    "/decoder_level1/decoder_level1.2/attn/ReduceMean_1",
    "/decoder_level1/decoder_level1.3/attn/ReduceMean",
    "/decoder_level1/decoder_level1.3/attn/ReduceMean_1",
    "/refinement/refinement.0/attn/ReduceMean",
    "/refinement/refinement.0/attn/ReduceMean_1",
    "/refinement/refinement.1/attn/ReduceMean",
    "/refinement/refinement.1/attn/ReduceMean_1",
    "/refinement/refinement.2/attn/ReduceMean",
    "/refinement/refinement.2/attn/ReduceMean_1",
    "/refinement/refinement.3/attn/ReduceMean",
    "/refinement/refinement.3/attn/ReduceMean_1",
];

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::precision::FloatPrecision;
    use crate::models::sharpen::SharpenVariant;

    #[test]
    fn moscow_adds_its_webgpu_nodes_to_the_shared_fp16_profile() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break. What the shared profile holds is pinned once, beside it in `providers::profile`.
        assert_eq!(
            SharpenVariant::Moscow(FloatPrecision::Fp16).profile(),
            cpu_and_gpu_at_fp16(Precision::Fp16).with_webgpu_cpu_nodes_at_fp16(Precision::Fp16, WEBGPU_CPU_NODES_FP16)
        );
        assert_eq!(SharpenVariant::Moscow(FloatPrecision::Fp32).profile(), cpu_and_gpu_at_fp16(Precision::Fp32));
    }

    #[test]
    fn the_webgpu_list_is_the_68_nodes_measured() {
        // Pinned by count and kind (attention `MatMul`s and `ReduceMean`s): the plugin ignores a name it does not find,
        // so a list that went stale against a re-export would bring the FP16 error back with nothing failing.
        let nodes = WEBGPU_CPU_NODES_FP16;

        assert_eq!(nodes.len(), 68);
        assert!(
            nodes.iter().all(|node| node.ends_with("/attn/MatMul") || node.contains("/attn/ReduceMean")),
            "{nodes:?}"
        );
    }
}
