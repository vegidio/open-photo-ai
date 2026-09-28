//! Malmö: a Restormer denoiser — the execution-provider profile measured for it. The denoising it runs is the
//! shared [`filter`](crate::models::filter), with no guard.

// Deleting the model is deleting this directory plus the arms in `DenoiseVariant` that name it.
//
// `profile`'s figures were measured against a re-export that fixes two CoreML incompatibilities in the stock
// Restormer graph, worth about -62% per tile combined (and without them FP16 is 49% slower than FP32):
//
// - 88 `ReduceL2` (from `F.normalize` in attention, 2 per block x 44 blocks): CoreML has no builder for it, so it's
//   rewritten as `Pow`/`ReduceSum`/`Sqrt`/`Clip`/`Div`, which it does support.
// - 6 `Reshape` + 3 `Transpose` (from the three `Conv` + `PixelUnshuffle(2)` downsample layers): these decompose to
//   rank-6 ops CoreML refuses. Instead the pixel-unshuffle is folded into the convolution itself, by scattering its
//   3x3 kernel into a zeroed 4x4 kernel per output offset and running at stride 2 — reproducing the shuffle exactly
//   in one node.
//
// Both rewrites are exact in real arithmetic. A third is in this export but not part of the recipe: LayerNorm over the
// channel axis in NCHW rather than through `to_3d`/`to_4d`. It's bit-identical and 176 nodes smaller but measures as a
// tie, and is kept only because `profile`'s figures were measured against it.

use crate::models::precision::Precision;
use crate::providers::profile::{EpProfile, cpu_and_gpu_at_fp16};

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML off the Neural Engine
/// at FP16, and the provider defaults at FP32.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Ported from the reference's `malmo.go` (not re-measured here, unlike Paris/Lyon) — acceptable since a wrong
    // CoreML-only, FP16-only setting only costs speed, never a wrong image.
    //
    // Kept off the Neural Engine at FP16, same as Gothenburg/Moscow/Novgorod: all four share this Restormer backbone.
    // Gothenburg's figures are not evidence for these, nor these for its — each was measured on its own, and two
    // checkpoints agreeing is a finding, not a licence to copy one's settings to the next.
    //
    // Measured on an M2 Max, macOS 26.6, ORT 1.26, per 256x256 tile, against the re-export above:
    //
    //   MLComputeUnits          FP32                FP16
    //   ALL (default)           147.0ms             197.9ms
    //   CPUAndGPU               146.8ms   (-0.1%)   138.6ms  (-29.9%)
    //   CPUAndNeuralEngine     1072.6ms  (+630%)    302.0ms  (+52.6%)
    //
    // Not declared at FP32 (-0.1% there). Fast-prediction and sequential execution mode measured within spread and
    // are left at defaults.
    //
    // Re-measuring this model requires first confirming its graph is still a single CoreML partition — without the
    // two rewrites above it's 92, and every figure here would then reflect partition handoff, not compute units.
    //
    // The WebGPU nodes at FP16 are correctness, not tuning. The published FP16 export keeps its attention L2 norms in
    // FP32 — their sums reach 1.4 million, which FP16 cannot hold, and the CPU and CoreML only hid that by promoting
    // them on their own. What WebGPU still gets wrong after that is its own: it accumulates the channel-attention
    // `q·kᵀ` product, a sum over every pixel of the tile, in FP16, which leaves the tile at 42 dB against FP32. Left to
    // the CPU provider, the 20 of those that matter bring it to 71 dB. Gothenburg's list is not this one — each was
    // found on its own graph. Measured on an M2 Max against the WebGPU plugin 0.4.0 and ONNX Runtime 1.30, per graph:
    // +3.3% on WebGPU, 537.7 ms to 555.7 ms. FP32 needs none.
    cpu_and_gpu_at_fp16(precision).with_webgpu_cpu_nodes_at_fp16(precision, WEBGPU_CPU_NODES_FP16)
}

/// The nodes WebGPU is kept off at FP16, as the graph names them — see `profile`.
const WEBGPU_CPU_NODES_FP16: &[&str] = &[
    "/encoder_level1/encoder_level1.0/attn/MatMul",
    "/encoder_level1/encoder_level1.1/attn/MatMul",
    "/encoder_level1/encoder_level1.2/attn/MatMul",
    "/encoder_level1/encoder_level1.3/attn/MatMul",
    "/encoder_level2/encoder_level2.1/attn/MatMul",
    "/encoder_level2/encoder_level2.3/attn/MatMul",
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
    fn malmo_adds_its_webgpu_nodes_to_the_shared_fp16_profile() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break. What the shared profile holds is pinned once, beside it in `providers::profile`.
        assert_eq!(
            DenoiseVariant::Malmo(FloatPrecision::Fp16).profile(),
            cpu_and_gpu_at_fp16(Precision::Fp16).with_webgpu_cpu_nodes_at_fp16(Precision::Fp16, WEBGPU_CPU_NODES_FP16)
        );
        assert_eq!(DenoiseVariant::Malmo(FloatPrecision::Fp32).profile(), cpu_and_gpu_at_fp16(Precision::Fp32));
    }

    #[test]
    fn the_webgpu_list_is_the_20_nodes_measured() {
        // Pinned by count and kind (attention `MatMul`s): the plugin ignores a name it does not find, so a list that
        // went stale against a re-export would bring the FP16 error back with nothing failing.
        let nodes = WEBGPU_CPU_NODES_FP16;

        assert_eq!(nodes.len(), 20);
        assert!(nodes.iter().all(|node| node.ends_with("/attn/MatMul")), "{nodes:?}");
    }
}
