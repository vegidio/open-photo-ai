//! Stockholm: the NAFNet denoiser a front end offers first — the magnitude past which its output is discarded, and the
//! execution-provider profile measured for it. The denoising it runs is the shared
//! [`filter`](crate::models::filter).

// Deleting the model is deleting this directory plus the arms in `DenoiseVariant` that name it.

use crate::models::filter::{Guard, Rescue};
use crate::models::precision::Precision;
use crate::providers::profile::EpProfile;

// Stockholm is NAFNet, which diverges on tiles unlike anything it was trained on — dark, heavily noisy ones above
// all, such as an underexposed night sky. It diverges in two ways:
//
//   blow-up    the raw output goes to 60 and beyond, up to thousands: a solid saturated block once decoded
//   cast       the output stays in range, but its colour and brightness have moved: a green or magenta square
//
// Legitimate output is of order 1 — the graph reads and writes `[0, 1]` — so a magnitude of 3.0 catches the first,
// and is the reference's threshold. It cannot see the second, which is what the drift is for.
//
// Measured on a night photograph (Nikon NEF, 6016x4016, 425 tiles), identically on CPU and CoreML at FP32: 92 tiles
// blew up and 8 more cast visibly — drifts of 0.03 to 0.25 — which the magnitude alone let through as broken squares.
// Legitimate tiles drifted at most 0.008 there, and at most 0.005 on the committed fixture, so 0.01 sits above every
// working tile measured. A second night photograph from the same camera: 62 blow-ups, 14 casts.
//
// The retry is what makes those tiles denoised rather than kept noisy. The same noise lifted towards a mean of 0.25
// is a tile the model knows: with it, the first photograph keeps 1 tile of 425 instead of 92 (and casts none), and
// the second keeps 1. A tile the model handles unaided is never retried, so on the fixture, where nothing diverges,
// the output is unchanged. Lifting every tile instead was measured too and is worse: a tile that is dark *and*
// extremely noisy blows up once lifted, where unaided it denoises cleanly — the lift is a rescue, not a default.
/// How Stockholm's tiles are judged, and the retry a diverged one gets before it keeps its own input.
pub(crate) const GUARD: Guard = Guard { magnitude: 3.0, rescue: Some(Rescue { drift: 0.01, exposure: 0.25 }) };

/// The execution-provider tuning measured for this model, at the precision it carries: the provider defaults at both
/// precisions, because nothing measured beat them, except for the nodes WebGPU cannot run correctly at FP16.
pub(crate) fn profile(precision: Precision) -> EpProfile {
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
    // Nothing is declared for speed, at either precision. CUDA and TensorRT were not measured.
    //
    // The WebGPU nodes at FP16 are correctness, not tuning. The published FP16 export keeps its layer norms' squares
    // and variances in FP32 — they reach 1.2 million, which FP16 cannot hold, and the CPU and CoreML only hid that by
    // promoting them on their own — and what WebGPU still gets wrong after that is its own: it sums a channel-attention
    // `GlobalAveragePool` over the whole 256x256 plane in FP16, and six of them lose enough to turn the tile to noise
    // (0 dB against FP32 without them). Left to the CPU provider they bring it to 74 dB. Measured on an M2 Max against
    // the WebGPU plugin 0.4.0 and ONNX Runtime 1.30, per graph: +12.6% on WebGPU, 76.2 ms to 85.8 ms. FP32 needs none.
    EpProfile::default().with_webgpu_cpu_nodes_at_fp16(precision, WEBGPU_CPU_NODES_FP16)
}

/// The nodes WebGPU is kept off at FP16, as the graph names them — see `profile`.
const WEBGPU_CPU_NODES_FP16: &[&str] = &[
    "/encoders.0/encoders.0.0/sca/sca.0/GlobalAveragePool",
    "/encoders.0/encoders.0.1/sca/sca.0/GlobalAveragePool",
    "/middle_blks/middle_blks.8/sca/sca.0/GlobalAveragePool",
    "/middle_blks/middle_blks.9/sca/sca.0/GlobalAveragePool",
    "/decoders.3/decoders.3.0/sca/sca.0/GlobalAveragePool",
    "/decoders.3/decoders.3.1/sca/sca.0/GlobalAveragePool",
];

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::denoise::DenoiseVariant;
    use crate::models::precision::FloatPrecision;
    use crate::providers::profile::{CoreMlComputeUnits, CoreMlSpecialization, ExecutionMode};

    #[test]
    fn the_guard_is_the_references_threshold_with_the_measured_rescue() {
        // Pinned as literals, for the reason `GUARD` gives: the magnitude is carried over and the rescue measured,
        // neither derived, so nothing else would notice either moving.
        assert_eq!(GUARD.magnitude, 3.0);
        assert_eq!(GUARD.rescue, Some(Rescue { drift: 0.01, exposure: 0.25 }));

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
    fn stockholm_declares_only_its_webgpu_nodes_and_only_at_fp16() {
        // Compared as whole profiles, so a field added to `EpProfile` later is covered by existing.
        assert_eq!(
            DenoiseVariant::Stockholm(FloatPrecision::Fp16).profile(),
            EpProfile::default().with_webgpu_cpu_nodes_at_fp16(Precision::Fp16, WEBGPU_CPU_NODES_FP16)
        );
        assert_eq!(DenoiseVariant::Stockholm(FloatPrecision::Fp32).profile(), EpProfile::default());
    }

    #[test]
    fn the_webgpu_list_is_the_six_global_average_pools_measured() {
        // Pinned by count and kind: the plugin ignores a name it does not find, so a list that went stale against a
        // re-export would bring the noise back with nothing failing.
        let nodes = WEBGPU_CPU_NODES_FP16;

        assert_eq!(nodes.len(), 6);
        assert!(nodes.iter().all(|node| node.ends_with("/sca/sca.0/GlobalAveragePool")), "{nodes:?}");
    }
}
