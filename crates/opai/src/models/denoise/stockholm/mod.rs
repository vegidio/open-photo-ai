//! Stockholm: the NAFNet denoiser a front end offers first — the magnitude past which its output is discarded, and the
//! execution-provider profile measured for it. The denoising it runs is the shared
//! [`filter`](crate::models::filter).

// Deleting the model is deleting this directory plus the arms in `DenoiseVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::EpProfile;

// Stockholm is NAFNet, which occasionally blows up numerically on a tile unlike anything it was trained on: its raw
// output on that tile goes to 1000 and beyond, and decoded it would be a solid saturated block in the middle of the
// photograph. Legitimate output is of order 1 — the graph reads and writes `[0, 1]` — so 3.0 sits safely above
// anything a working tile produces and far below the blow-up.
//
// **The threshold is the reference's, carried across on trust.** No photograph known to make Stockholm diverge has
// been observed in this project, so what is verified here is the mechanism — against a fake model made to explode,
// in `models::filter` — and not that 3.0 is where to catch it. The live check reports how many tiles the guard
// kept on the photograph it runs; one that trips it can be added as a fixture without the contract moving.
/// The magnitude past which a tile's raw output is treated as a blow-up and the tile keeps its own input.
pub(crate) const GUARD: f32 = 3.0;

/// The execution-provider tuning measured for this model, at the precision it carries: the provider defaults at both
/// precisions, because nothing measured beat them.
pub(crate) fn profile(_precision: Precision) -> EpProfile {
    // **Measured, and the default won.** Nobody had measured Stockholm, in the reference or here, and it is NAFNet
    // rather than the Restormer Gothenburg and Malmö are, so their findings were not carried over: `CpuAndGpu` was
    // tried because it is a standard candidate for any CoreML graph, not because a Restormer won with it.
    //
    // Swept on this project's build with `perf -- stockholm --provider <p> --precision <q> -n 15 -w 2`, one candidate
    // per build, on an M2 Max (64 GB, macOS 26.6) against the pinned ONNX Runtime 1.26, over the embedded 640x640
    // sample. **Whole-pipeline medians** — nine 256x256 tiles and the blend — with the graph measured as published:
    //
    //   stockholm, median of 15 runs          CoreML FP32   CoreML FP16   CPU FP32    CPU FP16
    //     default                              343.3ms       327.0ms      4888.3ms    4889.4ms
    //     default, again after the sweep       344.6ms       327.6ms      4849.0ms    5116.6ms
    //     CoreML CPUAndGPU                     452.4ms       341.3ms
    //     CoreML CPUAndNeuralEngine           2704.4ms       444.4ms
    //     sequential execution                 354.6ms       337.7ms      4825.2ms    4820.2ms
    //     CoreML FastPrediction                417.2ms       330.7ms
    //
    // The default measured twice brackets the run-to-run spread: under 0.5% on CoreML, and up to 4.6% on the CPU
    // provider, where the FP16 default drifted from 4889 ms to 5117 ms between its two runs.
    //
    // On CoreML every candidate is **slower** than the default at both precisions: the compute-unit restrictions by
    // 4-32% and 36-688%, sequential by 3.3% at both, and the fast-prediction hint by 21.5% at FP32 and a tie at FP16.
    // Unlike the Restormers, this graph is not better off away from the Neural Engine — CoreML's own choice under the
    // default compute units is already the fastest placement measured.
    //
    // On the CPU provider sequential execution is 1.3% and 1.4% under the first default, which is inside the spread
    // the default's own two runs show, so it is not declared at either precision.
    //
    // Nothing is declared, at either precision. CUDA and TensorRT were not measured.
    EpProfile::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::denoise::DenoiseVariant;
    use crate::models::precision::FloatPrecision;
    use crate::providers::profile::{CoreMlComputeUnits, CoreMlSpecialization, ExecutionMode};

    #[test]
    fn the_guard_is_the_references_threshold() {
        // Pinned as a literal, for the reason `GUARD` gives: it is carried over rather than derived, so nothing else
        // would notice it moving.
        assert_eq!(GUARD, 3.0);

        for precision in FloatPrecision::ALL {
            assert_eq!(DenoiseVariant::Stockholm(precision).guard(), Some(GUARD), "{precision:?}");
        }
    }

    // The measured outcome, pinned in the shape of Gothenburg's four so that it reads as a measurement rather than as
    // a profile nobody got round to. Each is asked through the variant, whose match is the half a refactor can break.

    #[test]
    fn stockholm_is_left_on_coremls_own_compute_units_at_both_precisions() {
        // Both restrictions measured slower at both precisions — see `profile`. The one a reader is most likely to
        // "fix" is `CpuAndGpu`, copied across from the family's two Restormers, and it costs 32% at FP32 here.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                DenoiseVariant::Stockholm(precision).profile().coreml_compute_units,
                CoreMlComputeUnits::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn stockholm_does_not_ask_for_one_node_at_a_time_at_either_precision() {
        // Slower on CoreML at both precisions, and within the spread on the CPU provider.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                DenoiseVariant::Stockholm(precision).profile().execution_mode,
                ExecutionMode::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn stockholm_does_not_ask_coreml_for_fast_prediction() {
        for precision in FloatPrecision::ALL {
            assert_eq!(
                DenoiseVariant::Stockholm(precision).profile().coreml_specialization,
                CoreMlSpecialization::default(),
                "{precision:?}"
            );
        }
    }

    #[test]
    fn stockholm_declares_nothing_at_either_precision() {
        // Compared as whole profiles, so a field added to `EpProfile` later is covered by existing.
        for precision in FloatPrecision::ALL {
            assert_eq!(DenoiseVariant::Stockholm(precision).profile(), EpProfile::default(), "{precision:?}");
        }
    }
}
