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
    cpu_and_gpu_at_fp16(precision)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::precision::FloatPrecision;
    use crate::models::sharpen::SharpenVariant;

    #[test]
    fn moscow_declares_the_shared_fp16_profile_through_its_variant() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break. What the shared profile holds is pinned once, beside it in `providers::profile`.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Moscow(precision).profile(),
                cpu_and_gpu_at_fp16(precision.into()),
                "{precision:?}"
            );
        }
    }
}
