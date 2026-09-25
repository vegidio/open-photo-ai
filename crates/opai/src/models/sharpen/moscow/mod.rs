//! Moscow: a Restormer sharpener a front end offers first — the execution-provider profile measured for it. The
//! sharpening it runs is the shared [`filter`](crate::models::filter), with no guard.

// Deleting the model is deleting this directory plus the arms in `SharpenVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

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
    fn moscow_is_kept_off_the_neural_engine_at_fp16() {
        // Nearly half the end-to-end time on the reference's measurement, and nothing else asserts it: dropping the
        // setting would still load the model and still return the right image, slower.
        assert_eq!(
            SharpenVariant::Moscow(FloatPrecision::Fp16).profile().coreml_compute_units,
            CoreMlComputeUnits::CpuAndGpu
        );
    }

    #[test]
    fn moscow_leaves_fp32_on_the_default_compute_units() {
        // Not load-bearing for speed — CoreML keeps an FP32 program off the Neural Engine anyway — but load-bearing
        // for not inviting the next editor to widen a setting that was never measured to help there.
        assert_eq!(
            SharpenVariant::Moscow(FloatPrecision::Fp32).profile().coreml_compute_units,
            CoreMlComputeUnits::default()
        );
    }

    #[test]
    fn moscow_does_not_ask_for_one_node_at_a_time_at_either_precision() {
        // Measured within spread. Pinned so a sweep of another model does not get copied onto this one.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Moscow(precision).profile().execution_mode,
                ExecutionMode::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn moscow_names_nothing_else() {
        // Every other option measured inside run-to-run spread. Compared as whole profiles, so a field added to
        // `EpProfile` later is covered by existing.
        assert_eq!(
            SharpenVariant::Moscow(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() }
        );
        assert_eq!(SharpenVariant::Moscow(FloatPrecision::Fp32).profile(), EpProfile::default());

        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Moscow(precision).profile().coreml_specialization,
                CoreMlSpecialization::default(),
                "{precision:?}"
            );
        }
    }
}
