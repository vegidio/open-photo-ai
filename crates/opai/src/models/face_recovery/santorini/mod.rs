//! Santorini: the StyleGAN-decoder restorer, and the one whose graph takes the image alone.
//!
//! The model's row, and nothing else: the execution-provider profile measured for this graph. The restoration it
//! runs is [`restore`](super::restore), the family's one contract, shared with [`athens`](super::athens).

// The two models differ at a single point, which is that this graph has no second input to bind a fidelity to, and
// that is answered by `FaceRecoveryVariant::weight` rather than here. Deleting the model is deleting this directory
// plus the arms in `FaceRecoveryVariant` that name it.

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlSpecialization, EpProfile, ExecutionMode};

/// The execution-provider tuning measured for this model. `precision` is ignored: both precisions share one profile.
pub(crate) fn profile(_precision: Precision) -> EpProfile {
    // Transcribed from the reference's `santorini.go`, whose comment carries both the numbers and the conditions they
    // were taken under. **No execution provider was exercised against this model in this project**: the figures were
    // taken on an M2 Max (CoreML) and an RTX 5090 (CUDA), and what this crate validates is that the settings are
    // *declared*, not that the percentages reproduce on other hardware. TensorRT has no figure for this graph in
    // either implementation.
    //
    // `precision` is ignored as the reference's `profileFor` ignores it: both of the settings below are properties of
    // the graph's shape and its export rather than of the precision it was exported at, and both precisions were
    // measured. The parameter stays so that a later re-measurement that splits them is a change to this function
    // rather than to its signature and every call site.
    EpProfile {
        // CoreML asked to specialize the graph for latency. A trade rather than a win: worth 1.04x at FP16 on an M2
        // Max and a tie at FP32 in steady state, at the price of 20-30ms of cold start in every FP32 round — which is
        // the specialization time Apple names as this hint's price.
        //
        // That cost is paid once per session build and the registry keeps models resident, so it is set for the graph
        // rather than for one precision. A model rebuilt per image would pay it every time and lose, which is the
        // condition to re-check before copying this onto anything.
        coreml_specialization: CoreMlSpecialization::FastPrediction,
        // A StyleGAN decoder hanging off a U-Net — a backbone rather than a branchy graph — so the inter-op pool never
        // has two independent nodes to hand out and only charges the handoff. Worth **1.09x at FP16 and 1.04x at FP32
        // on CUDA** with bit-identical output, and the CPU provider gains too, which is what shows it to be the
        // graph's shape rather than anything about CUDA. CoreML agrees at FP16 (-4.8%) and is a tie at FP32.
        //
        // **End to end the FP16 margin is much wider than the graph alone accounts for**: -9.8% over a two-face
        // sample, where twice the graph's own 2.4ms saving is 4.8ms against 22ms observed. The inter-op pool is
        // process-wide, so in parallel mode it competes for the same cores as the alignment and compositing work
        // around each face. Both figures are recorded because either one alone is misleading — the isolated one
        // understates it and the end-to-end one attributes to the graph what the surrounding work paid.
        execution_mode: ExecutionMode::Sequential,
        // Everything else is the runtime's own default, and two of them load-bearingly so: the compute units, and the
        // NCHW layout this graph fails without.
        //
        // **CoreML's compute units are not restricted, where Athens restricts them** — the setting most likely to be
        // copied over by someone tidying the family's two rows into agreement. Carrying Athens' rule here costs about
        // **9%**: this graph's FP16 export runs FP16 through its upsampling path, so the Neural Engine takes it whole,
        // where Athens is a CodeFormer whose normalisation and transpose mix thrashes on it. The difference is the
        // export rather than the architecture: with its `Resize` nodes held at FP32 — 84 `Cast` nodes wrapping all 42
        // upsamples — Athens' rule looks right for this graph too. If the weights are re-exported, this is the first
        // thing to re-measure.
        //
        // The reference states that cost twice and not alike: `santorini.go`'s comment says about 2% and
        // `santorini_test.go`'s — the one that sits on the assertion — says ~9%. The larger figure is carried here
        // because it is the one the reference pins its own test against, and either way the sign and the conclusion
        // are the same.
        //
        // Two CUDA options **break** this graph rather than slowing it, and both look like free wins in the option
        // list:
        //
        // - **The NHWC convolution layout fails at `Run`**, reporting that the input channels are not equal to the
        //   kernel channels times the group. That is exactly the modulated-convolution hazard
        //   `EpProfile::cuda_prefer_nhwc` documents: 23 of this graph's 95 convolutions take a weight computed by a
        //   `Reshape` rather than by a stored initializer. It is **not** symmetric with Athens, which declares the
        //   layout at FP16, nor with New York, which declares it at both.
        // - **Convolution-bias fusion** costs 1.4ms at FP32 and fails inside the cuDNN frontend at FP16 with
        //   `HEURISTIC_QUERY_FAILED`.
        //
        // Every other CUDA option measured on this graph lands within noise — the workspace cap, the arena extension
        // strategy, the default-stream copy, the EP-level unified stream and the tunable-op switch. Two are not
        // options at all: the convolution algorithm search must not be set to its *default* setting, which costs 2.1x
        // because all 95 convolutions then fall back, and TF32 must not be disabled, which costs 1.5x.
        ..EpProfile::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::face_recovery::FaceRecoveryVariant;
    use crate::models::precision::FloatPrecision;
    use crate::providers::profile::CoreMlComputeUnits;

    fn declared(precision: FloatPrecision) -> EpProfile {
        // Asked through the variant rather than of `profile` directly, because the variant's match is the half a
        // refactor can break: a row left pointing at `EpProfile::default()` still compiles and still restores.
        FaceRecoveryVariant::Santorini(precision).profile()
    }

    #[test]
    fn santorini_asks_coreml_to_specialize_its_graph_at_both_precisions() {
        // Mirrors `santorini_test.go`'s own reason for existing: nothing else asserts the row carries this, and it
        // is silent if it is dropped — the model still loads and still restores the right faces. The first
        // specialization strategy declared in this catalogue, so there is no second row to notice its absence.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                declared(precision).coreml_specialization,
                CoreMlSpecialization::FastPrediction,
                "Santorini at {precision:?} did not ask CoreML to specialize its graph for latency"
            );
        }
    }

    #[test]
    fn santorini_does_not_restrict_coremls_compute_units_where_athens_does() {
        // The load-bearing half, and the one a reader tidying the family's two rows into agreement would "fix".
        // Costing ~9%, and silent: restricting the units changes no pixel.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                declared(precision).coreml_compute_units,
                CoreMlComputeUnits::All,
                "Santorini at {precision:?} was restricted off the Neural Engine: its FP16 graph runs FP16 through \
                 its upsampling path, so the Neural Engine takes it whole and is faster for it"
            );
        }

        // And the contrast is real rather than asserted against a default that happens to agree: the sibling row
        // does restrict, at both precisions.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                FaceRecoveryVariant::Athens(precision).profile().coreml_compute_units,
                CoreMlComputeUnits::CpuAndGpu,
                "the contrast this test is about has gone: Athens no longer restricts at {precision:?}"
            );
        }
    }

    #[test]
    fn santorini_runs_sequentially_at_both_precisions() {
        // The one setting here that is not about CoreML, and the one most likely to be dropped as redundant — it
        // reads like a default being restated. Worth 1.09x at FP16 and 1.04x at FP32 through CUDA, and both
        // precisions want it, because what earns it is the graph's shape rather than its precision.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                declared(precision).execution_mode,
                ExecutionMode::Sequential,
                "Santorini at {precision:?} did not declare sequential execution: its graph is a backbone, so the \
                 inter-op pool only costs it a thread handoff per node"
            );
        }
    }

    #[test]
    fn santorini_keeps_cudas_default_nchw_layout_at_both_precisions() {
        // Not a slower setting on this graph but a **failing** one: 23 of its 95 convolutions take a weight computed
        // by a reshape rather than an initializer, and the run stops with a channel/group mismatch. Loud at run time
        // and invisible in a review of a two-field struct literal — and both of the family's siblings declare it,
        // Athens at FP16 and New York at both, so the pull towards symmetry is real.
        for precision in FloatPrecision::ALL {
            assert!(
                !declared(precision).cuda_prefer_nhwc,
                "Santorini at {precision:?} declared CUDA's NHWC layout, which fails at Run on this graph"
            );
        }
    }

    #[test]
    fn santorini_leaves_every_other_provider_setting_at_the_runtimes_own_default() {
        // The two CUDA defaults that are load-bearing for this graph are the reason: the convolution algorithm
        // search must not be set to its own default, which costs 2.1x, and TF32 must not be disabled, which costs
        // 1.5x. Both are carried by the `..EpProfile::default()` rather than named, so what this pins is that
        // nothing else was named either — convolution-bias fusion included, which fails in the cuDNN frontend here.
        let default = EpProfile::default();

        for precision in FloatPrecision::ALL {
            let carried = declared(precision);

            assert_eq!(carried.disable_mem_pattern, default.disable_mem_pattern);
            assert_eq!(carried.disabled_optimizers, default.disabled_optimizers);
            assert_eq!(carried.trt_options, default.trt_options);
        }
    }

    #[test]
    fn the_precision_a_profile_is_asked_for_does_not_change_it() {
        // Both settings are properties of the graph's shape and its export rather than of the precision it was
        // exported at, and both precisions were measured — which is what makes the ignored parameter deliberate
        // rather than an oversight.
        assert_eq!(profile(Precision::Fp32), profile(Precision::Fp16));
    }
}
