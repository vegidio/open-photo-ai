//! Lyon: the CIT-EC window-attention transformer — the square its graph was exported at and the execution-provider
//! profile measured for it. The adjustment it runs is the family's [`process`](super::process).

// CIT-EC (arXiv 2309.04366) is a 27.4M-parameter window-attention transformer: four residual groups of six blocks,
// window size 8, on an H/4 x W/4 feature grid. Two properties of that architecture decide how it has to be run — it
// cannot be tiled, below, and it cannot be exported with dynamic axes, on `CANVAS` — and between them they are the
// whole of why this family's contract is shaped the way it is.
//
// It cannot be tiled. Every one of the 24 blocks contains a channel-attention block whose adaptive average pool and a
// half-instance-norm block whose instance norm both reduce over the **whole spatial extent** — the paper's own words
// are that they are there "to acquire the global statistics". Give each tile its own statistics and each tile gets its
// own exposure correction: measured through the real graph with this project's exact tile geometry, 512-pixel tiles
// still leave a 50.9-level DC spread between tiles and 18.9 dB against the whole-image result. **No overlap width fixes
// a DC offset**, which is why the whole photograph goes through in one pass and the low-resolution result is applied as
// a gain map at full resolution, the way Paris does it.
//
// Export notes for anyone rebuilding these weights: two graph rewrites are load-bearing, and **without them CoreML is
// slower than the CPU provider**. CoreML rejects any tensor of rank above 5, and the stock window partition builds a
// rank-6 view: rewriting the partition and its inverse to stay within rank 5 takes the graph from 73 CoreML partitions
// to 25, and taking q/k/v by a single split instead of three gathers takes it to 1. Both are numerically exact —
// verified bit-identical — and together they take CoreML from 2506 ms to 830 ms. Load `params_ema`, not `params`; the
// checkpoint carries both.

use crate::models::precision::Precision;
use crate::providers::profile::{CoreMlComputeUnits, EpProfile};

// It cannot be exported with dynamic axes, though not for the reason one would expect. A dynamic export is numerically
// perfect — the tracer turns the mask calculation into real operations rather than baking a constant, and the result is
// pixel-identical to PyTorch at every resolution tried. **It is CoreML that refuses it**: with the height and width
// unbounded, every reshape and slice in a window-attention graph has an unbounded dimension, MLProgram reports *"has
// unbounded dimension which is not supported"* 720 times, the graph splits into 363 partitions and then fails at run
// time. Hence one fixed square.
//
// Why 1024: 512 is four times faster and renders a visibly different photograph: 23.1 dB against the whole-image
// reference and a 16-level global darkening on the over-exposed sample, which is not a subtlety. 1024 holds 46.5 dB and
// -1.0 levels. It costs 12.6 MB more rather than the 150 MB a larger canvas would suggest, because the twelve baked
// shifted-window masks are bit-identical and share one initializer.
/// The square this graph accepts, and the only one it accepts.
pub(crate) const CANVAS: u32 = 1024;

