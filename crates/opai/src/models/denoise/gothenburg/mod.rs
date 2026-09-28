//! Gothenburg: a Restormer denoiser — the execution-provider profile measured for it. The denoising it runs is the
//! shared [`filter`](crate::models::filter), with no guard.

// Deleting the model is deleting this directory plus the arms in `DenoiseVariant` that name it.
//
// `profile`'s figures were measured against a re-export that fixes two CoreML incompatibilities in the stock
// Restormer graph, worth -58.6% per tile combined (and without them FP16 is 57% slower than FP32):
//
// - 88 `ReduceL2` (from `F.normalize` in attention, 2 per block x 44 blocks): CoreML has no builder for it, so it's
//   rewritten as `Pow`/`ReduceSum`/`Sqrt`/`Clip`/`Div`, which it does support.
// - 6 `Reshape` + 3 `Transpose` (from the three `Conv` + `PixelUnshuffle(2)` downsample layers): these decompose to
//   rank-6 ops CoreML refuses, and a rank-5 rewrite doesn't help either since ORT's own builders reject it too.
//   Instead the pixel-unshuffle is folded into the convolution itself, by scattering its 3x3 kernel into a zeroed
//   4x4 kernel per output offset and running at stride 2 — reproducing the shuffle exactly in one node.
//
// Both rewrites are exact in real arithmetic (1.4e-06 vs. the stock architecture on the same checkpoint).

use crate::models::precision::Precision;
use crate::providers::profile::{EpProfile, cpu_and_gpu_at_fp16};

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML off the Neural Engine
/// at FP16, and the provider defaults at FP32.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Ported from the reference's `gothenburg.go` (not re-measured here, unlike Paris/Lyon) — acceptable since a
    // wrong CoreML-only, FP16-only setting only costs speed, never a wrong image.
    //
    // Kept off the Neural Engine at FP16, same as Moscow/Novgorod: all three share this Restormer backbone, whose
    // op mix (layer norm, reshape/transpose around channel-attention matmuls) isn't Neural-Engine-friendly, and
    // CoreML routes it there anyway by default.
    //
    // Measured on an M2 Max, macOS 26.6, ORT 1.26, per 256x256 tile, against the re-export above:
    //
    //   MLComputeUnits          FP32               FP16
    //   ALL (default)           142.5ms            179.4ms
    //   CPUAndGPU               142.7ms  (+0.1%)   133.3ms  (-25.7%)
    //   CPUAndNeuralEngine      959.5ms  (+571%)   292.7ms  (+46.7%)
    //
    // Not declared at FP32 (+0.1% there — CoreML bars FP32 from the Neural Engine anyway, so both configs compile
    // to one session).
    //
    // Sequential execution mode gains nothing at either precision (-0.4% FP32, +2.9% FP16): CoreML fuses this graph
    // into one node, leaving the inter-op pool nothing to schedule. Fast-prediction and low-precision GPU
    // accumulation measured within spread and are left at defaults.
    //
    // Re-measuring this model requires first confirming its graph is still a single CoreML partition — without the
    // two rewrites above it's 48, and every figure here would then reflect partition handoff, not compute units.
    //
    // The WebGPU nodes at FP16 are correctness, not tuning. The published FP16 export keeps its attention L2 norms in
    // FP32 — their sums reach 0.67 million, which FP16 cannot hold, and the CPU and CoreML only hid that by promoting
    // them on their own. What WebGPU still gets wrong after that is its own: it accumulates the channel-attention
    // `q·kᵀ` product, a sum over every pixel of the tile, in FP16, which leaves the tile at 51 dB against FP32. Left to
    // the CPU provider, the 22 of those that matter bring it to 71 dB. Measured on an M2 Max against the WebGPU plugin
    // 0.4.0 and ONNX Runtime 1.30, per graph: +3.6% on WebGPU, 532.9 ms to 552.2 ms. FP32 needs none.
    cpu_and_gpu_at_fp16(precision).with_webgpu_cpu_nodes_at_fp16(precision, WEBGPU_CPU_NODES_FP16)
}

/// The nodes WebGPU is kept off at FP16, as the graph names them — see `profile`.
const WEBGPU_CPU_NODES_FP16: &[&str] = &[
    "/encoder_level1/encoder_level1.0/attn/MatMul",
    "/encoder_level1/encoder_level1.1/attn/MatMul",
    "/encoder_level1/encoder_level1.2/attn/MatMul",
    "/encoder_level2/encoder_level2.0/attn/MatMul",
    "/encoder_level2/encoder_level2.1/attn/MatMul",
    "/encoder_level2/encoder_level2.2/attn/MatMul",
    "/encoder_level2/encoder_level2.3/attn/MatMul",
    "/encoder_level2/encoder_level2.5/attn/MatMul",
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
];

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::denoise::DenoiseVariant;
    use crate::models::precision::FloatPrecision;

    #[test]
    fn gothenburg_adds_its_webgpu_nodes_to_the_shared_fp16_profile() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break. What the shared profile holds is pinned once, beside it in `providers::profile`.
        assert_eq!(
            DenoiseVariant::Gothenburg(FloatPrecision::Fp16).profile(),
            cpu_and_gpu_at_fp16(Precision::Fp16).with_webgpu_cpu_nodes_at_fp16(Precision::Fp16, WEBGPU_CPU_NODES_FP16)
        );
        assert_eq!(DenoiseVariant::Gothenburg(FloatPrecision::Fp32).profile(), cpu_and_gpu_at_fp16(Precision::Fp32));
    }

    #[test]
    fn the_webgpu_list_is_the_22_nodes_measured() {
        // Pinned by count and kind (attention `MatMul`s): the plugin ignores a name it does not find, so a list that
        // went stale against a re-export would bring the FP16 error back with nothing failing.
        let nodes = WEBGPU_CPU_NODES_FP16;

        assert_eq!(nodes.len(), 22);
        assert!(nodes.iter().all(|node| node.ends_with("/attn/MatMul")), "{nodes:?}");
    }
}
