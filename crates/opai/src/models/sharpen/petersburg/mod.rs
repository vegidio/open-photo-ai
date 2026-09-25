//! Petersburg: the NAFNet sharpener — the magnitude past which its output is discarded, and the execution-provider
//! profile measured for it. The sharpening it runs is the shared [`filter`](crate::models::filter).

// Deleting the model is deleting this directory plus the arms in `SharpenVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::EpProfile;

// Petersburg is NAFNet, the architecture Stockholm is, and it shares the failure: on a tile unlike anything it was
// trained on its raw output can go to 1000 and beyond, and decoded it would be a solid saturated block in the middle
// of the photograph. Legitimate output is of order 1 — the graph reads and writes `[0, 1]` — so 3.0 sits safely above
// anything a working tile produces and far below the blow-up.
//
// **The threshold is the reference's, carried across on trust.** No photograph known to make Petersburg diverge has
// been observed in this project, so what is verified here is the mechanism — against a fake model made to explode,
// in `models::filter` — and not that 3.0 is where to catch it. The live check reports how many tiles the guard
// kept on the photograph it runs; one that trips it can be added as a fixture without the contract moving.
/// The magnitude past which a tile's raw output is treated as a blow-up and the tile keeps its own input.
pub(crate) const GUARD: f32 = 3.0;

/// The execution-provider tuning measured for this model, at the precision it carries: the provider defaults at both
/// precisions, because nothing measured beat them.
pub(crate) fn profile(_precision: Precision) -> EpProfile {
    // **Measured, and the default won.** Ported from the reference's `petersburg.go` (not re-measured here), whose
    // sweep is written down so that nobody pays for it again — and so that nobody "fills in" Moscow's setting because
    // the three models share a family.
    //
    // The fact that decides everything else: CoreML takes this graph as **one** partition and places all of it on the
    // GPU, at both precisions, without being told to. That is the opposite of Moscow and Novgorod, and the reason is
    // architectural. They are Restormers, whose blocks are normalization and transpose around a channel-attention
    // matmul, and CoreML reaches for the Neural Engine there and loses. NAFNet is a plain convolutional U-Net — 226
    // convolutions, half the estimated graph cost — and CoreML declines the Neural Engine on its own, so the default
    // compute units and `CpuAndGpu` are the same session.
    //
    // Measured by the reference on an M2 Max, macOS 26.6, ONNX Runtime 1.26, per 256x256 tile:
    //
    //   MLComputeUnits           FP32               FP16
    //   ALL (default)            34.4ms             34.3ms
    //   CPUAndGPU                34.2ms   (-0.5%)   34.4ms  (+0.2%)
    //   CPUAndNeuralEngine      248.1ms  (+622%)    37.0ms  (+7.7%)
    //   CPUOnly / CPU provider  471.9ms            468.6ms
    //
    // So the setting worth 45% on Moscow is worth nothing here, and declaring it would only restate a choice CoreML
    // has already made.
    //
    // Fast-prediction, sequential execution mode and low-precision GPU accumulation all measured within run-to-run
    // spread at both precisions, with the order alternated per round.
    //
    // One finding that is not a tuning result but changes what to recommend: the FP16 export is not faster. End to end
    // over the 640x640 sample it is 328ms against FP32's 328ms, because CoreML compiles both to the same GPU program.
    // Unlike Novgorod, where the precision choice is also a speed choice, FP16 here buys download size and nothing
    // else.
    //
    // Nothing is declared, at either precision. CUDA and TensorRT were not measured.
    EpProfile::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::precision::FloatPrecision;
    use crate::models::sharpen::SharpenVariant;
    use crate::providers::profile::{CoreMlComputeUnits, CoreMlSpecialization, ExecutionMode};

    #[test]
    fn the_guard_is_the_references_threshold() {
        // Pinned as a literal, for the reason `GUARD` gives: it is carried over rather than derived, so nothing else
        // would notice it moving.
        assert_eq!(GUARD, 3.0);

        for precision in FloatPrecision::ALL {
            assert_eq!(SharpenVariant::Petersburg(precision).guard(), Some(GUARD), "{precision:?}");
        }
    }

    // The measured outcome, pinned in the shape of Gothenburg's four so that it reads as a measurement rather than as
    // a profile nobody got round to. Each is asked through the variant, whose match is the half a refactor can break.

    #[test]
    fn petersburg_is_left_on_coremls_own_compute_units_at_both_precisions() {
        // The one a reader is most likely to "fix" is `CpuAndGpu`, copied across from the family's two Restormers. It
        // compiles to the same session here, so it would buy nothing and claim a finding that was never made.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Petersburg(precision).profile().coreml_compute_units,
                CoreMlComputeUnits::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn petersburg_does_not_ask_for_one_node_at_a_time_at_either_precision() {
        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Petersburg(precision).profile().execution_mode,
                ExecutionMode::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn petersburg_does_not_ask_coreml_for_fast_prediction() {
        for precision in FloatPrecision::ALL {
            assert_eq!(
                SharpenVariant::Petersburg(precision).profile().coreml_specialization,
                CoreMlSpecialization::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn petersburg_declares_nothing_at_either_precision() {
        // Compared as whole profiles, so a field added to `EpProfile` later is covered by existing.
        for precision in FloatPrecision::ALL {
            assert_eq!(SharpenVariant::Petersburg(precision).profile(), EpProfile::default(), "{precision:?}");
        }
    }
}
