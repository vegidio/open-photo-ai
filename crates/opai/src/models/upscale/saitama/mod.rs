//! Saitama: an RRDBNet publishing native 4x weights only.

use super::conv::{ConvVariant, FOUR_ONLY, PassShape};
use super::variant::UpscaleVariant;
use imaging::TileGeometry;
use imaging::tensor::Normalisation;

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

/// Saitama, native 4x weights only.
pub(crate) static SAITAMA: ConvVariant = ConvVariant {
    codename: "saitama",
    label: "Saitama",
    buckets: FOUR_ONLY,
    profile,
    variant: UpscaleVariant::Saitama,
    shape: PassShape { tiles: TileGeometry::DEFAULT, range: Normalisation::Unit },
};

/// The execution-provider tuning measured for this model, at the precision it carries.
fn profile(precision: Precision) -> EpProfile {
    // Measured on an M2 Max against ONNX Runtime 1.26, three invocations per configuration at 640x640:
    //
    //   saitama 4x, CoreML, median run
    //     FP16   ALL (default)               653.9 / 654.9 / 655.3 ms
    //     FP16   CPUAndNeuralEngine          539.0 / 539.6 / 553.3 ms     -17.6%
    //     FP32   (this arm is the default)   819.7 / 822.0 ms
    //
    // Both halves, because the asymmetry is the claim rather than the setting on its own: the FP16 margin is well clear
    // of the noise floor, and the FP32 arm is left alone.
    match precision {
        // The same answer as Kyoto, reached from the same op mix. Deliberately not folded into Kyoto's arm: the two
        // agree because their graphs agree, and one arm would read as a shared decision rather than as two
        // measurements landing in the same place. Saitama is the other RRDBNet here — 96 convolutions, 75 LeakyRelus,
        // 72 concatenations, no attention — so it wants what Kyoto wants for the reason Kyoto wants it, and the
        // convergence is evidence rather than duplication.
        //
        // Its FP16 export is what makes the setting worth anything — the export dependence `kyoto::profile` describes,
        // in its second instance. Against an export carrying `onnxconverter-common`'s `Resize` defect,
        // `CPUAndNeuralEngine` is **the worst** of the three choices rather than the best: 50% behind doing nothing,
        // where the fixed export puts it ahead. The GPU does not care either way, and ONNX Runtime hands CoreML the
        // whole graph as one partition on both — only a timing tells the two exports apart. The published artifact is
        // the fixed export, checked structurally: its two upsample `Resize` nodes run at FP16 with no `Cast` wrapped
        // around them.
        Precision::Fp16 => {
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndNeuralEngine, ..EpProfile::default() }
        }
        // Load-bearing, for the reason Kyoto's FP32 arm gives.
        _ => EpProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::precision::FloatPrecision;
    use crate::models::upscale::UpscaleVariant;

    #[test]
    fn saitama_declares_the_neural_engine_at_fp16_and_the_default_at_fp32() {
        // Both halves, because the asymmetry is the claim. The FP16 answer is Kyoto's, reached from the same RRDBNet
        // op mix; the FP32 arm is the one that would be a measured regression if the FP16 answer were generalised.
        assert_eq!(
            UpscaleVariant::Saitama(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndNeuralEngine, ..EpProfile::default() }
        );
        assert_eq!(UpscaleVariant::Saitama(FloatPrecision::Fp32).profile(), EpProfile::default());
    }
}
