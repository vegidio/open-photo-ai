//! Kyoto: an RRDBNet publishing native 2x and 4x weights.

use super::conv::{ConvVariant, PassShape, ScaleBucket};
use super::variant::UpscaleVariant;
use imaging::TileGeometry;
use imaging::tensor::Normalisation;

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

/// Kyoto, which publishes native 2x weights as well as 4x — so 8x is a 4x pass followed by a 2x one rather than two
/// 4x passes, and a small request is served by the smaller model.
pub(crate) static KYOTO: ConvVariant = ConvVariant {
    codename: "kyoto",
    label: "Kyoto",
    buckets: &[ScaleBucket::new(2, &[2]), ScaleBucket::new(4, &[4]), ScaleBucket::new(8, &[4, 2])],
    profile,
    variant: UpscaleVariant::Kyoto,
    shape: PassShape { tiles: TileGeometry::DEFAULT, range: Normalisation::Unit },
};

/// The execution-provider tuning measured for this model, at the precision it carries.
fn profile(precision: Precision) -> EpProfile {
    // Left alone, measured: the specialization strategy, low-precision accumulation on the GPU and the execution mode
    // are all within about 3% on this graph with bit-identical output, so the profile names none of them. The
    // execution mode is worth a note because Tokyo sets it and Kyoto deliberately does not: under CoreML the question
    // cannot arise, since the provider takes all 1024 nodes as a single partition, and on the CPU provider sequential
    // is a small consistent loss — RRDB's dense blocks feed each concatenation from several branches at once, which is
    // the wide, independent structure the inter-op pool exists for — measured here at +3.3% against Tokyo's -24.2% on
    // the same provider and precision, which is the sign flip `tokyo::profile` sets out.
    match precision {
        // The CPU and the Neural Engine. This is an RRDBNet — 351 convolutions, 279 LeakyRelus and 276
        // concatenations, no attention anywhere — which is exactly the dense-convolution mix the Neural Engine is
        // built for. At 2x, `ALL` already finds the Neural Engine and naming it changes nothing; at 4x, `ALL` splits
        // the graph and loses 25% to the transitions (211.1ms against 168.9ms on an M2 Max). Naming it is what makes
        // the two passes behave the same way — the setting is not there for the 2x tie, it is there so the 4x pass
        // stops being scheduled differently from its sibling.
        //
        // It depends on the export. Against an export that runs its two upsample `Resize` nodes at FP32 the same
        // comparison says the opposite: that leaves four `Cast` nodes wrapped around the largest tensors in the graph,
        // and two FP32 islands in the middle of an FP16 graph are enough to keep CoreML from giving the whole thing to
        // the Neural Engine. The lesson generalises past Kyoto: on CoreML, measure an FP16
        // graph's compute units only after checking that the float16 conversion left no FP32 islands in it, or the
        // measurement is of the conversion rather than of the model.
        //
        // `CPUAndNeuralEngine` makes the runtime throw at session build on a Mac with no Neural Engine, which would
        // drop this model to the CPU provider. That cannot be reached here: no `darwin_x64` ONNX Runtime is
        // published, so every Mac that runs inference at all is Apple Silicon and has one. Publishing an Intel runtime
        // would make this setting conditional. The margin is an M2 Max's, and the balance between the Neural Engine
        // and the GPU differs across Apple Silicon generations — the direction follows from the op mix and should
        // hold, but re-measure before quoting the number on other hardware.
        Precision::Fp16 => {
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndNeuralEngine, ..EpProfile::default() }
        }
        // Load-bearing rather than a formality, here and in Saitama's arm. CoreML's typed execution bars an FP32
        // MLProgram from the Neural Engine **altogether**, so at FP32 there is nothing to choose — `ALL` there already
        // means the CPU and the GPU. Asking for the Neural Engine anyway does not get it: it drops the graph onto the
        // CPU, which is a 9x regression at 2x and 20x at 4x. So writing the FP16 value for both precisions would not
        // be a harmless over-application of a good setting.
        _ => EpProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::precision::FloatPrecision;
    use crate::models::upscale::UpscaleVariant;

    #[test]
    fn kyoto_at_fp16_declares_the_neural_engine_and_nothing_else() {
        // It names the CPU and the Neural Engine because the 4x pass otherwise loses 25% to transitions on and off
        // the Neural Engine that the 2x pass does not.
        let kyoto = UpscaleVariant::Kyoto(FloatPrecision::Fp16).profile();

        assert_eq!(kyoto.coreml_compute_units, CoreMlComputeUnits::CpuAndNeuralEngine);
        // And nothing else: the specialization strategy, the execution mode and the rest measured within about 3% on
        // this graph with bit-identical output, so they are left at the provider defaults.
        assert_eq!(
            kyoto,
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndNeuralEngine, ..EpProfile::default() }
        );
    }

    #[test]
    fn kyoto_at_fp32_takes_the_default_compute_units_rather_than_the_fp16_answer() {
        // The load-bearing half. CoreML's typed execution bars an FP32 MLProgram from the Neural Engine altogether,
        // so naming it here would not restate the default — it would drop the graph onto the CPU, which is 9x at 2x
        // and 20x at 4x. Applying the FP16 answer to both precisions is a regression, not a harmless over-reach.
        assert_eq!(UpscaleVariant::Kyoto(FloatPrecision::Fp32).profile(), EpProfile::default());
    }
}
