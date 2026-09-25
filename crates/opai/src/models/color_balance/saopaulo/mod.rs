//! São Paulo: the mixed-illuminant model, whose graph does not return a corrected photograph.
//!
//! mixedillWB (Afifi et al., WACV 2022). It blends three renderings of the photograph — daylight, shade and
//! tungsten — by a per-pixel weight map over the three, letting different regions land on different illuminants.
//!
//! The square this graph was exported at, the channel layout its output carries and the execution-provider profile
//! measured for it. [`process`] is the contract it runs.

// A photograph lit by two sources — a window-lit room with a warm lamp, a subject in shade against a sunlit
// background — has no single global correction, whatever feature set one is fitted over, which is why this model
// blends rather than corrects. Daylight is the photograph itself (see `RENDERINGS`), and shade and tungsten come from
// Deep_White_Balance's editing network, the same architecture `rio` runs.
//
// One graph holds all of it. Deep_White_Balance ships `net_awb.pth`, `net_t.pth` and `net_s.pth` as separate files,
// but they are bit-identical splits of one multi-task network: a shared encoder plus one decoder head each. Keeping
// the shade and tungsten heads on the shared encoder reproduces both files exactly and runs the encoder once instead
// of twice, and the weight predictor bolts on after them for one `Concat` — so the whole fixed-resolution half of this
// pipeline is a single session. The AWB head is dropped: it is what Rio ships, and nothing here reads it.
//
// Deleting the model is deleting this directory plus the arms in `ColorBalanceVariant` and `ColorBalance::pipeline`
// that name it.

pub(crate) mod process;

use crate::models::precision::Precision;
use crate::providers::profile::EpProfile;
use imaging::tensor::Normalisation;

// 656 is Deep_White_Balance's own working size and the size Rio ships, so the editing half runs at exactly the scale it
// was tuned for and one padded square serves both halves — built once, cropped once, with no second geometry for a
// rounding difference to enter through. Padding rather than squashing is worth 3.9 dB at this size: the predictor's
// whole job is to find regions lit differently, so squashing distorts exactly the spatial structure it keys on.
//
// Declared here rather than inherited from `rio::CANVAS`, because this number **cannot** be traded the way Rio's can.
// Rio's square is affordable because its graph's output is **never shown** — it is only ever sampled into an 11-term
// fit. **Half of this graph's output is shown.** The weight map is upsampled to full resolution and multiplies the
// photograph, so its size is a real cap on how finely the blend can follow an illuminant boundary rather than a
// sampling resolution. Dropping to 512 costs **3.7 dB** and to 384 costs **7.6 dB**.
/// The square this graph accepts, and the only one it accepts.
pub(crate) const CANVAS: u32 = 656;

// Declared here rather than inherited from `rio::process`, where the same constant lives: it is a property of an
// **export**, and the second export says so where it is written. The two agree today, which is a fact about two graphs
// rather than about the family.
/// The range this graph reads and writes: `[0, 1]`.
pub(crate) const RANGE: Normalisation = Normalisation::Unit;

// **The only number in this layout**, and everything below is derived from it exactly as the reference's `MixedSpec`
// derives its own. Three is what the `_D_S_T` checkpoint was trained for.
/// How many renderings the graph blends: daylight, shade and tungsten.
pub(crate) const SETTINGS: usize = 3;

// Daylight is not synthesized because a graph returning it would be handing back what it was given. `SETTINGS - 1`
// rather than a second constant that could disagree with it.
/// How many of the settings the graph has to **synthesize**: two, as daylight is not one of them — it is the
/// photograph itself.
pub(crate) const RENDERINGS: usize = SETTINGS - 1;

// **This is the contract between the conversion script and this code**, not an implementation detail. Getting it
// wrong means reading a weight plane where a rendering is and rendering nonsense from correctly loaded weights, with
// no error anywhere. Named rather than inlined at its one call site so a test can pin the layout with no session
// open — the same reason upscale's `GraphSpec` makes its naming rule a method.
/// How many channels the graph's output carries: one weight plane per setting, then one three-channel rendering per
/// synthesized setting.
///
/// ```text
///   input   [1, 3, 656, 656]   the reflection-padded square
///   output  [1, 9, 656, 656]   3 weight planes (daylight, shade, tungsten), then shade's RGB, then tungsten's
/// ```
pub(crate) const CHANNELS: usize = SETTINGS + 3 * RENDERINGS;

/// The index of the first channel of the `index`-th synthesized rendering.
///
/// Counted from zero in the order the settings are declared **minus daylight**, so 0 is shade and 1 is tungsten for
/// a `_D_S_T` graph. This is what [`Samples::new`](super::samples::Samples::new)'s plane offset is handed.
///
/// # Panics
///
/// Panics where `index` is not below [`RENDERINGS`].
pub(crate) const fn rendering_offset(index: usize) -> u32 {
    assert!(index < RENDERINGS, "a rendering outside the layout this graph was exported with");

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the layout is nine channels; every offset is below it"
    )]
    let offset = (SETTINGS + 3 * index) as u32;

    offset
}

