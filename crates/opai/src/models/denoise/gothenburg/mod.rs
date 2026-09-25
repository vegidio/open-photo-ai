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
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

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

    // The reference's four tests on this profile, ported. Each is asked through the variant rather than of `profile`
    // directly, because the variant's match is the half a refactor can break.

    #[test]
    fn gothenburg_is_kept_off_the_neural_engine_at_fp16() {
        // Worth 25.7% per tile, and nothing else asserts it: dropping the setting would still load the model and
        // still return the right image, a third slower.
        assert_eq!(
            DenoiseVariant::Gothenburg(FloatPrecision::Fp16).profile().coreml_compute_units,
            CoreMlComputeUnits::CpuAndGpu
        );
    }

    #[test]
    fn gothenburg_leaves_fp32_on_the_default_compute_units() {
        // Not load-bearing for speed — the two compile to one session at FP32 — but load-bearing for not inviting
        // the next editor to widen a setting that was never measured to help there.
        assert_eq!(
            DenoiseVariant::Gothenburg(FloatPrecision::Fp32).profile().coreml_compute_units,
            CoreMlComputeUnits::default()
        );
    }

    #[test]
    fn gothenburg_does_not_ask_for_one_node_at_a_time_at_either_precision() {
        // Sequential is the largest CoreML win on several graphs here, and this is not one of them. Pinned so a sweep
        // of another model does not get copied onto this one.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                DenoiseVariant::Gothenburg(precision).profile().execution_mode,
                ExecutionMode::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn gothenburg_names_nothing_else() {
        // Every other option measured inside run-to-run spread. Compared as whole profiles, so a field added to
        // `EpProfile` later is covered by existing.
        assert_eq!(
            DenoiseVariant::Gothenburg(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() }
        );
        assert_eq!(DenoiseVariant::Gothenburg(FloatPrecision::Fp32).profile(), EpProfile::default());

        for precision in FloatPrecision::ALL {
            assert_eq!(
                DenoiseVariant::Gothenburg(precision).profile().coreml_specialization,
                CoreMlSpecialization::default(),
                "{precision:?}"
            );
        }
    }
}
