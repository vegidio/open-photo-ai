//! Novgorod: a Restormer sharpener — the execution-provider profile measured for it. The sharpening it runs is the
//! shared [`filter`](crate::models::filter), with no guard.

// Deleting the model is deleting this directory plus the arms in `SharpenVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::{EpProfile, cpu_and_gpu_at_fp16};

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML off the Neural Engine
/// at FP16, and the provider defaults at FP32.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Ported from the reference's `novgorod.go` (not re-measured here) — acceptable since a wrong CoreML-only,
    // FP16-only setting only costs speed, never a wrong image.
    //
    // Kept off the Neural Engine at FP16. Restormer is a transformer, not the convolutional stack the Neural Engine is
    // built for: its 44 blocks are mostly layer normalization, reshape and transpose around a channel-attention
    // matmul. Left to its default compute units, CoreML takes the Neural Engine anyway and spends more time crossing
    // on and off it than it saves.
    //
    // Measured by the reference on an M2 Max, per 256x256 tile:
    //
    //   FP16, ALL (default)     180ms
    //   FP16, CPUAndGPU         143ms
    //   FP32                    152ms
    //
    // That difference is what decides the precision. At 143ms FP16 is the fastest way to run this model; at 180ms it
    // is slower than FP32's 152ms, so a user picking FP16 for speed would get the opposite.
    //
    // Not declared at FP32, where CoreML bars the program from the Neural Engine anyway. Moscow's figures are not
    // evidence for these, nor these for Moscow's — each was measured on its own, and two checkpoints of one
    // architecture agreeing is a finding, not a licence to copy one's settings to the next.
    //
    // The WebGPU nodes at FP16 are correctness, not tuning, and nothing is wrong with the export: every tensor it
    // declares FP16 fits. What goes wrong is WebGPU's own — it accumulates the attention's normalizing means, sums over
    // every pixel of the tile, in FP16 until they read as zero, and the division after them turns the attention to
    // infinities; the tile comes out at 17 dB against FP32. Left to the CPU provider, the 54 means and `q·kᵀ` products
    // that matter bring it to 72 dB. Moscow's list is not this one — each was found on its own graph. Measured on an M2
    // Max against the WebGPU plugin 0.4.0 and ONNX Runtime 1.30, per graph: +11.6% on WebGPU, 524.5 ms to 585.4 ms.
    // FP32 needs none.
    cpu_and_gpu_at_fp16(precision).with_webgpu_cpu_nodes_at_fp16(precision, WEBGPU_CPU_NODES_FP16)
}

/// The nodes WebGPU is kept off at FP16, as the graph names them — see `profile`.
const WEBGPU_CPU_NODES_FP16: &[&str] = &[
    "/encoder_level1/encoder_level1.0/attn/MatMul",
    "/encoder_level1/encoder_level1.1/attn/MatMul",
    "/encoder_level1/encoder_level1.2/attn/MatMul",
    "/encoder_level1/encoder_level1.3/attn/MatMul",
    "/encoder_level2/encoder_level2.3/attn/MatMul",
    "/decoder_level3/decoder_level3.4/attn/MatMul",
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
    "/decoder_level3/decoder_level3.0/attn/ReduceMean",
    "/decoder_level3/decoder_level3.0/attn/ReduceMean_1",
    "/decoder_level3/decoder_level3.4/attn/ReduceMean",
    "/decoder_level3/decoder_level3.4/attn/ReduceMean_1",
    "/decoder_level3/decoder_level3.5/attn/ReduceMean",
    "/decoder_level3/decoder_level3.5/attn/ReduceMean_1",
    "/decoder_level2/decoder_level2.0/attn/ReduceMean",
    "/decoder_level2/decoder_level2.0/attn/ReduceMean_1",
    "/decoder_level2/decoder_level2.1/attn/ReduceMean",
    "/decoder_level2/decoder_level2.1/attn/ReduceMean_1",
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
    fn novgorod_adds_its_webgpu_nodes_to_the_shared_fp16_profile() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break. What the shared profile holds is pinned once, beside it in `providers::profile`.
        assert_eq!(
            SharpenVariant::Novgorod(FloatPrecision::Fp16).profile(),
            cpu_and_gpu_at_fp16(Precision::Fp16).with_webgpu_cpu_nodes_at_fp16(Precision::Fp16, WEBGPU_CPU_NODES_FP16)
        );
        assert_eq!(SharpenVariant::Novgorod(FloatPrecision::Fp32).profile(), cpu_and_gpu_at_fp16(Precision::Fp32));
    }

    #[test]
    fn the_webgpu_list_is_the_54_nodes_measured() {
        // Pinned by count and kind (attention `MatMul`s and `ReduceMean`s): the plugin ignores a name it does not find,
        // so a list that went stale against a re-export would bring the FP16 error back with nothing failing.
        let nodes = WEBGPU_CPU_NODES_FP16;

        assert_eq!(nodes.len(), 54);
        assert!(
            nodes.iter().all(|node| node.ends_with("/attn/MatMul") || node.contains("/attn/ReduceMean")),
            "{nodes:?}"
        );
    }
}
