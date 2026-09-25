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
    cpu_and_gpu_at_fp16(precision)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::precision::FloatPrecision;
    use crate::models::sharpen::SharpenVariant;

    #[test]
    fn novgorod_declares_the_shared_fp16_profile_through_its_variant() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break. What the shared profile holds is pinned once, beside it in `providers::profile`.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Novgorod(precision).profile(),
                cpu_and_gpu_at_fp16(precision.into()),
                "{precision:?}"
            );
        }
    }
}
