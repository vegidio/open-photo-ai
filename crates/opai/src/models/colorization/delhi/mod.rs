//! Delhi: a DDColor colorizer a front end offers first — the execution-provider profile declared for it. The
//! colorization it runs is the family's [`process`](super::process), through the Ab contract it shares with Mumbai.

// Deleting the model is deleting this directory plus the arms in `ColorizationVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

/// The execution-provider tuning declared for this model, at the precision it carries: CoreML off the Neural Engine
/// at FP16, and the provider defaults at FP32.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Ported from the reference's `delhi.go`, and **not measured on Delhi's own graph, there or here**. The reference
    // declares the same setting as Mumbai's beside it and records no figure and no reasoning for it. Delhi is the same
    // DDColor architecture — a ConvNeXt encoder feeding a transformer colour decoder — so Mumbai's op-mix argument
    // plausibly carries, but that is an inference rather than a measurement. See `mumbai::profile` for the figures
    // that do exist.
    //
    // Written out rather than left empty because an empty file here would read as "unmeasured, so the default", and
    // the setting is not the default. It is kept rather than dropped for the same reason: the reference ships it, and
    // parity is the target. Re-measuring Delhi is how to confirm or revise it. A wrong CoreML-only, FP16-only setting
    // costs speed, never a wrong image.
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
    fn delhi_is_kept_off_the_neural_engine_at_fp16_and_nothing_else() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break.
        assert_eq!(
            ColorizationVariant::Delhi(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() },
            "Delhi at FP16 is not the setting the reference declares for it"
        );
    }

    #[test]
    fn delhi_declares_nothing_at_fp32() {
        assert_eq!(
            ColorizationVariant::Delhi(FloatPrecision::Fp32).profile(),
            EpProfile::default(),
            "Delhi at FP32 declared a setting nothing measured"
        );
    }
}