/// The execution-provider tuning measured for this model, at the precision it carries.
///
/// **The provider defaults, at both precisions — and that is a measurement rather than an omission.**
pub(crate) fn profile(_precision: Precision) -> EpProfile {
    // Everything below is an M2 Max against the ONNX Runtime this application pins, at the 656 square.
    //
    // Nothing beats the CoreML defaults. The fast-prediction specialization is the only row that even looks like a
    // win — **3.3% at FP16** — and it sits inside that row's own 6-7% run-to-run spread. Execution mode is likewise a
    // wash, which is worth recording because this application runs the parallel mode rather than ONNX Runtime's
    // sequential one: on a graph that is a single fused CoreML node the inter-op pool has nothing to schedule either
    // way.
    //
    // Rio's answer is not carried across, in either direction: the load-bearing part of this file. Rio and São Paulo
    // are the same family, the same canvas and the same precisions, and `rio::profile` declares the fast-prediction
    // specialization at FP16. It was measured on **the editing network alone**. This graph is that network plus a
    // weight predictor plus two internal resamples — a different op mix, which is exactly what a profile is a
    // measurement of.
    //
    // The one unambiguous result of the sweep is the compute units: restricting this graph to the CPU and Neural
    // Engine costs **+62% at FP16 and +732% at FP32**, at no accuracy gain. That is the same conclusion Rio reaches
    // from the opposite direction and for the same underlying reason — Rio is 52 convolutional nodes the Neural Engine
    // takes whole, while this is 511 nodes including resamples and a softmax, which it cannot.
    //
    // Before re-measuring any of it, confirm that **both** graphs are still one CoreML partition, on the runtime this
    // application pins. The FP16 export opens with a `Cast` consuming the graph input directly, which ONNX Runtime's
    // CoreML builder declines; the conversion script splices a `Clip(input, 0, 1)` ahead of it to take the graph to
    // 511 of 511 nodes. On a newer runtime that stray `Cast` is absorbed and the graph reads as fully fused either way,
    // so a check run there silently passes a graph the shipping runtime splits.
    //
    // Both precisions ship. They are bit-identical between the CPU and CoreML providers, so which provider a machine
    // picks cannot change what the user sees, and FP16 measures **66.8 dB** against FP32 on the same provider — a
    // largest single-channel error of one 8-bit level — at half the download, 16.7 MB against 33.2 MB. FP16 is the
    // slower of the two on CoreML on an M2 Max, which is unusual and is not a partitioning failure: its minimum is the
    // faster of the two, but it carries an 11-18% spread where FP32 sits at 1-3%, so its median lands behind.
    //
    // Re-confirmed on the whole pipeline with `perftest saopaulo --precision <p> -p coreml -n 20`, on an M2 Max
    // (64 GB, macOS 26.6) against the pinned ONNX Runtime 1.26.0, over the embedded 640x640 sample, with this function
    // edited between runs. These medians cover the presentation resample, the graph, the two fits and the fused
    // full-resolution blend, where the reference's figures above are of the graph alone:
    //
    //   saopaulo, median of 20 runs                       FP32      FP16
    //     the provider's default                        108.5 ms  116.3 ms    (σ 1.6, 2.3)
    //     FastPrediction                                108.4 ms  116.8 ms    (σ 0.3, 2.4)
    //     CPUAndNeuralEngine, median of 10              393.0 ms  144.0 ms    (σ 1.6, 1.3)
    //
    // **Nothing beats the defaults**, at either precision: the specialization lands within 0.5 ms of them and on the
    // wrong side of them at FP16, well inside that row's own spread. FP16 being the slower of the two is reproduced
    // too, which is the unusual result the reference records and is not a partitioning failure.
    //
    // **The compute-unit figures reconcile with the reference's exactly**, which is worth writing down because at
    // first reading they do not: +262% and +24% here against +732% and +62% there. Those are graph-only and these are
    // the whole pipeline, so solving each for the constant cost of everything around the graph —
    // `8.32·G + P = 393.0` against `G + P = 108.5`, and `1.62·G + P = 144.0` against `G + P = 116.3` — gives
    // **P ≈ 70 ms at both precisions**, from two independent measurements. That is the pipeline cost on this image,
    // and it is what dilutes the graph's own ratio.
    //
    // Both graphs were confirmed to be one CoreML partition on 1.26.0 first, as the check above asks:
    // `CoreMLExecutionProvider::GetCapability` reports **511 of 511 nodes at FP16 and 483 of 483 at FP32, one
    // partition each** — the reference's own numbers, spliced `Clip` and all.
    //
    // It takes a precision it does not read because the sweep was taken at both and answered the same at both, which
    // is a different statement from a model whose profile is precision-independent by construction. The shape matches
    // `rio::profile`, whose two precisions do not agree, so a later measurement that separates them is an arm here
    // rather than a change to the seam that calls it.
    EpProfile::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::color_balance::ColorBalanceVariant;
    use crate::models::precision::FloatPrecision;
    use crate::providers::profile::{CoreMlComputeUnits, CoreMlSpecialization};

    #[test]
    fn the_channel_layout_is_the_one_the_conversion_script_exports() {
        // Pinned as literals with no session open, which is the whole reason the layout is named rather than
        // inlined. Every one of these is a number the graph's exporter and this code have to agree on, and a
        // disagreement is a photograph rendered out of the wrong planes rather than anything a runtime reports.
        assert_eq!(SETTINGS, 3, "the settings this graph blends");
        assert_eq!(RENDERINGS, 2, "daylight is the photograph, so only two settings are synthesized");
        assert_eq!(CHANNELS, 9, "three weight planes, then one three-channel rendering per synthesized setting");

        // The offsets, which are what `Samples` is handed. Shade first, then tungsten, each after the three weight
        // planes — a rendering read one plane out lands half on a weight map and renders a photograph.
        assert_eq!(rendering_offset(0), 3, "shade does not begin after the weight planes");
        assert_eq!(rendering_offset(1), 6, "tungsten does not begin after shade");

        // And the derivation holds rather than the literals merely agreeing with themselves: the last rendering
        // ends exactly at the channel count, so no plane is unaccounted for and none is read twice.
        assert_eq!(rendering_offset(RENDERINGS - 1) as usize + 3, CHANNELS, "the layout does not close");
    }

    #[test]
    fn the_graph_reads_the_unit_range_rather_than_the_signed_one_face_recovery_uses() {
        // The literal, pinned rather than left to be read off a call. The two ranges differ in nothing a runtime
        // can report — a graph trained on `[0, 1]` and fed `[-1, 1]` loads, runs and returns a worse photograph.
        assert_eq!(RANGE, Normalisation::Unit, "São Paulo's graph was switched to the signed range");
    }

    #[test]
    fn sao_paulo_runs_at_one_square_and_it_is_the_one_the_graph_was_exported_at() {
        // Pinned as a literal because it is not a tunable, and asked through the variant as well because that arm
        // is the half a refactor can break: nothing else says it reaches this file.
        assert_eq!(CANVAS, 656);

        for precision in FloatPrecision::ALL {
            assert_eq!(ColorBalanceVariant::SaoPaulo(precision).canvas(), CANVAS, "{precision:?}");
        }
    }

    #[test]
    fn sao_paulo_declares_the_provider_defaults_at_both_precisions() {
        // The measurement, stated as a claim rather than left as an absence: nothing beats the CoreML defaults at
        // this canvas (see `profile`).
        //
        // Asked through the variant rather than of `profile` directly, for the reason Rio's own test gives: the
        // variant's match is what a refactor can break, and a profile that stopped reaching this file would still
        // load São Paulo and still render the right photograph.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                ColorBalanceVariant::SaoPaulo(precision).profile(),
                EpProfile::default(),
                "São Paulo at {precision:?} declared a setting the sweep did not find"
            );
        }
    }

    #[test]
    fn sao_paulo_is_never_restricted_off_coremls_gpu() {
        // The one unambiguous result of the sweep (see `profile`), stated as its own test because it is the setting a
        // tidy-up would most plausibly add.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                ColorBalanceVariant::SaoPaulo(precision).profile().coreml_compute_units,
                CoreMlComputeUnits::default(),
                "São Paulo at {precision:?} was kept off the GPU its 511 nodes need"
            );
        }
    }

    #[test]
    fn rio_and_sao_paulo_do_not_share_one_profile() {
        // The contrast that says a profile is not carried between two models of one family (see `profile`), stated
        // where a tidy-up unifying the two arms would have to walk past it.
        //
        // Asked at FP16, which is the precision Rio declares anything at.
        let rio = ColorBalanceVariant::Rio(FloatPrecision::Fp16).profile();
        let sao_paulo = ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16).profile();

        assert_eq!(
            rio.coreml_specialization,
            CoreMlSpecialization::FastPrediction,
            "Rio stopped declaring the specialization measured for it"
        );
        assert_eq!(
            sao_paulo.coreml_specialization,
            CoreMlSpecialization::default(),
            "São Paulo gained the neighbour's specialization, which its own sweep could not separate from noise"
        );
        assert_ne!(rio, sao_paulo, "the family's two profiles were unified into one");
    }

    #[test]
    #[should_panic(expected = "a rendering outside the layout")]
    fn a_rendering_outside_the_layout_is_not_an_offset() {
        // The guard rather than a silent index past the tensor: a loop that asked for a third rendering would
        // otherwise read the plane after tungsten, which on this export is nothing at all.
        let _ = rendering_offset(RENDERINGS);
    }
}
