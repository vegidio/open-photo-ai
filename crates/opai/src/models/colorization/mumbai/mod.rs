//! Mumbai: a DDColor colorizer — the execution-provider profile measured for it. The colorization it runs is the
//! family's [`process`](super::process), through the Ab contract it shares with Delhi.

// Deleting the model is deleting this directory plus the arms in `ColorizationVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML off the Neural Engine
/// at FP16, and the provider defaults at FP32.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Ported from the reference's `mumbai.go`, not re-measured on this project's build.
    //
    // Kept off the Neural Engine at FP16. On the graph as published today that is worth 34.9% (298.8 ms to 194.5 ms).
    // On the re-exported graph described below it is the difference between a correct image and a broken one: run
    // with the default compute units against an FP32 reference, the rebuilt FP16 graph deviates by 334 Lab a/b units,
    // on planes whose whole range is about ±64, and takes 923 ms against the 95 ms it takes on the GPU. CPU and GPU is
    // the one setting that cannot reach that compiler.
    //
    // DDColor is a ConvNeXt encoder feeding a transformer colour decoder, and the op mix says so: 155 Gemms, 147
    // transposes and 64 normalizations against 51 convolutions — the opposite of what a unit built for dense
    // convolution wants.
    //
    // The published graph hides this rather than avoiding it. Five nodes the CoreML provider declines partition it into
    // six CoreML subgraphs, so the Neural Engine never sees the whole model. That is also why FP16 is currently the
    // SLOWER export — 246 ms against FP32's 185 ms — because one of those CPU nodes is the `bqc,bchw->bqhw` product
    // against a [1, 256, 512, 512] feature map, which crosses the partition boundary as 268 MB per run. Anyone
    // re-measuring Mumbai first establishes how many partitions the published graph compiles to, or the figures here
    // cannot be compared.
    //
    // Export note, to collapse it to one partition: the four Pads in PixelShuffle_ICNR's blur (MLProgram supports only
    // `constant` and `reflect`) rewritten as a Slice+Concat of the border row and column, and the colour decoder's
    // Einsum, which the provider has no builder for, as Reshape+MatMul+Reshape. Numerically a no-op at 3.1e-5 in a/b,
    // worth -45.3% at FP32 and -61.4% at FP16, and it makes FP16 the more accurate export rather than the less. It is
    // independent of this profile, so the two do not have to land in the same release.
    //
    // Sequential execution, CoreML's fast-prediction hint and low-precision GPU accumulation all measured within 0.7%
    // and bit-identical, and none is set.
    //
    // None of this helps the cold start, which the reference records as the graph's real remaining cost: CoreML
    // spending about five minutes compiling the MLProgram on a first run, whether it is producing one program or six.
    // The one first run made on this project's build (M2 Max, 2026-09-23) took 8 s; why was not investigated.
    match precision {
        Precision::Fp16 => EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() },
        _ => EpProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::colorization::ColorizationVariant;
    use crate::models::precision::FloatPrecision;

    #[test]
    fn mumbai_is_kept_off_the_neural_engine_at_fp16_and_nothing_else() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break. On a single-partition export the default compute units make this graph wrong, not slow.
        assert_eq!(
            ColorizationVariant::Mumbai(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() },
            "Mumbai at FP16 is not the setting measured for it"
        );
    }

    #[test]
    fn mumbai_declares_nothing_at_fp32() {
        assert_eq!(
            ColorizationVariant::Mumbai(FloatPrecision::Fp32).profile(),
            EpProfile::default(),
            "Mumbai at FP32 declared a setting nothing measured"
        );
    }
}
