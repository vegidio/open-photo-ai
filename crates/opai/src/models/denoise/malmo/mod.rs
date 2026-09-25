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
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

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
    match precision {
        Precision::Fp16 => EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() },
        _ => EpProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::denoise::DenoiseVariant;
    use crate::models::precision::FloatPrecision;
    use crate::providers::profile::{CoreMlSpecialization, ExecutionMode};

    // Gothenburg's four, held separately here. The reference pinned only Gothenburg's, but the two profiles are the
    // same claim, and an unpinned copy is the one a sweep of another model would overwrite.

    #[test]
    fn malmo_is_kept_off_the_neural_engine_at_fp16() {
        // Worth 29.9% per tile, and nothing else asserts it.
        assert_eq!(
            DenoiseVariant::Malmo(FloatPrecision::Fp16).profile().coreml_compute_units,
            CoreMlComputeUnits::CpuAndGpu
        );
    }

    #[test]
    fn malmo_leaves_fp32_on_the_default_compute_units() {
        assert_eq!(
            DenoiseVariant::Malmo(FloatPrecision::Fp32).profile().coreml_compute_units,
            CoreMlComputeUnits::default()
        );
    }

    #[test]
    fn malmo_does_not_ask_for_one_node_at_a_time_at_either_precision() {
        for precision in FloatPrecision::ALL {
            assert_eq!(
                DenoiseVariant::Malmo(precision).profile().execution_mode,
                ExecutionMode::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn malmo_names_nothing_else() {
        assert_eq!(
            DenoiseVariant::Malmo(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() }
        );
        assert_eq!(DenoiseVariant::Malmo(FloatPrecision::Fp32).profile(), EpProfile::default());

        for precision in FloatPrecision::ALL {
            assert_eq!(
                DenoiseVariant::Malmo(precision).profile().coreml_specialization,
                CoreMlSpecialization::default(),
                "{precision:?}"
            );
        }
    }
}