/// The execution-provider tuning measured for this model, at the precision it carries: CoreML off the Neural Engine
/// at FP16, and the provider defaults at FP32.
pub(crate) fn profile(precision: Precision) -> EpProfile {
    // Transcribed from the reference's `lyon.go` and re-confirmed on this project's own sweep. Everything below is an
    // M2 Max against ONNX Runtime 1.26 at the 1024 square.
    //
    // CoreML off the Neural Engine, at FP16 only: worth **2.6x** — 1770 ms against 687 ms — and the mechanism is
    // recorded rather than inferred, because CoreML's own compute plan reports it: under the default compute units 363
    // of this graph's operations are placed on the Neural Engine among 1626 placed on the GPU, so every run pays
    // hundreds of transitions between the two.
    //
    // The same setting is also **more accurate**, which is unusual enough to be part of the declaration rather than a
    // note beside it. The operations placed on the Neural Engine run at its reduced internal precision, so against an
    // FP32 CPU reference the default compute units score 69.3 dB where the restriction scores 74.0 dB. Faster and
    // closer to the reference, and it turns this model's FP16 graph from slower than its FP32 one into faster than it.
    //
    // The Neural Engine is absent from those figures because **it will not take this graph at all** — the compiler
    // reports that it failed to compile the model — so what such a configuration measures is the fallback path, at
    // 5.7 s at FP32 and 16.2 s at FP16.
    //
    // No sequential execution mode, where Paris declares one; see `LightAdjustmentVariant::profile` before carrying
    // either answer across. A sequential mode, CoreML's latency specialization hint and its low-precision GPU
    // accumulation all land within 0.3% of doing nothing for Lyon, with bit-identical output, and there is a reason
    // rather than an accident in each: this graph is compiled into a **single fused CoreML operation** and is already
    // fixed-shape and resident — which is the case the specialization hint exists to buy, and which leaves the inter-op
    // pool nothing to schedule.
    //
    // The one node CoreML will not take: the FP16 graph is 2011 of its 2012 nodes on CoreML, and that is not worth a
    // re-export. The export is FP32 in and FP32 out per the catalogue's convention, so it opens with a `Cast` that
    // consumes the graph input, and ONNX Runtime's CoreML `Cast` builder declines a `Cast` with no producer node —
    // leaving one 1x3x1024x1024 conversion on the CPU partition. Splicing any node ahead of it fixes the placement; a
    // `Clip(input, 0, 1)` does it without needing a graph transformer switched off, and is the model's real input
    // contract rather than a trick. It measures within 1 ms of the shipping graph.
    //
    // **Anyone re-measuring Lyon must first confirm that its graph is still compiled as a single CoreML partition.**
    // Without the two export rewrites described in this module's header it compiles as the 73 partitions counted there,
    // and CoreML is then slower than the CPU provider — which would make every figure above a measurement of partition
    // handoff rather than of compute units.
    //
    // Re-confirmed here, precondition first, and the precondition holds. Under the option set this crate actually
    // writes — which matters, because without `ModelFormat: MLProgram` the provider declines the graph outright and
    // reports **zero** partitions — ONNX Runtime 1.26 reports:
    //
    //   CoreMLExecutionProvider::GetCapability, number of partitions supported by CoreML: 1
    //     number of nodes in the graph: 2012   number of nodes supported by CoreML: 2011
    //   Node(s) placed on [CoreMLExecutionProvider]. Number of nodes: 1
    //   Node(s) placed on [CPUExecutionProvider].    Number of nodes: 1
    //
    // One partition, 2011 of 2012 nodes, and the graph placed as a **single fused node** beside the one leading `Cast`
    // described above. That is exactly what the figures below were taken against, so they are a measurement of compute
    // units rather than of partition handoff.
    //
    // With that established, `perftest lyon --precision <p> -p <provider> -n 10` on an M2 Max (64 GB, macOS 26.6), over
    // the embedded 640x640 sample. Whole-pipeline medians, where the reference's figures are of the graph alone:
    //
    //   lyon, median of 10 runs       CoreML      CPU provider
    //     FP32                        920.3ms         10.097s
    //     FP16                        762.8ms         10.828s
    //
    // **Both claims reproduce.** FP16 is **faster than FP32** on CoreML, by 17%, which is the inversion the
    // compute-unit restriction is credited with. And subtracting the ~70 ms the pipeline costs on this image — measured
    // independently by `paris`' sweep — leaves about 690 ms for the graph at FP16 against the reference's 687, and
    // about 850 ms at FP32 against its 830. CoreML is **14x** the CPU provider here, which is the other half of the
    // precondition: a graph that had fallen apart into partitions would not be.
    match precision {
        Precision::Fp16 => EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() },
        _ => EpProfile::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::light_adjustment::LightAdjustmentVariant;
    use crate::models::precision::FloatPrecision;
    use crate::providers::profile::ExecutionMode;

    #[test]
    fn lyon_carries_the_compute_units_and_nothing_else_at_fp16() {
        // The whole declaration, asked through the variant so that the arm reaching this file is inside what is
        // checked.
        assert_eq!(
            LightAdjustmentVariant::Lyon(FloatPrecision::Fp16).profile(),
            EpProfile { coreml_compute_units: CoreMlComputeUnits::CpuAndGpu, ..EpProfile::default() },
            "Lyon at FP16 is not the one setting measured for it"
        );
    }

    #[test]
    fn lyon_does_not_ask_for_one_node_at_a_time_where_paris_does() {
        // The contrast the `execution-providers` capability states as a requirement, pinned here because it is the
        // one a reader is most likely to "tidy": two models of one family, measured on the same machine, reaching
        // opposite answers — see `LightAdjustmentVariant::profile`.
        let lyon = LightAdjustmentVariant::Lyon(FloatPrecision::Fp16).profile();
        let paris = LightAdjustmentVariant::Paris(FloatPrecision::Fp16).profile();

        assert_eq!(
            lyon.execution_mode,
            ExecutionMode::default(),
            "Lyon declared an execution mode nothing measured"
        );
        assert_eq!(paris.execution_mode, ExecutionMode::Sequential, "the contrast this test rests on has moved");
        assert_ne!(lyon, paris, "the two models of this family declared one profile");

        // And the half they agree on, which is what makes the disagreement above a measurement rather than an
        // oversight: both are kept off the Neural Engine, and both were measured separately to get there.
        assert_eq!(lyon.coreml_compute_units, paris.coreml_compute_units);
    }

    #[test]
    fn lyon_declares_nothing_at_fp32() {
        // The precision split, as Paris follows it and for the same reason: the compute-unit figures above are FP16
        // measurements, and an FP32 MLProgram cannot reach the Neural Engine in the first place.
        assert_eq!(
            LightAdjustmentVariant::Lyon(FloatPrecision::Fp32).profile(),
            EpProfile::default(),
            "Lyon at FP32 declared a setting nothing measured"
        );
    }

    #[test]
    fn lyon_runs_at_one_square_and_it_is_the_one_the_graph_was_exported_at() {
        // Not a tunable, and less so here than for Paris: the dynamic export CoreML refuses is the alternative, and
        // 512 renders a visibly different photograph at 23.1 dB.
        assert_eq!(CANVAS, 1024);

        for precision in FloatPrecision::ALL {
            assert_eq!(LightAdjustmentVariant::Lyon(precision).canvas(), CANVAS, "{precision:?}");
        }
    }
}
