//! Novgorod: a Restormer sharpener — the execution-provider profile measured for it. The sharpening it runs is the
//! shared [`filter`](crate::models::filter), with no guard.

// Deleting the model is deleting this directory plus the arms in `SharpenVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

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
    match precision {
        Precision::Fp16 => EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() },
        _ => EpProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::precision::FloatPrecision;
    use crate::models::sharpen::SharpenVariant;
    use crate::providers::profile::{CoreMlSpecialization, ExecutionMode};

    // Gothenburg's four tests on this profile, ported. Each is asked through the variant rather than of `profile`
    // directly, because the variant's match is the half a refactor can break.

    #[test]
    fn novgorod_is_kept_off_the_neural_engine_at_fp16() {
        // Worth 37ms a tile, and the difference between FP16 being the fast precision and the slow one. Nothing else
        // asserts it: dropping the setting would still load the model and still return the right image.
        assert_eq!(
            SharpenVariant::Novgorod(FloatPrecision::Fp16).profile().coreml_compute_units,
            CoreMlComputeUnits::CpuAndGpu
        );
    }

    #[test]
    fn novgorod_leaves_fp32_on_the_default_compute_units() {
        // Not load-bearing for speed — CoreML keeps an FP32 program off the Neural Engine anyway — but load-bearing
        // for not inviting the next editor to widen a setting that was never measured to help there.
        assert_eq!(
            SharpenVariant::Novgorod(FloatPrecision::Fp32).profile().coreml_compute_units,
            CoreMlComputeUnits::default()
        );
    }

    #[test]
    fn novgorod_does_not_ask_for_one_node_at_a_time_at_either_precision() {
        // Pinned so a sweep of another model does not get copied onto this one.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Novgorod(precision).profile().execution_mode,
                ExecutionMode::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn novgorod_names_nothing_else() {
        // Compared as whole profiles, so a field added to `EpProfile` later is covered by existing.
        assert_eq!(
            SharpenVariant::Novgorod(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() }
        );
        assert_eq!(SharpenVariant::Novgorod(FloatPrecision::Fp32).profile(), EpProfile::default());

        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Novgorod(precision).profile().coreml_specialization,
                CoreMlSpecialization::default(),
                "{precision:?}"
            );
        }
    }
}
