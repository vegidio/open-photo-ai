//! Rio: the single-output white-balance model a front end offers first.
//!
//! The square this graph was exported at and the execution-provider profile measured for it. [`process`] is the
//! contract it runs.

// Rio is Deep_White_Balance (arXiv 2004.01354), the single-task AWB network, from the upstream `net_awb.pth`
// checkpoint. It is a plain 4-level U-Net — 46 tensors, and the graph's 46 initializers are those tensors unchanged,
// so this export folds nothing and re-exporting it is a shape change rather than a rebuild. That is the opposite of
// `light_adjustment::paris`, whose export folds a stack of dead transformer blocks down to what they compute.
//
// Deleting the model is deleting this directory plus the arms in `ColorBalanceVariant` and `ColorBalance::pipeline`
// that name it.

pub(crate) mod process;

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlSpecialization, EpProfile};

// A fixed square, because a graph with dynamic spatial axes does not run on CoreML at all with static input shapes
// required — which is what a model declaring no profile gets: the provider declines every one of its 52 nodes, so a
// Mac runs it on CPU kernels at roughly 15x the CoreML time.
//
// **Unlike Paris and Lyon, Rio has a second way out of this, and it is not what ships.** Its axes are dynamic but its
// operations are not — Conv, Relu, MaxPool, ConvTranspose and Concat, nothing that needs a bounded dimension — so
// declaring dynamic shapes and keeping a dynamic-axes graph does reach CoreML, as one partition, at the same speed.
// That is rejected for three reasons. CoreML re-specialises per shape on the flexible path, costing 34-96 ms on the
// first run of each new aspect ratio. It emits a warning from inside that path about a pooling output being too
// small, which is a warning today and nothing anyone should build on. And it is a macOS-only fix: TensorRT wants
// explicit optimisation profiles for a dynamic graph, which this variant has none to give it.
//
// 656 is the ceiling a dynamic-shape run scales a photograph to, so the scale the model sees is the same either way
// and only the extension is new.
//
// **What a square canvas costs is measurable here in a way it is not for light adjustment, because the model's output
// is never shown.** It feeds the 11-term polynomial fit in `process`, which barely moves with the resolution it was
// sampled at (see `mapping`). Scored on the final full-resolution image over 72 photographs, 656 holds a 41.0 dB worst
// case and a largest DC shift of 0.62 levels — below what a viewer can see, and a long way clear of the bar Paris
// ships at (31.7 dB worst, 5.61 levels). Smaller canvases fall off fast: 512 is 32.0 dB worst and 2.70 levels.
//
// The cost is inference on the pixels the extension adds. A 656 square is 1.46x the pixels of the 656x448 an ordinary
// landscape photograph runs at with dynamic shapes, and a small image runs at the canvas like everything else rather
// than at its own size. On every provider that is not the CPU that trade is one-sided, and on the CPU it is charged
// against a run whose full-resolution polynomial apply is the larger half of the work.
//
// São Paulo also reports 656, for a reason that does not let its number be traded the way this one can; see
// `saopaulo::CANVAS`.
/// The square this graph accepts, and the only one it accepts.
pub(crate) const CANVAS: u32 = 656;

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML's fast-prediction
/// specialization at FP16 only, and the provider's defaults otherwise — compute units included, at both precisions.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Transcribed from the reference's `rio.go`, which carries both the figures and the conditions they were taken
    // under: an M2 Max against ONNX Runtime 1.26 at the 656 square.
    //
    // Rio **wants** the Neural Engine where Paris and Lyon refuse it. The load-bearing part of this file, and the
    // setting most likely to be copied across by someone tidying two colour-correcting families into one answer.
    //
    // Paris and Lyon both restrict CoreML to the CPU and GPU at FP16, because CoreML scatters their graphs across the
    // Neural Engine and the GPU and they pay for every transition. **Rio is the opposite case.** Its graph is 52
    // convolutional nodes, which is what the Neural Engine is built for, and it lands there whole rather than in
    // pieces — so restricting it to the CPU and GPU costs **+91%**, at no accuracy gain. Neither family's compute-unit
    // answer is evidence for the other's, and neither may be carried across on the grounds that both correct colour.
    // The compute units are therefore left at the provider's default here, at both precisions, and the test below pins
    // that as a claim rather than as an omission.
    //
    // FP16 is worth having on this model in a way it is not on most, as a property of the pipeline rather than of the
    // graph: the model's output is only ever sampled into the global colour fit, so half precision is averaged away
    // rather than shown. It scores 84.5 dB against FP32 through CoreML, with a largest DC shift of 0.058 levels.
    match precision {
        // CoreML asked to specialize the graph for prediction latency, worth **-3.0%** at FP16. It is the smallest
        // figure any model in this catalogue has declared a setting on, and it is declared because the *mechanism* is
        // understood rather than because the number is large: the specialization trades compile time for prediction
        // latency and is only worth paying for on a fixed-shape graph that stays resident, which is exactly what this
        // graph is at a fixed square. `face_recovery::santorini` is the only other model here that earns it.
        //
        // Re-confirmed on the whole pipeline with `perftest rio --precision <p> -p coreml -n 20`, on an M2 Max (64 GB,
        // macOS 26.6) against the pinned ONNX Runtime 1.26, over the embedded 640x640 sample, with the specialization
        // toggled here between runs. These medians cover the presentation resample, the graph, the 11-term fit and
        // two full-resolution passes, where the reference's figure is of the graph alone:
        //
        //   rio at FP16, median of 20 runs      repeated four and three times
        //     FastPrediction                      65.1  65.3  65.3  65.6 ms
        //     the provider's default              65.4  65.8  65.9      ms
        //
        // That is **-0.6% on the pipeline**, and the honest reading of it is that this harness cannot resolve the
        // claim rather than that it disagrees with it. The absolute saving is 0.38 ms, against the 0.43 ms that -3.0%
        // of the reference's 14.48 ms graph works out to — so the size is right and the direction is right, but the
        // pipeline around the graph costs about 50 ms on this image whatever ran, and a 3% difference in a seventh of
        // the total is at the edge of what twenty runs separate. The two sets above barely overlap, which is the whole
        // of the separation there is.
        //
        // **It is therefore not a contradiction, and the declaration stands** — on the mechanism rather than on this
        // number. Two controls say the measurement is not noise-only: the FP32 rows, which are the *same*
        // configuration measured twice because nothing is declared there, land on 64.8 and 65.4 ms — the wash the
        // reference records, and a run-to-run spread of 0.9% that the FP16 gap sits just outside.
        //
        // The **first** FP16 run of that session measured 57.8 ms, and nothing reproduced it across seven later runs
        // in either configuration: a cold machine, not a setting, and the kind of figure that gets mistaken for a
        // finding.
        Precision::Fp16 => {
            EpProfile { coreml_specialization: CoreMlSpecialization::FastPrediction, ..EpProfile::default() }
        }
        // Not declared at FP32. Both configurations bottom out at 14.48 ms there, so what separates them is variance
        // rather than compute — and a declaration that is worth something at one precision and nothing at the other
        // is not carried to the precision that did not earn it. That is the precision-split rule Paris, Lyon, Saitama
        // and Kyoto follow, reached here from a different measurement.
        _ => EpProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::color_balance::ColorBalanceVariant;
    use crate::models::light_adjustment::LightAdjustmentVariant;
    use crate::models::precision::FloatPrecision;
    use crate::providers::profile::CoreMlComputeUnits;

    #[test]
    fn rio_carries_the_one_measured_setting_at_fp16() {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break: nothing else asserts the arm reaches this file, and a profile left at the default
        // would still load, still correct the right photograph, and cost 3% more on an M2 Max.
        assert_eq!(
            ColorBalanceVariant::Rio(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_specialization: CoreMlSpecialization::FastPrediction, ..EpProfile::default() },
            "Rio at FP16 is not the setting measured for it"
        );
    }

    #[test]
    fn rio_declares_nothing_at_fp32() {
        // The other half, and the claim rather than a formality: both configurations bottom out at 14.48 ms at
        // FP32, so carrying the FP16 answer across would be applying a measurement to a precision that did not
        // earn it.
        assert_eq!(
            ColorBalanceVariant::Rio(FloatPrecision::Fp32).profile(),
            EpProfile::default(),
            "Rio at FP32 declared a setting nothing measured"
        );
    }

    #[test]
    fn rio_is_never_restricted_off_the_neural_engine() {
        // The contrast with Paris and Lyon, stated as its own test because it is a **claim** rather than an
        // omission: the two families' compute-unit answers are opposite measurements rather than one convention (see
        // `profile`). A tidy-up that unified them would pass every other test in this file.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                ColorBalanceVariant::Rio(precision).profile().coreml_compute_units,
                CoreMlComputeUnits::default(),
                "Rio at {precision:?} was kept off the Neural Engine its graph lands on whole"
            );
        }
    }

    #[test]
    fn rio_and_paris_answer_the_neural_engine_question_oppositely() {
        // The contrast above, stated where a tidy-up would have to walk past it rather than only in this file's
        // prose. `rio_is_never_restricted_off_the_neural_engine` says Rio's half; on its own it reads like a model
        // that simply has no compute-unit answer, and the mistake this guards against is someone reading Paris's
        // arm as the house style for a colour-correcting graph and carrying it here.
        //
        // Asked at FP16, which is the precision both declare anything at.
        let rio = ColorBalanceVariant::Rio(FloatPrecision::Fp16).profile().coreml_compute_units;
        let paris = LightAdjustmentVariant::Paris(FloatPrecision::Fp16).profile().coreml_compute_units;

        assert_eq!(paris, CoreMlComputeUnits::CpuAndGpu, "Paris stopped restricting CoreML off the Neural Engine");
        assert_eq!(rio, CoreMlComputeUnits::All, "Rio was kept off the Neural Engine its graph lands on whole");
        assert_ne!(rio, paris, "the two families' compute-unit answers were unified into one");
    }

    #[test]
    fn rio_runs_at_one_square_and_it_is_the_one_the_graph_was_exported_at() {
        // Pinned as a literal because it is not a tunable: the whole of the canvas-size argument above — 41.0 dB
        // worst case at 656 against 32.0 at 512 — was taken at this number, and a graph that accepts one size
        // accepts this one.
        assert_eq!(CANVAS, 656);

        for precision in FloatPrecision::ALL {
            assert_eq!(ColorBalanceVariant::Rio(precision).canvas(), CANVAS, "{precision:?}");
        }
    }
}
